//! `PoseBank`: every donor track, stylized sampling, masks and the `poses.json` override file.
//!
//! `assets/anim/poses.json` (optional, hot-reloaded every ~0.5 s by mtime):
//! ```json
//! {
//!   "style": { "exaggeration": 1.15, "hold": 0.45, "bias": 0.55, "stepped_fps": null, "speed": 1.0 },
//!   "edits": { "ual1/Punch_Jab": { "2": { "hand_r": [x, y, z, w] } } },
//!   "authored": {
//!     "authored/bow_draw": {
//!       "duration": 0.6, "looping": false,
//!       "keys": [ { "time": 0.0, "tag": "start", "base": "ual1/Pistol_Aim_Neutral#0",
//!                   "bones": { "hand_l": [x, y, z, w] } } ]
//!     }
//!   }
//! }
//! ```
//! `edits` replace LOCAL rotations of a key (track → key index → bone name → quat).
//! `authored` tracks are built from a base key (`"{track}#{key}"` or `"rest"`) plus bone
//! overrides, and are sampled exactly like donor tracks.
use super::donor::{self, DonorLib};
use super::extract::{self, Key, Track};
use super::retarget::{self, Clip, clip_time, frame_count, frame_time};
use super::timing::{self, Style};
use super::{Bones, LIBRARIES, MODEL_GLB, Mask, Pose, Skeleton};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

#[derive(Resource)]
pub struct PoseBank {
    pub skeleton: Skeleton,
    pub bones: Bones,
    pub rest: Pose,
    pub tracks: HashMap<String, Track>,
    /// Default style (from poses.json `style`, else `Style::default()`).
    pub style: Style,
    /// Per-mask per-bone weights (len = skeleton.len()).
    pub masks: HashMap<Mask, Vec<f32>>,
}

/// `assets/anim/poses.json`.
pub fn poses_path() -> PathBuf {
    Path::new(crate::ASSETS).join("anim/poses.json")
}

impl PoseBank {
    /// Load all `super::LIBRARIES`, retarget, extract, apply poses.json. Logs a summary line
    /// `[bank] N tracks (ual1 a, ual2 b, sekiro c), K keys` on success.
    pub fn build() -> Result<Self, String> {
        Self::build_with(&poses_path()).map(|(bank, _)| bank)
    }

    /// `build` with an explicit override file; also returns the bookkeeping for hot reload.
    pub fn build_with(poses: &Path) -> Result<(Self, PosesFile), String> {
        let started = std::time::Instant::now();
        let root = Path::new(crate::ASSETS);
        // Parse the GLBs in parallel; they are independent and dominate start-up.
        let libs: Vec<DonorLib> = std::thread::scope(|scope| {
            let handles: Vec<_> = LIBRARIES
                .iter()
                .map(|&(lib, rel)| scope.spawn(move || donor::load(lib, &root.join(rel))))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err("donor loader panicked".into())))
                .collect::<Result<Vec<_>, String>>()
        })?;
        let model_lib = LIBRARIES
            .iter()
            .position(|&(_, rel)| rel == MODEL_GLB)
            .ok_or("MODEL_GLB is not one of LIBRARIES")?;
        let skeleton = donor::skeleton(&libs[model_lib]);
        let bones = Bones::new(&skeleton)
            .ok_or("canonical skeleton is missing a named bone (pelvis, spine_01, ...)")?;

        let per_lib: Vec<Vec<Track>> = std::thread::scope(|scope| {
            let handles: Vec<_> = libs
                .iter()
                .map(|lib| {
                    let (skeleton, bones) = (&skeleton, &bones);
                    scope.spawn(move || {
                        retarget::retarget(lib, skeleton, bones)
                            .iter()
                            .map(|clip| extract::extract(clip, skeleton, bones))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().map_err(|_| "retarget/extract panicked".to_string()))
                .collect::<Result<Vec<_>, String>>()
        })?;

        let mut counts = Vec::new();
        let mut tracks = HashMap::new();
        for (lib, lib_tracks) in libs.iter().zip(per_lib) {
            counts.push(format!("{} {}", lib.lib, lib_tracks.len()));
            for t in lib_tracks {
                tracks.insert(t.name.clone(), t);
            }
        }
        let masks = build_masks(&skeleton, &bones);
        let mut bank = PoseBank {
            rest: skeleton.rest_pose(),
            skeleton,
            bones,
            tracks,
            style: Style::default(),
            masks,
        };

        let mut file = PosesFile::new(poses.to_path_buf());
        file.mtime = modified(poses);
        match load_overrides(poses) {
            Ok(overrides) => file.apply(&mut bank, overrides),
            Err(e) => error!("[bank] {e} (ignoring overrides)"),
        }

        let keys: usize = bank.tracks.values().map(|t| t.keys.len()).sum();
        let authored = if file.authored.is_empty() {
            String::new()
        } else {
            format!(", authored {}", file.authored.len())
        };
        info!(
            "[bank] {} tracks ({}{authored}), {keys} keys in {:.2}s",
            bank.tracks.len(),
            counts.join(", "),
            started.elapsed().as_secs_f32()
        );
        Ok((bank, file))
    }

    pub fn has(&self, name: &str) -> bool {
        self.tracks.contains_key(name)
    }
    pub fn duration(&self, name: &str) -> f32 {
        self.tracks.get(name).map_or(0.0, |t| t.duration)
    }
    /// Stylized sample: find the surrounding keys, ease between them with `style`, then
    /// exaggerate from rest. Unknown names return rest (and warn once).
    /// `style.speed` is applied by whoever advances `time`.
    pub fn sample(&self, name: &str, time: f32, looping: bool, style: &Style) -> Pose {
        let Some(track) = self.tracks.get(name) else {
            warn_unknown(name);
            return self.rest.clone();
        };
        let pose = sample_keys(&track.keys, track.duration, time, looping, style);
        if pose.rot.len() != self.rest.rot.len() {
            return self.rest.clone();
        }
        timing::exaggerate(&pose, &self.rest, style.exaggeration)
    }
    /// Dense, un-stylized sample (for comparisons and root travel).
    pub fn sample_raw(&self, name: &str, time: f32, looping: bool) -> Pose {
        let Some(track) = self.tracks.get(name) else {
            warn_unknown(name);
            return self.rest.clone();
        };
        let pose = retarget::sample_looping(&track.dense, time, looping);
        if pose.rot.len() != self.rest.rot.len() {
            return self.rest.clone();
        }
        pose
    }
    /// Model-space horizontal root travel between two clip times.
    pub fn travel(&self, name: &str, t0: f32, t1: f32) -> Vec3 {
        self.tracks
            .get(name)
            .map_or(Vec3::ZERO, |t| retarget::travel(&t.dense, t0, t1))
    }
    pub fn mask(&self, mask: Mask) -> &[f32] {
        &self.masks[&mask]
    }
    /// Write current `edits`/`authored`/`style` back to poses.json (used by `tweak`).
    pub fn save_overrides(&self, overrides: &Overrides) -> Result<(), String> {
        save_overrides_to(&poses_path(), overrides)
    }
}

/// Write `overrides` as pretty JSON with sorted keys (stable diffs), via a temp file so the
/// hot reloader never sees a half-written file.
fn save_overrides_to(path: &Path, overrides: &Overrides) -> Result<(), String> {
    let value = serde_json::to_value(overrides).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&sorted(value)).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text + "\n").map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Rebuild JSON objects with keys in sorted order.
fn sorted(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sorted).collect())
        }
        other => other,
    }
}

/// Read poses.json; a missing file is an empty override set.
pub fn load_overrides(path: &Path) -> Result<Overrides, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Overrides::default()),
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Overrides::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn warn_unknown(name: &str) {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let set = WARNED.get_or_init(Default::default);
    if let Ok(mut set) = set.lock()
        && set.insert(name.to_string())
    {
        warn!("[bank] unknown track '{name}', using the rest pose");
    }
}

/// Stylized key interpolation (no exaggeration).
fn sample_keys(keys: &[Key], duration: f32, time: f32, looping: bool, style: &Style) -> Pose {
    match keys {
        [] => {
            return Pose {
                rot: Vec::new(),
                pelvis: Vec3::ZERO,
            };
        }
        [only] => return only.pose.clone(),
        _ => {}
    }
    let looping = looping && duration > 0.0;
    let t = clip_time(time, duration, looping);
    // Stepped pacing; a clamped one-shot still reaches its final pose.
    let t = if !looping && t >= duration {
        t
    } else {
        clip_time(timing::quantize(t, style), duration, looping)
    };
    let first = &keys[0];
    let last = &keys[keys.len() - 1];
    let next = keys.partition_point(|k| k.time <= t);
    let (a, b, ta, tb, t) = if next == 0 {
        if !looping {
            return first.pose.clone();
        }
        (last, first, last.time - duration, first.time, t)
    } else if next == keys.len() {
        if !looping {
            return last.pose.clone();
        }
        (last, first, last.time, first.time + duration, t)
    } else {
        let (a, b) = (&keys[next - 1], &keys[next]);
        (a, b, a.time, b.time, t)
    };
    let span = tb - ta;
    if !(span > 1e-6) {
        return a.pose.clone();
    }
    let u = ((t - ta) / span).clamp(0.0, 1.0);
    a.pose.lerp(&b.pose, timing::ease(u, style))
}

fn sanitize_style(s: Style) -> Style {
    let d = Style::default();
    let pick = |v: f32, lo: f32, hi: f32, fallback: f32| {
        if v.is_finite() {
            v.clamp(lo, hi)
        } else {
            fallback
        }
    };
    Style {
        exaggeration: pick(s.exaggeration, 0.0, 3.0, d.exaggeration),
        hold: pick(s.hold, 0.0, 1.0, d.hold),
        bias: pick(s.bias, 0.05, 0.95, d.bias),
        stepped_fps: s.stepped_fps.filter(|f| f.is_finite() && *f > 0.0),
        speed: pick(s.speed, 0.01, 10.0, d.speed),
    }
}

fn quat_from(q: [f32; 4]) -> Option<Quat> {
    let q = Quat::from_array(q);
    let len = q.length();
    (q.is_finite() && len > 1e-6).then(|| q / len)
}

/// Per-mask per-bone weights.
fn build_masks(s: &Skeleton, b: &Bones) -> HashMap<Mask, Vec<f32>> {
    let n = s.len();
    // Bone `i` is `root` or one of its descendants.
    let within = |i: usize, root: usize| {
        let mut cursor = Some(i);
        let mut guard = n + 1;
        while let Some(c) = cursor
            && guard > 0
        {
            if c == root {
                return true;
            }
            guard -= 1;
            cursor = s.parent[c];
        }
        false
    };
    let subtree = |roots: &[usize]| -> Vec<f32> {
        (0..n)
            .map(|i| {
                if roots.iter().any(|&r| within(i, r)) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect()
    };
    let mut upper = subtree(&[b.spine[0]]);
    upper[b.spine[0]] = 0.5;
    let mut lower = subtree(&[b.thigh[0], b.thigh[1]]);
    lower[b.pelvis] = 1.0;
    let mut masks = HashMap::from([
        (Mask::Full, vec![1.0; n]),
        (Mask::Upper, upper),
        (Mask::Lower, lower),
        (Mask::Arms, subtree(&[b.clavicle[0], b.clavicle[1]])),
        (Mask::ArmL, subtree(&[b.clavicle[0]])),
        (Mask::ArmR, subtree(&[b.clavicle[1]])),
        (Mask::Head, subtree(&[b.neck])),
    ]);
    for (mask, weights) in masks.iter_mut() {
        if *mask != Mask::Full {
            weights[b.root] = 0.0;
        }
    }
    masks
}

/// Parsed poses.json.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Overrides {
    #[serde(default)]
    pub style: Option<Style>,
    #[serde(default)]
    pub edits: HashMap<String, HashMap<String, HashMap<String, [f32; 4]>>>,
    #[serde(default)]
    pub authored: HashMap<String, AuthoredTrack>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthoredTrack {
    pub duration: f32,
    #[serde(default)]
    pub looping: bool,
    pub keys: Vec<AuthoredKey>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthoredKey {
    pub time: f32,
    pub tag: super::extract::Tag,
    pub base: String,
    #[serde(default)]
    pub bones: HashMap<String, [f32; 4]>,
}

/// poses.json bookkeeping: the file's mtime and what the applied overrides changed, so a
/// reload can undo them before applying the new file.
#[derive(Resource)]
pub struct PosesFile {
    pub path: PathBuf,
    /// The overrides currently applied (what `tweak` should edit and save back).
    pub current: Overrides,
    mtime: Option<SystemTime>,
    timer: Timer,
    /// Original keys of donor tracks touched by `edits`.
    base_keys: HashMap<String, Vec<Key>>,
    /// Names of the inserted `authored` tracks.
    authored: Vec<String>,
}

impl PosesFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            current: Overrides::default(),
            mtime: None,
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            base_keys: HashMap::new(),
            authored: Vec::new(),
        }
    }

    /// Undo the previous overrides, then apply `overrides`. Bad entries are logged and skipped.
    pub fn apply(&mut self, bank: &mut PoseBank, overrides: Overrides) {
        for (name, keys) in self.base_keys.drain() {
            if let Some(track) = bank.tracks.get_mut(&name) {
                track.keys = keys;
            }
        }
        for name in self.authored.drain(..) {
            bank.tracks.remove(&name);
        }
        bank.style = overrides.style.map_or_else(Style::default, sanitize_style);

        // Edits on donor tracks first so authored bases see them, then authored tracks, then
        // edits on authored tracks.
        let donor_edits = |name: &String| !overrides.authored.contains_key(name);
        self.apply_edits(bank, &overrides, donor_edits, true);
        let mut names: Vec<&String> = overrides.authored.keys().collect();
        names.sort();
        for name in names {
            if bank.tracks.contains_key(name) {
                warn!("[bank] poses.json authored '{name}' collides with an existing track; skipped");
                continue;
            }
            match build_authored(bank, name, &overrides.authored[name]) {
                Ok(track) => {
                    bank.tracks.insert(name.clone(), track);
                    self.authored.push(name.clone());
                }
                Err(e) => warn!("[bank] poses.json authored '{name}': {e}"),
            }
        }
        let authored = self.authored.clone();
        self.apply_edits(bank, &overrides, |name| authored.contains(name), false);
        self.current = overrides;
    }

    fn apply_edits(
        &mut self,
        bank: &mut PoseBank,
        overrides: &Overrides,
        filter: impl Fn(&String) -> bool,
        remember: bool,
    ) {
        let PoseBank {
            skeleton, tracks, ..
        } = bank;
        for (track_name, keys) in overrides.edits.iter().filter(|(n, _)| filter(n)) {
            let Some(track) = tracks.get_mut(track_name) else {
                warn!("[bank] poses.json edits: unknown track '{track_name}'");
                continue;
            };
            if remember && !self.base_keys.contains_key(track_name) {
                self.base_keys.insert(track_name.clone(), track.keys.clone());
            }
            for (index, bones) in keys {
                let Some(key) = index.parse::<usize>().ok().and_then(|i| track.keys.get_mut(i))
                else {
                    warn!("[bank] poses.json edits: '{track_name}' has no key '{index}'");
                    continue;
                };
                for (bone, q) in bones {
                    match (skeleton.find(bone), quat_from(*q)) {
                        (Some(b), Some(q)) => key.pose.rot[b] = q,
                        (None, _) => warn!("[bank] poses.json edits: unknown bone '{bone}'"),
                        (_, None) => warn!("[bank] poses.json edits: bad quat for '{bone}'"),
                    }
                }
            }
        }
    }
}

/// Resolve an authored base: `"rest"` or `"{track}#{key}"`.
fn resolve_base(bank: &PoseBank, base: &str) -> Result<Pose, String> {
    if base == "rest" {
        return Ok(bank.rest.clone());
    }
    let (track, index) = base
        .rsplit_once('#')
        .ok_or_else(|| format!("base '{base}' is not \"rest\" or \"track#key\""))?;
    let index: usize = index
        .parse()
        .map_err(|_| format!("base '{base}': bad key index"))?;
    bank.tracks
        .get(track)
        .ok_or_else(|| format!("base '{base}': unknown track"))?
        .keys
        .get(index)
        .map(|k| k.pose.clone())
        .ok_or_else(|| format!("base '{base}': no key {index}"))
}

fn build_authored(bank: &PoseBank, name: &str, a: &AuthoredTrack) -> Result<Track, String> {
    if a.keys.is_empty() {
        return Err("no keys".into());
    }
    let mut keys = Vec::with_capacity(a.keys.len());
    for k in &a.keys {
        let mut pose = resolve_base(bank, &k.base)?;
        for (bone, q) in &k.bones {
            let b = bank
                .skeleton
                .find(bone)
                .ok_or_else(|| format!("unknown bone '{bone}'"))?;
            pose.rot[b] = quat_from(*q).ok_or_else(|| format!("bad quat for '{bone}'"))?;
        }
        let time = if k.time.is_finite() { k.time.max(0.0) } else { 0.0 };
        keys.push(Key {
            time,
            pose,
            tag: k.tag,
        });
    }
    keys.sort_by(|x, y| x.time.total_cmp(&y.time));
    keys.dedup_by(|later, earlier| (later.time - earlier.time).abs() < 1e-5);
    let last = keys.last().map_or(0.0, |k| k.time);
    let duration = if a.duration.is_finite() {
        a.duration.max(last)
    } else {
        last
    };
    // Dense frames: plain key interpolation, no travel.
    let linear = Style {
        hold: 0.0,
        stepped_fps: None,
        ..Style::default()
    };
    let n = frame_count(duration);
    let frames: Vec<Pose> = (0..n)
        .map(|i| sample_keys(&keys, duration, frame_time(i, duration), a.looping, &linear))
        .collect();
    Ok(Track {
        name: name.to_string(),
        duration,
        looping: a.looping,
        keys,
        left_contact: None,
        stride: 0.0,
        dense: Clip {
            track: name.to_string(),
            duration,
            looping: a.looping,
            travel: vec![Vec3::ZERO; frames.len()],
            frames,
        },
    })
}

fn hot_reload(time: Res<Time>, mut file: ResMut<PosesFile>, mut bank: ResMut<PoseBank>) {
    let file = &mut *file;
    if !file.timer.tick(time.delta()).just_finished() {
        return;
    }
    let mtime = modified(&file.path);
    if mtime == file.mtime {
        return;
    }
    file.mtime = mtime;
    match load_overrides(&file.path) {
        Ok(overrides) => {
            file.apply(&mut bank, overrides);
            info!(
                "[bank] reloaded {} ({} edited tracks, {} authored)",
                file.path.display(),
                file.base_keys.len(),
                file.authored.len()
            );
        }
        Err(e) => error!("[bank] {e} (keeping the previous overrides)"),
    }
}

pub fn plugin(app: &mut App) {
    // Built synchronously so every Startup system can rely on the resource.
    let (bank, file) = PoseBank::build_with(&poses_path()).unwrap_or_else(|e| {
        panic!(
            "[bank] cannot build the pose bank from {}/anim (donor GLBs missing or corrupt?): {e}",
            crate::ASSETS
        )
    });
    app.insert_resource(bank)
        .insert_resource(file)
        .add_systems(Update, hot_reload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::extract::Tag;

    fn bank() -> &'static PoseBank {
        static BANK: OnceLock<PoseBank> = OnceLock::new();
        BANK.get_or_init(|| {
            let started = std::time::Instant::now();
            let (bank, _) = PoseBank::build_with(Path::new("__no_poses__.json")).expect("bank");
            println!("bank built in {:.2}s", started.elapsed().as_secs_f32());
            bank
        })
    }

    fn assert_valid(p: &Pose, n: usize) {
        assert_eq!(p.rot.len(), n);
        for q in &p.rot {
            assert!(q.is_finite() && (q.length() - 1.0).abs() < 1e-3, "{q}");
        }
        assert!(p.pelvis.is_finite());
    }

    #[test]
    fn builds_every_library() {
        let b = bank();
        let count = |lib: &str| b.tracks.keys().filter(|k| k.starts_with(&format!("{lib}/"))).count();
        // Bind clips (A_TPose / T-Pose) are skipped.
        assert_eq!(count("ual1"), 44);
        assert_eq!(count("ual2"), 42);
        assert_eq!(count("sekiro"), 56);
        assert!(b.has("ual1/Punch_Jab") && b.has("sekiro/Inward Slash"));
        assert!(!b.has("ual1/A_TPose"));
    }

    #[test]
    fn punch_samples_are_finite_and_normalized() {
        let b = bank();
        let n = b.skeleton.len();
        let d = b.duration("ual1/Punch_Jab");
        assert!(d > 0.0);
        for style in [Style::default(), Style { stepped_fps: Some(12.0), hold: 1.0, ..default() }] {
            for t in [0.0, d * 0.37, d, d + 1.0, -1.0, f32::NAN] {
                assert_valid(&b.sample("ual1/Punch_Jab", t, false, &style), n);
                assert_valid(&b.sample("ual1/Punch_Jab", t, true, &style), n);
            }
        }
        assert_valid(&b.sample_raw("ual1/Punch_Jab", d * 0.5, false), n);
        // End of a one-shot is its last key (exaggerated from rest).
        let end = b.sample("ual1/Punch_Jab", d, false, &Style { exaggeration: 1.0, ..default() });
        let last = &b.tracks["ual1/Punch_Jab"].keys.last().unwrap().pose;
        for (x, y) in end.rot.iter().zip(&last.rot) {
            assert!(x.angle_between(*y) < 2e-3);
        }
    }

    #[test]
    fn unknown_track_is_rest() {
        let b = bank();
        assert_eq!(b.sample("nope/none", 0.3, true, &Style::default()), b.rest);
        assert_eq!(b.sample_raw("nope/none", 0.3, true), b.rest);
        assert_eq!(b.travel("nope/none", 0.0, 1.0), Vec3::ZERO);
    }

    #[test]
    fn looping_sample_wraps() {
        let b = bank();
        let style = Style::default();
        let d = b.duration("ual1/Walk_Loop");
        let a = b.sample("ual1/Walk_Loop", 0.2, true, &style);
        let c = b.sample("ual1/Walk_Loop", 0.2 + 3.0 * d, true, &style);
        for (x, y) in a.rot.iter().zip(&c.rot) {
            assert!(x.angle_between(*y) < 2e-3);
        }
    }

    #[test]
    fn masks_cover_expected_bones() {
        let b = bank();
        let bn = &b.bones;
        for mask in [
            Mask::Full,
            Mask::Upper,
            Mask::Lower,
            Mask::ArmL,
            Mask::ArmR,
            Mask::Arms,
            Mask::Head,
        ] {
            assert_eq!(b.mask(mask).len(), b.skeleton.len());
            let root = b.mask(mask)[bn.root];
            assert_eq!(root, if mask == Mask::Full { 1.0 } else { 0.0 }, "{mask:?}");
        }
        let upper = b.mask(Mask::Upper);
        assert_eq!(upper[bn.spine[0]], 0.5);
        assert_eq!(upper[bn.hand[0]], 1.0);
        assert_eq!(upper[b.skeleton.find("index_03_r").unwrap()], 1.0);
        assert_eq!(upper[bn.head], 1.0);
        assert_eq!(upper[bn.pelvis], 0.0);
        assert_eq!(upper[bn.thigh[0]], 0.0);
        let lower = b.mask(Mask::Lower);
        assert_eq!(lower[bn.pelvis], 1.0);
        assert_eq!(lower[bn.ball[1]], 1.0);
        assert_eq!(lower[bn.spine[0]], 0.0);
        let arm_l = b.mask(Mask::ArmL);
        assert_eq!(arm_l[bn.clavicle[0]], 1.0);
        assert_eq!(arm_l[bn.hand[1]], 0.0);
        assert_eq!(b.mask(Mask::Arms)[bn.hand[1]], 1.0);
        let head = b.mask(Mask::Head);
        assert_eq!(head.iter().filter(|&&w| w > 0.0).count(), 2);
    }

    #[test]
    fn overrides_round_trip() {
        let mut o = Overrides {
            style: Some(Style {
                stepped_fps: Some(12.0),
                ..default()
            }),
            ..default()
        };
        o.edits.entry("ual1/Punch_Jab".into()).or_default().insert(
            "2".into(),
            HashMap::from([("hand_r".to_string(), [0.0, 0.0, 0.0, 1.0])]),
        );
        o.authored.insert(
            "authored/test".into(),
            AuthoredTrack {
                duration: 0.6,
                looping: false,
                keys: vec![AuthoredKey {
                    time: 0.0,
                    tag: Tag::Start,
                    base: "rest".into(),
                    bones: HashMap::from([("hand_l".to_string(), [0.0, 0.7071, 0.0, 0.7071])]),
                }],
            },
        );
        let text = serde_json::to_string_pretty(&o).unwrap();
        let back: Overrides = serde_json::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&o).unwrap(),
            serde_json::to_value(&back).unwrap()
        );
        // Partial style objects fill in defaults.
        let partial: Overrides = serde_json::from_str(r#"{ "style": { "hold": 0.2 } }"#).unwrap();
        let s = partial.style.unwrap();
        assert_eq!(s.hold, 0.2);
        assert_eq!(s.exaggeration, Style::default().exaggeration);
        // Written files read back identically.
        let dir = std::env::temp_dir().join(format!("gb_bank_test_{}", std::process::id()));
        let path = dir.join("poses.json");
        save_overrides_to(&path, &o).unwrap();
        let read = load_overrides(&path).unwrap();
        assert_eq!(serde_json::to_value(&o).unwrap(), serde_json::to_value(&read).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_overrides(&dir.join("missing.json")).unwrap().edits.is_empty());
    }

    #[test]
    fn overrides_apply_and_undo() {
        // A private copy: the shared bank stays pristine for the other tests.
        let (mut b, mut file) =
            PoseBank::build_with(Path::new("__no_poses_either__.json")).expect("bank");
        let hand = b.skeleton.find("hand_r").unwrap();
        let original = b.tracks["ual1/Punch_Jab"].keys[1].pose.rot[hand];
        let q = Quat::from_rotation_z(0.3);
        let mut o = Overrides::default();
        o.style = Some(Style {
            hold: 7.0,
            ..default()
        });
        o.edits.entry("ual1/Punch_Jab".into()).or_default().insert(
            "1".into(),
            HashMap::from([("hand_r".to_string(), q.to_array())]),
        );
        o.edits.entry("nope".into()).or_default();
        o.authored.insert(
            "authored/pose".into(),
            AuthoredTrack {
                duration: 0.5,
                looping: false,
                keys: vec![
                    AuthoredKey {
                        time: 0.0,
                        tag: Tag::Start,
                        base: "ual1/Punch_Jab#1".into(),
                        bones: HashMap::new(),
                    },
                    AuthoredKey {
                        time: 0.5,
                        tag: Tag::End,
                        base: "rest".into(),
                        bones: HashMap::new(),
                    },
                ],
            },
        );
        file.apply(&mut b, o);
        assert_eq!(b.style.hold, 1.0, "style sanitized");
        assert!(b.tracks["ual1/Punch_Jab"].keys[1].pose.rot[hand].angle_between(q) < 2e-3);
        let authored = &b.tracks["authored/pose"];
        assert!(authored.keys[0].pose.rot[hand].angle_between(q) < 2e-3, "base sees edits");
        assert_valid(&b.sample("authored/pose", 0.25, false, &Style::default()), b.skeleton.len());
        assert_eq!(authored.dense.frames.len(), frame_count(0.5));

        file.apply(&mut b, Overrides::default());
        assert!(!b.has("authored/pose"));
        assert_eq!(b.tracks["ual1/Punch_Jab"].keys[1].pose.rot[hand], original);
        assert_eq!(b.style, Style::default());
    }
}

//! A fighter's repertoire: moves beyond its moveset's fixed chains.
//!
//! - `StrikeLibrary`: single strikes cut out of the hand-animated libraries the user picked to
//!   match the look (`library_track`: UAL's punches, and Kevin Iglesias' packs once converted to
//!   `kev*`): no CMU or Rokoko mocap, which reads stiff next to them. Cut by
//!   `segment`: forward kinematics on the bank at `SAMPLE_HZ`, the hand / foot speed relative to the
//!   pelvis (in leg lengths per second), peaks over `PUNCH_SPEED` / `KICK_SPEED` that reach out
//!   (`REACH`) ahead of the body (`AHEAD`), upright (head over the pelvis), not spinning
//!   (`MAX_TURN`), with a wind-up and a
//!   recovery that fit a fight (`MAX_WINDUP`, `MAX_RECOVER`); the arc must pass in front of the body
//!   between the peak and the band's end (a hook is fastest out to the side), and a band may run into the
//!   end of a clip that continues in `<track>_Rec` (UAL's Melee_Hook), or begin at frame 0 of a library clip
//!   (authored to start on its own wind-up). Each becomes an unarmed `Attack`
//!   whose windows come from the measured contact (`strike_attack`). Built once the bank is ready.
//! - `Repertoire` (every `Fighter`): extra attacks per moveset and slot. An unarmed fighter starts
//!   with a few library strikes (`SEED_PUNCHES`, `SEED_KICKS`, seeded by entity), the player with
//!   all of them, so its chains vary
//!   (`combat::start_swing` swaps one in with `VARIANT_CHANCE`).
//! - Learning: `combat::resolve` reports every melee blow that reaches a fighter as `Witnessed`
//!   (the attacker's moveset and attack). After `LEARN_AFTER` such blows of the same move, the
//!   fighter learns it: a copy under its own name with its own timing (clip speed ×
//!   `LEARN_SPEED` range), used whenever it wields that moveset. The player is told in the log.
//!
//! Ownership: `Repertoire` (here), `StrikeLibrary` (here).
use super::hud::log::{HudLog, LogKind};
use super::moveset::{Attack, ClipRef, Hitbox};
use super::{Fighter, Player};
use crate::motion::bank::PoseBank;
use crate::motion::{Bones, SAMPLE_HZ, Skeleton};
use bevy::prelude::*;
use std::collections::HashMap;

/// Hand / foot speed relative to the pelvis (leg lengths per second) that counts as a strike.
pub const PUNCH_SPEED: f32 = 4.0;
pub const KICK_SPEED: f32 = 6.0;
/// Reach at the peak, as a share of the limb's length, and the band of a window around a peak
/// (speed above this share of the peak).
pub const REACH: f32 = 0.75;
const BAND: f32 = 0.2;
pub const MAX_WINDUP: f32 = 0.5;
pub const MAX_RECOVER: f32 = 0.8;
/// Largest pelvis turn across a strike (rad): spins and turns are not strikes.
pub const MAX_TURN: f32 = 2.1;
/// A strike must go out ahead of the body: limb direction from the pelvis against the facing
/// (cos) at the peak.
pub const AHEAD: f32 = 0.5;
/// Kicks lift the foot at least this far over the lowest foot (leg lengths).
const KICK_LIFT: f32 = 0.35;
/// Peaks closer than this (s) are one strike.
const SEPARATION: f32 = 0.4;
/// Library strikes each unarmed fighter starts with, and the chance a chained attack is one of
/// its extras instead of the moveset's.
pub const SEED_PUNCHES: usize = 4;
pub const SEED_KICKS: usize = 2;
pub const VARIANT_CHANCE: f32 = 0.35;
/// Blows of the same move it takes to learn it, and the learned copy's clip speed range.
pub const LEARN_AFTER: u32 = 3;
pub const LEARN_SPEED: (f32, f32) = (0.88, 1.12);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Light,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Punch,
    Kick,
}

/// One strike found in a track.
#[derive(Clone, Debug, PartialEq)]
pub struct Strike {
    pub track: String,
    pub kind: Kind,
    /// Canonical bone that strikes ("hand_l", "foot_r", ...).
    pub bone: String,
    /// Seconds: band start, peak, band end.
    pub start: f32,
    pub peak: f32,
    pub end: f32,
    /// Peak speed (leg lengths / s).
    pub speed: f32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct StrikeLibrary {
    pub punches: Vec<Attack>,
    pub kicks: Vec<Attack>,
}

#[derive(Clone, Debug)]
pub struct Extra {
    pub set: String,
    pub slot: Slot,
    pub attack: Attack,
}

#[derive(Component, Clone, Debug, Default)]
pub struct Repertoire {
    pub extras: Vec<Extra>,
    /// Blows witnessed per (moveset, attack name).
    seen: HashMap<(String, String), u32>,
    seeded: bool,
}

impl Repertoire {
    /// Extra attacks for `set` in `slot`.
    pub fn for_slot<'a>(&'a self, set: &'a str, slot: Slot) -> impl Iterator<Item = &'a Attack> + 'a {
        self.extras.iter().filter(move |e| e.set == set && e.slot == slot).map(|e| &e.attack)
    }
}

/// A melee blow that reached `learner`, from `who` (the attacker's name).
#[derive(Message, Clone, Debug)]
pub struct Witnessed {
    pub learner: Entity,
    pub who: String,
    pub set: String,
    pub slot: Slot,
    pub attack: Attack,
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, Repertoire>()
        .add_message::<Witnessed>()
        .add_systems(Update, (build_library, seed, learn).chain());
}

/// xorshift32 → 0..1.
pub fn roll(state: &mut u32) -> f32 {
    let mut x = (*state).max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn yaw_of_pelvis(world: &[Transform], bones: &Bones) -> f32 {
    let across = world[bones.thigh[0]].translation - world[bones.thigh[1]].translation;
    across.z.atan2(across.x)
}

/// Direction of `limb` from the pelvis against the body's facing (cos, flat on the ground): +1 straight ahead.
pub fn aim_at(world: &[Transform], bones: &Bones, limb: usize) -> f32 {
    let left = (world[bones.thigh[0]].translation - world[bones.thigh[1]].translation).with_y(0.0).normalize_or(Vec3::X);
    let dir = (world[limb].translation - world[bones.pelvis].translation).with_y(0.0).normalize_or_zero();
    dir.dot(left.cross(Vec3::Y))
}

/// A speed peak `segment` turned down, and the gate that turned it down.
#[derive(Clone, Debug, PartialEq)]
pub struct Rejected {
    pub bone: String,
    pub kind: Kind,
    pub peak: f32,
    pub speed: f32,
    pub reason: &'static str,
}

/// Every strike in `track` (see the module docs).
pub fn segment(bank: &PoseBank, track: &str) -> Vec<Strike> {
    segment_explained(bank, track).0
}

/// `segment`, plus every peak over the speed threshold that a later gate turned down (Kev reads these
/// to say why a clip gave no strike).
pub fn segment_explained(bank: &PoseBank, track: &str) -> (Vec<Strike>, Vec<Rejected>) {
    let (skeleton, bones): (&Skeleton, &Bones) = (&bank.skeleton, &bank.bones);
    let duration = bank.duration(track);
    let dt = 1.0 / SAMPLE_HZ;
    let n = (duration / dt) as usize + 1;
    if n < 8 {
        return (Vec::new(), Vec::new());
    }
    let frames: Vec<Vec<Transform>> = (0..n).map(|i| skeleton.fk(&bank.sample_raw(track, i as f32 * dt, false), bones.pelvis)).collect();
    let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
    let len = |a: usize, b: usize| rest[a].translation.distance(rest[b].translation);
    let leg = len(bones.thigh[0], bones.calf[0]) + len(bones.calf[0], bones.foot[0]);
    let arm = len(bones.upperarm[0], bones.lowerarm[0]) + len(bones.lowerarm[0], bones.hand[0]);
    if !(leg > 1e-3 && arm > 1e-3) {
        return (Vec::new(), Vec::new());
    }
    let yaw: Vec<f32> = {
        let raw: Vec<f32> = frames.iter().map(|w| yaw_of_pelvis(w, bones)).collect();
        // Unwrap.
        let mut out = Vec::with_capacity(raw.len());
        let mut acc = 0.0;
        for (i, y) in raw.iter().enumerate() {
            if i > 0 {
                let mut d = y - raw[i - 1];
                d = (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                acc += d;
            }
            out.push(raw[0] + acc);
        }
        out
    };
    // UAL splits some strikes in two (Melee_Hook, then Melee_Hook_Rec): the swing runs into the end of
    // the first clip and finishes in the second, so a band reaching the end is whole.
    let continues = bank.tracks.contains_key(&format!("{track}_Rec"));
    // A library clip is authored as one strike from its own wind-up (UAL's Melee_Hook is already moving at
    // frame 0), so a band reaching the start is its wind-up; a long mocap take cut there is mid-motion.
    let authored = library_track(track);
    let floor = frames.iter().flat_map(|w| [w[bones.foot[0]].translation.y, w[bones.foot[1]].translation.y]).fold(f32::INFINITY, f32::min);
    let mut out = Vec::new();
    let mut rejected = Vec::new();
    let names = |b: usize| skeleton.names[b].clone();
    for side in 0..2 {
        for (kind, limb, root, full, threshold) in [
            (Kind::Punch, bones.hand[side], bones.upperarm[side], arm, PUNCH_SPEED),
            (Kind::Kick, bones.foot[side], bones.thigh[side], leg, KICK_SPEED),
        ] {
            let rel: Vec<Vec3> = frames.iter().map(|w| w[limb].translation - w[bones.pelvis].translation).collect();
            let speed: Vec<f32> = (0..n)
                .map(|i| {
                    let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
                    rel[b].distance(rel[a]) / ((b - a).max(1) as f32 * dt) / leg
                })
                .collect();
            let mut last_peak = f32::NEG_INFINITY;
            for i in 1..n - 1 {
                let v = speed[i];
                if v < threshold || v < speed[i - 1] || v < speed[i + 1] {
                    continue;
                }
                let t = i as f32 * dt;
                if t - last_peak < SEPARATION {
                    continue;
                }
                // Peaks closer than SEPARATION are one strike: the fastest stands for it (a hook's hand
                // twitches at 0.07 s, then whips at 0.23 s).
                let ahead_n = (SEPARATION / dt) as usize;
                if speed[i + 1..(i + ahead_n).min(n - 1) + 1].iter().any(|&x| x > v) {
                    continue;
                }
                let mut reject = |reason| rejected.push(Rejected { bone: names(limb), kind, peak: t, speed: v, reason });
                let w = &frames[i];
                // Furthest reach within two frames of the peak.
                let reach = (0..5)
                    .map(|k| (i + k).saturating_sub(2).min(n - 1))
                    .map(|j| frames[j][limb].translation.distance(frames[j][root].translation))
                    .fold(0.0, f32::max);
                if reach < REACH * full {
                    reject("short of reach");
                    continue;
                }
                if kind == Kind::Kick && (w[limb].translation.y - floor) < KICK_LIFT * leg {
                    reject("kick too low");
                    continue;
                }
                if w[bones.head].translation.y < w[bones.pelvis].translation.y + 0.25 * leg {
                    reject("not upright");
                    continue;
                }
                let (mut lo, mut hi) = (i, i);
                while lo > 0 && speed[lo] > BAND * v {
                    lo -= 1;
                }
                while hi < n - 1 && speed[hi] > BAND * v {
                    hi += 1;
                }
                if lo == 0 && !authored {
                    reject("starts before the clip");
                    continue;
                }
                if hi == n - 1 && !continues {
                    reject("runs past the clip end");
                    continue;
                }
                // The arc must pass in front of the body between the peak and the band's end: a jab is
                // ahead at its peak, a hook is fastest out to the side and comes around in front after.
                if (i..=hi).map(|j| aim_at(&frames[j], bones, limb)).fold(f32::NEG_INFINITY, f32::max) < AHEAD {
                    reject("not ahead of the body");
                    continue;
                }
                let (windup, recover) = ((i - lo) as f32 * dt, (hi - i) as f32 * dt);
                if windup > MAX_WINDUP {
                    reject("wind-up too long");
                    continue;
                }
                if recover > MAX_RECOVER {
                    reject("recovery too long");
                    continue;
                }
                if (yaw[hi] - yaw[lo]).abs() > MAX_TURN {
                    reject("turns too far");
                    continue;
                }
                last_peak = t;
                out.push(Strike { track: track.to_string(), kind, bone: names(limb), start: lo as f32 * dt, peak: t, end: hi as f32 * dt, speed: v });
            }
        }
    }
    out.sort_by(|a, b| a.peak.total_cmp(&b.peak));
    rejected.sort_by(|a, b| a.peak.total_cmp(&b.peak));
    (out, rejected)
}

/// An unarmed attack playing `s`: the clip window around it, active frames on the measured contact.
pub fn strike_attack(s: &Strike, duration: f32) -> Attack {
    let start = (s.start - 0.12).max(0.0);
    let end = (s.end + 0.3).min(duration).max(s.peak + 0.1);
    let len = (end - start).max(0.1);
    let windup = ((s.peak - 0.05 - start) / len).clamp(0.05, 0.9);
    let active = ((s.peak + 0.07 - start) / len).clamp(windup + 0.02, 0.95);
    let cancel = (active + 0.15).min(0.97);
    let fast = ((s.speed - PUNCH_SPEED) / PUNCH_SPEED).clamp(0.0, 1.0);
    let (damage, poise, knockback, stamina, travel, radius) = match s.kind {
        Kind::Punch => (7.0 + 3.0 * fast, 10.0, 1.0, 9.0, 0.25, 0.1),
        Kind::Kick => (13.0 + 4.0 * fast, 30.0, 3.0, 18.0, 0.35, 0.12),
    };
    Attack {
        name: format!("{}@{:.1}", s.track, s.peak),
        clip: ClipRef { track: s.track.clone(), start, end: Some(end), speed: 1.0, mask: default(), style: None },
        recover: None,
        windup,
        active,
        cancel,
        damage,
        poise,
        knockback,
        hitbox: Some(Hitbox { bone: s.bone.clone(), tip: None, radius }),
        travel,
        stamina,
        fires: false,
        charge: 0.0,
    }
}

/// Strike clips the library may cut from: UAL's punches and hooks (not their recoveries or the
/// sword combos) and Kevin Iglesias' unarmed clips (`kev*/` punches and kicks; his weapon swings
/// would read as punches to the cutter). The user's call: these match the hand-animated look; the
/// mocap takes do not.
pub fn library_track(track: &str) -> bool {
    let Some((lib, clip)) = track.split_once('/') else { return false };
    match lib {
        "ual1" | "ual2" => (clip.contains("Punch") || clip.contains("Melee_Hook") || clip.contains("Kick")) && !clip.ends_with("_Rec"),
        _ => lib.starts_with("kev") && ["Punch", "Kick", "Unarmed", "Boxing"].iter().any(|k| clip.contains(k)),
    }
}

fn build_library(mut commands: Commands, bank: Option<Res<PoseBank>>, library: Option<Res<StrikeLibrary>>) {
    if library.is_some() {
        return;
    }
    let Some(bank) = bank else { return };
    let started = std::time::Instant::now();
    let mut lib = StrikeLibrary::default();
    let mut tracks: Vec<&String> = bank.tracks.keys().filter(|k| library_track(k)).collect();
    tracks.sort();
    for track in tracks {
        let duration = bank.duration(track);
        for s in segment(&bank, track) {
            let attack = strike_attack(&s, duration);
            match s.kind {
                Kind::Punch => lib.punches.push(attack),
                Kind::Kick => lib.kicks.push(attack),
            }
        }
    }
    info!("[repertoire] {} punches, {} kicks from the UAL / Kevin Iglesias clips in {:.2}s", lib.punches.len(), lib.kicks.len(), started.elapsed().as_secs_f32());
    commands.insert_resource(lib);
}

/// Each fighter starts with a few library strikes for its fists; the player gets all of them.
fn seed(library: Option<Res<StrikeLibrary>>, mut fighters: Query<(Entity, &mut Repertoire, Has<Player>)>) {
    let Some(library) = library else { return };
    for (entity, mut rep, player) in &mut fighters {
        if rep.seeded {
            continue;
        }
        rep.seeded = true;
        if player {
            for (pool, slot) in [(&library.punches, Slot::Light), (&library.kicks, Slot::Heavy)] {
                for attack in pool.iter() {
                    if !rep.extras.iter().any(|e| e.attack.name == attack.name) {
                        rep.extras.push(Extra { set: "unarmed".into(), slot, attack: attack.clone() });
                    }
                }
            }
            continue;
        }
        let mut rng = entity.to_bits() as u32 ^ 0x9e37_79b9;
        for (pool, count, slot) in [(&library.punches, SEED_PUNCHES, Slot::Light), (&library.kicks, SEED_KICKS, Slot::Heavy)] {
            for _ in 0..count.min(pool.len()) {
                let pick = (roll(&mut rng) * pool.len() as f32) as usize % pool.len();
                let attack = pool[pick].clone();
                if !rep.extras.iter().any(|e| e.attack.name == attack.name) {
                    rep.extras.push(Extra { set: "unarmed".into(), slot, attack });
                }
            }
        }
    }
}

/// The learned copy of `attack` for a learner with random state `rng`.
pub fn learned_copy(attack: &Attack, who: &str, rng: &mut u32) -> Attack {
    let speed = LEARN_SPEED.0 + (LEARN_SPEED.1 - LEARN_SPEED.0) * roll(rng);
    let mut a = attack.clone();
    a.name = format!("{} (from {who})", attack.name);
    a.clip.speed *= speed;
    if let Some(r) = a.recover.as_mut() {
        r.speed *= speed;
    }
    a
}

fn learn(
    mut seen: MessageReader<Witnessed>,
    mut fighters: Query<(&mut Repertoire, Has<Player>)>,
    mut log: MessageWriter<HudLog>,
) {
    for w in seen.read() {
        let Ok((mut rep, player)) = fighters.get_mut(w.learner) else { continue };
        let key = (w.set.clone(), w.attack.name.clone());
        let count = rep.seen.entry(key).or_insert(0);
        *count += 1;
        if *count != LEARN_AFTER {
            continue;
        }
        let known = rep.extras.iter().any(|e| e.set == w.set && (e.attack.name == w.attack.name || e.attack.clip.track == w.attack.clip.track && e.attack.clip.start == w.attack.clip.start));
        if known {
            continue;
        }
        let mut rng = w.learner.to_bits() as u32 ^ (rep.extras.len() as u32).wrapping_mul(0x85eb_ca6b);
        let attack = learned_copy(&w.attack, &w.who, &mut rng);
        info!("[repertoire] {} learned {} ({})", w.learner, attack.name, w.set);
        if player {
            log.write(HudLog::new(LogKind::Info, format!("Learned {}'s {} ({})", w.who, w.attack.name, w.set)));
        }
        rep.extras.push(Extra { set: w.set.clone(), slot: w.slot, attack });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_only_cuts_ual_and_kevin_iglesias_strikes() {
        assert!(library_track("ual1/Punch_Jab") && library_track("ual1/Punch_Cross") && library_track("ual2/Melee_Hook"));
        assert!(library_track("kev_melee/Kick_Front") && library_track("kev/Punch_Left"));
        assert!(!library_track("kev_melee/Attack1H01_R") && !library_track("kev_melee/Run01_Forward"), "weapon swings and runs are not fist strikes");
        assert!(!library_track("ual2/Melee_Hook_Rec"), "recoveries are not strikes");
        assert!(!library_track("ual2/Sword_Regular_Combo") && !library_track("ual1/Walk_Loop"));
        assert!(!library_track("cmu/144_20_punch_sequence") && !library_track("rokoko/Kick_Side") && !library_track("sekiro/Zombie Kick"));
        assert!(!library_track("Punch_Jab"), "a bare clip name has no library");
    }
    use std::sync::OnceLock;

    fn bank() -> &'static PoseBank {
        static BANK: OnceLock<PoseBank> = OnceLock::new();
        BANK.get_or_init(|| PoseBank::build().unwrap())
    }

    #[test]
    fn finds_strikes_in_the_martial_arts_takes() {
        let bank = bank();
        let punches = segment(bank, "cmu/144_20_punch_sequence");
        assert!(punches.iter().filter(|s| s.kind == Kind::Punch).count() >= 4, "{punches:?}");
        let kicks = segment(bank, "cmu/144_05_front_kicking");
        assert!(kicks.iter().any(|s| s.kind == Kind::Kick), "{kicks:?}");
        for s in punches.iter().chain(&kicks) {
            assert!(s.start < s.peak && s.peak < s.end);
            assert!(s.peak - s.start <= MAX_WINDUP + 1e-3 && s.end - s.peak <= MAX_RECOVER + 1e-3);
        }
        // Standing still is not a strike.
        assert!(segment(bank, "ual1/Idle_Loop").is_empty());
    }

    /// The strike table Kev reads (tools/kev/h2h.py), so the analysis and the game use one cutter:
    /// every library track plus the tracks in Kev's hand-to-hand pool, run through `segment` and
    /// `strike_attack` exactly as the game does. Writes `$KEV_STRIKES` (default
    /// staging/kev/strikes.json). Run: `cargo test -p guildbound export_strike_table -- --ignored`.
    #[test]
    #[ignore = "writes a file for Kev; run on demand"]
    fn export_strike_table() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pool: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("tools/kev/h2h.json")).expect("h2h.json")).expect("h2h.json parses");
        let bank = bank();
        let mut tracks: Vec<String> = bank.tracks.keys().filter(|k| library_track(k)).cloned().collect();
        for e in pool["pool"].as_array().into_iter().flatten() {
            if let Some(t) = e["track"].as_str() {
                tracks.push(t.to_string());
            }
        }
        tracks.sort();
        tracks.dedup();
        let mut rows = Vec::new();
        let mut rejected = Vec::new();
        for track in &tracks {
            if !bank.tracks.contains_key(track) {
                continue;
            }
            let duration = bank.duration(track);
            let (strikes, turned_down) = segment_explained(bank, track);
            for r in turned_down {
                rejected.push(serde_json::json!({
                    "track": track, "kind": format!("{:?}", r.kind), "bone": r.bone, "peak": r.peak, "speed": r.speed, "reason": r.reason,
                }));
            }
            for s in strikes {
                let a = strike_attack(&s, duration);
                rows.push(serde_json::json!({
                    "track": s.track, "kind": format!("{:?}", s.kind), "bone": s.bone,
                    "start": s.start, "peak": s.peak, "end": s.end, "speed": s.speed, "duration": duration,
                    "attack": {"start": a.clip.start, "end": a.clip.end, "windup": a.windup, "active": a.active, "cancel": a.cancel},
                }));
            }
        }
        let out = std::env::var("KEV_STRIKES").map(std::path::PathBuf::from).unwrap_or_else(|_| root.join("staging/kev/strikes.json"));
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let body = serde_json::json!({"tracks": tracks, "strikes": rows, "rejected": rejected});
        std::fs::write(&out, serde_json::to_string_pretty(&body).unwrap()).unwrap();
        assert!(!rows.is_empty(), "no strikes cut from {} tracks", tracks.len());
    }

    /// Best direction of a strike against the body's facing (cos) from its peak to its end: +1 straight ahead.
    fn aim(bank: &PoseBank, s: &Strike) -> f32 {
        let (sk, b) = (&bank.skeleton, &bank.bones);
        let limb = sk.find(&s.bone).unwrap();
        let steps = ((s.end - s.peak) * SAMPLE_HZ).round() as usize;
        (0..=steps)
            .map(|k| {
                let w = sk.fk(&bank.sample_raw(&s.track, s.peak + k as f32 / SAMPLE_HZ, false), b.pelvis);
                aim_at(&w, b, limb)
            })
            .fold(f32::NEG_INFINITY, f32::max)
    }

    #[test]
    fn the_library_is_large_and_every_strike_goes_forward() {
        let bank = bank();
        let mut all = Vec::new();
        for track in bank.tracks.keys().filter(|k| k.starts_with("cmu/")) {
            all.extend(segment(bank, track));
        }
        let punches = all.iter().filter(|s| s.kind == Kind::Punch).count();
        let kicks = all.len() - punches;
        let forward = all.iter().filter(|s| aim(bank, s) >= AHEAD - 1e-3).count();
        eprintln!("library: {punches} punches, {kicks} kicks, {forward} of {} forward", all.len());
        assert!(punches >= 40 && kicks >= 15, "{punches} punches, {kicks} kicks");
        assert_eq!(forward, all.len(), "every library strike goes forward");
    }

    #[test]
    fn three_blows_of_the_same_move_teach_it_once() {
        #[derive(Resource, Default)]
        struct Told(usize);
        let mut app = App::new();
        app.add_message::<Witnessed>()
            .add_message::<HudLog>()
            .init_resource::<Told>()
            .add_systems(Update, (learn, |mut r: MessageReader<HudLog>, mut told: ResMut<Told>| told.0 += r.read().count()).chain());
        let player = app.world_mut().spawn((Repertoire::default(), Player)).id();
        let s = Strike { track: "cmu/x".into(), kind: Kind::Punch, bone: "hand_r".into(), start: 0.2, peak: 0.4, end: 0.6, speed: 6.0 };
        let attack = Attack { name: "slash_b".into(), ..strike_attack(&s, 2.0) };
        let blow = || Witnessed { learner: player, who: "Opponent".into(), set: "katana".into(), slot: Slot::Light, attack: attack.clone() };
        for _ in 0..5 {
            app.world_mut().write_message(blow());
            app.update();
        }
        let rep = app.world().get::<Repertoire>(player).unwrap();
        let learned: Vec<_> = rep.for_slot("katana", Slot::Light).collect();
        assert_eq!(learned.len(), 1, "learned once, after the third blow");
        assert!(learned[0].name.contains("Opponent"));
        assert_eq!(rep.for_slot("unarmed", Slot::Light).count(), 0, "only for the moveset it came from");
        assert_eq!(app.world().resource::<Told>().0, 1, "the player is told once");
    }

    #[test]
    fn a_strike_attack_lands_on_the_measured_contact() {
        let s = Strike { track: "cmu/x".into(), kind: Kind::Punch, bone: "hand_r".into(), start: 2.0, peak: 2.2, end: 2.45, speed: 6.0 };
        let a = strike_attack(&s, 10.0);
        let (from, to) = (a.clip.start, a.clip.end.unwrap());
        let at = |f: f32| from + f * (to - from);
        assert!(at(a.windup) < s.peak && at(a.active) > s.peak, "active window {}..{} misses {}", at(a.windup), at(a.active), s.peak);
        assert!(a.windup < a.active && a.active < a.cancel && a.cancel < 1.0);
        assert_eq!(a.hitbox.as_ref().unwrap().bone, "hand_r");
    }

    #[test]
    fn a_learned_move_keeps_its_windows_and_gets_its_own_tempo() {
        let s = Strike { track: "cmu/x".into(), kind: Kind::Kick, bone: "foot_l".into(), start: 1.0, peak: 1.3, end: 1.7, speed: 8.0 };
        let base = strike_attack(&s, 5.0);
        let mut rng = 7;
        let copy = learned_copy(&base, "Varro", &mut rng);
        assert!(copy.name.contains("Varro"));
        assert_eq!((copy.windup, copy.active, copy.cancel), (base.windup, base.active, base.cancel));
        assert!((LEARN_SPEED.0..=LEARN_SPEED.1).contains(&copy.clip.speed));
    }
}

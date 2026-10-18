//! Data-driven movesets: `assets/anim/movesets/*.json`, one file per weapon / school / tool.
//! Loaded once at startup into `Movesets`. Every `track` must be a `PoseBank` track name
//! (`"{lib}/{clip}"` or `"authored/..."`); `validate` warns about unknown tracks.
//! Weapon meshes are attached to the hand bone of the equipped moveset (see `attach_weapons`).
use crate::motion::Mask;
use crate::motion::timing::Style;
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Unarmed,
    Melee,
    Ranged,
    Magic,
    Tool,
}

fn one() -> f32 {
    1.0
}

/// A window of a bank track.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ClipRef {
    pub track: String,
    /// Start / end seconds in the track (end defaults to the track duration).
    #[serde(default)]
    pub start: f32,
    #[serde(default)]
    pub end: Option<f32>,
    #[serde(default = "one")]
    pub speed: f32,
    #[serde(default)]
    pub mask: Mask,
    #[serde(default)]
    pub style: Option<Style>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Hitbox {
    /// Canonical bone the hit volume follows (e.g. "hand_r", "foot_r", or the weapon hand).
    pub bone: String,
    /// Segment from `bone` origin to `tip` (bone-local metres). Blades use the weapon length.
    #[serde(default)]
    pub tip: Option<[f32; 3]>,
    pub radius: f32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Attack {
    pub name: String,
    pub clip: ClipRef,
    /// Played when the chain stops after this attack: the way back to the stance for clips that
    /// end mid-motion (UAL2 sword / hook clips come as `X` + `X_Rec`). The next chained attack
    /// starts from this attack's own end instead.
    #[serde(default)]
    pub recover: Option<ClipRef>,
    /// Fractions of the clip window: active frames are [windup, active); after `cancel` the next
    /// chained attack (or a dodge) may start.
    pub windup: f32,
    pub active: f32,
    pub cancel: f32,
    pub damage: f32,
    pub poise: f32,
    pub knockback: f32,
    #[serde(default)]
    pub hitbox: Option<Hitbox>,
    /// Forward lunge distance (m) on top of the clip's own root travel, eased in from the clip
    /// start to the end of the active window.
    #[serde(default)]
    pub travel: f32,
    #[serde(default)]
    pub stamina: f32,
    /// Ranged / magic: fire `Moveset::projectile` at the start of the active window.
    #[serde(default)]
    pub fires: bool,
    /// Heavy attacks: seconds the input may be held to charge (damage ×(1 + charge)).
    #[serde(default)]
    pub charge: f32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Guard {
    pub clip: ClipRef,
    /// Pose time held while blocking (seconds in the track).
    pub hold: f32,
    /// Seconds after pressing block during which an incoming hit is parried.
    pub parry_window: f32,
    /// 0..1 damage blocked.
    pub absorb: f32,
    /// Frontal arc (degrees) that is protected.
    pub arc: f32,
    #[serde(default)]
    pub parry: Option<ClipRef>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Aim {
    /// Upper-body pose while aiming (held at `hold` seconds) and optional draw / release clips.
    pub clip: ClipRef,
    pub hold: f32,
    #[serde(default)]
    pub draw: Option<ClipRef>,
    #[serde(default)]
    pub release: Option<ClipRef>,
    #[serde(default)]
    pub reload: Option<ClipRef>,
    /// Seconds to full draw (bow) / charge (spell); 0 = instant.
    #[serde(default)]
    pub draw_time: f32,
    /// Body yaw relative to the aim while aiming (deg): an archer clip shoots side-on, so the
    /// body turns this far for the shot to leave along the aim.
    #[serde(default)]
    pub body_yaw_deg: f32,
    /// Pitch of the shot in the clip's own pose at full draw (deg, + up): the spine bends by the
    /// aim's pitch less this (`ContactRequest.aim`).
    #[serde(default)]
    pub pose_pitch_deg: f32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ProjectileDef {
    /// "arrow" | "bolt" | "knife" | "orb" | "spark" | "boulder" | "beam".
    pub kind: String,
    pub speed: f32,
    #[serde(default)]
    pub gravity: f32,
    pub radius: f32,
    pub damage: f32,
    pub poise: f32,
    pub knockback: f32,
    #[serde(default = "one_u32")]
    pub count: u32,
    #[serde(default)]
    pub spread: f32,
    #[serde(default = "lifetime")]
    pub lifetime: f32,
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    /// Magic element name ("fire", "frost", "storm", "arcane"...), drives VFX tint/behaviour.
    #[serde(default)]
    pub element: Option<String>,
    /// Hand that spawns it ("r" / "l").
    #[serde(default = "right")]
    pub hand: String,
}

fn one_u32() -> u32 {
    1
}
fn lifetime() -> f32 {
    4.0
}
fn right() -> String {
    "r".into()
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Primitive {
    /// "box" | "cylinder" | "sphere" | "sword" (built by `combat`: `size` = blade width,
    /// thickness, length; guard, grip and pommel follow from them).
    pub shape: String,
    pub size: [f32; 3],
    pub color: [f32; 3],
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct WeaponDef {
    /// glTF path relative to assets (scene 0), e.g. "packs/weapons/longsword.glb".
    #[serde(default)]
    pub mesh: Option<String>,
    #[serde(default)]
    pub primitive: Option<Primitive>,
    /// `mesh` placement under the weapon root (degrees XYZ Euler, then metres in the root's
    /// unscaled frame), so the blade runs along the root's +Y from the grip at its origin, as
    /// hitboxes assume. E.g. `battleaxe.glb` points its head down −Z with the grip at z = 0.17.
    #[serde(default)]
    pub mesh_rotation_deg: [f32; 3],
    #[serde(default)]
    pub mesh_offset: [f32; 3],
    /// "r" | "l".
    pub hand: String,
    /// Grip transform relative to the hand bone.
    #[serde(default)]
    pub offset: [f32; 3],
    /// Grip rotation relative to the hand bone (degrees, XYZ Euler).
    #[serde(default)]
    pub rotation_deg: [f32; 3],
    #[serde(default = "one")]
    pub scale: f32,
    /// Blade/haft length along the weapon's local +Y after rotation (used for hitboxes).
    #[serde(default)]
    pub length: f32,
    /// Where the weapon rides on the body when sheathed (`sheath`); `None` uses the default for
    /// the moveset's name (greatsword / axe on the back, katana left hip, dagger right hip).
    #[serde(default)]
    pub carry: Option<Carry>,
    /// Kilograms, for the carry posture (`play::carry`): a dagger 0.4, a katana 1.1, a
    /// greatsword 3.
    #[serde(default)]
    pub mass: f32,
}

/// A sheathed weapon's place on the body, in MODEL space at the rest pose (+Z forward, +X the
/// body's left, metres from the feet); it rides on `at`'s bone from there.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Carry {
    /// "back" (rides the chest, `spine_03`) or "hip" (rides the pelvis).
    pub at: String,
    /// Grip point (the weapon root's origin).
    pub grip: [f32; 3],
    /// Direction of the weapon's +Y (grip to tip).
    pub blade: [f32; 3],
    /// Direction the weapon's +Z faces.
    pub face: [f32; 3],
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Stance {
    /// Upper-body overlay while idle/moving with this moveset (held at `hold`).
    pub clip: ClipRef,
    pub hold: f32,
    /// 0..1 overlay weight while moving (1 when idle).
    #[serde(default = "one")]
    pub moving_weight: f32,
    /// Optional locomotion replacements (e.g. magic walk).
    #[serde(default)]
    pub walk: Option<String>,
    #[serde(default)]
    pub idle: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Moveset {
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub weapon: Option<WeaponDef>,
    #[serde(default)]
    pub offhand: Option<WeaponDef>,
    #[serde(default)]
    pub stance: Option<Stance>,
    /// Light chain (primary taps).
    #[serde(default)]
    pub light: Vec<Attack>,
    /// Heavy chain (primary hold).
    #[serde(default)]
    pub heavy: Vec<Attack>,
    #[serde(default)]
    pub sprint_attack: Option<Attack>,
    #[serde(default)]
    pub air_attack: Option<Attack>,
    #[serde(default)]
    pub guard: Option<Guard>,
    #[serde(default)]
    pub aim: Option<Aim>,
    #[serde(default)]
    pub projectile: Option<ProjectileDef>,
    /// Seconds between shots / casts.
    #[serde(default)]
    pub cooldown: f32,
}

#[derive(Resource, Default)]
pub struct Movesets(pub HashMap<String, Moveset>);

impl Movesets {
    pub fn get(&self, name: &str) -> Option<&Moveset> {
        self.0.get(name)
    }
    /// Load every `*.json` in `dir`. Files that fail to parse are reported, not fatal.
    pub fn load(dir: &std::path::Path) -> Self {
        let mut out = HashMap::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            warn!("[moveset] no directory {}", dir.display());
            return Self(out);
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        paths.sort();
        for path in paths {
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<Moveset>(&s).map_err(|e| e.to_string()))
            {
                Ok(set) => {
                    out.insert(set.name.clone(), set);
                }
                Err(e) => error!("[moveset] {}: {e}", path.display()),
            }
        }
        info!("[moveset] loaded {} movesets", out.len());
        Self(out)
    }
    /// Every track referenced by every moveset.
    pub fn tracks(&self) -> Vec<(String, String)> {
        let mut v = Vec::new();
        let mut push = |set: &str, c: &ClipRef| v.push((set.to_string(), c.track.clone()));
        for (name, s) in &self.0 {
            for a in s.light.iter().chain(&s.heavy).chain(&s.sprint_attack).chain(&s.air_attack) {
                push(name, &a.clip);
                if let Some(r) = &a.recover {
                    push(name, r);
                }
            }
            if let Some(g) = &s.guard {
                push(name, &g.clip);
                if let Some(p) = &g.parry {
                    push(name, p);
                }
            }
            if let Some(a) = &s.aim {
                push(name, &a.clip);
                for c in [&a.draw, &a.release, &a.reload].into_iter().flatten() {
                    push(name, c);
                }
            }
            if let Some(st) = &s.stance {
                push(name, &st.clip);
            }
        }
        v
    }
}

pub fn movesets_dir() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("anim/movesets")
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Movesets::load(&movesets_dir()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shipped moveset parses, every attack window is ordered, and every track exists in
    /// the pose bank.
    #[test]
    fn shipped_movesets_are_valid() {
        let files = std::fs::read_dir(movesets_dir()).expect("movesets dir").count();
        let sets = Movesets::load(&movesets_dir());
        assert!(files > 0);
        assert_eq!(sets.0.len(), files, "a moveset failed to parse or two share a name");
        for (name, set) in &sets.0 {
            let attacks = set.light.iter().chain(&set.heavy).chain(&set.sprint_attack).chain(&set.air_attack);
            for a in attacks {
                assert!(
                    0.0 <= a.windup && a.windup < a.active && a.active <= a.cancel && a.cancel <= 1.0,
                    "{name}/{}: windup {} active {} cancel {}",
                    a.name,
                    a.windup,
                    a.active,
                    a.cancel
                );
                assert!(a.damage > 0.0 && a.poise >= 0.0 && a.knockback >= 0.0, "{name}/{}", a.name);
            }
            if set.kind == Kind::Melee {
                let weapon = set.weapon.as_ref().expect("melee sets hold a weapon");
                assert!(weapon.length > 0.0, "{name}: weapon length");
            }
        }
        let (bank, _) = crate::motion::bank::PoseBank::build_with(std::path::Path::new("__no_poses__.json"))
            .expect("bank");
        for (set, track) in sets.tracks() {
            assert!(bank.has(&track), "{set}: unknown track {track}");
        }
    }
}

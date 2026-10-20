//! Per-limb hurtboxes: every rigged fighter carries `Hurtboxes`, eleven capsules placed on its bones
//! each frame (head sphere, two torso segments, upper and lower arms, thighs, calves; the ragdoll's
//! layout), so a blow or a shot lands on the limb it actually crosses, or passes between them.
//!
//! - `combat::hit_test` keeps the body capsule as a broad phase, then `refine`s against these; the
//!   region scales the damage (`Region::damage`: head 1.5, torso 1, arms 0.7, legs 0.8).
//! - `magic` and `ranged` projectiles do the same with a moving point (`refine` with radius).
//! - Built in `PostUpdate` after transforms propagate, so hit tests read last frame's pose, the
//!   same one the player saw.
use super::Fighter;
use super::combat::sweep;
use crate::motion::bank::PoseBank;
use crate::motion::Rig;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Head,
    Torso,
    Arm,
    Leg,
}

impl Region {
    /// Damage multiplier for a blow landing here.
    pub fn damage(self) -> f32 {
        match self {
            Region::Head => 1.5,
            Region::Torso => 1.0,
            Region::Arm => 0.7,
            Region::Leg => 0.8,
        }
    }
}

/// One capsule: a world segment and its radius.
#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub region: Region,
    pub seg: [Vec3; 2],
    pub radius: f32,
}

#[derive(Component, Clone, Debug, Default)]
pub struct Hurtboxes(pub Vec<Part>);

/// (bone, bone the capsule reaches to or None for the head sphere, region, radius m).
const LAYOUT: [(&str, Option<&str>, Region, f32); 11] = [
    ("pelvis", Some("spine_02"), Region::Torso, 0.16),
    ("spine_02", Some("neck_01"), Region::Torso, 0.18),
    ("Head", None, Region::Head, 0.12),
    ("upperarm_l", Some("lowerarm_l"), Region::Arm, 0.06),
    ("upperarm_r", Some("lowerarm_r"), Region::Arm, 0.06),
    ("lowerarm_l", Some("hand_l"), Region::Arm, 0.05),
    ("lowerarm_r", Some("hand_r"), Region::Arm, 0.05),
    ("thigh_l", Some("calf_l"), Region::Leg, 0.09),
    ("thigh_r", Some("calf_r"), Region::Leg, 0.09),
    ("calf_l", Some("foot_l"), Region::Leg, 0.065),
    ("calf_r", Some("foot_r"), Region::Leg, 0.065),
];
/// Head sphere centre past the head joint along neck → head (m).
const HEAD_REACH: f32 = 0.1;

/// The earliest part a capsule swept from `prev` to `now` (radius `radius`) touches: (sweep fraction,
/// contact point, region). None when it passes between them.
pub fn refine(prev: [Vec3; 2], now: [Vec3; 2], radius: f32, parts: &[Part]) -> Option<(f32, Vec3, Region)> {
    parts
        .iter()
        .filter_map(|p| sweep(prev, now, radius, p.seg, p.radius).map(|(f, _, point)| (f, point, p.region)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, attach).add_systems(PostUpdate, place.after(TransformSystems::Propagate));
}

fn attach(mut commands: Commands, fighters: Query<Entity, (With<Fighter>, With<Rig>, Without<Hurtboxes>)>) {
    for e in &fighters {
        commands.entity(e).insert(Hurtboxes::default());
    }
}

fn place(bank: Option<Res<PoseBank>>, mut bodies: Query<(&Rig, &mut Hurtboxes)>, globals: Query<&GlobalTransform>, mut layout: Local<Vec<(usize, Option<usize>, Region, f32)>>) {
    let Some(bank) = bank else { return };
    if layout.is_empty() {
        let sk = &bank.skeleton;
        *layout = LAYOUT
            .iter()
            .filter_map(|&(bone, end, region, r)| Some((sk.find(bone)?, end.and_then(|e| sk.find(e)), region, r)))
            .collect();
        if let Some(neck) = sk.find("neck_01") {
            // The head sphere reaches from the neck: stash the neck as its "end" for the direction.
            for l in layout.iter_mut().filter(|l| l.2 == Region::Head) {
                l.1 = Some(neck);
            }
        }
    }
    let at = |rig: &Rig, i: usize| -> Option<Vec3> {
        let e = rig.bones.get(i).copied().filter(|e| *e != Entity::PLACEHOLDER)?;
        let t = globals.get(e).ok()?.translation();
        t.is_finite().then_some(t)
    };
    for (rig, mut boxes) in &mut bodies {
        boxes.0.clear();
        for &(bone, end, region, radius) in layout.iter() {
            let (Some(a), Some(b)) = (at(rig, bone), end.and_then(|e| at(rig, e))) else { continue };
            let seg = if region == Region::Head {
                let c = a + (a - b).normalize_or(Vec3::Y) * HEAD_REACH;
                [c, c]
            } else {
                [a, b]
            };
            boxes.0.push(Part { region, seg, radius });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Vec<Part> {
        vec![
            Part { region: Region::Head, seg: [Vec3::new(0.0, 1.65, 0.0); 2], radius: 0.12 },
            Part { region: Region::Torso, seg: [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.45, 0.0)], radius: 0.18 },
            Part { region: Region::Leg, seg: [Vec3::new(0.1, 0.1, 0.0), Vec3::new(0.1, 0.9, 0.0)], radius: 0.09 },
            Part { region: Region::Leg, seg: [Vec3::new(-0.1, 0.1, 0.0), Vec3::new(-0.1, 0.9, 0.0)], radius: 0.09 },
        ]
    }

    fn fist(y: f32, x0: f32, x1: f32) -> ([Vec3; 2], [Vec3; 2]) {
        let p = |x: f32| [Vec3::new(x, y, -0.5), Vec3::new(x, y, -0.3)];
        (p(x0), p(x1))
    }

    #[test]
    fn a_blow_lands_on_the_limb_it_crosses() {
        let parts = body();
        let (prev, now) = fist(1.65, -0.6, 0.6);
        let prev = prev.map(|p| p + Vec3::Z * 0.4);
        let now = now.map(|p| p + Vec3::Z * 0.4);
        assert_eq!(refine(prev, now, 0.1, &parts).map(|h| h.2), Some(Region::Head));
        let (prev, now) = fist(1.2, -0.6, 0.6);
        let (prev, now) = (prev.map(|p| p + Vec3::Z * 0.4), now.map(|p| p + Vec3::Z * 0.4));
        assert_eq!(refine(prev, now, 0.1, &parts).map(|h| h.2), Some(Region::Torso));
    }

    #[test]
    fn a_thrust_between_the_legs_misses() {
        let parts = body();
        // A narrow point driven forward through the gap between the legs, below the torso.
        let prev = [Vec3::new(0.0, 0.5, -0.6), Vec3::new(0.0, 0.5, -0.6)];
        let now = [Vec3::new(0.0, 0.5, 0.6), Vec3::new(0.0, 0.5, 0.6)];
        assert!(refine(prev, now, 0.0, &parts).is_none());
    }

    #[test]
    fn head_blows_hit_hardest() {
        assert!(Region::Head.damage() > Region::Torso.damage());
        assert!(Region::Torso.damage() > Region::Leg.damage());
        assert!(Region::Leg.damage() > Region::Arm.damage());
    }
}

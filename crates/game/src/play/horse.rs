//! Horses: a small procedural mount under a rider, built from primitives as a child of the rider's
//! root (so it goes and turns with the rider; the rider's controller drives both).
//! - Body: barrel, neck, head with ears and a blaze, mane, tail, four two-part legs with hooves,
//!   a saddle and barding that falls over the rider's legs.
//! - Gait (`gait`): the legs swing in a trot (diagonal pairs) with the knees folding on the
//!   forward swing, by a phase that advances with the distance the rider covers (`STRIDE`); the
//!   barrel bobs twice a stride, the head nods, the tail sways. At rest the horse breathes and now
//!   and then shifts a hoof.
//! - Seat (`seat`): the rider gets `select::Mounted` (the seated clip holds its legs), and its
//!   model is raised until its pelvis sits on the saddle, measured from the bones (so it works for
//!   any rider size).
//! - Body: a capsule collider in the character layer, so others bump into the horse; the rider's
//!   controller leaves it out of its own slide and ground probe (`Ridden.1`).
use super::select::Mounted;
use super::{HALF_HEIGHT, Mover};
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Ground covered per trot cycle (m), saddle top above the ground (m), the seated clip.
const STRIDE: f32 = 1.5;
pub const SADDLE: f32 = 1.12;
const SEAT_TRACK: &str = "ual1/Sitting_Idle_Loop";
const SEAT_TIME: f32 = 0.5;
/// How fast the rider's model eases onto the saddle (per s).
const SEAT_RATE: f32 = 6.0;

/// The horse under a rider (on the horse's root entity).
#[derive(Component)]
pub struct Horse {
    pub rider: Entity,
    phase: f32,
    clock: f32,
    /// Leg pivots [front left, front right, hind left, hind right] and their knees.
    hips: [Entity; 4],
    knees: [Entity; 4],
    head: Entity,
    tail: Entity,
    barrel: Entity,
}

/// On a rider: its horse and the horse's collider (which the rider's own controller ignores).
#[derive(Component)]
pub struct Ridden(pub Entity, pub Entity);

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (gait, seat));
}

/// Leg swing (rad) and knee fold for a leg at trot `phase` (0..1) at `speed` (m/s).
pub fn leg_angles(phase: f32, offset: f32, speed: f32) -> (f32, f32) {
    let amp = (speed * 0.18).clamp(0.0, 0.6);
    let a = TAU * (phase + offset);
    let swing = amp * a.sin();
    // The knee folds while the leg comes forward (swing rising).
    let fold = (amp * 1.6 * a.cos()).max(0.0);
    (swing, fold)
}

/// Spawn a horse under `rider` (a character root) and seat the rider on it.
pub fn spawn_horse(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rider: Entity,
    coat: Color,
    trim: Color,
) -> Entity {
    let mat = |materials: &mut Assets<StandardMaterial>, c: Color, rough: f32| {
        materials.add(StandardMaterial { base_color: c, perceptual_roughness: rough, ..default() })
    };
    let coat_m = mat(materials, coat, 0.8);
    let dark = mat(materials, Color::srgb(0.08, 0.06, 0.05), 0.9);
    let leather = mat(materials, Color::srgb(0.22, 0.13, 0.07), 0.7);
    let cloth = mat(materials, trim, 0.9);
    let white = mat(materials, Color::srgb(0.85, 0.82, 0.76), 0.8);
    let root = commands
        .spawn((Name::new("Horse"), Transform::from_xyz(0.0, -HALF_HEIGHT, 0.0), Visibility::default(), ChildOf(rider)))
        .id();
    let mut part = |commands: &mut Commands, parent: Entity, name: &str, mesh: Mesh, m: &Handle<StandardMaterial>, tf: Transform| {
        commands.spawn((Name::new(name.to_string()), Mesh3d(meshes.add(mesh)), MeshMaterial3d(m.clone()), tf, ChildOf(parent))).id()
    };
    let pivot = |commands: &mut Commands, parent: Entity, name: &str, at: Vec3| {
        commands.spawn((Name::new(name.to_string()), Transform::from_translation(at), Visibility::default(), ChildOf(parent))).id()
    };
    // The barrel (everything above the legs bobs with it).
    let barrel = pivot(commands, root, "Horse barrel", Vec3::ZERO);
    part(commands, barrel, "Horse body", Capsule3d::new(0.3, 0.75).into(), &coat_m, Transform::from_xyz(0.0, 0.82, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)));
    // Neck and head.
    let neck_tf = Transform::from_xyz(0.0, 1.05, 0.55).with_rotation(Quat::from_rotation_x(0.55));
    part(commands, barrel, "Horse neck", Capsule3d::new(0.15, 0.42).into(), &coat_m, neck_tf);
    part(commands, barrel, "Horse mane", Cuboid::new(0.05, 0.55, 0.12).into(), &dark, Transform::from_xyz(0.0, 1.13, 0.47).with_rotation(Quat::from_rotation_x(0.55)));
    let head = pivot(commands, barrel, "Horse head pivot", Vec3::new(0.0, 1.3, 0.72));
    part(commands, head, "Horse head", Cuboid::new(0.2, 0.22, 0.46).into(), &coat_m, Transform::from_xyz(0.0, -0.06, 0.18).with_rotation(Quat::from_rotation_x(0.45)));
    part(commands, head, "Horse blaze", Cuboid::new(0.06, 0.01, 0.3).into(), &white, Transform::from_xyz(0.0, 0.04, 0.2).with_rotation(Quat::from_rotation_x(0.45)));
    for s in [-1.0f32, 1.0] {
        part(commands, head, "Horse ear", Cone { radius: 0.04, height: 0.12 }.into(), &coat_m, Transform::from_xyz(s * 0.07, 0.1, -0.02));
    }
    // Tail.
    let tail = pivot(commands, barrel, "Horse tail pivot", Vec3::new(0.0, 0.95, -0.66));
    part(commands, tail, "Horse tail", Cuboid::new(0.08, 0.55, 0.06).into(), &dark, Transform::from_xyz(0.0, -0.27, -0.04));
    // Saddle and barding (it hangs over the rider's legs).
    part(commands, barrel, "Saddle", Cuboid::new(0.46, 0.1, 0.5).into(), &leather, Transform::from_xyz(0.0, SADDLE - 0.05, -0.02));
    for s in [-1.0f32, 1.0] {
        part(commands, barrel, "Barding", Cuboid::new(0.03, 0.5, 0.72).into(), &cloth, Transform::from_xyz(s * 0.33, 0.86, -0.02).with_rotation(Quat::from_rotation_z(s * 0.12)));
    }
    // Legs: hip pivot, upper leg hanging from it, knee pivot, lower leg, hoof.
    let spots = [Vec3::new(0.16, 0.7, 0.42), Vec3::new(-0.16, 0.7, 0.42), Vec3::new(0.16, 0.7, -0.42), Vec3::new(-0.16, 0.7, -0.42)];
    let mut hips = [Entity::PLACEHOLDER; 4];
    let mut knees = [Entity::PLACEHOLDER; 4];
    for (i, at) in spots.into_iter().enumerate() {
        let hip = pivot(commands, root, "Horse hip", at);
        part(commands, hip, "Horse thigh", Capsule3d::new(0.07, 0.24).into(), &coat_m, Transform::from_xyz(0.0, -0.17, 0.0));
        let knee = pivot(commands, hip, "Horse knee", Vec3::new(0.0, -0.35, 0.0));
        part(commands, knee, "Horse shin", Capsule3d::new(0.045, 0.24).into(), &coat_m, Transform::from_xyz(0.0, -0.16, 0.0));
        part(commands, knee, "Horse hoof", Cylinder::new(0.06, 0.07).into(), &dark, Transform::from_xyz(0.0, -0.32, 0.0));
        hips[i] = hip;
        knees[i] = knee;
    }
    // A body others bump into (it rides on the rider's kinematic body; the rider ignores it).
    let collider = commands
        .spawn((
            Name::new("Horse collider"),
            avian3d::prelude::Collider::capsule(0.32, 0.8),
            avian3d::prelude::CollisionLayers::new(super::Phys::Character, [super::Phys::World, super::Phys::Character]),
            Transform::from_xyz(0.0, 0.82, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ChildOf(root),
        ))
        .id();
    commands.entity(root).insert(Horse { rider, phase: 0.0, clock: 0.0, hips, knees, head, tail, barrel });
    commands.entity(rider).insert((Ridden(root, collider), Mounted { track: SEAT_TRACK.to_string(), time: SEAT_TIME }));
    root
}

/// Trot by the rider's speed; breathe at rest.
fn gait(time: Res<Time>, mut horses: Query<&mut Horse>, riders: Query<&Mover>, mut parts: Query<&mut Transform>) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for mut h in &mut horses {
        let Ok(mover) = riders.get(h.rider) else { continue };
        let speed = mover.velocity.with_y(0.0).length();
        h.phase = (h.phase + speed * dt / STRIDE).fract();
        h.clock += dt;
        // Trot: diagonal pairs (front left with hind right).
        for (i, offset) in [0.0f32, 0.5, 0.5, 0.0].into_iter().enumerate() {
            let (swing, fold) = leg_angles(h.phase, offset, speed);
            // Hind knees fold the other way.
            let knee = if i < 2 { -fold } else { fold };
            if let Ok(mut t) = parts.get_mut(h.hips[i]) {
                t.rotation = Quat::from_rotation_x(-swing);
            }
            if let Ok(mut t) = parts.get_mut(h.knees[i]) {
                t.rotation = Quat::from_rotation_x(knee);
            }
        }
        let moving = (speed / 2.0).min(1.0);
        let bob = (TAU * h.phase * 2.0).sin().abs() * 0.05 * moving + (h.clock * 1.7).sin() * 0.008 * (1.0 - moving);
        if let Ok(mut t) = parts.get_mut(h.barrel) {
            t.translation.y = bob;
        }
        if let Ok(mut t) = parts.get_mut(h.head) {
            t.rotation = Quat::from_rotation_x((TAU * h.phase * 2.0).sin() * 0.12 * moving + (h.clock * 0.6).sin() * 0.05);
        }
        if let Ok(mut t) = parts.get_mut(h.tail) {
            t.rotation = Quat::from_rotation_z((h.clock * 2.3).sin() * 0.25) * Quat::from_rotation_x(0.3 + 0.5 * moving);
        }
    }
}

/// Ease each rider's model up or down until its pelvis sits on the saddle.
fn seat(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    riders: Query<(&Rig, &GlobalTransform, &Ridden)>,
    globals: Query<&GlobalTransform>,
    mut models: Query<&mut Transform>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (rig, root, ridden) in &riders {
        let (Some(pelvis), Ok(horse)) = (rig.bones.get(bank.bones.pelvis).and_then(|b| globals.get(*b).ok()), globals.get(ridden.0)) else {
            continue;
        };
        // The saddle in the world (the horse root sits at the rider's feet).
        let want = horse.translation().y + SADDLE + 0.04;
        let gap = want - pelvis.translation().y;
        let Ok(mut m) = models.get_mut(rig.model) else { continue };
        let scale = root.compute_transform().scale.y.max(1e-3);
        m.translation.y = (m.translation.y + gap / scale * (SEAT_RATE * dt).min(1.0)).clamp(-HALF_HEIGHT - 0.5, -HALF_HEIGHT + 1.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trot_pairs_the_diagonal_legs_and_rests_still() {
        let (a, _) = leg_angles(0.2, 0.0, 4.0);
        let (b, _) = leg_angles(0.2, 0.5, 4.0);
        assert!((a + b).abs() < 1e-5, "front left and front right swing opposite");
        assert_eq!(leg_angles(0.3, 0.0, 0.0), (0.0, 0.0), "standing still");
        let (s, _) = leg_angles(0.25, 0.0, 30.0);
        assert!(s <= 0.6 + 1e-5, "capped");
    }
}

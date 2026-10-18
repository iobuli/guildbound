//! Hinged doors you open by hand: one with a knob (the hand closes round the knob, turns it,
//! and rides it as the leaf swings) and one without (both palms flat on the leaf, pushing).
//!
//! Each door is an `Interactable` root with a frame (two posts and a lintel, solid) and a leaf
//! hung on a hinge: the `Door leaf` child sits at the hinge post and turns about its vertical
//! axis; the planks (and knobs, on both faces) are its children. A thin `Door block` collider
//! fills the doorway while the leaf is shut and is switched off (`ColliderDisabled`) as soon as it
//! starts to open, so the open leaf never shoves anyone.
//!
//! Either face works. `aim` (before `Act`) serves the face of whoever is using the door
//! (`interact::Using` on it), else of the nearest character, judged from the leaf's current turn
//! (so an open leaf works too), and rewrites the door's `Interactable` (`stand`, `face_yaw`,
//! `hands`, `prompt`) and its `interact::Grasp` (a `Sphere` on the knob, or `Flat` on the leaf
//! face) for that face. `toggle` reacts to `UseEffect` (the knob has turned / the push has
//! landed): it flips `PropState.on`, and an opening leaf swings away from the user who opened it
//! (their side of the leaf at that moment). It logs how far the user's hand was from its target.
//!
//! The knob (a ball with a ridge across its face) sits on a `Knob` pivot that `turn_knobs` turns
//! about the spindle with the same weight that rolls the user's wrist (`interact::hand_weight`),
//! so knob and hand turn together, before the latch lets the leaf go at the use beat.
//!
//! `swing` eases the hinge, but never into a body: the leaf (hinge to free edge) is checked
//! against every character's capsule circle at its next angle; on contact the character is
//! nudged out of the way (`Mover.external`, at most `NUDGE` m/s) and the leaf holds that frame.
//!
//! Ownership: the door kinds' `Interactable` stand / face / hands / prompt and `Grasp`, their
//! `PropState.on`, the leaf and knob transforms, the block's `ColliderDisabled`, and a nudge on
//! `Mover.external` while a leaf pushes a character (all here).
use super::interact::{Grasp, Phase, PropState, UseEffect, Using, hand_weight};
use super::world::{Kit, UseSpec, place};
use super::{CAPSULE_RADIUS, Interactable, Mover, PlaySet};
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use crate::motion::hand::Shape;
use avian3d::prelude::*;
use bevy::prelude::*;

pub const KNOB_DOOR: Vec3 = Vec3::new(-2.0, 0.0, -7.5);
pub const PUSH_DOOR: Vec3 = Vec3::new(-4.5, 0.0, -7.5);
/// Leaf width, height, thickness (m).
const W: f32 = 1.0;
const H: f32 = 2.1;
const T: f32 = 0.06;
/// How far the leaf swings (rad) and how fast `amount` follows (1/s).
const OPEN: f32 = 1.6;
const RATE: f32 = 1.4;
/// Knob: distance from the hinge along the leaf, height, how far it stands off the face, radius.
const KNOB_X: f32 = W - 0.11;
const KNOB_Y: f32 = 1.0;
const KNOB_OUT: f32 = 0.055;
const KNOB_R: f32 = 0.032;
/// Wrist roll while the knob turns (rad).
const KNOB_TURN: f32 = -0.9;
/// Push door: where the two palms go (along the leaf from the hinge) and their height.
const PUSH_X: [f32; 2] = [0.42, 0.78];
const PUSH_Y: f32 = 1.12;
/// Standing distance back from the contact point (m), and the sideways shift that puts the knob
/// at the right hand.
const STAND_BACK: f32 = 0.52;
const KNOB_SIDE: f32 = 0.18;
/// Largest speed (m/s) at which a swinging leaf nudges a character out of its way.
const NUDGE: f32 = 1.2;
/// Interaction timing on `ual1/Interact` (2 s): the hands hold over `REACH`, the use lands at `BEAT`.
const TRACK: &str = "ual1/Interact";
const REACH: (f32, f32) = (0.45, 1.5);
const BEAT: f32 = 0.9;

#[derive(Component, Debug, Clone, Copy)]
pub struct HingedDoor {
    pub knob: bool,
    /// Which face the door is serving (+1 = the leaf's +Z face, −1 = its −Z face).
    side: f32,
    /// The way the leaf opens (+1 / −1 about +Y), set when it starts to open.
    open_dir: f32,
    /// 0 shut .. 1 fully open.
    amount: f32,
}

#[derive(Component)]
struct Leaf;

/// The pivot a knob turns on (spindle along the leaf's Z); `side` is the face it is on.
#[derive(Component)]
struct Knob {
    side: f32,
}

#[derive(Component)]
struct Block;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, aim.before(PlaySet::Act))
        .add_systems(Update, (toggle, turn_knobs, swing).chain().after(super::interact::drive));
}

/// Which face of a leaf (`leaf_world`, the leaf in world space) a point is on: +1 / −1.
pub fn face_of(leaf_world: &Transform, point: Vec3) -> f32 {
    let normal = leaf_world.rotation * Vec3::Z;
    if (point - leaf_world.translation).dot(normal) >= 0.0 { 1.0 } else { -1.0 }
}

/// Whether a leaf hung at `hinge` (world), turned `angle` about +Y from the root's `yaw`
/// rotation, comes within `radius` (+ half the leaf's thickness) of `body` in the ground plane.
pub fn leaf_hits(hinge: Vec3, base: Quat, angle: f32, body: Vec3, radius: f32) -> bool {
    let along = (base * Quat::from_rotation_y(angle) * Vec3::X).with_y(0.0);
    let (a, b) = (hinge.with_y(0.0), (hinge + along * W).with_y(0.0));
    let p = body.with_y(0.0);
    let ab = b - a;
    let u = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * u) < radius + T * 0.5
}

/// The face's contact geometry in leaf space: hand points `[left, right]`, grasp shapes, and the
/// point to stand back from.
fn contact(knob: bool, side: f32) -> ([Option<Vec3>; 2], [Option<Shape>; 2], Vec3) {
    if knob {
        let centre = Vec3::new(KNOB_X, KNOB_Y, side * (T * 0.5 + KNOB_OUT));
        let shape = Shape::Sphere { center: centre, radius: KNOB_R };
        ([None, Some(centre)], [None, Some(shape)], centre)
    } else {
        // Facing the leaf from its −Z face the body looks along +Z, so its left hand is the one
        // farther from the hinge; from the +Z face it is the nearer one.
        let (left, right) = if side < 0.0 { (PUSH_X[1], PUSH_X[0]) } else { (PUSH_X[0], PUSH_X[1]) };
        let face = side * T * 0.5;
        let point = |x: f32| Vec3::new(x, PUSH_Y, face);
        let flat = |x: f32| Shape::Flat { point: point(x), normal: Vec3::Z * side };
        ([Some(point(left)), Some(point(right))], [Some(flat(left)), Some(flat(right))], Vec3::new(0.5 * (PUSH_X[0] + PUSH_X[1]), PUSH_Y, face))
    }
}

/// Where to stand (root space, on the ground) and the yaw to face, for a contact point `c` on a
/// leaf turned by `leaf` (root space), served from `side`.
fn stand_for(leaf: &Transform, c: Vec3, side: f32, knob: bool) -> (Vec3, f32) {
    let normal = leaf.rotation * Vec3::Z * side;
    let facing = (-normal).with_y(0.0).normalize_or(Vec3::Z);
    let at = leaf.transform_point(c);
    let right = facing.cross(Vec3::Y);
    let shift = if knob { -right * KNOB_SIDE } else { Vec3::ZERO };
    let stand = (at - facing * STAND_BACK + shift).with_y(0.0);
    (stand, facing.x.atan2(facing.z))
}

#[allow(clippy::type_complexity)]
fn aim(
    bodies: Query<&GlobalTransform, With<Mover>>,
    users: Query<(&Using, &GlobalTransform)>,
    mut doors: Query<(Entity, &mut HingedDoor, &Transform, &mut Interactable, &mut Grasp, &PropState, &Children)>,
    leaves: Query<&Transform, (With<Leaf>, Without<HingedDoor>)>,
) {
    for (entity, mut door, root, mut it, mut grasp, prop, children) in &mut doors {
        let Some(leaf) = children.iter().find_map(|c| leaves.get(c).ok()) else { continue };
        let leaf_world = root.mul_transform(*leaf);
        // Whoever is using this door, else whoever is nearest.
        let user = users.iter().find(|(u, _)| u.target == entity).map(|(_, g)| g.translation());
        let nearest = || {
            bodies
                .iter()
                .map(|b| b.translation())
                .min_by(|a, b| a.distance_squared(root.translation).total_cmp(&b.distance_squared(root.translation)))
        };
        if let Some(body) = user.or_else(nearest) {
            door.side = face_of(&leaf_world, body);
        }
        let (hands, shapes, centre) = contact(door.knob, door.side);
        let (stand, face) = stand_for(leaf, centre, door.side, door.knob);
        it.stand = stand;
        it.face_yaw = face;
        it.hands = hands;
        grasp.shapes = shapes;
        grasp.twist = [0.0, if door.knob { KNOB_TURN } else { 0.0 }];
        let label = if prop.on { "Close door" } else { "Open door" };
        if it.prompt != label {
            it.prompt = label.to_string();
        }
    }
}

#[allow(clippy::type_complexity)]
fn toggle(
    mut effects: MessageReader<UseEffect>,
    bank: Option<Res<PoseBank>>,
    mut doors: Query<(&mut HingedDoor, &mut PropState, &Transform, &Children)>,
    leaves: Query<&Transform, (With<Leaf>, Without<HingedDoor>)>,
    users: Query<(&GlobalTransform, Option<&Rig>)>,
    globals: Query<&GlobalTransform>,
) {
    for e in effects.read() {
        let Ok((mut door, mut prop, root, children)) = doors.get_mut(e.target) else { continue };
        let Some(leaf) = children.iter().find_map(|c| leaves.get(c).ok()) else { continue };
        let leaf_world = root.mul_transform(*leaf);
        let user = users.get(e.user).ok();
        prop.on = !prop.on;
        if prop.on && door.amount < 0.05 {
            // Away from the opener: the free edge (+X from the hinge) turns toward −side·Z, and a
            // turn of +θ about +Y carries +X toward −Z, so the turn's sign is the opener's side.
            door.open_dir = user.map_or(door.side, |(g, _)| face_of(&leaf_world, g.translation()));
        }
        // How close the hand got to what it reached for (the knob, or the middle of the palms).
        let (targets, _, _) = contact(door.knob, door.side);
        let reach = user.and_then(|(_, rig)| {
            let (bank, rig) = (bank.as_deref()?, rig?);
            let mut worst = 0.0f32;
            for (i, t) in targets.iter().enumerate() {
                let Some(t) = t else { continue };
                let hand = globals.get(*rig.bones.get(bank.bones.hand[i])?).ok()?.translation();
                worst = worst.max(hand.distance(leaf_world.transform_point(*t)));
            }
            Some(worst)
        });
        let what = if door.knob { "knob door" } else { "push door" };
        let state = if prop.on { "opens" } else { "closes" };
        match reach {
            Some(d) => info!("[door] {what} {state} (hand {d:.2} m from its target)"),
            None => info!("[door] {what} {state}"),
        }
    }
}

/// Knobs turn with the hand that holds them.
#[allow(clippy::type_complexity)]
fn turn_knobs(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    users: Query<&Using>,
    doors: Query<(Entity, &HingedDoor, &Interactable, &Children)>,
    leaves: Query<&Children, With<Leaf>>,
    mut knobs: Query<(&Knob, &mut Transform)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, door, it, children) in &doors {
        if !door.knob {
            continue;
        }
        let duration = bank.as_deref().filter(|b| b.has(&it.track)).map_or(it.duration, |b| b.duration(&it.track)).max(0.2);
        let turn = users
            .iter()
            .find(|u| u.target == entity && u.phase == Phase::Use)
            .map_or(0.0, |u| hand_weight(u.t, duration, it.looping, it.reach));
        for leaf in children.iter().filter_map(|c| leaves.get(c).ok()) {
            for child in leaf.iter() {
                let Ok((knob, mut tf)) = knobs.get_mut(child) else { continue };
                // Only the knob in the hand turns; the far one follows the spindle back.
                let goal = if knob.side == door.side { KNOB_TURN * turn * knob.side } else { 0.0 };
                let (_, _, now) = tf.rotation.to_euler(EulerRot::XYZ);
                let next = now + (goal - now) * (12.0 * dt).min(1.0);
                tf.rotation = Quat::from_rotation_z(next);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn swing(
    mut commands: Commands,
    time: Res<Time>,
    mut doors: Query<(&mut HingedDoor, &PropState, &Transform, &Children)>,
    mut leaves: Query<&mut Transform, (With<Leaf>, Without<HingedDoor>)>,
    blocks: Query<(Entity, Has<ColliderDisabled>), With<Block>>,
    mut bodies: Query<(&GlobalTransform, &mut Mover)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let eased = |a: f32| a * a * (3.0 - 2.0 * a);
    for (mut door, prop, root, children) in &mut doors {
        let goal = if prop.on { 1.0 } else { 0.0 };
        let next = door.amount + (goal - door.amount).clamp(-RATE * dt, RATE * dt);
        // Never into a body: nudge it along the leaf's motion and hold this frame.
        if next != door.amount {
            let hinge = root.transform_point(Vec3::new(-W * 0.5, 0.0, 0.0));
            let (from, to) = (door.open_dir * OPEN * eased(door.amount), door.open_dir * OPEN * eased(next));
            let mut blocked = false;
            for (g, mut mover) in &mut bodies {
                let p = g.translation();
                if (p.y - root.translation.y).abs() > H + 1.0 || !leaf_hits(hinge, root.rotation, to, p, CAPSULE_RADIUS) {
                    continue;
                }
                blocked = true;
                // The leaf's motion at the body: tangential to the hinge, in the turn's direction.
                let arm = (p - hinge).with_y(0.0);
                let push = (Vec3::Y.cross(arm) * (to - from).signum()).normalize_or_zero() * NUDGE;
                if push.length_squared() > mover.external.with_y(0.0).length_squared() {
                    mover.external.x = push.x;
                    mover.external.z = push.z;
                }
            }
            if !blocked {
                door.amount = next;
            }
        }
        let eased = eased(door.amount);
        let shut = door.amount < 0.02 && !prop.on;
        for child in children.iter() {
            if let Ok(mut tf) = leaves.get_mut(child) {
                tf.rotation = Quat::from_rotation_y(door.open_dir * OPEN * eased);
            }
            if let Ok((e, disabled)) = blocks.get(child) {
                if shut && disabled {
                    commands.entity(e).remove::<ColliderDisabled>();
                } else if !shut && !disabled {
                    commands.entity(e).insert(ColliderDisabled);
                }
            }
        }
    }
}

/// The two doors, free-standing frames facing ±Z west of the noticeboard.
pub(crate) fn spawn_doors(kit: &mut Kit) {
    door(kit, "Knob door", KNOB_DOOR, true);
    door(kit, "Push door", PUSH_DOOR, false);
}

fn door(kit: &mut Kit, name: &str, at: Vec3, knob: bool) {
    let (wood, dark, metal, stone) = (kit.pal.wood.clone(), kit.pal.dark_wood.clone(), kit.pal.metal.clone(), kit.pal.stone.clone());
    let (hands, _, centre) = contact(knob, -1.0);
    let (stand, face) = stand_for(&Transform::from_xyz(-W * 0.5, 0.0, 0.0), centre, -1.0, knob);
    let root = place(kit, name, at, UseSpec {
        kind: if knob { "door_knob" } else { "door_push" },
        prompt: "Open door",
        track: TRACK,
        duration: 2.0,
        looping: false,
        stand,
        face: Some(face),
        hands,
        grip: Some("Door leaf"),
        beat: Some(BEAT),
        reach: Some(REACH),
    });
    kit.commands.entity(root).insert((HingedDoor { knob, side: -1.0, open_dir: -1.0, amount: 0.0 }, Grasp::default()));

    // Frame: posts either side of the opening and a lintel.
    let post = Vec3::new(0.12, H + 0.15, 0.16);
    for x in [-(W * 0.5 + 0.06), W * 0.5 + 0.06] {
        kit.part(root, "Door post", Cuboid::new(post.x, post.y, post.z), &stone, Transform::from_xyz(x, post.y * 0.5, 0.0), Some(Collider::cuboid(post.x, post.y, post.z)));
    }
    kit.part(root, "Door lintel", Cuboid::new(W + 0.24, 0.15, 0.16), &stone, Transform::from_xyz(0.0, H + 0.075, 0.0), Some(Collider::cuboid(W + 0.24, 0.15, 0.16)));
    kit.commands.spawn((
        Name::new("Door block"),
        Block,
        Collider::cuboid(W, H, T),
        super::world::world_layers(),
        Transform::from_xyz(0.0, H * 0.5, 0.0),
        ChildOf(root),
    ));

    // The leaf, hung at the left post: +X runs from the hinge to the free edge.
    let leaf = kit
        .commands
        .spawn((Name::new("Door leaf"), Leaf, Transform::from_xyz(-W * 0.5, 0.0, 0.0), Visibility::default(), ChildOf(root)))
        .id();
    kit.part(leaf, "Door planks", Cuboid::new(W - 0.02, H - 0.02, T), &wood, Transform::from_xyz(W * 0.5, H * 0.5, 0.0), None);
    for y in [0.45, H - 0.45] {
        for z in [-1.0f32, 1.0] {
            kit.part(leaf, "Door rail", Cuboid::new(W - 0.08, 0.1, 0.02), &dark, Transform::from_xyz(W * 0.5, y, z * (T * 0.5 + 0.01)), None);
        }
    }
    for y in [0.3, H - 0.3] {
        kit.part(leaf, "Door hinge", Cylinder::new(0.025, 0.16), &metal, Transform::from_xyz(0.0, y, 0.0), None);
    }
    for z in [-1.0f32, 1.0] {
        if knob {
            // Rose plate on the leaf, then the knob on a pivot turning about the spindle.
            kit.part(leaf, "Knob rose", Cylinder::new(0.035, 0.008), &metal, Transform::from_xyz(KNOB_X, KNOB_Y, z * (T * 0.5 + 0.004)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), None);
            let pivot = kit
                .commands
                .spawn((Name::new("Knob pivot"), Knob { side: z }, Transform::from_xyz(KNOB_X, KNOB_Y, z * T * 0.5), Visibility::default(), ChildOf(leaf)))
                .id();
            kit.part(pivot, "Knob stem", Cylinder::new(0.012, KNOB_OUT), &metal, Transform::from_xyz(0.0, 0.0, z * KNOB_OUT * 0.5).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), None);
            kit.part(pivot, "Door knob", Sphere::new(KNOB_R), &metal, Transform::from_xyz(0.0, 0.0, z * KNOB_OUT), None);
            // A ridge across the knob's face, so its turn shows.
            kit.part(pivot, "Knob ridge", Cuboid::new(KNOB_R * 2.3, 0.014, 0.022), &dark, Transform::from_xyz(0.0, 0.0, z * (KNOB_OUT + KNOB_R * 0.55)), None);
        } else {
            // Push plates where the palms go.
            for x in PUSH_X {
                kit.part(leaf, "Push plate", Cuboid::new(0.14, 0.2, 0.006), &metal, Transform::from_xyz(x, PUSH_Y, z * (T * 0.5 + 0.003)), None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_sits_at_the_right_hand_on_both_faces() {
        for side in [-1.0f32, 1.0] {
            let leaf = Transform::from_xyz(-W * 0.5, 0.0, 0.0);
            let (hands, shapes, centre) = contact(true, side);
            assert!(hands[0].is_none() && hands[1].is_some());
            assert!(matches!(shapes[1], Some(Shape::Sphere { .. })));
            let (stand, yaw) = stand_for(&leaf, centre, side, true);
            let facing = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            let right = facing.cross(Vec3::Y);
            let knob = leaf.transform_point(centre).with_y(0.0);
            // On the stander's side of the leaf, ahead of them and to their right.
            assert!(stand.z * side > 0.0, "side {side}: stand {stand}");
            assert!((knob - stand).dot(facing) > 0.4);
            assert!((knob - stand).dot(right) > 0.1);
        }
    }

    #[test]
    fn push_palms_are_left_and_right_of_the_body() {
        for side in [-1.0f32, 1.0] {
            let leaf = Transform::from_xyz(-W * 0.5, 0.0, 0.0);
            let (hands, shapes, centre) = contact(false, side);
            let (stand, yaw) = stand_for(&leaf, centre, side, false);
            let right = Vec3::new(yaw.sin(), 0.0, yaw.cos()).cross(Vec3::Y);
            let l = leaf.transform_point(hands[0].unwrap()) - stand;
            let r = leaf.transform_point(hands[1].unwrap()) - stand;
            assert!(r.dot(right) > l.dot(right), "side {side}: right hand must be to the right");
            let Some(Shape::Flat { normal, .. }) = shapes[0] else { panic!() };
            assert_eq!(normal, Vec3::Z * side);
        }
    }

    #[test]
    fn the_leaf_swings_away_from_its_user_not_the_nearest_body() {
        // A shut leaf (hinge at x = −W/2, faces ±Z); the user on its −Z face, an NPC on its +Z face.
        let leaf = Transform::from_xyz(-W * 0.5, 0.0, 0.0);
        let user = Vec3::new(0.3, 1.0, -0.9);
        let npc = Vec3::new(0.2, 1.0, 0.6);
        assert_eq!(face_of(&leaf, user), -1.0);
        assert_eq!(face_of(&leaf, npc), 1.0);
        // `toggle` turns the leaf by open_dir = the user's side: the free edge moves away to +Z.
        let open_dir = face_of(&leaf, user);
        let free_edge = Quat::from_rotation_y(open_dir * OPEN) * Vec3::X * W + leaf.translation;
        assert!(free_edge.z > 0.5, "free edge {free_edge}");
        assert!(free_edge.distance(user) > 1.0);
    }

    #[test]
    fn a_body_in_the_arc_stops_the_leaf_and_one_behind_the_hinge_does_not() {
        let hinge = Vec3::new(-W * 0.5, 0.0, 0.0);
        // Opening toward +Z (angle −OPEN): a body standing half a metre out on +Z in the arc.
        let in_arc = Vec3::new(-0.1, 1.0, 0.45);
        assert!(!leaf_hits(hinge, Quat::IDENTITY, 0.0, in_arc, CAPSULE_RADIUS), "shut leaf clear of it");
        assert!(leaf_hits(hinge, Quat::IDENTITY, -OPEN * 0.6, in_arc, CAPSULE_RADIUS), "the swinging leaf meets it");
        // Behind the hinge post, outside the leaf's reach.
        let behind = Vec3::new(-W * 0.5 - 0.7, 1.0, 0.3);
        for k in 0..=10 {
            assert!(!leaf_hits(hinge, Quat::IDENTITY, -OPEN * k as f32 / 10.0, behind, CAPSULE_RADIUS));
        }
    }

    #[test]
    fn an_open_leaf_is_served_from_its_new_face() {
        // Opened from the −Z face, the leaf swings toward +Z: its −Z face now looks along −X.
        let leaf = Transform::from_xyz(-W * 0.5, 0.0, 0.0).with_rotation(Quat::from_rotation_y(-OPEN));
        let (_, _, centre) = contact(true, -1.0);
        let (stand, _) = stand_for(&leaf, centre, -1.0, true);
        let knob = leaf.transform_point(centre);
        assert!(knob.z > 0.5, "the free edge swung away to +Z: {knob}");
        assert!(stand.distance(knob.with_y(0.0)) < 0.7);
    }
}

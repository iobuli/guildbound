//! Hinged doors you open by hand: one with a knob (the hand closes round the knob, turns it,
//! and rides it as the leaf swings) and one without (both palms flat on the leaf, pushing).
//!
//! Each door is an `Interactable` root with a frame (two posts and a lintel, solid) and a leaf
//! hung on a hinge: the `Door leaf` child sits at the hinge post and turns about its vertical
//! axis; the planks (and knobs, on both faces) are its children. A thin `Door block` collider
//! fills the doorway while the leaf is shut and is switched off (`ColliderDisabled`) as soon as it
//! starts to open, so the open leaf never shoves anyone.
//!
//! Either face works. `aim` (before `Act`) finds the nearest character and which face of the
//! leaf it is on (from the leaf's current turn, so an open leaf works too), then rewrites the
//! door's `Interactable` (`stand`, `face_yaw`, `hands`, `prompt`) and its `interact::Grasp` (a
//! `Sphere` on the knob, or `Flat` on the leaf face) for that face. While anyone is within
//! `FREEZE` of the doorway it leaves them alone, so the side cannot flip mid-use. `toggle` reacts
//! to `UseEffect` (the knob has turned / the push has landed): it flips `PropState.on`, and an
//! opening leaf swings away from whoever opened it. `swing` eases the hinge.
//!
//! Ownership: the door kinds' `Interactable` stand / face / hands / prompt and `Grasp`, their
//! `PropState.on`, the leaf transform and the block's `ColliderDisabled` (all here).
use super::interact::{Grasp, PropState, UseEffect};
use super::world::{Kit, UseSpec, place};
use super::{Interactable, Mover, PlaySet};
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
/// Nobody within this distance of the doorway: the door may switch the face it serves (m).
const FREEZE: f32 = 1.3;
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

#[derive(Component)]
struct Block;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, aim.before(PlaySet::Act))
        .add_systems(Update, (toggle, swing).chain().after(super::interact::drive));
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

fn aim(
    bodies: Query<&GlobalTransform, With<Mover>>,
    mut doors: Query<(&mut HingedDoor, &Transform, &mut Interactable, &mut Grasp, &PropState, &Children)>,
    leaves: Query<&Transform, (With<Leaf>, Without<HingedDoor>)>,
) {
    for (mut door, root, mut it, mut grasp, prop, children) in &mut doors {
        let Some(leaf) = children.iter().find_map(|c| leaves.get(c).ok()) else { continue };
        let leaf_world = root.mul_transform(*leaf);
        let nearest = bodies
            .iter()
            .map(|b| b.translation())
            .min_by(|a, b| a.distance_squared(root.translation).total_cmp(&b.distance_squared(root.translation)));
        if let Some(body) = nearest {
            let flat_dist = (body - root.translation).with_y(0.0).length();
            if flat_dist > FREEZE {
                let normal = leaf_world.rotation * Vec3::Z;
                door.side = if (body - leaf_world.translation).dot(normal) >= 0.0 { 1.0 } else { -1.0 };
            }
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

fn toggle(mut effects: MessageReader<UseEffect>, mut doors: Query<(&mut HingedDoor, &mut PropState)>) {
    for e in effects.read() {
        let Ok((mut door, mut prop)) = doors.get_mut(e.target) else { continue };
        prop.on = !prop.on;
        if prop.on && door.amount < 0.05 {
            // Away from the opener: the free edge (+X from the hinge) turns toward −side·Z, and a
            // turn of +θ about +Y carries +X toward −Z.
            door.open_dir = door.side;
        }
        info!("[door] {} {}", if door.knob { "knob door" } else { "push door" }, if prop.on { "opens" } else { "closes" });
    }
}

fn swing(
    mut commands: Commands,
    time: Res<Time>,
    mut doors: Query<(&mut HingedDoor, &PropState, &Children)>,
    mut leaves: Query<&mut Transform, With<Leaf>>,
    blocks: Query<(Entity, Has<ColliderDisabled>), With<Block>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (mut door, prop, children) in &mut doors {
        let goal = if prop.on { 1.0 } else { 0.0 };
        door.amount += (goal - door.amount).clamp(-RATE * dt, RATE * dt);
        let eased = door.amount * door.amount * (3.0 - 2.0 * door.amount);
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
            let stem = T * 0.5 + KNOB_OUT * 0.5;
            kit.part(leaf, "Knob stem", Cylinder::new(0.012, KNOB_OUT), &metal, Transform::from_xyz(KNOB_X, KNOB_Y, z * stem).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), None);
            kit.part(leaf, "Door knob", Sphere::new(KNOB_R), &metal, Transform::from_xyz(KNOB_X, KNOB_Y, z * (T * 0.5 + KNOB_OUT)), None);
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

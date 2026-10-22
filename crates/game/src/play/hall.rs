//! The Guild Hall complex on the terrace east of the courtyard (`world::TERRACE_MIN..MAX`), built
//! once at startup with `world::Kit`:
//! - a timber **gate** at the courtyard's east edge and a packed path to the foot of the steps;
//! - the **hall**: the ground floor of the `guildhouse` (which builds the building itself: podium,
//!   walls, upper floor, roof, tower), its floor `FLOOR_Y` up on the podium, a west door and a
//!   +z door to the yard, wall sconces. Inside: the **front desk** (counter, bell, ledgers behind;
//!   the receptionist stands at `DESK_STAND`), an indoor **bounty board** (interactable `board`),
//!   and the **lodging** corner (three `bed`s along the −z wall, a table and chairs);
//! - the **training ground** north of the hall: a sand yard, a fence, a stone-marked **duel ring**
//!   (`RING_CENTER`, `RING_RADIUS`), dummies, a weapon rack and a **balance beam** (`BEAM_START`–
//!   `BEAM_END`, `beam::Beam`) with a mounting block at each end.
//! **Doors**: both doorways hang a pair of leaves (`Door`) that swing open when anyone comes
//! within `DOOR_REACH` and close behind them (`swing`). Each leaf is a kinematic body, so a
//! closed door stops the camera and an open one leaves the gap.
//! Who works here and what they do is `guild` / `occupations`; otherwise this module only builds.
use super::world::{Kit, UseSpec, place, spawn_dummy};
use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// Hall footprint (x, z): the guildhouse's main block, wall centre lines (`guildhouse.json`
/// `main_block`, front on x = 25.75). Wall thickness (m).
pub const HALL_MIN: Vec2 = Vec2::new(25.75, -15.4);
pub const HALL_MAX: Vec2 = Vec2::new(35.65, -2.6);
pub const WALL_T: f32 = 0.4;
/// The hall floor: the top of the guildhouse podium (m).
pub const FLOOR_Y: f32 = 1.5;
/// The west door (centre z, width, height) and the +z door to the yard (centre x).
pub const DOOR_Z: f32 = -9.0;
const DOOR_W: f32 = 2.6;
const DOOR_H: f32 = 3.2;
pub const YARD_DOOR_X: f32 = 27.4;
/// The foot of the west steps (x) and of the yard steps (z): where walkers line up to climb.
pub const STEPS_FOOT_X: f32 = 20.75;
pub const YARD_STEPS_FOOT_Z: f32 = 1.5;
/// Where the receptionist stands behind the desk (feet) and the yaw she faces (the door).
pub const DESK_STAND: Vec3 = Vec3::new(34.4, FLOOR_Y, -9.0);
pub const DESK_FACE: f32 = -FRAC_PI_2;
/// The counter's centre line (x) the player talks across.
pub const DESK_X: f32 = 33.2;
/// The training ground's west fence (x).
const YARD_X0: f32 = 24.0;
/// The duel ring in the training ground.
pub const RING_CENTER: Vec3 = Vec3::new(32.0, 0.0, 7.0);
pub const RING_RADIUS: f32 = 3.6;
/// Where the Captain waits by the ring.
/// The balance beam's top line (along +x by the north fence) and width (m).
pub const BEAM_START: Vec3 = Vec3::new(28.6, 0.6, 12.0);
pub const BEAM_END: Vec3 = Vec3::new(37.6, 0.6, 12.0);
const BEAM_WIDTH: f32 = 0.14;
pub const CAPTAIN_POST: Vec3 = Vec3::new(36.5, 0.0, 7.0);
/// The gate at the courtyard's east edge.
pub const GATE: Vec3 = Vec3::new(16.5, 0.0, DOOR_Z);

/// Doors open for anyone this close (m), swinging this far (rad) at this rate (1/s).
const DOOR_REACH: f32 = 2.6;
const DOOR_OPEN: f32 = 1.75;
const DOOR_RATE: f32 = 5.0;

/// One door leaf: hinge yaw closed, opening direction (+1 / −1), current opening 0..1.
#[derive(Component)]
pub struct Door {
    closed: f32,
    dir: f32,
    open: f32,
    /// The doorway's centre: both leaves open for anyone near it.
    centre: Vec3,
}

/// Inside the hall's walls (x, z).
pub fn inside(p: Vec3) -> bool {
    p.x > HALL_MIN.x && p.x < HALL_MAX.x && p.z > HALL_MIN.y && p.z < HALL_MAX.y
}

/// Where to walk next from `feet` toward `goal`: straight there unless the hall's walls are in the
/// way (one end inside, the other out), then through the doorway that makes the shorter trip —
/// first to a point on this side of it (from outside: the foot of its steps, lined up with the
/// door), then to one just past it.
pub fn route(feet: Vec3, goal: Vec3) -> Vec3 {
    if inside(feet) == inside(goal) {
        return goal;
    }
    // (doorway centre, unit pointing out of the hall, distance out to the foot of its steps)
    let doors = [
        (Vec3::new(HALL_MIN.x, 0.0, DOOR_Z), Vec3::NEG_X, HALL_MIN.x - STEPS_FOOT_X + 0.6),
        (Vec3::new(YARD_DOOR_X, 0.0, HALL_MAX.y), Vec3::Z, YARD_STEPS_FOOT_Z - HALL_MAX.y + 0.6),
    ];
    let flat = |v: Vec3| v.with_y(0.0);
    let (door, out, reach) = doors
        .into_iter()
        .min_by(|a, b| {
            let cost = |d: Vec3| flat(d - feet).length() + flat(goal - d).length();
            cost(a.0).total_cmp(&cost(b.0))
        })
        .unwrap_or(doors[0]);
    let side = if inside(feet) { -1.0 } else { 1.0 };
    let near = door + out * side * if side > 0.0 { reach } else { 0.9 };
    let past = door - out * side * 1.2;
    // Lined up with the doorway already: go through.
    let lined = flat(feet - door).dot(out) * side > 0.0 && flat(feet - near).length() < 0.6;
    if lined || flat(feet - door).length() < 0.8 { past.with_y(feet.y) } else { near.with_y(feet.y) }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, swing)
        .add_systems(Update, walk.after(super::PlaySet::Input).before(super::PlaySet::Act));
}

/// `GB_WALK=x,z`: from 1 s the player walks toward that point (to see the doors swing).
pub(crate) fn walk(time: Res<Time>, mut target: Local<Option<Option<Vec3>>>, mut players: Query<(&Transform, &mut super::Intent), With<super::Player>>) {
    let goal = *target.get_or_insert_with(|| {
        std::env::var("GB_WALK").ok().and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() == 2).then(|| Vec3::new(v[0], 0.0, v[1]))
        })
    });
    let Some(goal) = goal else { return };
    if time.elapsed_secs() < 1.0 {
        return;
    }
    for (tf, mut intent) in &mut players {
        let to = (goal - tf.translation).with_y(0.0);
        if to.length() > 0.4 {
            intent.move_dir = to.normalize() * 0.6;
            intent.walk = true;
        }
    }
}

/// Leaves swing open toward a character in reach of their doorway and close once all are gone.
fn swing(
    time: Res<Time>,
    bodies: Query<&GlobalTransform, With<super::Mover>>,
    mut doors: Query<(&mut Door, &mut Transform, &GlobalTransform)>,
) {
    let dt = time.delta_secs();
    for (mut d, mut tf, g) in &mut doors {
        let hinge = g.translation();
        let c = d.centre;
        let near = bodies.iter().any(|b| Vec2::new(b.translation().x - c.x, b.translation().z - c.z).length() < DOOR_REACH);
        let want = if near { 1.0 } else { 0.0 };
        let before = d.open;
        d.open += (want - d.open) * (DOOR_RATE * dt).min(1.0);
        if (before < 0.9) != (d.open < 0.9) {
            info!("[hall] door at ({:.1}, {:.1}) {}", hinge.x, hinge.z, if d.open >= 0.9 { "open" } else { "closing" });
        }
        let eased = d.open * d.open * (3.0 - 2.0 * d.open);
        tf.rotation = Quat::from_rotation_y(d.closed + d.dir * DOOR_OPEN * eased);
    }
}

/// A pair of leaves in a doorway centred at `centre`, spanning `width` along `along` (unit,
/// horizontal), hinged at both jambs, opening inward toward `inward`.
fn doors(kit: &mut Kit, centre: Vec3, along: Vec3, inward: Vec3, width: f32) {
    let (wood, dark, metal) = (kit.pal.wood.clone(), kit.pal.dark_wood.clone(), kit.pal.metal.clone());
    let leaf = width * 0.5 - 0.04;
    let base = (-along.z).atan2(along.x);
    for side in [-1.0f32, 1.0] {
        let hinge = centre - along * side * (width * 0.5 - 0.02);
        // The leaf extends from its hinge toward the doorway centre (+local X toward centre).
        let closed = if side > 0.0 { base } else { base + PI };
        // Opening inward: the leaf's free end swings toward `inward`.
        let tip = Quat::from_rotation_y(closed) * Vec3::X;
        let dir = if tip.cross(inward).y > 0.0 { 1.0 } else { -1.0 };
        let e = kit
            .commands
            .spawn((
                Name::new("Door leaf"),
                Door { closed, dir, open: 0.0, centre },
                RigidBody::Kinematic,
                Transform::from_translation(hinge).with_rotation(Quat::from_rotation_y(closed)),
                Visibility::default(),
            ))
            .id();
        // The leaf is the doorway. A kinematic body follows the swing, so a closed door stops
        // the camera and an open one leaves the gap.
        kit.part(e, "Door planks", Cuboid::new(leaf, DOOR_H - 0.08, 0.08), &wood, Transform::from_xyz(leaf * 0.5, (DOOR_H - 0.08) * 0.5, 0.0), Some(Collider::cuboid(leaf, DOOR_H - 0.08, 0.08)));
        for y in [0.5, DOOR_H - 0.6] {
            kit.part(e, "Door brace", Cuboid::new(leaf, 0.12, 0.1), &dark, Transform::from_xyz(leaf * 0.5, y, 0.0), None);
        }
        kit.part(e, "Door ring", Cylinder::new(0.06, 0.03), &metal, Transform::from_xyz(leaf - 0.18, 1.1, 0.06).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), None);
    }
}

pub fn spawn_hall(kit: &mut Kit) {
    spawn_gate_and_path(kit);
    doors(kit, Vec3::new(HALL_MIN.x, FLOOR_Y, DOOR_Z), Vec3::Z, Vec3::X, DOOR_W);
    doors(kit, Vec3::new(YARD_DOOR_X, FLOOR_Y, HALL_MAX.y), Vec3::X, Vec3::NEG_Z, DOOR_W);
    spawn_desk(kit);
    spawn_board(kit);
    spawn_lodging(kit);
    spawn_training_ground(kit);
    spawn_lights(kit);
}

fn spawn_gate_and_path(kit: &mut Kit) {
    let (dark, soil, wood) = (kit.pal.dark_wood.clone(), kit.pal.soil.clone(), kit.pal.wood.clone());
    for dz in [-1.6, 1.6] {
        kit.solid(
            "Gate post",
            Cuboid::new(0.3, 3.4, 0.3),
            &dark,
            Transform::from_translation(GATE + Vec3::new(0.0, 1.7, dz)),
            Some(Collider::cuboid(0.3, 3.4, 0.3)),
        );
    }
    kit.solid("Gate beam", Cuboid::new(0.35, 0.3, 4.0), &dark, Transform::from_translation(GATE + Vec3::Y * 3.35), None);
    kit.solid(
        "Gate sign",
        Cuboid::new(0.08, 0.55, 1.9),
        &wood,
        Transform::from_translation(GATE + Vec3::new(-0.2, 2.85, 0.0)),
        None,
    );
    // Packed path from the courtyard through the gate to the foot of the steps.
    let (from, to) = (14.0, STEPS_FOOT_X);
    kit.solid(
        "Guild path",
        Cuboid::new(to - from, 0.012, 2.6),
        &soil,
        Transform::from_xyz((from + to) * 0.5, 0.006, DOOR_Z),
        None,
    );
}

fn spawn_desk(kit: &mut Kit) {
    let (wood, dark, metal, cloth, straw) =
        (kit.pal.wood.clone(), kit.pal.dark_wood.clone(), kit.pal.metal.clone(), kit.pal.cloth.clone(), kit.pal.straw.clone());
    // Counter across the hall, facing the door.
    let len = 4.4;
    kit.solid(
        "Front desk",
        Cuboid::new(0.7, 1.05, len),
        &dark,
        Transform::from_xyz(DESK_X, FLOOR_Y + 0.525, DOOR_Z),
        Some(Collider::cuboid(0.7, 1.05, len)),
    );
    kit.solid("Desk top", Cuboid::new(0.85, 0.06, len + 0.15), &wood, Transform::from_xyz(DESK_X, FLOOR_Y + 1.08, DOOR_Z), None);
    kit.solid("Desk bell", Cylinder::new(0.07, 0.07), &metal, Transform::from_xyz(DESK_X - 0.15, FLOOR_Y + 1.15, DOOR_Z + 0.9), None);
    kit.solid("Ledger", Cuboid::new(0.3, 0.05, 0.4), &cloth, Transform::from_xyz(DESK_X, FLOOR_Y + 1.14, DOOR_Z - 0.8), None);
    // Shelves of ledgers and scrolls on the wall behind the desk.
    let back = HALL_MAX.x - WALL_T * 0.5 - 0.25;
    for (k, y) in [0.9f32, 1.6, 2.3].map(|y| y + FLOOR_Y).into_iter().enumerate() {
        kit.solid("Shelf", Cuboid::new(0.45, 0.06, 3.6), &wood, Transform::from_xyz(back, y, DOOR_Z), None);
        for j in 0..7 {
            let z = DOOR_Z - 1.5 + j as f32 * 0.5 + (k as f32 * 0.17).sin() * 0.1;
            let mat = if (j + k) % 3 == 0 { &cloth } else { &straw };
            kit.solid("Ledgers", Cuboid::new(0.3, 0.32, 0.18), mat, Transform::from_xyz(back, y + 0.19, z), None);
        }
    }
}

fn spawn_board(kit: &mut Kit) {
    let (wood, dark, straw) = (kit.pal.wood.clone(), kit.pal.dark_wood.clone(), kit.pal.straw.clone());
    // Against the −z wall (front of the hearth), notices facing into the hall (+Z).
    let at = Vec3::new(27.6, FLOOR_Y, HALL_MIN.y + WALL_T * 0.5 + 0.12);
    let board = place(kit, "Bounty board", at, UseSpec {
        kind: "board",
        prompt: "Read the bounties",
        track: "ual1/Interact",
        duration: 1.2,
        looping: false,
        stand: Vec3::new(0.0, 0.0, -0.8),
        face: Some(0.0),
        hands: [None, Some(Vec3::new(0.15, 1.35, -0.06))],
        grip: None,
        beat: None,
        reach: None,
    });
    kit.commands.entity(board).insert(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(PI)));
    kit.part(board, "Board planks", Cuboid::new(2.2, 1.2, 0.06), &wood, Transform::from_xyz(0.0, 1.45, 0.0), Some(Collider::cuboid(2.2, 1.2, 0.06)));
    kit.part(board, "Board frame", Cuboid::new(2.4, 0.08, 0.1), &dark, Transform::from_xyz(0.0, 2.08, 0.0), None);
    for (x, y, tilt) in [(-0.8, 1.6, 0.06), (-0.3, 1.5, -0.04), (0.2, 1.65, 0.08), (0.75, 1.5, -0.05), (-0.55, 1.15, -0.07), (0.45, 1.12, 0.03)] {
        kit.part(board, "Bounty", Cuboid::new(0.3, 0.38, 0.01), &straw, Transform::from_xyz(x, y, -0.04).with_rotation(Quat::from_rotation_z(tilt)), None);
    }
}

fn spawn_lodging(kit: &mut Kit) {
    let (wood, dark, cloth, straw) =
        (kit.pal.wood.clone(), kit.pal.dark_wood.clone(), kit.pal.cloth.clone(), kit.pal.straw.clone());
    // Three beds along the −z wall behind the desk (heads to the wall: turned half round).
    let head_z = HALL_MIN.y + WALL_T * 0.5 + 0.05;
    for (i, x) in [32.4f32, 33.6, 34.8].into_iter().enumerate() {
        let at = Vec3::new(x, FLOOR_Y, head_z + 1.05);
        let bed = place(kit, &format!("Bed {}", i + 1), at, UseSpec {
            kind: "bed",
            prompt: "Rest",
            track: "ual1/Sitting_Idle_Loop",
            duration: 4.0,
            looping: true,
            stand: Vec3::new(0.62, 0.0, 0.0),
            face: Some(-FRAC_PI_2),
            hands: [None, None],
            grip: None,
            beat: None,
            reach: None,
        });
        kit.commands.entity(bed).insert(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(PI)));
        kit.part(bed, "Bed frame", Cuboid::new(0.95, 0.38, 2.0), &dark, Transform::from_xyz(0.0, 0.19, 0.0), Some(Collider::cuboid(0.95, 0.38, 2.0)));
        kit.part(bed, "Mattress", Cuboid::new(0.85, 0.12, 1.9), &straw, Transform::from_xyz(0.0, 0.44, 0.0), None);
        kit.part(bed, "Blanket", Cuboid::new(0.88, 0.05, 1.1), &cloth, Transform::from_xyz(0.0, 0.52, -0.35), None);
        kit.part(bed, "Pillow", Cuboid::new(0.6, 0.1, 0.3), &straw, Transform::from_xyz(0.0, 0.54, 0.75), None);
        kit.part(bed, "Headboard", Cuboid::new(0.95, 0.9, 0.08), &wood, Transform::from_xyz(0.0, 0.45, 0.98), None);
    }
    // Table and chairs for the lodgers.
    let table = Vec3::new(28.4, FLOOR_Y, -11.6);
    kit.solid("Lodging table", Cuboid::new(1.6, 0.08, 0.9), &wood, Transform::from_translation(table + Vec3::Y * 0.74), Some(Collider::cuboid(1.6, 0.08, 0.9)));
    for (x, z) in [(-0.7, -0.35), (0.7, -0.35), (-0.7, 0.35), (0.7, 0.35)] {
        kit.solid("Table leg", Cuboid::new(0.08, 0.7, 0.08), &dark, Transform::from_translation(table + Vec3::new(x, 0.35, z)), None);
    }
    for (dx, dz, yaw) in [(-0.5, -1.0, 0.0), (0.5, 1.0, PI)] {
        let at = table + Vec3::new(dx, 0.0, dz);
        let chair = place(kit, "Lodging chair", at, UseSpec {
            kind: "chair",
            prompt: "Sit",
            track: "ual1/Sitting_Idle_Loop",
            duration: 4.0,
            looping: true,
            stand: Vec3::new(0.0, 0.0, -0.05),
            face: Some(yaw),
            hands: [None, None],
            grip: None,
            beat: None,
            reach: None,
        });
        kit.commands.entity(chair).insert(Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)));
        kit.part(chair, "Chair seat", Cuboid::new(0.45, 0.06, 0.45), &wood, Transform::from_xyz(0.0, 0.45, 0.0), Some(Collider::cuboid(0.45, 0.06, 0.45)));
        kit.part(chair, "Chair back", Cuboid::new(0.45, 0.5, 0.05), &dark, Transform::from_xyz(0.0, 0.72, 0.22), None);
    }
}

fn spawn_training_ground(kit: &mut Kit) {
    let (sand, dark, stone, metal) =
        (kit.pal.sand.clone(), kit.pal.dark_wood.clone(), kit.pal.stone.clone(), kit.pal.metal.clone());
    let (x0, x1, z0, z1) = (YARD_X0, 41.0, 0.0, 13.0);
    let (cx, cz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5);
    kit.solid("Yard sand", Cuboid::new(x1 - x0, 0.015, z1 - z0), &sand, Transform::from_xyz(cx, 0.008, cz), None);
    // Fence: posts and two rails, open toward the hall's yard door.
    let post = |kit: &mut Kit, x: f32, z: f32| {
        kit.solid("Yard post", Cylinder::new(0.07, 1.2), &dark, Transform::from_xyz(x, 0.6, z), Some(Collider::cylinder(0.07, 1.2)));
    };
    let mut x = x0;
    while x <= x1 + 1e-3 {
        post(kit, x, z1);
        if (x - YARD_DOOR_X).abs() > 1.6 {
            post(kit, x, z0);
        }
        x += 1.7;
    }
    let mut z = z0;
    while z <= z1 + 1e-3 {
        post(kit, x0, z);
        post(kit, x1, z);
        z += 1.6;
    }
    for (y, h) in [(0.55, 0.06), (1.05, 0.06)] {
        kit.solid("Yard rail", Cuboid::new(x1 - x0, h, 0.06), &dark, Transform::from_xyz(cx, y, z1), Some(Collider::cuboid(x1 - x0, h, 0.06)));
        kit.solid("Yard rail", Cuboid::new(0.06, h, z1 - z0), &dark, Transform::from_xyz(x0, y, cz), Some(Collider::cuboid(0.06, h, z1 - z0)));
        kit.solid("Yard rail", Cuboid::new(0.06, h, z1 - z0), &dark, Transform::from_xyz(x1, y, cz), Some(Collider::cuboid(0.06, h, z1 - z0)));
    }
    // The duel ring: a dark sand circle edged with stones.
    let ring = super::vfx::ring(RING_RADIUS - 0.08, RING_RADIUS + 0.08);
    kit.solid("Duel ring line", ring, &dark, Transform::from_translation(RING_CENTER + Vec3::Y * 0.02), None);
    for k in 0..16 {
        let a = k as f32 / 16.0 * std::f32::consts::TAU;
        let p = RING_CENTER + Vec3::new(a.cos(), 0.0, a.sin()) * (RING_RADIUS + 0.3);
        kit.solid("Ring stone", Cylinder::new(0.14, 0.18), &stone, Transform::from_translation(p + Vec3::Y * 0.09), None);
    }
    // Dummies along the west fence, a weapon rack on the east.
    for (i, z) in [3.0f32, 6.5, 10.0].into_iter().enumerate() {
        spawn_dummy(kit, Vec3::new(26.2, 0.0, z), FRAC_PI_2, &format!("Yard dummy {}", i + 1));
    }
    // The balance beam along the north fence, a mounting block at each end.
    let (a, b) = (BEAM_START, BEAM_END);
    let mid = (a + b) * 0.5;
    let len = (b - a).length();
    kit.solid("Balance beam", Cuboid::new(len, 0.14, BEAM_WIDTH), &dark, Transform::from_translation(mid - Vec3::Y * 0.07), Some(Collider::cuboid(len, 0.14, BEAM_WIDTH)));
    let posts = 4;
    for k in 0..=posts {
        let p = a.lerp(b, k as f32 / posts as f32);
        let h = a.y - 0.14;
        kit.solid("Beam post", Cuboid::new(0.12, h, 0.12), &dark, Transform::from_translation(p.with_y(h * 0.5)), None);
        if k == 0 || k == posts {
            kit.solid("Beam foot", Cuboid::new(0.12, 0.08, 0.7), &dark, Transform::from_translation(p.with_y(0.04)), None);
        }
    }
    for (end, out) in [(a, -1.0f32), (b, 1.0)] {
        let p = end + Vec3::X * out * 0.55;
        kit.solid("Beam block", Cuboid::new(0.9, 0.3, 0.9), &stone, Transform::from_translation(p.with_y(0.15)), Some(Collider::cuboid(0.9, 0.3, 0.9)));
    }
    kit.commands.spawn((Name::new("Beam line"), super::beam::Beam { a, b, sway: 1.0, name: "beam" }));
    let rack = Vec3::new(40.2, 0.0, 7.0);
    for dz in [-1.0, 1.0] {
        kit.solid("Rack post", Cuboid::new(0.1, 1.5, 0.1), &dark, Transform::from_translation(rack + Vec3::new(0.0, 0.75, dz)), None);
    }
    kit.solid("Rack bar", Cuboid::new(0.08, 0.08, 2.2), &dark, Transform::from_translation(rack + Vec3::Y * 1.35), None);
    for k in 0..4 {
        let dz = -0.75 + k as f32 * 0.5;
        kit.solid(
            "Practice blade",
            Cuboid::new(0.04, 1.1, 0.07),
            &metal,
            Transform::from_translation(rack + Vec3::new(-0.08, 0.75, dz)).with_rotation(Quat::from_rotation_z(0.12)),
            None,
        );
    }
}

fn spawn_lights(kit: &mut Kit) {
    let (flame, dark) = (kit.pal.flame.clone(), kit.pal.dark_wood.clone());
    let sconce = |kit: &mut Kit, at: Vec3, range: f32| {
        let holder = kit.solid("Sconce", Cuboid::new(0.12, 0.3, 0.12), &dark, Transform::from_translation(at - Vec3::Y * 0.2), None);
        let _ = holder;
        let f = kit.solid("Sconce flame", Sphere::new(0.09), &flame, Transform::from_translation(at), None);
        kit.commands.entity(f).insert((
            NotShadowCaster,
            PointLight {
                color: Color::srgb(1.0, 0.66, 0.32),
                intensity: 70_000.0,
                range,
                shadow_maps_enabled: false,
                ..default()
            },
        ));
    };
    // Inside: by the desk and over the lodging. (The lanterns at the west door, the hearth and
    // the lounge lamp are the guildhouse's.)
    sconce(kit, Vec3::new(HALL_MAX.x - 0.35, FLOOR_Y + 2.6, DOOR_Z - 2.6), 11.0);
    sconce(kit, Vec3::new(33.6, FLOOR_Y + 2.6, HALL_MIN.y + 0.35), 10.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_complex_sits_on_the_terrace() {
        let on = |x: f32, z: f32| crate::play::world::terrace_distance(x, z) <= 0.0;
        assert!(on(HALL_MIN.x, HALL_MIN.y) && on(HALL_MAX.x, HALL_MAX.y));
        assert!(on(41.0, 13.0), "yard corner");
        assert!(on(GATE.x, GATE.z) && on(DESK_STAND.x, DESK_STAND.z));
        // The ring fits in the yard, clear of the fence.
        assert!(RING_CENTER.x - RING_RADIUS > HALL_MIN.x + 1.0 && RING_CENTER.z + RING_RADIUS < 13.0);
        // The receptionist stands behind the counter, between it and the back wall.
        assert!(DESK_STAND.x > DESK_X + 0.5 && DESK_STAND.x < HALL_MAX.x - 0.5);
        // The yard door opens off the hall's +z wall onto the yard, clear of the fence posts.
        assert!(YARD_DOOR_X > YARD_X0 + 1.6 && YARD_DOOR_X < HALL_MAX.x);
    }

    #[test]
    fn walkers_leave_the_hall_by_a_door() {
        let lodging = Vec3::new(28.4, FLOOR_Y, -10.6);
        let beam_end = Vec3::new(26.6, 0.0, 12.0);
        let w = route(lodging, beam_end);
        assert!((w.x - YARD_DOOR_X).abs() < 0.1 && w.z < HALL_MAX.y, "to the yard door first: {w}");
        // At the doorway: through it.
        let at_door = Vec3::new(YARD_DOOR_X, 0.0, HALL_MAX.y - 0.9);
        assert!(route(at_door, beam_end).z > HALL_MAX.y, "then out");
        // Both outside: straight.
        assert_eq!(route(Vec3::new(30.0, 0.0, 5.0), beam_end), beam_end);
        // From the gate to the desk: the west door.
        let w = route(Vec3::new(17.0, 0.0, -9.0), DESK_STAND);
        assert!((w.z - DOOR_Z).abs() < 0.1 && w.x < STEPS_FOOT_X, "to the foot of the steps: {w}");
        // Lined up at the foot: up the steps and in.
        let w = route(Vec3::new(STEPS_FOOT_X - 0.6, 0.0, DOOR_Z), DESK_STAND);
        assert!(w.x > HALL_MIN.x, "{w}");
        // From the yard: to the foot of the yard steps first.
        let w = route(beam_end, DESK_STAND);
        assert!((w.x - YARD_DOOR_X).abs() < 0.1 && w.z > YARD_STEPS_FOOT_Z, "{w}");
    }
}

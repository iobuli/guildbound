//! Courtyard dressing and physics props (avian3d): Quaternius kit pieces placed from one table; crates and
//! barrels the fighters bump and knock around. Fighters get kinematic colliders
//! that chase the sim position by velocity (Overgrowth: velocity = distance / dt, never a teleport), so
//! contacts resolve and props get shoved with the fighter's momentum. Props never feed back into the sim.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::sim_bridge::Sim;

#[derive(Component)]
pub struct FighterBody(pub usize);

/// Advance physics by the same time as rules, including pause/single-step.
/// Substeps stay at or below 1/120s even during batched remote checks.
pub fn physics_clock(sim: Res<Sim>, real: Res<Time>, physical: Res<crate::possession::PhysicalBody>, mut time: ResMut<Time<Physics>>, mut substeps: ResMut<SubstepCount>) {
    // the Studio pauses the sim; a Possession physical body still needs physics, on real time
    let step = if physical.on { real.delta_secs().min(1.0 / 30.0) } else { sim.frame_step_s.max(0.0) };
    time.pause();
    time.advance_by(std::time::Duration::from_secs_f32(step));
    substeps.0 = (step / guildbound_sim::STEP).ceil().max(2.0) as u32;
}

#[derive(Clone, Copy)]
enum Body {
    /// Scenery out of reach: no collider.
    None,
    Static,
    /// Knocked around by fighters: mass (kg) and a box (full size, m) present from the first frame. A collider
    /// built from the mesh only appears once the scene has loaded; until then the body fell through the floor.
    Crate(f32, [f32; 3]),
    /// Upright cylinder: mass, radius, height.
    Drum(f32, f32, f32),
}

/// Measured from the kits' glTF bounds (Crate_Wooden 1.08 x 1.20 x 1.16 at node scale 0.78; Barrel r 0.35, h 0.9),
/// before the per-row scale.
const CRATE: Body = Body::Crate(10.0, [0.84, 0.94, 0.90]);
const BARREL: Body = Body::Drum(25.0, 0.35, 0.90);

const PROPS: &str = "packs/_sources/quaternius/Fantasy_Props_MegaKit/Exports/glTF";
const NATURE: &str = "packs/_sources/quaternius/Stylized_Nature_MegaKit/glTF";
const VILLAGE: &str = "packs/_sources/quaternius/Medieval_Village_MegaKit/Medieval Village MegaKit[Standard]/glTF";

/// The courtyard dressing, one row per piece: kit, model, position (x, z), yaw (deg), scale, body.
/// Quaternius MegaKits (CC0, the boss's kits); models are metre-scale with their origin on the ground.
/// Add a row to add a piece.
const DRESSING: &[(&str, &str, f32, f32, f32, f32, Body)] = &[
    // knockable clutter inside the court (where the primitive crates were)
    (PROPS, "Crate_Wooden", -3.0, 3.0, 10.0, 0.75, CRATE),
    (PROPS, "Crate_Wooden", -3.7, 3.4, -25.0, 0.75, CRATE),
    (PROPS, "Barrel", -3.3, 4.1, 0.0, 1.0, BARREL),
    (PROPS, "Crate_Wooden", 3.5, -2.5, 40.0, 0.75, CRATE),
    (PROPS, "Barrel_Apples", 4.1, -3.1, 0.0, 1.0, BARREL),
    (PROPS, "Crate_Wooden", 2.8, 3.8, -5.0, 0.75, CRATE),
    (PROPS, "Barrel", -4.2, -3.6, 0.0, 1.0, BARREL),
    // training corner pieces, just inside the chalk line
    (PROPS, "Dummy", -8.6, -8.6, 45.0, 1.0, Body::Static),
    (PROPS, "Dummy", 8.6, 8.6, 225.0, 1.0, Body::Static),
    // the guild's yard outside the fence
    (PROPS, "WeaponStand", -12.2, -2.0, 90.0, 1.0, Body::None),
    (PROPS, "Bench", -12.2, 2.5, 90.0, 1.0, Body::None),
    (PROPS, "Workbench", 12.2, -2.5, -90.0, 1.0, Body::None),
    (PROPS, "Anvil", 12.0, 0.2, -90.0, 1.0, Body::None),
    (PROPS, "Chest_Wood", 12.4, 2.6, -90.0, 1.0, Body::None),
    (PROPS, "Stall_Cart_Empty", 6.5, 13.2, 180.0, 1.0, Body::None),
    (VILLAGE, "Prop_Wagon", -6.5, -13.4, 90.0, 1.0, Body::None),
    // nature ring
    (NATURE, "Pine_2", -17.0, -9.0, 0.0, 1.1, Body::None),
    (NATURE, "CommonTree_1", -18.5, 4.0, 60.0, 1.0, Body::None),
    (NATURE, "CommonTree_3", -15.5, 14.0, 120.0, 1.0, Body::None),
    (NATURE, "Pine_2", -6.0, 19.0, 200.0, 0.9, Body::None),
    (NATURE, "CommonTree_1", 5.0, 18.5, 15.0, 1.1, Body::None),
    (NATURE, "Pine_2", 16.0, 15.0, 90.0, 1.2, Body::None),
    (NATURE, "CommonTree_3", 18.0, 3.0, 300.0, 0.95, Body::None),
    (NATURE, "Pine_2", 17.5, -10.0, 30.0, 1.0, Body::None),
    (NATURE, "CommonTree_1", 8.0, -18.0, 250.0, 1.05, Body::None),
    (NATURE, "Pine_2", -4.0, -19.5, 140.0, 1.15, Body::None),
    (NATURE, "CommonTree_3", -14.0, -16.0, 80.0, 1.0, Body::None),
    (NATURE, "Bush_Common", -12.6, 7.0, 0.0, 1.0, Body::None),
    (NATURE, "Bush_Common", 12.8, -7.5, 90.0, 1.1, Body::None),
    (NATURE, "Bush_Common", -9.0, 13.0, 45.0, 0.9, Body::None),
    (NATURE, "Bush_Common", 9.5, -13.0, 180.0, 1.0, Body::None),
    (NATURE, "Rock_Medium_1", 14.5, 9.0, 30.0, 0.8, Body::None),
    (NATURE, "Rock_Medium_1", -14.0, -5.0, 200.0, 0.7, Body::None),
    (NATURE, "Grass_Common_Tall", -11.6, 9.4, 0.0, 1.0, Body::None),
    (NATURE, "Grass_Common_Tall", 11.4, -9.6, 70.0, 1.0, Body::None),
    (NATURE, "Grass_Common_Tall", 11.7, 9.8, 140.0, 0.8, Body::None),
    (NATURE, "Grass_Common_Tall", -11.8, -9.1, 210.0, 0.9, Body::None),
];

/// Half size of the open world (m); the sim's bound. The courtyard is the 20 m square at its centre.
pub const WORLD_HALF: f32 = 60.0;

fn hash(a: u64, b: u64) -> u64 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h.wrapping_mul(0x1656_67B1_9E37_79F9)
}

fn unit(i: u64, salt: u64) -> f32 {
    (hash(i, salt) % 100_000) as f32 / 100_000.0
}

/// The open world around the courtyard, scattered deterministically: (kit, model, x, z, yaw, scale, trunk
/// radius for the sim or 0). Kept clear: the courtyard and its yard (r < 17), the four roads leaving the
/// entrances (|x| < 2.5 or |z| < 2.5), and the market clearing NE.
pub fn world_scatter() -> Vec<(&'static str, String, f32, f32, f32, f32, f32)> {
    const TREES: [&str; 12] = ["CommonTree_1", "CommonTree_2", "CommonTree_3", "CommonTree_4", "CommonTree_5", "Pine_1", "Pine_2", "Pine_3", "Pine_4", "Pine_5", "TwistedTree_1", "TwistedTree_3"];
    const SMALL: [(&str, f32); 9] = [("Bush_Common", 0.0), ("Bush_Common_Flowers", 0.0), ("Grass_Common_Tall", 0.0), ("Grass_Wispy_Tall", 0.0), ("Flower_3_Group", 0.0), ("Flower_4_Group", 0.0), ("Fern_1", 0.0), ("Mushroom_Common", 0.0), ("Plant_1_Big", 0.0)];
    const ROCKS: [&str; 3] = ["Rock_Medium_1", "Rock_Medium_2", "Rock_Medium_3"];
    let market = (30.0f32, 30.0f32);
    let clear = |x: f32, z: f32, pad: f32| {
        let r = (x * x + z * z).sqrt();
        r < 17.0 + pad || (x - crate::studio::STAGE.x).hypot(z - crate::studio::STAGE.z) < 7.0 + pad || x.abs() < 2.5 + pad || z.abs() < 2.5 + pad || ((x - market.0).powi(2) + (z - market.1).powi(2)).sqrt() < 9.0 + pad
            || x.abs() > WORLD_HALF - 2.0 || z.abs() > WORLD_HALF - 2.0
    };
    let mut out = Vec::new();
    let mut place = |count: u64, salt: u64, pad: f32, mut pick: Box<dyn FnMut(u64) -> (&'static str, String, f32, f32)>| {
        let mut k = 0;
        let mut i = 0;
        while k < count && i < count * 20 {
            i += 1;
            let x = (unit(i, salt) * 2.0 - 1.0) * WORLD_HALF;
            let z = (unit(i, salt + 1) * 2.0 - 1.0) * WORLD_HALF;
            if clear(x, z, pad) {
                continue;
            }
            let (kit, model, scale, trunk) = pick(i);
            out.push((kit, model, x, z, unit(i, salt + 2) * 360.0, scale, trunk));
            k += 1;
        }
    };
    place(110, 11, 1.5, Box::new(|i| { let t = TREES[(hash(i, 3) % TREES.len() as u64) as usize]; (NATURE, t.to_owned(), 0.8 + 0.5 * unit(i, 4), 0.45) }));
    place(26, 21, 1.0, Box::new(|i| { let r = ROCKS[(hash(i, 5) % 3) as usize]; let s = 0.5 + 0.7 * unit(i, 6); (NATURE, r.to_owned(), s, 1.2 * s) }));
    place(260, 31, 0.0, Box::new(|i| { let (m, t) = SMALL[(hash(i, 7) % SMALL.len() as u64) as usize]; (NATURE, m.to_owned(), 0.8 + 0.5 * unit(i, 8), t) }));
    // market clearing: a stall row, carts and crates around a well-trodden square
    for (m, dx, dz, yaw) in [("Stall_Empty", -4.0, 4.5, 180.0), ("Stall_Cart_Empty", 0.5, 5.0, 180.0), ("Stall_Empty", 5.0, 4.5, 180.0), ("Barrel_Apples", -2.0, 3.2, 0.0), ("FarmCrate_Apple", 2.5, 3.0, 20.0), ("FarmCrate_Carrot", 3.4, 3.4, -15.0), ("Table_Large", -5.0, -2.0, 90.0), ("Bench", -3.0, -4.5, 0.0), ("Cauldron", 4.0, -3.0, 0.0), ("Chest_Wood", 6.0, -1.0, -90.0)] {
        out.push((PROPS, m.to_owned(), market.0 + dx, market.1 + dz, yaw, 1.0, 0.0));
    }
    out.push((VILLAGE, "Prop_Wagon".to_owned(), market.0 - 7.0, market.1 - 6.0, 30.0, 1.0, 0.0));
    out
}

/// Sim obstacles for the open world: tree trunks and rocks from the scatter.
pub fn obstacles() -> Vec<(guildbound_sim::V2, f32)> {
    let mut out: Vec<_> = world_scatter().into_iter().filter(|e| e.6 > 0.0).map(|e| (guildbound_sim::V2::new(e.2, e.3), e.6)).collect();
    out.extend(crate::npc::obstacles());
    out
}

pub fn spawn(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((RigidBody::Static, Collider::half_space(Vec3::Y), Transform::default()));
    let hull = || ColliderConstructorHierarchy::new(ColliderConstructor::ConvexHullFromMesh);
    let mut place = |path: String, x: f32, z: f32, yaw: f32, scale: f32, body: Body| {
        let scene: Handle<bevy::world_serialization::WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        let rot = Quat::from_rotation_y(yaw.to_radians());
        let view = (bevy::world_serialization::WorldAssetRoot(scene), Transform::from_scale(Vec3::splat(scale)));
        match body {
            Body::None => {
                commands.spawn((view.0, Transform::from_xyz(x, 0.0, z).with_rotation(rot).with_scale(Vec3::splat(scale))));
            }
            Body::Static => {
                commands.spawn((view.0, Transform::from_xyz(x, 0.0, z).with_rotation(rot).with_scale(Vec3::splat(scale)), RigidBody::Static, hull()));
            }
            Body::Crate(kg, [w, h, d]) => {
                let half = 0.5 * h * scale;
                commands.spawn((Transform::from_xyz(x, half + 0.002, z).with_rotation(rot), Visibility::default(), RigidBody::Dynamic, Collider::cuboid(w * scale, h * scale, d * scale), Mass(kg)))
                    .with_child((view.0, view.1.with_translation(Vec3::Y * -half)));
            }
            Body::Drum(kg, r, h) => {
                let half = 0.5 * h * scale;
                commands.spawn((Transform::from_xyz(x, half + 0.002, z).with_rotation(rot), Visibility::default(), RigidBody::Dynamic, Collider::cylinder(r * scale, h * scale), Mass(kg)))
                    .with_child((view.0, view.1.with_translation(Vec3::Y * -half)));
            }
        }
    };
    for &(kit, model, x, z, yaw, scale, body) in DRESSING {
        place(format!("{kit}/{model}.gltf"), x, z, yaw, scale, body);
    }
    for (kit, model, x, z, yaw, scale, _) in world_scatter() {
        place(format!("{kit}/{model}.gltf"), x, z, yaw, scale, Body::None);
    }
    // fence ring with entrance gaps: 2 m village fence sections on every side, 4..10 m out from centre
    for side in [-1.0f32, 1.0] {
        for along in [-9.0f32, -7.0, -5.0, 5.0, 7.0, 9.0] {
            place(format!("{VILLAGE}/Prop_WoodenFence_Extension1.gltf"), side * 10.6, along, 90.0, 1.0, Body::Static);
            place(format!("{VILLAGE}/Prop_WoodenFence_Extension1.gltf"), along, side * 10.6, 0.0, 1.0, Body::Static);
        }
    }
}

/// Kinematic fighter bodies reach the sim position this frame by velocity.
pub fn drive_bodies(sim: Res<Sim>, mut bodies: Query<(&FighterBody, &Transform, &mut LinearVelocity)>) {
    let dt = sim.frame_step_s.max(1e-4);
    for (b, t, mut v) in &mut bodies {
        let Some(f) = sim.world.fighters.get(b.0) else { continue };
        let target = Vec3::new(f.position.x, 0.9 + f.height, f.position.z);
        v.0 = (target - t.translation) / dt;
    }
}

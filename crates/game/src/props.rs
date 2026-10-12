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
pub fn physics_clock(sim: Res<Sim>, mut time: ResMut<Time<Physics>>, mut substeps: ResMut<SubstepCount>) {
    time.pause();
    time.advance_by(std::time::Duration::from_secs_f32(sim.frame_step_s.max(0.0)));
    substeps.0 = (sim.frame_step_s / guildbound_sim::STEP).ceil().max(2.0) as u32;
}

#[derive(Clone, Copy)]
enum Body {
    /// Scenery out of reach: no collider.
    None,
    Static,
    /// Knocked around by fighters (kg).
    Dynamic(f32),
}

const PROPS: &str = "packs/_sources/quaternius/Fantasy_Props_MegaKit/Exports/glTF";
const NATURE: &str = "packs/_sources/quaternius/Stylized_Nature_MegaKit/glTF";
const VILLAGE: &str = "packs/_sources/quaternius/Medieval_Village_MegaKit/Medieval Village MegaKit[Standard]/glTF";

/// The courtyard dressing, one row per piece: kit, model, position (x, z), yaw (deg), scale, body.
/// Quaternius MegaKits (CC0, the boss's kits); models are metre-scale with their origin on the ground.
/// Add a row to add a piece.
const DRESSING: &[(&str, &str, f32, f32, f32, f32, Body)] = &[
    // knockable clutter inside the court (where the primitive crates were)
    (PROPS, "Crate_Wooden", -3.0, 3.0, 10.0, 0.75, Body::Dynamic(10.0)),
    (PROPS, "Crate_Wooden", -3.7, 3.4, -25.0, 0.75, Body::Dynamic(10.0)),
    (PROPS, "Barrel", -3.3, 4.1, 0.0, 1.0, Body::Dynamic(25.0)),
    (PROPS, "Crate_Wooden", 3.5, -2.5, 40.0, 0.75, Body::Dynamic(10.0)),
    (PROPS, "Barrel_Apples", 4.1, -3.1, 0.0, 1.0, Body::Dynamic(30.0)),
    (PROPS, "Crate_Wooden", 2.8, 3.8, -5.0, 0.75, Body::Dynamic(10.0)),
    (PROPS, "Barrel", -4.2, -3.6, 0.0, 1.0, Body::Dynamic(25.0)),
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

pub fn spawn(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((RigidBody::Static, Collider::half_space(Vec3::Y), Transform::default()));
    let hull = || ColliderConstructorHierarchy::new(ColliderConstructor::ConvexHullFromMesh);
    let mut place = |path: String, x: f32, z: f32, yaw: f32, scale: f32, body: Body| {
        let scene: Handle<bevy::world_serialization::WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        let t = Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(yaw.to_radians())).with_scale(Vec3::splat(scale));
        let mut e = commands.spawn((bevy::world_serialization::WorldAssetRoot(scene), t));
        match body {
            Body::None => {}
            Body::Static => {
                e.insert((RigidBody::Static, hull()));
            }
            Body::Dynamic(kg) => {
                e.insert((RigidBody::Dynamic, hull(), Mass(kg)));
            }
        }
    };
    for &(kit, model, x, z, yaw, scale, body) in DRESSING {
        place(format!("{kit}/{model}.gltf"), x, z, yaw, scale, body);
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

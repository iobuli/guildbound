//! Physics props (avian3d): crates the fighters bump and knock around. Fighters get kinematic colliders
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

pub fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((RigidBody::Static, Collider::half_space(Vec3::Y), Transform::default()));
    let crate_mesh = meshes.add(Cuboid::new(0.6, 0.6, 0.6));
    let wood = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.38, 0.22), perceptual_roughness: 0.85, ..default() });
    let barrel_mesh = meshes.add(Cylinder::new(0.3, 0.8));
    let iron = materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.30, 0.30), metallic: 0.6, perceptual_roughness: 0.5, ..default() });
    let spots = [(-3.0, 3.0), (-3.6, 3.4), (-3.3, 3.9), (3.5, -2.5), (4.0, -3.0), (2.8, 3.8), (-4.2, -3.6)];
    for (k, (x, z)) in spots.into_iter().enumerate() {
        if k % 3 == 2 {
            commands.spawn((
                Mesh3d(barrel_mesh.clone()),
                MeshMaterial3d(iron.clone()),
                Transform::from_xyz(x, 0.4, z),
                RigidBody::Dynamic,
                Collider::cylinder(0.3, 0.8),
                Mass(25.0),
            ));
        } else {
            commands.spawn((
                Mesh3d(crate_mesh.clone()),
                MeshMaterial3d(wood.clone()),
                Transform::from_xyz(x, 0.3, z),
                RigidBody::Dynamic,
                Collider::cuboid(0.6, 0.6, 0.6),
                Mass(10.0),
            ));
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

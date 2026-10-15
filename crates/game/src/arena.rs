//! A code-built practice courtyard: quiet combat floor, visible limits, four
//! entrances and paired guild markers. Fixed 20 m square at the centre of the open world.
//! Decoration never introduces a second movement or collision authority.
use bevy::prelude::*;

#[derive(Component)]
pub struct Courtyard;

pub fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let stone = materials.add(StandardMaterial { base_color: Color::srgb(0.44, 0.43, 0.37), perceptual_roughness: 0.95, ..default() });
    let border = materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.29, 0.27), perceptual_roughness: 0.9, ..default() });
    let chalk = materials.add(StandardMaterial { base_color: Color::srgb(0.74, 0.65, 0.44), perceptual_roughness: 1.0, ..default() });
    let wood = materials.add(StandardMaterial { base_color: Color::srgb(0.23, 0.16, 0.10), perceptual_roughness: 0.9, ..default() });
    let cloth = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.36, 0.36), perceptual_roughness: 0.95, ..default() });
    commands.spawn((Courtyard, Transform::default(), Visibility::default())).with_children(|p| {
        let mut block = |pos: Vec3, scale: Vec3, material: &Handle<StandardMaterial>| {
            p.spawn((Mesh3d(cube.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(pos).with_scale(scale)));
        };
        block(Vec3::new(0.0, -0.04, 0.0), Vec3::new(20.0, 0.08, 20.0), &stone);
        for side in [-1.0, 1.0] {
            block(Vec3::new(side * 9.85, 0.005, 0.0), Vec3::new(0.22, 0.01, 20.0), &chalk);
            block(Vec3::new(0.0, 0.005, side * 9.85), Vec3::new(20.0, 0.01, 0.22), &chalk);
            // Starting lines and segmented inner court markings.
            block(Vec3::new(0.0, 0.008, side * 1.5), Vec3::new(1.0, 0.016, 0.06), &chalk);
            // the fence itself is dressing (props.rs: Quaternius village fence sections)
            for x in [-10.8, 10.8] {
                let z = side * 10.8;
                block(Vec3::new(x, 0.18, z), Vec3::new(0.9, 0.36, 0.9), &border);
                block(Vec3::new(x, 1.6, z), Vec3::new(0.2, 2.8, 0.2), &wood);
                block(Vec3::new(x, 2.2, z + 0.13), Vec3::new(0.8, 1.0, 0.06), &cloth);
                // Two offset bars form our simple guild mark.
                block(Vec3::new(x - 0.12, 2.25, z + 0.17), Vec3::new(0.06, 0.55, 0.02), &chalk);
                block(Vec3::new(x + 0.12, 2.05, z + 0.17), Vec3::new(0.06, 0.55, 0.02), &chalk);
            }
        }
    });
}

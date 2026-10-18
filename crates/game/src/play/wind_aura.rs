//! Wind aura (assets/shaders/wind_aura.wgsl): a shell that shows at its silhouette, with light streaks combed along a
//! wind direction. F9 shows or hides a test shell at the origin.
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(ShaderType, Reflect, Debug, Clone, Copy)]
pub struct WindAuraParams {
    pub color: Vec4,
    pub wind: Vec4,
    pub clock: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WindAura {
    #[uniform(0)]
    pub params: WindAuraParams,
}

impl Material for WindAura {
    fn fragment_shader() -> ShaderRef {
        "shaders/wind_aura.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Component)]
struct TestShell;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WindAura>::default())
        .add_systems(Startup, spawn_test)
        .add_systems(Update, (toggle, advance_clock));
}

fn spawn_test(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<WindAura>>) {
    let material = materials.add(WindAura {
        params: WindAuraParams {
            color: Vec4::new(0.45, 0.9, 1.0, 1.0),
            // direction (x, z), gust speed, streak density
            wind: Vec4::new(1.0, 0.25, 2.0, 1.6),
            clock: Vec4::new(0.0, 2.5, 0.0, 0.0),
        },
    });
    commands.spawn((
        TestShell,
        Mesh3d(meshes.add(Sphere::new(1.2))),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 1.0, 0.0),
        Visibility::Hidden,
    ));
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut shells: Query<&mut Visibility, With<TestShell>>) {
    if keys.just_pressed(KeyCode::F9) {
        for mut visibility in &mut shells {
            *visibility = if *visibility == Visibility::Hidden { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

fn advance_clock(time: Res<Time>, mut materials: ResMut<Assets<WindAura>>) {
    for (_, material) in materials.iter_mut() {
        material.params.clock.x = time.elapsed_secs();
    }
}

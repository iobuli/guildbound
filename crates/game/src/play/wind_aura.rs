//! Wind aura (assets/shaders/wind_aura.wgsl): a shell that shows at its silhouette, with light streaks combed along a
//! wind direction. Debug view 10 (`debug::View::WindShell`, F12 list) shows a test shell at the origin.
//! The Aura Ward is wind: every new `magic::WardShell` gets a slightly larger wind shell as a child
//! (it scales, breathes and fades with the ward), tinted by the owner's side, streaking along
//! `GrassWind.direction`; and while a ward is up it pushes the meadow away from its owner through
//! `GrassWind.gust` (blades bend outward, one ward at a time: the nearest to the camera).
use super::grass::{GrassGust, GrassWind};
use super::magic::{Ward, WardShell};
use super::Player;
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
        .add_systems(Update, (toggle, advance_clock, dress_wards, ward_gust));
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

fn toggle(toggles: Res<super::debug::DebugToggles>, mut shells: Query<&mut Visibility, With<TestShell>>) {
    if !toggles.is_changed() {
        return;
    }
    let want = if toggles.get(super::debug::View::WindShell) { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in &mut shells {
        *visibility = want;
    }
}

fn advance_clock(time: Res<Time>, mut materials: ResMut<Assets<WindAura>>) {
    for (_, material) in materials.iter_mut() {
        material.params.clock.x = time.elapsed_secs();
    }
}

/// Wind shell: a bit larger than the ward's own, streaks combed along the meadow's wind.
const SHELL_SCALE: f32 = 1.08;
const AURA_TINT: Vec4 = Vec4::new(0.55, 0.95, 1.0, 1.0);
/// The ward pushes the grass out to this radius (m), bending blades by this much (rad).
const GUST_RADIUS: f32 = 3.2;
const GUST_BEND: f32 = 0.9;

/// Give each new ward shell its wind shell.
fn dress_wards(
    mut commands: Commands,
    wind: Res<GrassWind>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindAura>>,
    shells: Query<(Entity, &WardShell), Added<WardShell>>,
    players: Query<(), With<Player>>,
) {
    for (shell, ward) in &shells {
        let dir = wind.direction.normalize_or(Vec2::X);
        // The player's ward is cool cyan; anyone else's is a warmer white-gold.
        let tint = if players.contains(ward.owner) { AURA_TINT } else { Vec4::new(1.0, 0.85, 0.5, 1.0) };
        let material = materials.add(WindAura {
            params: WindAuraParams {
                color: tint,
                wind: Vec4::new(dir.x, dir.y, 2.4 + wind.strength * 2.0, 1.6),
                clock: Vec4::new(0.0, 2.5, 0.0, 0.0),
            },
        });
        commands.spawn((
            Name::new("Ward wind"),
            Mesh3d(meshes.add(Sphere::new(1.0))),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::splat(SHELL_SCALE)),
            ChildOf(shell),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// While a ward is up, blow the grass away from its owner.
fn ward_gust(
    mut wind: ResMut<GrassWind>,
    wards: Query<(&Transform, &Ward)>,
    camera: Query<&Transform, With<super::camera::MainCamera>>,
) {
    let eye = camera.iter().next().map_or(Vec3::ZERO, |t| t.translation);
    let nearest = wards
        .iter()
        .min_by(|a, b| a.0.translation.distance(eye).total_cmp(&b.0.translation.distance(eye)));
    wind.gust = match nearest {
        Some((tf, ward)) => {
            // Fades in and out with the ward's remaining life.
            let fade = (ward.left / 0.4).clamp(0.0, 1.0);
            GrassGust { centre: tf.translation, radius: GUST_RADIUS, strength: GUST_BEND * fade }
        }
        None => GrassGust::default(),
    };
}

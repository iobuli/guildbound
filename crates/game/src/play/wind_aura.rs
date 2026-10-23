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

#[derive(Resource)]
struct ShellGeometry(Handle<Mesh>);

#[derive(Component)]
struct ChargeVisual {
    shell: Entity,
    particles: Entity,
    material: Handle<WindAura>,
    focus: Option<Entity>,
}

#[derive(Component)]
struct ChargeShell;

#[derive(Component)]
struct ChargeFocus;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WindAura>::default())
        .add_systems(Startup, spawn_test)
        .add_systems(Update, (toggle, advance_clock, dress_wards, ward_gust));
    app.add_systems(Update, charge_shells.after(super::PlaySet::Think));
}

fn spawn_test(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<WindAura>>) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().uv(20, 12));
    commands.insert_resource(ShellGeometry(mesh.clone()));
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
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(1.2)),
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
    geometry: Res<ShellGeometry>,
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
            Mesh3d(geometry.0.clone()),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::splat(SHELL_SCALE)),
            ChildOf(shell),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// The charge is a thin changing rim with ascending motes, leaving the body and animation visible.
fn charge_shells(
    mut commands: Commands, time: Res<Time>, geometry: Res<ShellGeometry>,
    vfx: Option<Res<super::vfx::Vfx>>, wind: Res<GrassWind>,
    mut materials: ResMut<Assets<WindAura>>,
    bank: Option<Res<crate::motion::bank::PoseBank>>, globals: Query<&GlobalTransform>,
    casters: Query<(Entity, Option<&super::spell_aim::Aiming>, &super::magic::Grimoire,
        &super::magic::Energy, Option<&ChargeVisual>, &Transform, Option<&crate::motion::Rig>),
        (With<Player>, Without<ChargeShell>)>,
    mut shells: Query<&mut Transform, (With<ChargeShell>, Without<Player>, Without<ChargeFocus>)>,
    mut foci: Query<(&mut Transform, &mut MeshMaterial3d<StandardMaterial>),
        (With<ChargeFocus>, Without<Player>, Without<ChargeShell>)>,
) {
    let Some(vfx) = vfx else { return };
    for (owner, aim, grimoire, energy, visual, root, rig) in &casters {
        let Some(aim) = aim else {
            if let Some(visual) = visual {
                commands.entity(visual.shell).try_despawn();
                commands.entity(visual.particles).try_despawn();
                if let Some(focus) = visual.focus { commands.entity(focus).try_despawn(); }
                commands.entity(owner).try_remove::<ChargeVisual>();
            }
            continue;
        };
        let id = if aim.which == super::spell_aim::Which::Art { grimoire.art } else { grimoire.spell };
        let Some(a) = super::magic::ability(id) else { continue };
        let charge = super::magic::affordable_charge(a, energy, grimoire.is_mastered(id), aim.t / 1.5);
        let tint = super::magic_fx::charge_color(a, charge).to_linear();
        let dir = wind.direction.normalize_or(Vec2::X);
        let params = WindAuraParams {
            color: Vec4::new(tint.red * 1.5, tint.green * 1.5, tint.blue * 1.5, 0.2 + charge * 0.7),
            wind: Vec4::new(dir.x, dir.y, 2.0 + charge * 6.0, 1.4 + charge),
            clock: Vec4::new(time.elapsed_secs(), 3.5, charge, 1.0),
        };
        let scale = Vec3::new(0.65 + charge * 0.12, 0.95 + charge * 0.08, 0.65 + charge * 0.12);
        let focus_at = bank.as_ref().map_or(root.translation + Vec3::Y * 0.5,
            |b| super::magic::cast_origin(a, rig, b, &globals, root));
        let focus_scale = Vec3::splat(0.065 + charge * 0.075);
        if let Some(visual) = visual {
            if let Some(mut m) = materials.get_mut(&visual.material) { m.params = params; }
            if let Ok(mut tf) = shells.get_mut(visual.shell) { tf.scale = scale; }
            if let Some(focus) = visual.focus
                && let Ok((mut tf, mut mat)) = foci.get_mut(focus) {
                tf.translation = focus_at; tf.scale = focus_scale;
                mat.0 = vfx.charged_material(a.element, charge);
            }
        } else {
            let material = materials.add(WindAura { params });
            let shell = commands.spawn((Name::new("Charging wind"), ChargeShell,
                Mesh3d(geometry.0.clone()), MeshMaterial3d(material.clone()),
                Transform::from_scale(scale), ChildOf(owner), bevy::light::NotShadowCaster)).id();
            let particles = commands.spawn((Name::new("Rising charge motes"), vfx.rising(a.element),
                Transform::default(), ChildOf(owner))).id();
            let focus = super::magic_fx::two_handed(a).then(|| commands.spawn((
                Name::new("Two-palm spell focus"), ChargeFocus, Mesh3d(vfx.ball_mesh.clone()),
                MeshMaterial3d(vfx.charged_material(a.element, charge)),
                Transform::from_translation(focus_at).with_scale(focus_scale),
                bevy::light::NotShadowCaster)).id());
            commands.entity(owner).insert(ChargeVisual { shell, particles, material, focus });
        }
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

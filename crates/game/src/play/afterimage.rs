//! Quick strikes read as quick, not fast-forwarded: when a swing's attack speed is above
//! `combat::ANTICIPATE_FROM`, its strike frames leave fading copies of the weapon (or of the fist,
//! bare-handed) along the blow's path, and the body carries through past the contact with a spring
//! kick on the spine and the striking arm. `combat` shortens the wind-up and holds the gathered
//! pose a beat (`phase_speed`, `Swing.anticipate`); this module adds what the eye reads.
//!
//! Ownership: `Afterimage` entities (spawned and despawned here), `StrikeFx` on fighters, and
//! pushes to `MotionLayers.impulses` at the start of a quick strike.
use super::combat::{ANTICIPATE_FROM, Melee, WeaponSlot};
use super::{Fighter, PlaySet};
use crate::motion::bank::PoseBank;
use crate::motion::{MotionLayers, Rig};
use bevy::prelude::*;

/// Seconds an afterimage lasts, seconds between copies, and how many shades it fades through.
pub const LIFE: f32 = 0.14;
pub const EVERY: f32 = 1.0 / 45.0;
const SHADES: usize = 4;
/// Follow-through: spring kick (rad/s) per unit of attack speed above 1, on the spine and arm.
const OVERSHOOT: f32 = 2.6;
/// Fist ghost radius (m) for bare-handed strikes.
const FIST: f32 = 0.06;

/// A fading copy; `left` seconds to live.
#[derive(Component)]
pub struct Afterimage {
    pub left: f32,
}

/// Per fighter: the followed swing's clock last frame, time since the last copy, and whether its
/// follow-through has fired.
#[derive(Component, Default)]
pub struct StrikeFx {
    last: f32,
    since: f32,
    kicked: bool,
}

#[derive(Resource)]
struct Ghosts {
    shades: Vec<Handle<StandardMaterial>>,
    fist: Handle<Mesh>,
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, StrikeFx>()
        .add_systems(Startup, setup)
        .add_systems(Update, (trail, fade).chain().in_set(PlaySet::Act).after(super::combat::CombatSet));
}

/// Alpha of an afterimage with `left` of `LIFE` seconds remaining.
pub fn alpha(left: f32) -> f32 {
    (left / LIFE).clamp(0.0, 1.0).powf(1.5) * 0.55
}

fn shade(left: f32) -> usize {
    (((1.0 - (left / LIFE).clamp(0.0, 1.0)) * SHADES as f32) as usize).min(SHADES - 1)
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>) {
    let shades = (0..SHADES)
        .map(|i| {
            let left = LIFE * (1.0 - (i as f32 + 0.5) / SHADES as f32);
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.85, 0.92, 1.0, alpha(left)),
                emissive: LinearRgba::rgb(0.25, 0.3, 0.4),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(Ghosts { shades, fist: meshes.add(Sphere::new(FIST)) });
}

#[allow(clippy::type_complexity)]
fn trail(
    time: Res<Time>,
    mut commands: Commands,
    ghosts: Option<Res<Ghosts>>,
    bank: Option<Res<PoseBank>>,
    mut fighters: Query<(&Melee, &mut StrikeFx, Option<&WeaponSlot>, Option<&Rig>, Option<&mut MotionLayers>)>,
    children: Query<&Children>,
    meshes: Query<(&Mesh3d, &GlobalTransform)>,
    globals: Query<&GlobalTransform>,
    active_images: Query<(), With<Afterimage>>,
    active_flashes: Query<(), With<super::vfx::Flash>>,
    limits: Res<super::vfx::VfxLimits>,
) {
    let (Some(ghosts), Some(bank)) = (ghosts, bank) else { return };
    let dt = time.delta_secs();
    let mut accents = active_images.iter().count() + active_flashes.iter().count();
    for (melee, mut fx, slot, rig, layers) in &mut fighters {
        let Some((speed, elapsed, active)) = melee.swing_speed() else {
            *fx = StrikeFx::default();
            continue;
        };
        // A chained swing starts its clock again.
        if elapsed < fx.last {
            *fx = StrikeFx::default();
        }
        fx.last = elapsed;
        if !active || (slot.is_none() && speed <= ANTICIPATE_FROM) {
            continue;
        }
        // Follow-through: once per swing, at the first strike frame.
        if !fx.kicked && slot.is_none() {
            fx.kicked = true;
            if let Some(mut layers) = layers {
                let b = &bank.bones;
                let kick = OVERSHOOT * (speed - 1.0);
                for (bone, share) in [(b.spine[1], 0.5), (b.spine[2], 0.8), (b.upperarm[1], 1.0), (b.lowerarm[1], 0.7)] {
                    layers.impulses.push((bone, Vec3::Y * kick * share));
                }
            }
        }
        fx.since += dt;
        if fx.since < EVERY {
            continue;
        }
        fx.since = 0.0;
        let mut spawn = |mesh: Handle<Mesh>, at: Transform| {
            if accents >= limits.flashes { return; }
            accents += 1;
            commands.spawn((Afterimage { left: LIFE }, Mesh3d(mesh), MeshMaterial3d(ghosts.shades[0].clone()), at,
                bevy::light::NotShadowCaster));
        };
        match slot {
            Some(slot) => {
                for node in children.iter_descendants(slot.weapon) {
                    if let Ok((mesh, g)) = meshes.get(node) {
                        spawn(mesh.0.clone(), g.compute_transform());
                    }
                }
            }
            None => {
                let hand = rig.and_then(|r| r.bones.get(bank.bones.hand[1]).copied()).and_then(|h| globals.get(h).ok());
                if let Some(g) = hand {
                    spawn(ghosts.fist.clone(), Transform::from_translation(g.translation()));
                }
            }
        }
    }
}

fn fade(time: Res<Time>, mut commands: Commands, ghosts: Option<Res<Ghosts>>, mut images: Query<(Entity, &mut Afterimage, &mut MeshMaterial3d<StandardMaterial>)>) {
    let Some(ghosts) = ghosts else { return };
    let dt = time.delta_secs();
    for (e, mut a, mut mat) in &mut images {
        a.left -= dt;
        if a.left <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        let want = &ghosts.shades[shade(a.left)];
        if mat.0 != *want {
            mat.0 = want.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn afterimages_fade_out_and_go() {
        let mut app = App::new();
        app.insert_resource(Ghosts { shades: (0..SHADES).map(|_| Handle::default()).collect(), fist: Handle::default() })
            .init_resource::<Time>()
            .add_systems(Update, fade);
        let e = app.world_mut().spawn((Afterimage { left: LIFE }, MeshMaterial3d::<StandardMaterial>(Handle::default()))).id();
        assert!(alpha(LIFE) > alpha(LIFE * 0.5) && alpha(LIFE * 0.5) > alpha(0.01));
        assert_eq!(shade(LIFE), 0);
        assert_eq!(shade(0.001), SHADES - 1);
        for _ in 0..30 {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs_f32(1.0 / 60.0));
            app.update();
        }
        assert!(app.world().get_entity(e).is_err(), "despawned after its life");
    }
}

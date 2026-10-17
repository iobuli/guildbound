//! Footsteps, landings and ambience.
//!
//! Footsteps come from the drawn feet, not a clock: after transform propagation each character's
//! heel and ball are measured against the ground under them (a short ray down against
//! `Phys::World`). A foot that rose past `LIFT_UP` and comes back under `PLANT` plants: one
//! `Sfx` step, its kind from the surface (terrain: grass, the courtyard's bare earth, the pond's
//! sand, rock on steep or high ground; anything else, stairs, boxes, props: hard), louder with
//! speed. A wearer of plate armour (`BodyPadding`) jingles a little with each step. A touchdown
//! after `LAND_MIN` m/s of fall plays a landing, louder with the impact.
//!
//! Ambience: a looping wind bed, and a bird call every `BIRD_GAP` seconds somewhere near the
//! camera. `audio` owns these two loops; one-shots go through `fx` (`Sfx`).
use super::camera::MainCamera;
use super::fx::{Sfx, SfxKind};
use super::world::{POND_CENTER, POND_RADIUS, Terrain, WATER_LEVEL, courtyard_distance};
use super::{MoveMode, Mover, Phys};
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use crate::motion::body::BodyPadding;
use avian3d::prelude::*;
use bevy::audio::Volume;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use std::collections::HashMap;

/// A foot counts as down below this height (m) over the ground and must rise past `LIFT_UP`
/// before it can plant again.
const PLANT: f32 = 0.03;
const LIFT_UP: f32 = 0.065;
/// The same foot never steps twice within this long (s).
const STEP_GAP: f32 = 0.16;
/// Steps need this much horizontal speed (m/s).
const STEP_SPEED: f32 = 0.35;
/// Ray from this far above the sole, this far down (m).
const RAY_UP: f32 = 0.4;
const RAY_DOWN: f32 = 0.9;
/// Step volume: base plus per m/s, capped.
const STEP_BASE: f32 = 0.16;
const STEP_PER_SPEED: f32 = 0.07;
const STEP_MAX: f32 = 0.6;
/// Landings: least fall speed (m/s) and volume per m/s.
const LAND_MIN: f32 = 2.5;
const LAND_PER_SPEED: f32 = 0.09;
/// Ambience: wind bed volume, seconds between bird calls (min, max), how far away they sing.
const WIND_VOLUME: f32 = 0.22;
const BIRD_GAP: (f32, f32) = (5.0, 14.0);
const BIRD_VOLUME: f32 = 0.12;

pub fn plugin(app: &mut App) {
    app.init_resource::<Feet>()
        .add_systems(Startup, start_ambience)
        .add_systems(PostUpdate, footsteps.after(TransformSystems::Propagate))
        .add_systems(Update, birds);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Grass,
    Dirt,
    Sand,
    Hard,
}

impl Surface {
    fn sfx(self) -> SfxKind {
        match self {
            Surface::Grass => SfxKind::StepGrass,
            Surface::Dirt => SfxKind::StepDirt,
            Surface::Sand => SfxKind::StepSand,
            Surface::Hard => SfxKind::StepHard,
        }
    }
}

/// What the terrain is made of at a point (the rules `world::ground_color` paints by).
fn terrain_surface(p: Vec3, normal: Vec3) -> Surface {
    let pond = Vec2::new(p.x, p.z).distance(POND_CENTER);
    if pond < POND_RADIUS + 1.5 || p.y < WATER_LEVEL + 0.05 {
        Surface::Sand
    } else if courtyard_distance(p.x, p.z) < 0.5 {
        Surface::Dirt
    } else if normal.y < 0.8 || p.y > 8.0 {
        Surface::Hard
    } else {
        Surface::Grass
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Foot {
    /// Risen past `LIFT_UP` since the last plant.
    lifted: bool,
    since: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Walker {
    feet: [Foot; 2],
    grounded: bool,
}

#[derive(Resource, Default)]
struct Feet(HashMap<Entity, Walker>);

#[allow(clippy::too_many_arguments)]
fn footsteps(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    spatial: SpatialQuery,
    mut feet: ResMut<Feet>,
    walkers: Query<(Entity, &Rig, &Mover, &GlobalTransform, Has<BodyPadding>)>,
    bones: Query<&GlobalTransform>,
    terrain: Query<(), With<Terrain>>,
    mut sfx: MessageWriter<Sfx>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs();
    let b = bank.bones;
    let rest = bank.skeleton.fk(&bank.skeleton.rest_pose(), b.pelvis);
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    feet.0.retain(|e, _| walkers.contains(*e));
    for (entity, rig, mover, root, armoured) in &walkers {
        let state = feet.0.entry(entity).or_default();
        let on_ground = mover.grounded && mover.mode == MoveMode::Ground;
        if on_ground && !state.grounded && mover.impact > LAND_MIN {
            sfx.write(Sfx {
                kind: SfxKind::Land,
                at: root.translation(),
                volume: (mover.impact * LAND_PER_SPEED).min(1.0),
            });
        }
        state.grounded = on_ground;
        let speed = Vec2::new(mover.velocity.x, mover.velocity.z).length();
        for side in 0..2 {
            let foot = &mut state.feet[side];
            foot.since += dt;
            let (Some(&ankle), Some(&ball)) = (rig.bones.get(b.foot[side]), rig.bones.get(b.ball[side])) else {
                continue;
            };
            let (Ok(ankle), Ok(ball)) = (bones.get(ankle), bones.get(ball)) else { continue };
            // Sole points: each joint lowered by its rest height above the soles.
            let heel = ankle.translation() - Vec3::Y * rest[b.foot[side]].translation.y;
            let toe = ball.translation() - Vec3::Y * rest[b.ball[side]].translation.y;
            let sole = if heel.y < toe.y { heel } else { toe };
            let Some(hit) = spatial.cast_ray(sole + Vec3::Y * RAY_UP, Dir3::NEG_Y, RAY_UP + RAY_DOWN, true, &filter)
            else {
                foot.lifted = true;
                continue;
            };
            let height = RAY_UP - hit.distance;
            if height > LIFT_UP {
                foot.lifted = true;
                continue;
            }
            if height > PLANT || !foot.lifted {
                continue;
            }
            foot.lifted = false;
            if !on_ground || speed < STEP_SPEED || foot.since < STEP_GAP {
                continue;
            }
            foot.since = 0.0;
            let ground = sole.with_y(sole.y - height);
            let surface = if terrain.contains(hit.entity) { terrain_surface(ground, hit.normal) } else { Surface::Hard };
            let volume = (STEP_BASE + STEP_PER_SPEED * speed).min(STEP_MAX);
            sfx.write(Sfx {
                kind: surface.sfx(),
                at: ground,
                volume,
            });
            if armoured {
                sfx.write(Sfx {
                    kind: SfxKind::Jingle,
                    at: ground + Vec3::Y,
                    volume: volume * 0.35,
                });
            }
        }
    }
}

#[derive(Resource)]
struct Ambience {
    birds: Vec<Handle<AudioSource>>,
    next_bird: f32,
    seed: u32,
}

impl Ambience {
    fn rand(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x & 0xFFFF) as f32 / 65535.0
    }
}

fn start_ambience(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Name::new("Wind bed"),
        AudioPlayer::new(assets.load("audio/synth/ambience_wind.wav")),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(WIND_VOLUME)),
    ));
    commands.insert_resource(Ambience {
        birds: (0..4).map(|i| assets.load(format!("audio/synth/bird_{i}.wav"))).collect(),
        next_bird: BIRD_GAP.0,
        seed: 0x2545_F491,
    });
}

fn birds(
    mut commands: Commands,
    time: Res<Time>,
    ambience: Option<ResMut<Ambience>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let Some(mut amb) = ambience else { return };
    amb.next_bird -= time.delta_secs();
    if amb.next_bird > 0.0 {
        return;
    }
    amb.next_bird = BIRD_GAP.0 + (BIRD_GAP.1 - BIRD_GAP.0) * amb.rand();
    if camera.single().is_err() || amb.birds.is_empty() {
        return;
    }
    let i = ((amb.rand() * amb.birds.len() as f32) as usize).min(amb.birds.len() - 1);
    let speed = 0.9 + 0.25 * amb.rand();
    let volume = BIRD_VOLUME * (0.5 + 0.5 * amb.rand());
    commands.spawn((
        Name::new("Bird"),
        AudioPlayer::new(amb.birds[i].clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(speed),
    ));
}

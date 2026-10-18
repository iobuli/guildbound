//! Impact ragdoll (`Update / PlaySet::Act`, after `CombatSet`). On a `RagdollRequest` (from
//! combat: knockback above `Feel.ragdoll_threshold`; or F6 on the player, for testing) the body
//! goes to physics:
//! - `take_over` spawns 11 dynamic segments (pelvis, chest, head, upper / lower arms, thighs,
//!   calves) at the bones' current world transforms, linked by `SphericalJoint`s whose swing and
//!   twist limits are centred on the pose at the moment of the hit (so limbs bend a plausible
//!   amount from where they were, never through the body), on `Phys::Ragdoll` (world and each
//!   other, not the character capsules). Every segment leaves with the body's velocity plus the
//!   blow's; the one nearest the blow gets `KICK` times more. The `Action` is cleared (combat
//!   drops its reaction) and `Mover.mode` becomes `Ragdoll`, so the controller and `select`
//!   leave the body alone.
//! - `drive` reads the segments back every frame into one full-body `Source::Pose` layer with
//!   `MotionLayers.raw` (springs and contact bypassed): simulated bones take their segment's
//!   rotation, the others (spine_01 / 03, neck, clavicles, hands, feet, fingers) keep their
//!   local rotation from the moment of the hit. The character root follows the pelvis segment
//!   (rotation kept) so the camera, hit tests and the minimap follow the body.
//! - Alive and settled (`SETTLE_SPEED` for `SETTLE_TIME`, at least `MIN_TIME`, at most
//!   `MAX_TIME`): the root is put back on the ground under the pelvis facing the feet, the
//!   segments despawn, and the get-up `Action` (`Owner::Ragdoll`, `ActionKind::GetUp`,
//!   `ual2/LayToIdle`) plays; the springs ease the pose from the lying body into the clip.
//!   Dead bodies stay down as ragdolls until combat revives them (its own get-up then plays).
//! - Defeat (`limp`): a fighter killed without a ragdoll-sized blow plays the first `DEATH_LIMP`
//!   s of its death clip (the knees go), then goes limp the same way, so the body falls onto
//!   whatever ground is there (slopes, steps, rocks) instead of a flat-floor clip sinking into it.
//!   When a dead body first comes to rest, every segment is checked against the ground under it
//!   (`[ragdoll] down` log line); a segment found under the surface is lifted back onto it.
//! - A new request while down kicks the segments again.
//! - Weight: a blow is one impulse on the whole body (`fling`): the centre of mass takes the push
//!   (capped at `MAX_FLING`) and the off-centre part spins the body by its inertia (capped at
//!   `MAX_SPIN`), so every segment leaves with one rigid motion; the segment struck gets a little
//!   extra (`KICK`).
//! - Air recovery (player): jump while flung clear of the ground (`RECOVER_HEIGHT`, after
//!   `RECOVER_AFTER` s, `RECOVER_COST` stamina) to leave the ragdoll in the air with the momentum;
//!   the landing then bounces toward the lock-on target or nearest foe into a run (`bounce`).
use super::combat::CombatSet;
use super::{
    Action, ActionKind, ActionLayer, Fighter, HALF_HEIGHT, Intent, MoveMode, Mover, Owner, Phys, PlaySet, Player, StaminaSpend,
};
use crate::motion::bank::PoseBank;
use crate::motion::{Layer, Mask, MotionLayers, Pose, Rig, Skeleton, Source};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Hand a character's body to physics: inserted by `combat` (knockback above
/// `Feel.ragdoll_threshold`) or a debug key. `ragdoll` consumes it (removes it when it takes
/// over); from then until the get-up ends it owns the body, `Mover.mode` (`Ragdoll`) and
/// `MotionLayers.raw`, and the get-up `Action` (`Owner::Ragdoll`, `ActionKind::GetUp`).
#[derive(Component, Clone, Copy, Debug)]
pub struct RagdollRequest {
    /// World velocity (m/s) the body leaves with.
    pub velocity: Vec3,
    /// World point the blow landed on (the segment nearest it gets the extra push).
    pub point: Vec3,
}

/// The segment nearest the blow gets this much more of the push (a local snap on top of the
/// whole-body fling).
const KICK: f32 = 1.15;
/// Fling limits: the body's centre never leaves faster than `MAX_FLING` m/s, nor spins faster
/// than `MAX_SPIN` rad/s.
const MAX_FLING: f32 = 18.0;
const MAX_SPIN: f32 = 12.0;
/// Air recovery (player): earliest (s), height above the ground the pelvis needs (m), stamina it
/// costs, the fastest the body keeps (m/s); the landing bounce toward the nearest foe (m/s, up m/s)
/// and how far to look for one (m).
const RECOVER_AFTER: f32 = 0.25;
const RECOVER_HEIGHT: f32 = 0.7;
const RECOVER_COST: f32 = 15.0;
const RECOVER_KEEP: f32 = 12.0;
const BOUNCE: f32 = 7.5;
const BOUNCE_UP: f32 = 3.0;
const BOUNCE_LOOK: f32 = 18.0;

/// A blow on a body of point masses `(position, mass)` that should move its centre of mass by
/// `dv` (m/s), landing at `hit`: the centre's velocity change (capped at `MAX_FLING`) and the
/// spin the same impulse gives about the centre (ω = I⁻¹ (r × J), capped at `MAX_SPIN`), so a
/// blow high on a light body spins it hard and one through the middle just throws it.
pub fn fling(masses: &[(Vec3, f32)], hit: Vec3, dv: Vec3) -> (Vec3, Vec3, Vec3) {
    let total: f32 = masses.iter().map(|(_, m)| m).sum::<f32>().max(1e-3);
    let com = masses.iter().map(|(p, m)| *p * *m).sum::<Vec3>() / total;
    let dv = dv.clamp_length_max(MAX_FLING);
    // Inertia tensor about the centre (point masses plus a small own term each).
    let mut inertia = Mat3::ZERO;
    for (p, m) in masses {
        let r = *p - com;
        inertia += (Mat3::IDENTITY * r.length_squared() - Mat3::from_cols(r * r.x, r * r.y, r * r.z)) * *m;
        inertia += Mat3::IDENTITY * (0.01 * m);
    }
    let impulse = dv * total;
    let torque = (hit - com).cross(impulse);
    let omega = if inertia.determinant().abs() > 1e-6 { inertia.inverse() * torque } else { Vec3::ZERO };
    let omega = if omega.is_finite() { omega.clamp_length_max(MAX_SPIN) } else { Vec3::ZERO };
    (com, dv, omega)
}
/// Settled: every segment slower than this (m/s) for `SETTLE_TIME` s, after at least `MIN_TIME`
/// s down; get up after `MAX_TIME` s regardless.
const SETTLE_SPEED: f32 = 0.6;
const SETTLE_TIME: f32 = 0.35;
const MIN_TIME: f32 = 0.9;
const MAX_TIME: f32 = 4.0;
/// Get-up clip and its playback rate (combat's knockdown uses the same).
const GET_UP: &str = "ual2/LayToIdle";
const GET_UP_RATE: f32 = 1.3;
/// Damping on every segment, and the joints' compliance (softens the limit stops).
const LINEAR_DAMPING: f32 = 0.3;
/// Segment friction: bodies come to rest on a slope instead of sledging down it.
const SEGMENT_FRICTION: f32 = 1.0;
const ANGULAR_DAMPING: f32 = 1.2;
const LIMIT_COMPLIANCE: f32 = 1e-5;
/// Defeat: seconds of the death clip before the body goes limp, and the push it falls with (m/s,
/// backwards from the body's facing) on top of its own velocity.
const DEATH_LIMP: f32 = 0.3;
const DEATH_SLUMP: f32 = 1.2;
/// Most sideways speed a dead body keeps as it goes limp (m/s).
const LIMP_DRIFT: f32 = 2.0;
/// A settled dead segment whose surface is more than this below the ground is lifted onto it (m).
const SUNK: f32 = 0.03;
/// Deepest a segment is looked for under the surface (m); anything deeper is left alone.
const MAX_SINK: f32 = 0.6;
/// F6 throws the player backwards at this speed (m/s).
const DEBUG_PUSH: f32 = 6.0;

/// One simulated segment: its bone, the bone its capsule reaches to (`None`: a sphere `reach`
/// metres past the bone along parent → bone), parent segment, radius (m), mass (kg), swing
/// half-angle and twist limit (degrees, about the pose at the hit).
struct Seg {
    bone: &'static str,
    end: Option<&'static str>,
    parent: Option<usize>,
    radius: f32,
    mass: f32,
    swing: f32,
    twist: f32,
}

const SEGS: [Seg; 11] = [
    Seg { bone: "pelvis", end: Some("spine_02"), parent: None, radius: 0.13, mass: 12.0, swing: 0.0, twist: 0.0 },
    Seg { bone: "spine_02", end: Some("neck_01"), parent: Some(0), radius: 0.15, mass: 15.0, swing: 35.0, twist: 30.0 },
    Seg { bone: "Head", end: None, parent: Some(1), radius: 0.11, mass: 5.0, swing: 45.0, twist: 60.0 },
    Seg { bone: "upperarm_l", end: Some("lowerarm_l"), parent: Some(1), radius: 0.055, mass: 2.5, swing: 85.0, twist: 60.0 },
    Seg { bone: "upperarm_r", end: Some("lowerarm_r"), parent: Some(1), radius: 0.055, mass: 2.5, swing: 85.0, twist: 60.0 },
    Seg { bone: "lowerarm_l", end: Some("hand_l"), parent: Some(3), radius: 0.045, mass: 1.8, swing: 75.0, twist: 40.0 },
    Seg { bone: "lowerarm_r", end: Some("hand_r"), parent: Some(4), radius: 0.045, mass: 1.8, swing: 75.0, twist: 40.0 },
    Seg { bone: "thigh_l", end: Some("calf_l"), parent: Some(0), radius: 0.08, mass: 9.0, swing: 65.0, twist: 25.0 },
    Seg { bone: "thigh_r", end: Some("calf_r"), parent: Some(0), radius: 0.08, mass: 9.0, swing: 65.0, twist: 25.0 },
    Seg { bone: "calf_l", end: Some("foot_l"), parent: Some(7), radius: 0.06, mass: 4.0, swing: 75.0, twist: 10.0 },
    Seg { bone: "calf_r", end: Some("foot_r"), parent: Some(8), radius: 0.06, mass: 4.0, swing: 75.0, twist: 10.0 },
];
const HEAD: usize = 2;
/// Head sphere centre past the head joint along neck → head (m).
const HEAD_REACH: f32 = 0.1;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, fling_smoke.after(PlaySet::Input).before(PlaySet::Act));
    app.add_systems(
        Update,
        (debug_key, limp, take_over, drive, bounce)
            .chain()
            .in_set(PlaySet::Act)
            .after(CombatSet),
    );
}

/// On a ragdolled character: its segments (per `SEGS`, `PLACEHOLDER` where the bone is
/// missing), joint entities, the pose at the hit, and the clocks.
#[derive(Component, Clone, Debug)]
pub struct Ragdolled {
    segs: Vec<Entity>,
    joints: Vec<Entity>,
    /// Canonical bone index per segment.
    bones: Vec<usize>,
    /// Local pose at the hit (non-simulated bones keep it).
    snapshot: Pose,
    /// Pelvis in model space at the hit (the root follows it).
    pelvis_model: Vec3,
    t: f32,
    still: f32,
    /// A dead body's rest was checked against the ground (`drive`).
    checked: bool,
}

/// The get-up clip in progress after a ragdoll (`Owner::Ragdoll`).
#[derive(Component, Clone, Copy, Debug)]
pub struct GettingUp {
    t: f32,
    duration: f32,
}

fn debug_key(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut commands: Commands,
    players: Query<(Entity, &Transform, &Mover), With<Player>>,
) {
    if !keys.is_some_and(|k| k.just_pressed(KeyCode::F6)) {
        return;
    }
    for (entity, tf, mover) in &players {
        commands.entity(entity).try_insert(RagdollRequest {
            velocity: -mover.forward() * DEBUG_PUSH + Vec3::Y * 2.0,
            point: tf.translation + Vec3::Y * 0.4,
        });
    }
}

/// Defeat: a dead fighter that isn't a ragdoll yet goes limp `DEATH_LIMP` s into its death clip
/// (combat's `Dead` action), falling with its own velocity and a slump backwards.
#[allow(clippy::type_complexity)]
fn limp(
    mut commands: Commands,
    mut bodies: Query<(Entity, &Transform, &mut Mover, &Action, &Fighter), (Without<Ragdolled>, Without<RagdollRequest>, Without<GettingUp>)>,
) {
    for (entity, tf, mut mover, action, fighter) in &mut bodies {
        if !fighter.dead || action.kind != ActionKind::Dead || action.elapsed < DEATH_LIMP {
            continue;
        }
        // The body falls from where it stands: at most a gentle drift along the ground, never up
        // (`take_over` adds this velocity to every segment). The request adds only the slump.
        let v = mover.velocity;
        let flat = Vec3::new(v.x, 0.0, v.z).clamp_length_max(LIMP_DRIFT);
        mover.velocity = if v.is_finite() { Vec3::new(flat.x, v.y.min(0.0), flat.z) } else { Vec3::ZERO };
        info!("[ragdoll] limp: body velocity {v:.2?} -> {:.2?}", mover.velocity);
        commands.entity(entity).try_insert(RagdollRequest {
            velocity: -mover.forward() * DEATH_SLUMP,
            point: tf.translation + Vec3::Y * 0.3,
        });
    }
}

/// Smoke: `GB_FLING=1` throws the player forward and up at 3 s and presses jump at 3.45 s (air
/// recovery, then the landing bounce).
fn fling_smoke(
    time: Res<Time>,
    mut on: Local<Option<bool>>,
    mut commands: Commands,
    mut players: Query<(Entity, &Transform, &Mover, &mut Intent), With<Player>>,
) {
    if !*on.get_or_insert_with(|| std::env::var("GB_FLING").is_ok_and(|v| v.trim() == "1")) {
        return;
    }
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    let Ok((e, tf, mover, mut intent)) = players.single_mut() else { return };
    if t >= 3.0 && t - dt < 3.0 {
        commands.entity(e).try_insert(RagdollRequest {
            velocity: mover.forward() * 9.0 + Vec3::Y * 7.0,
            point: tf.translation + Vec3::Y * 0.5,
        });
    }
    if t >= 3.45 && t - dt < 3.45 {
        intent.jump_pressed = true;
    }
}

/// World rotation and position of every bone (no scale), from the bone entities.
fn bone_world(rig: &Rig, globals: &Query<&GlobalTransform>, i: usize) -> Option<(Quat, Vec3)> {
    let e = rig.bones.get(i).copied().filter(|e| *e != Entity::PLACEHOLDER)?;
    let (_, r, t) = globals.get(e).ok()?.to_scale_rotation_translation();
    (r.is_finite() && t.is_finite()).then_some((r.normalize(), t))
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn take_over(
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    mut bodies: Query<(
        Entity,
        &RagdollRequest,
        &Rig,
        &mut Mover,
        &mut Action,
        &mut MotionLayers,
        Option<&Ragdolled>,
        Has<GettingUp>,
    )>,
    bone_locals: Query<&Transform, Without<Rig>>,
    globals: Query<&GlobalTransform>,
    mut velocities: Query<(&Transform, &mut LinearVelocity, &mut AngularVelocity), Without<Rig>>,
    spatial: SpatialQuery,
) {
    let Some(bank) = bank else { return };
    for (entity, request, rig, mut mover, mut action, mut layers, ragdolled, getting_up) in &mut bodies {
        commands.entity(entity).try_remove::<RagdollRequest>();
        if !(request.velocity.is_finite() && request.point.is_finite()) {
            continue;
        }
        // Already down: kick the segments again.
        if let Some(r) = ragdolled {
            let masses: Vec<(Vec3, f32)> = r
                .segs
                .iter()
                .zip(&SEGS)
                .filter_map(|(s, seg)| velocities.get(*s).ok().map(|(tf, _, _)| (tf.translation, seg.mass)))
                .collect();
            let (com, dv, omega) = fling(&masses, request.point, request.velocity);
            for &seg in &r.segs {
                if let Ok((tf, mut v, mut w)) = velocities.get_mut(seg) {
                    let near = tf.translation.distance(request.point) < 0.35;
                    v.0 += (dv + omega.cross(tf.translation - com)) * if near { KICK } else { 1.0 };
                    w.0 += omega;
                }
            }
            continue;
        }
        let skeleton = &bank.skeleton;
        let n = skeleton.len();
        if rig.bones.len() != n {
            continue;
        }
        // The pose being shown (compose wrote it into the bone entities last frame).
        let mut snapshot = bank.rest.clone();
        for (i, &e) in rig.bones.iter().enumerate() {
            if let Ok(tf) = bone_locals.get(e) {
                snapshot.rot[i] = tf.rotation;
                if i == bank.bones.pelvis {
                    snapshot.pelvis = tf.translation - skeleton.rest[i].translation;
                }
            }
        }
        // Nothing may start under the ground (a pose that sinks a foot or knee into the terrain
        // would be shoved out violently): lift the whole body by the deepest bone's depth.
        let lift = start_lift(&spatial, rig, &globals, skeleton);
        if lift > 0.0 {
            info!("[ragdoll] spawn lifted {lift:.3} m out of the ground");
        }
        let world = |i: usize| bone_world(rig, &globals, i).map(|(r, p)| (r, p + Vec3::Y * lift));
        let Some(segs_out) = spawn_segments(&mut commands, skeleton, world, mover.velocity, request) else {
            continue;
        };
        let (segs, joints, bones) = segs_out;
        let model_pelvis = skeleton.fk(&snapshot, bank.bones.pelvis)[bank.bones.pelvis].translation;
        commands.entity(entity).try_remove::<GettingUp>();
        commands.entity(entity).try_insert(Ragdolled {
            segs,
            joints,
            bones,
            snapshot,
            pelvis_model: model_pelvis,
            t: 0.0,
            still: 0.0,
            checked: false,
        });
        if getting_up || !action.is_free() {
            action.clear();
        }
        mover.mode = MoveMode::Ragdoll;
        mover.grounded = false;
        mover.root_motion = Vec3::ZERO;
        mover.external = Vec3::ZERO;
        mover.face = None;
        layers.raw = true;
    }
}

/// How far the body must rise so no segment's bone sits below the ground under it (m): the
/// deepest bone of `SEGS`, measured with a ray from above the pelvis.
fn start_lift(spatial: &SpatialQuery, rig: &Rig, globals: &Query<&GlobalTransform>, skeleton: &Skeleton) -> f32 {
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    let mut lift = 0.0f32;
    for seg in &SEGS {
        let Some(i) = skeleton.find(seg.bone) else { continue };
        let Some((_, p)) = bone_world(rig, globals, i) else { continue };
        let from = p + Vec3::Y * 0.8;
        if let Some(hit) = spatial.cast_ray(from, Dir3::NEG_Y, 3.0, false, &filter) {
            let ground = from.y - hit.distance;
            lift = lift.max(ground + seg.radius - p.y);
        }
    }
    lift.clamp(0.0, 1.0)
}

type Spawned = (Vec<Entity>, Vec<Entity>, Vec<usize>);

/// Segments and joints at the bones' world transforms; `None` when a needed bone is missing.
fn spawn_segments(
    commands: &mut Commands,
    skeleton: &Skeleton,
    world: impl Fn(usize) -> Option<(Quat, Vec3)>,
    body_velocity: Vec3,
    request: &RagdollRequest,
) -> Option<Spawned> {
    let mut bones = Vec::with_capacity(SEGS.len());
    let mut placed = Vec::with_capacity(SEGS.len());
    for seg in &SEGS {
        let i = skeleton.find(seg.bone)?;
        let (rot, pos) = world(i)?;
        let end = match seg.end {
            Some(end) => world(skeleton.find(end)?)?.1,
            None => {
                let parent = skeleton.parent[i].and_then(&world)?.1;
                pos + (pos - parent).normalize_or(Vec3::Y) * HEAD_REACH
            }
        };
        bones.push(i);
        placed.push((rot, pos, end));
    }
    let nearest = placed
        .iter()
        .enumerate()
        .min_by(|a, b| {
            let d = |p: &(Quat, Vec3, Vec3)| point_segment(request.point, p.1, p.2);
            d(a.1).total_cmp(&d(b.1))
        })
        .map(|(i, _)| i);
    // The blow as one impulse on the whole body: each segment leaves with the body's velocity,
    // the centre's push and the spin about the centre at its own place.
    let centre = |k: usize| {
        let (_, pos, end) = placed[k];
        if SEGS[k].end.is_some() { (pos + end) * 0.5 } else { end }
    };
    let masses: Vec<(Vec3, f32)> = (0..SEGS.len()).map(|k| (centre(k), SEGS[k].mass)).collect();
    let (com, dv, omega) = fling(&masses, request.point, request.velocity);
    let mut segs = Vec::with_capacity(SEGS.len());
    for (k, (seg, &(rot, pos, end))) in SEGS.iter().zip(&placed).enumerate() {
        let local_end = rot.inverse() * (end - pos);
        let collider = if seg.end.is_some() {
            Collider::capsule_endpoints(seg.radius, Vec3::ZERO, local_end)
        } else {
            Collider::compound(vec![(local_end, Quat::IDENTITY, Collider::sphere(seg.radius))])
        };
        let kick = if Some(k) == nearest { KICK } else { 1.0 };
        let id = commands
            .spawn((
                Name::new(format!("Ragdoll {}", seg.bone)),
                RagdollSegment,
                RigidBody::Dynamic,
                collider,
                Mass(seg.mass),
                CollisionLayers::new(Phys::Ragdoll, [Phys::World, Phys::Ragdoll]),
                Transform::from_translation(pos).with_rotation(rot),
                // Physics pose set up front, so the joints built this frame read the real pose.
                Position(pos),
                Rotation(rot),
                LinearVelocity(body_velocity + (dv + omega.cross(centre(k) - com)) * kick),
                AngularVelocity(omega),
                LinearDamping(LINEAR_DAMPING),
                Friction::new(SEGMENT_FRICTION),
                AngularDamping(ANGULAR_DAMPING),
                SweptCcd::default(),
            ))
            .id();
        segs.push(id);
    }
    let mut joints = Vec::new();
    for (k, seg) in SEGS.iter().enumerate() {
        let Some(p) = seg.parent else { continue };
        let (_, pos, end) = placed[k];
        // Twist about the limb (joint → end), swing in a cone around where it pointed at the hit.
        let basis = Quat::from_rotation_arc(Vec3::Y, (end - pos).normalize_or(Vec3::Y));
        let (swing, twist) = (seg.swing.to_radians(), seg.twist.to_radians());
        // Frames given locally on each body from the exact spawn pose (a world anchor and basis
        // resolved by the solver later left twisted poses, like a death, starting outside their
        // limits: the limbs snapped and the body launched itself).
        let (prot, ppos) = (placed[p].0, placed[p].1);
        let mut joint = SphericalJoint::new(segs[p], segs[k])
            .with_local_anchor1(prot.inverse() * (pos - ppos))
            .with_local_anchor2(Vec3::ZERO)
            .with_local_basis1(prot.inverse() * basis)
            .with_local_basis2(placed[k].0.inverse() * basis)
            .with_swing_limits(-swing, swing)
            .with_twist_limits(-twist, twist);
        joint.swing_compliance = LIMIT_COMPLIANCE;
        joint.twist_compliance = LIMIT_COMPLIANCE;
        joints.push(commands.spawn((Name::new(format!("Ragdoll joint {}", seg.bone)), joint, JointCollisionDisabled)).id());
    }
    Some((segs, joints, bones))
}

/// Marker on ragdoll segment bodies.
#[derive(Component)]
pub struct RagdollSegment;

fn point_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-8)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The pose shown while ragdolled: simulated bones take their segment's model-space rotation,
/// the rest keep their snapshot local rotation. `seg_rot[k]` are the segments' world rotations,
/// `model_rot` the model's world rotation.
pub fn pose_from_segments(skeleton: &Skeleton, snapshot: &Pose, bones: &[usize], seg_rot: &[Quat], model_rot: Quat) -> Pose {
    let mut pose = snapshot.clone();
    let mut model = vec![Quat::IDENTITY; skeleton.len()];
    let inv_model = model_rot.inverse();
    for i in 0..skeleton.len() {
        let parent = skeleton.parent[i].map_or(skeleton.base.rotation, |p| model[p]);
        let target = bones.iter().position(|&b| b == i).and_then(|k| seg_rot.get(k)).map(|r| inv_model * *r);
        match target {
            Some(m) if m.is_finite() => {
                pose.rot[i] = (parent.inverse() * m).normalize();
                model[i] = m;
            }
            _ => model[i] = parent * pose.rot[i],
        }
    }
    pose
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn drive(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    spatial: SpatialQuery,
    mut commands: Commands,
    mut bodies: Query<
        (
            Entity,
            &Rig,
            &mut Transform,
            &mut Mover,
            &mut Action,
            &mut MotionLayers,
            &Fighter,
            Option<&mut Ragdolled>,
            Option<&mut GettingUp>,
            Option<&Intent>,
            Has<Player>,
        ),
        Without<RagdollSegment>,
    >,
    models: Query<&Transform, (Without<Rig>, Without<RagdollSegment>)>,
    segments: Query<(&Transform, &LinearVelocity), With<RagdollSegment>>,
    mut stamina: MessageWriter<StaminaSpend>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, rig, mut root, mut mover, mut action, mut layers, fighter, ragdolled, getting_up, intent, player) in &mut bodies {
        if let Some(mut up) = getting_up {
            // Our get-up clip: advance it until it ends or another owner takes the body.
            if action.owner != Owner::Ragdoll || action.kind != ActionKind::GetUp {
                commands.entity(entity).try_remove::<GettingUp>();
                continue;
            }
            up.t += dt;
            if up.t >= up.duration {
                action.clear();
                mover.move_scale = 1.0;
                commands.entity(entity).try_remove::<GettingUp>();
                continue;
            }
            if let Some(layer) = action.layer.as_mut() {
                layer.time = up.t * GET_UP_RATE;
            }
            action.elapsed = up.t;
            mover.move_scale = 0.0;
            mover.root_motion = Vec3::ZERO;
            continue;
        }
        let Some(mut r) = ragdolled else { continue };
        let read: Vec<Option<(Transform, Vec3)>> =
            r.segs.iter().map(|s| segments.get(*s).ok().map(|(t, v)| (*t, v.0))).collect();
        let Some(Some((pelvis, pelvis_v))) = read.first().copied() else {
            // Segments gone (despawned elsewhere): hand the body back as it is.
            release(&mut commands, entity, &r, &mut mover, &mut layers);
            continue;
        };
        r.t += dt;
        let fastest = read.iter().flatten().map(|(_, v)| v.length()).fold(0.0, f32::max);
        r.still = if fastest < SETTLE_SPEED { r.still + dt } else { 0.0 };

        // Root follows the pelvis segment; the model keeps the root's rotation.
        let model_local = models.get(rig.model).copied().unwrap_or_default();
        let offset = root.rotation * model_local.transform_point(r.pelvis_model);
        let at = pelvis.translation - offset;
        if at.is_finite() {
            root.translation = at;
        }
        mover.velocity = pelvis_v;
        let model_rot = root.rotation * model_local.rotation;
        let seg_rot: Vec<Quat> = read.iter().map(|s| s.map_or(Quat::NAN, |(t, _)| t.rotation)).collect();
        let pose = pose_from_segments(&bank.skeleton, &r.snapshot, &r.bones, &seg_rot, model_rot);
        layers.raw = true;
        layers.layers = vec![Layer {
            source: Source::Pose(pose),
            weight: 1.0,
            mask: Mask::Full,
            style: None,
        }];

        // Air recovery: jump while flung clear of the ground to snap out of it, keeping the
        // momentum (once per fling; costs stamina; too close to the ground and you just land).
        let wants = player && intent.is_some_and(|i| i.jump_pressed);
        if wants && !fighter.dead && r.t >= RECOVER_AFTER && fighter.stamina >= RECOVER_COST {
            let filter = SpatialQueryFilter::from_mask(Phys::World);
            let clear = spatial.cast_ray(pelvis.translation, Dir3::NEG_Y, 8.0, true, &filter).map_or(8.0, |h| h.distance);
            if clear > RECOVER_HEIGHT {
                let flat = pelvis_v.with_y(0.0);
                let yaw = if flat.length_squared() > 0.25 { flat.x.atan2(flat.z) } else { mover.yaw };
                root.translation = pelvis.translation;
                root.rotation = Quat::from_rotation_y(yaw);
                mover.yaw = yaw;
                release(&mut commands, entity, &r, &mut mover, &mut layers);
                mover.mode = MoveMode::Air;
                mover.grounded = false;
                mover.velocity = Vec3::ZERO;
                let keep = pelvis_v.clamp_length_max(RECOVER_KEEP);
                mover.external = keep.with_y(keep.y.max(0.0) + 2.0);
                stamina.write(StaminaSpend { who: entity, amount: RECOVER_COST });
                commands.entity(entity).try_insert(AirRecovered(0.0));
                info!("[ragdoll] air recovery at {:.1} m up, carrying {:.1} m/s", clear, keep.length());
                continue;
            }
        }
        let settled = (r.t >= MIN_TIME && r.still >= SETTLE_TIME) || r.t >= MAX_TIME;
        if fighter.dead && settled && !r.checked {
            r.checked = true;
            let filter = SpatialQueryFilter::from_mask(Phys::World);
            let mut lowest = f32::MAX;
            for (k, (seg, s)) in r.segs.iter().zip(&read).enumerate() {
                let Some((tf, _)) = s else { continue };
                // Up from under the segment to the first surface: with the ground one sheet, that
                // surface is the one the segment rests on or sank through (a ray from high above
                // would start inside rock under an overhang or beside a cliff and read "sunk").
                // Nothing within reach below means the segment is clear of the ground: measure the
                // gap down from it instead, for the log.
                let r_seg = SEGS[k].radius;
                let start = tf.translation - Vec3::Y * MAX_SINK;
                let gap = if let Some(u) = spatial.cast_ray(start, Dir3::Y, MAX_SINK + r_seg, false, &filter) {
                    (tf.translation.y - r_seg) - (start.y + u.distance)
                } else if let Some(d) = spatial.cast_ray(tf.translation, Dir3::NEG_Y, 3.0, false, &filter) {
                    d.distance - r_seg
                } else {
                    continue;
                };
                lowest = lowest.min(gap);
                if gap < -SUNK {
                    commands
                        .entity(*seg)
                        .try_insert((Transform { translation: tf.translation + Vec3::Y * -gap, ..*tf }, LinearVelocity::ZERO, AngularVelocity::ZERO));
                }
            }
            if lowest < f32::MAX {
                info!("[ragdoll] down: lowest segment {:+.3} m from the ground{}", lowest, if lowest < -SUNK { " (lifted)" } else { "" });
            }
        }
        let revived = !fighter.dead && action.owner == Owner::Hit && action.kind == ActionKind::GetUp;
        if !(revived || (settled && !fighter.dead)) {
            continue;
        }
        // Stand the root on the ground under the pelvis, facing the feet (the get-up clip starts
        // on the back with the head behind).
        let head = read.get(HEAD).copied().flatten().map_or(pelvis.translation, |(t, _)| t.translation);
        let toward_feet = Vec3::new(pelvis.translation.x - head.x, 0.0, pelvis.translation.z - head.z);
        let yaw = if toward_feet.length_squared() > 1e-4 { toward_feet.x.atan2(toward_feet.z) } else { mover.yaw };
        let filter = SpatialQueryFilter::from_mask(Phys::World);
        let from = pelvis.translation + Vec3::Y * 0.6;
        let ground = spatial
            .cast_ray(from, Dir3::NEG_Y, 4.0, true, &filter)
            .map_or(pelvis.translation.y - 0.15, |hit| from.y - hit.distance);
        root.translation = Vec3::new(pelvis.translation.x, ground + HALF_HEIGHT, pelvis.translation.z);
        if player {
            info!("[ragdoll] player came to rest at ({:.1}, {:.1})", pelvis.translation.x, pelvis.translation.z);
        }
        root.rotation = Quat::from_rotation_y(yaw);
        mover.yaw = yaw;
        mover.velocity = Vec3::ZERO;
        release(&mut commands, entity, &r, &mut mover, &mut layers);
        if !revived {
            let duration = bank.duration(GET_UP) / GET_UP_RATE;
            action.start(ActionKind::GetUp, Owner::Ragdoll, Some(ActionLayer::new(GET_UP, 0.0, Mask::Full)));
            mover.move_scale = 0.0;
            commands.entity(entity).try_insert(GettingUp { t: 0.0, duration });
        }
    }
}

/// Recovered in the air (seconds since): the landing bounces toward the nearest foe.
#[derive(Component, Clone, Copy, Debug)]
pub struct AirRecovered(f32);

/// On touchdown after an air recovery: spring off the landing spot toward the lock-on target or
/// the nearest foe (else where the stick points, else ahead) and run on.
#[allow(clippy::type_complexity)]
fn bounce(
    time: Res<Time>,
    mut commands: Commands,
    mut deeds: MessageWriter<super::deeds::Deed>,
    mut bodies: Query<(Entity, &Transform, &mut Mover, &Fighter, Option<&Intent>, &mut AirRecovered)>,
    others: Query<(Entity, &Transform, &Fighter), Without<AirRecovered>>,
) {
    for (e, tf, mut mover, fighter, intent, mut rec) in &mut bodies {
        rec.0 += time.delta_secs();
        if rec.0 > 6.0 || fighter.dead {
            commands.entity(e).try_remove::<AirRecovered>();
            continue;
        }
        if mover.mode != MoveMode::Ground || rec.0 < 0.05 {
            continue;
        }
        let me = tf.translation;
        let locked = intent.and_then(|i| i.target).and_then(|t| others.get(t).ok()).map(|(_, t, _)| t.translation);
        let nearest = || {
            others
                .iter()
                .filter(|(o, t, f)| *o != e && !f.dead && f.team != fighter.team && t.translation.distance(me) < BOUNCE_LOOK)
                .min_by(|a, b| a.1.translation.distance(me).total_cmp(&b.1.translation.distance(me)))
                .map(|(_, t, _)| t.translation)
        };
        let toward = locked.or_else(nearest).map(|p| (p - me).with_y(0.0).normalize_or_zero());
        let stick = intent.map(|i| i.move_dir.with_y(0.0)).filter(|d| d.length_squared() > 0.04).map(|d| d.normalize());
        let dir = toward.filter(|d| d.length_squared() > 0.5).or(stick).unwrap_or_else(|| mover.forward());
        mover.external = dir * BOUNCE + Vec3::Y * BOUNCE_UP;
        mover.yaw = dir.x.atan2(dir.z);
        commands.entity(e).try_remove::<AirRecovered>();
        deeds.write(super::deeds::Deed::new(e, "land", "Recover", 1.0));
        info!("[ragdoll] landing bounce toward ({:.1}, {:.1})", dir.x, dir.z);
    }
}

/// Despawn the segments and joints and give the body back to the controller and `select`.
fn release(commands: &mut Commands, entity: Entity, r: &Ragdolled, mover: &mut Mover, layers: &mut MotionLayers) {
    for &e in r.joints.iter().chain(&r.segs) {
        commands.entity(e).try_despawn();
    }
    commands.entity(entity).try_remove::<Ragdolled>();
    mover.mode = MoveMode::Ground;
    mover.grounded = true;
    layers.raw = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank() -> PoseBank {
        PoseBank::build_with(std::path::Path::new("__no_poses__.json")).expect("bank").0
    }

    /// Segments sitting exactly where their bones are give back the pose they were built from.
    #[test]
    fn segments_at_the_bones_reproduce_the_pose() {
        let b = bank();
        let s = &b.skeleton;
        let pose = b.sample_raw("ual1/Punch_Jab", 0.3, false);
        let model_rot = Quat::from_rotation_y(0.7);
        let fk = s.fk(&pose, b.bones.pelvis);
        let bones: Vec<usize> = SEGS.iter().map(|g| s.find(g.bone).expect("bone")).collect();
        let seg_rot: Vec<Quat> = bones.iter().map(|&i| model_rot * fk[i].rotation).collect();
        let out = pose_from_segments(s, &pose, &bones, &seg_rot, model_rot);
        for &i in &bones {
            assert!(out.rot[i].angle_between(pose.rot[i]) < 1e-3, "{}", s.names[i]);
        }
    }

    /// Turning one segment turns its bone and carries the unsimulated bones below it along.
    #[test]
    fn a_turned_segment_turns_its_bone_only() {
        let b = bank();
        let s = &b.skeleton;
        let fk = s.fk(&b.rest, b.bones.pelvis);
        let bones: Vec<usize> = SEGS.iter().map(|g| s.find(g.bone).expect("bone")).collect();
        let mut seg_rot: Vec<Quat> = bones.iter().map(|&i| fk[i].rotation).collect();
        let k = 6; // lowerarm_r
        seg_rot[k] = Quat::from_rotation_x(0.5) * seg_rot[k];
        let out = pose_from_segments(s, &b.rest, &bones, &seg_rot, Quat::IDENTITY);
        let forearm = bones[k];
        assert!(out.rot[forearm].angle_between(b.rest.rot[forearm]) > 0.3);
        let hand = s.find("hand_r").unwrap();
        assert!(out.rot[hand].angle_between(b.rest.rot[hand]) < 1e-4, "hand keeps its local rotation");
        let upper = bones[4];
        assert!(out.rot[upper].angle_between(b.rest.rot[upper]) < 1e-3);
    }

    #[test]
    fn segment_table_is_well_formed() {
        for (k, seg) in SEGS.iter().enumerate() {
            assert!(seg.parent.is_none_or(|p| p < k), "{}: parent after child", seg.bone);
            assert!(seg.radius > 0.0 && seg.mass > 0.0 && seg.swing >= 0.0 && seg.twist >= 0.0);
        }
        assert_eq!(SEGS[HEAD].bone, "Head");
        assert_eq!(SEGS[1].bone, "spine_02");
    }

    /// A real ragdoll dropped from standing height (rest pose, thrown backwards) lands on a
    /// ground plane, stays in one piece, comes to rest and never sinks through the floor.
    #[test]
    fn ragdoll_falls_settles_and_holds_together() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let b = bank();
        let s = b.skeleton.clone();
        let fk = s.fk(&b.rest, b.bones.pelvis);
        let feet = fk.iter().map(|t| t.translation.y).fold(f32::MAX, f32::min);
        let lift = Vec3::Y * (0.05 - feet);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, bevy::asset::AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
            .insert_resource(Gravity(Vec3::NEG_Y * 9.81));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::half_space(Vec3::Y),
            CollisionLayers::new(Phys::World, LayerMask::ALL),
            Transform::default(),
        ));
        let request = RagdollRequest {
            velocity: Vec3::new(0.0, 1.5, -5.0),
            point: lift + Vec3::Y * 1.2,
        };
        let (segs, joints, _) = {
            let world = app.world_mut();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let out = spawn_segments(&mut commands, &s, |i| Some((fk[i].rotation, fk[i].translation + lift)), Vec3::ZERO, &request)
                .expect("segments");
            queue.apply(world);
            out
        };
        assert_eq!(segs.len(), SEGS.len());
        assert_eq!(joints.len(), SEGS.len() - 1);
        let mut still = 0.0;
        let mut lowest = f32::MAX;
        for frame in 0..(60 * 6) {
            app.update();
            let w = app.world_mut();
            let mut fastest = 0.0f32;
            for &e in &segs {
                let t = w.get::<Transform>(e).unwrap().translation;
                let v = w.get::<LinearVelocity>(e).unwrap().0;
                assert!(t.is_finite() && v.is_finite(), "frame {frame}: blew up");
                lowest = lowest.min(t.y);
                fastest = fastest.max(v.length());
            }
            still = if fastest < SETTLE_SPEED { still + 1.0 / 60.0 } else { 0.0 };
        }
        let w = app.world_mut();
        let pos = |k: usize| w.get::<Transform>(segs[k]).unwrap().translation;
        // At rest on the floor: nothing sank, nothing is still in the air.
        assert!(lowest > -0.1, "sank to {lowest}");
        assert!(still >= SETTLE_TIME, "never settled");
        assert!(pos(0).y < 0.4, "pelvis still up at {}", pos(0).y);
        // Thrown backwards: it moved that way and stayed in one piece (joints hold).
        assert!(pos(0).z < lift.z - 0.3, "pelvis z {}", pos(0).z);
        for (k, seg) in SEGS.iter().enumerate() {
            if let Some(p) = seg.parent {
                let d = pos(k).distance(pos(p));
                assert!(d < 0.7, "{} drifted {d} m from its parent", seg.bone);
            }
        }
    }

    /// A body falling onto a slope made of terrain-style triangles (a one-sided trimesh, like
    /// `world`'s ground) comes to rest on it: no segment ends up under the surface.
    #[test]
    fn ragdoll_on_a_trimesh_slope_stays_above_ground() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let b = bank();
        let s = b.skeleton.clone();
        let fk = s.fk(&b.rest, b.bones.pelvis);
        let feet = fk.iter().map(|t| t.translation.y).fold(f32::MAX, f32::min);
        // Ground rising along +x at 25 degrees, as a grid of triangles (wound like the terrain).
        let slope = 25f32.to_radians().tan();
        let ground = |x: f32| x * slope;
        let n = 40usize;
        let (lo, step) = (-40.0f32, 2.0f32);
        let mut verts = Vec::new();
        for iz in 0..=n {
            for ix in 0..=n {
                let (x, z) = (lo + ix as f32 * step, lo + iz as f32 * step);
                verts.push(Vec3::new(x, ground(x), z));
            }
        }
        let stride = (n + 1) as u32;
        let mut tris = Vec::new();
        for iz in 0..n as u32 {
            for ix in 0..n as u32 {
                let a = iz * stride + ix;
                let (bb, c) = (a + 1, a + stride);
                tris.extend([[a, c, bb], [bb, c, c + 1]]);
            }
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, bevy::asset::AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
            .insert_resource(Gravity(Vec3::NEG_Y * 9.81));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::trimesh(verts, tris),
            CollisionLayers::new(Phys::World, LayerMask::ALL),
            Transform::default(),
        ));
        // Standing on the slope at x = 0 and thrown downhill hard (a killing blow).
        let lift = Vec3::Y * (0.05 - feet);
        let request = RagdollRequest {
            velocity: Vec3::new(-7.0, 2.0, 0.0),
            point: lift + Vec3::Y * 1.2,
        };
        let segs = {
            let world = app.world_mut();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let out = spawn_segments(&mut commands, &s, |i| Some((fk[i].rotation, fk[i].translation + lift)), Vec3::ZERO, &request)
                .expect("segments");
            queue.apply(world);
            out.0
        };
        let mut worst = f32::MAX;
        for _ in 0..(60 * 6) {
            app.update();
        }
        let w = app.world_mut();
        for (k, &e) in segs.iter().enumerate() {
            let t = w.get::<Transform>(e).unwrap().translation;
            assert!(t.is_finite());
            // Height of the segment's centre above the slope, along the slope's normal.
            let normal = Vec3::new(-slope, 1.0, 0.0).normalize();
            let above = (t - Vec3::new(t.x, ground(t.x), t.z)).dot(normal) - SEGS[k].radius;
            worst = worst.min(above);
        }
        let pelvis = w.get::<Transform>(segs[0]).unwrap().translation;
        assert!(pelvis.x.abs() < 38.0 && pelvis.z.abs() < 38.0, "left the test slope at {pelvis}");
        assert!(worst > -0.05, "a segment sank {worst:.3} m into the slope (pelvis at {pelvis})");
    }

    /// The defeat pose (`Death01` a moment in, knees going, arms across the body) handed to
    /// physics on the ground falls and stays down: it never launches itself (limbs that start
    /// inside each other must not shove the body into the air).
    #[test]
    fn a_limp_death_pose_falls_instead_of_launching() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let b = bank();
        let s = b.skeleton.clone();
        let pose = b.sample_raw("ual1/Death01", DEATH_LIMP, false);
        let fk = s.fk(&pose, b.bones.pelvis);
        let feet = fk.iter().map(|t| t.translation.y).fold(f32::MAX, f32::min);
        let lift = Vec3::Y * (0.02 - feet);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, bevy::asset::AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
            .insert_resource(Gravity(Vec3::NEG_Y * 9.81));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::half_space(Vec3::Y),
            CollisionLayers::new(Phys::World, LayerMask::ALL),
            Transform::default(),
        ));
        let request = RagdollRequest { velocity: Vec3::new(0.0, 0.0, 1.2), point: lift + Vec3::Y * 0.3 };
        let segs = {
            let world = app.world_mut();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let out = spawn_segments(&mut commands, &s, |i| Some((fk[i].rotation, fk[i].translation + lift)), Vec3::ZERO, &request)
                .expect("segments");
            queue.apply(world);
            out.0
        };
        let start = fk[s.find("pelvis").unwrap()].translation.y + lift.y;
        let mut highest = f32::MIN;
        let mut fastest = 0.0f32;
        for f in 0..(60 * 3) {
            app.update();
            let w = app.world_mut();
            highest = highest.max(w.get::<Transform>(segs[0]).unwrap().translation.y);
            for (k, &e) in segs.iter().enumerate() {
                let v = w.get::<LinearVelocity>(e).unwrap().0.length();
                if v > fastest && v > 4.0 {
                    eprintln!("frame {f}: {} at {v:.1} m/s", SEGS[k].bone);
                }
                fastest = fastest.max(v);
            }
        }
        assert!(highest < start + 0.3, "pelvis rose from {start:.2} to {highest:.2} m (fastest segment {fastest:.1} m/s)");
        assert!(fastest < 6.0, "a segment reached {fastest:.1} m/s");
    }

    #[test]
    fn a_blow_through_the_centre_throws_and_one_off_centre_spins() {
        let body = [(Vec3::new(0.0, 1.0, 0.0), 30.0), (Vec3::new(0.0, 1.6, 0.0), 20.0), (Vec3::new(0.0, 0.4, 0.0), 16.0)];
        let total: f32 = body.iter().map(|b| b.1).sum();
        let com = body.iter().map(|b| b.0 * b.1).sum::<Vec3>() / total;
        let (_, dv, w) = fling(&body, com, Vec3::X * 6.0);
        assert!((dv - Vec3::X * 6.0).length() < 1e-4 && w.length() < 1e-3, "through the centre: no spin");
        let (_, _, w) = fling(&body, com + Vec3::Y * 0.6, Vec3::X * 6.0);
        assert!(w.z < -0.5, "a high blow tips the body over: {w}");
        let (_, dv, w) = fling(&body, com + Vec3::Y * 0.6, Vec3::X * 100.0);
        assert!(dv.length() <= MAX_FLING + 1e-3 && w.length() <= MAX_SPIN + 1e-3, "capped");
    }
}

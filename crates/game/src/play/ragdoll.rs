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
//! - A new request while down kicks the segments again.
use super::combat::CombatSet;
use super::{Action, ActionKind, ActionLayer, Fighter, HALF_HEIGHT, MoveMode, Mover, Owner, Phys, PlaySet, Player};
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

/// The segment nearest the blow leaves this many times faster than the rest of the push.
const KICK: f32 = 1.6;
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
const LINEAR_DAMPING: f32 = 0.1;
const ANGULAR_DAMPING: f32 = 1.2;
const LIMIT_COMPLIANCE: f32 = 1e-5;
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
    app.add_systems(
        Update,
        (debug_key, take_over, drive)
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
    mut velocities: Query<(&Transform, &mut LinearVelocity), Without<Rig>>,
) {
    let Some(bank) = bank else { return };
    for (entity, request, rig, mut mover, mut action, mut layers, ragdolled, getting_up) in &mut bodies {
        commands.entity(entity).try_remove::<RagdollRequest>();
        if !(request.velocity.is_finite() && request.point.is_finite()) {
            continue;
        }
        // Already down: kick the segments again.
        if let Some(r) = ragdolled {
            for &seg in &r.segs {
                if let Ok((tf, mut v)) = velocities.get_mut(seg) {
                    let near = tf.translation.distance(request.point) < 0.35;
                    v.0 += request.velocity * if near { KICK } else { 1.0 };
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
        let world = |i: usize| bone_world(rig, &globals, i);
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
                LinearVelocity(body_velocity + request.velocity * kick),
                LinearDamping(LINEAR_DAMPING),
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
        let mut joint = SphericalJoint::new(segs[p], segs[k])
            .with_anchor(pos)
            .with_basis(basis)
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
        ),
        Without<RagdollSegment>,
    >,
    models: Query<&Transform, (Without<Rig>, Without<RagdollSegment>)>,
    segments: Query<(&Transform, &LinearVelocity), With<RagdollSegment>>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, rig, mut root, mut mover, mut action, mut layers, fighter, ragdolled, getting_up) in &mut bodies {
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

        let settled = (r.t >= MIN_TIME && r.still >= SETTLE_TIME) || r.t >= MAX_TIME;
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
}

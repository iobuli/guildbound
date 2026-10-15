//! The physics body: spawn it (Avian bodies + joints + sim::api body), and the per-tick drive.
//! Per tick, FixedPostUpdate before PhysicsSystems::First (GUILDBOUND_BRIEF §API):
//!   sync_measured (Avian state) -> step -> +tau on body i, -tau on its parent, foot_force on each foot.

use avian3d::dynamics::integrator::IntegrationSystems;
use avian3d::dynamics::solver::schedule::SubstepSchedule;
use avian3d::dynamics::solver::solver_body::{SolverBody, SolverBodyInertia};
use avian3d::prelude::*;
use bevy::prelude::*;
use guildbound_sim::anim::gait::{GaitInput, GaitState, gait_joints};
use guildbound_sim::api::{BodyId, BodySpec, BodyState, Iso, JointRole, MeasuredJoint, Mode, Sim};
use guildbound_sim::api::{BodyMode, GaitDrive};
use guildbound_sim::math::{Quat as SQuat, Vec3 as SVec3};
use guildbound_sim::phys_gait::{GaitParams, gait_cmd};

use crate::rig::{self, Seg};

/// The sim side (pure Rust motor writer), owned by the game as a resource.
#[derive(Resource, Default)]
pub struct SimRes {
    pub sim: Sim,
}

/// One spawned physical body: Avian entities per joint, anchors, feet, and the sim handle.
pub struct Physical {
    pub id: BodyId,
    pub links: Vec<Entity>,
    pub parent: Vec<Option<usize>>,
    /// Joint anchor in the body's local frame (anchor - centre at rest).
    pub anchor_local: Vec<Vec3>,
    pub feet: [Option<usize>; 2],
    pub names: Vec<&'static str>,
    pub masses: Vec<f32>,
    /// Avian joint entities (for despawn on model swap).
    pub joints: Vec<Entity>,
    /// Rig profile name (model picker).
    pub model: String,
    pub source: Option<crate::source::SourceMotion>,
}

/// Every physical body in the scene, plus the last state read from sim (for the overlay and logs).
#[derive(Resource, Default)]
pub struct Bodies {
    pub list: Vec<Physical>,
    pub last: Vec<BodyState>,
    pub ticks: u64,
    /// One anim::proc state per body (same order as `list`).
    pub procs: Vec<guildbound_sim::anim::proc::ProcState>,
    /// One strike boost per body (same order as `list`), fed to anim::ability::apply_fade every tick.
    pub boosts: Vec<guildbound_sim::anim::ability::StrikeBoost>,
    /// One gait controller per body (Grounded walking/standing), wanted velocity (world, m/s) and heading.
    pub gaits: Vec<guildbound_sim::anim::gait::GaitState>,
    pub v_want: Vec<Vec3>,
    pub heading_want: Vec<Option<f64>>,
    /// Standing COM height per body (locked after REST_LOCK ticks), and ticks since spawn.
    pub rest_h: Vec<Option<f64>>,
    pub age: Vec<u64>,
    /// Get-up (Recovering sub-state): ticks spent lying still, and the running stand-up timeline.
    pub settle: Vec<guildbound_sim::anim::proc::SettleState>,
    pub limp: Vec<bool>,
    /// phys_gait weight-shift: foot waiting to lift (GaitCmd.pending), carried tick to tick.
    pub pending: Vec<Option<usize>>,
    pub getup: Vec<Option<guildbound_sim::anim::proc::GetupState>>,
}

/// Settled this long (ticks, 0.5 s; anim body_settled) before the get-up starts.
const SETTLE_TICKS: u32 = 60;

/// Ticks after spawn before the standing COM height is taken (matches sim's rest lock).
const REST_LOCK: u64 = 60;

/// Marks an Avian rigid body as joint `index` of a Physical body.
#[derive(Component)]
pub struct Link {
    #[allow(dead_code)]
    pub index: usize,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SimRes>()
        .init_resource::<Bodies>()
        .add_systems(Startup, spawn_default)
        .add_systems(FixedPostUpdate, drive.before(PhysicsSystems::First))
        .add_systems(
            SubstepSchedule,
            substep_motor.before(IntegrationSystems::Velocity),
        );
}

fn spawn_default(
    mut commands: Commands,
    mut sim: ResMut<SimRes>,
    mut bodies: ResMut<Bodies>,
    bench: Option<Res<crate::comparison::Settings>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    if bench.is_some() {
        return;
    }
    log_tuning(&sim.sim);
    // [0] = player fighter, [1] = enemy dummy (same rig, both physical, both motor-held).
    let model = rig::available_models()
        .into_iter()
        .next()
        .unwrap_or_default();
    rig::log_rig(&model, &rig::segments_for(&model));
    let mut p = spawn_body(
        &mut commands,
        &mut sim.sim,
        &rig::segments_for(&model),
        Vec3::ZERO,
        Some(crate::voxel::Style::for_model(&model)),
        &mut meshes,
        &mut mats,
    );
    p.source = crate::source::SourceMotion::load(&model).ok();
    p.model = model;
    bodies.list.push(p);
    let d = spawn_body(
        &mut commands,
        &mut sim.sim,
        &rig::segments_for("dummy"),
        Vec3::new(-1.4, 0.0, 0.6),
        Some(crate::voxel::Style::Mannequin),
        &mut meshes,
        &mut mats,
    );
    let mut d = d;
    d.model = "dummy".into();
    d.source = crate::source::SourceMotion::load("dummy").ok();
    bodies.list.push(d);
}

/// Tuning is fixed (sim::api Tuning defaults, Physics' values); printed once at boot.
pub fn log_tuning(sim: &Sim) {
    let t = &sim.tuning;
    println!(
        "TUNING kp {} kd {} kb {} kbd {} substeps {} foot_damp {} mu {} k_com {} ramp {} | mass {} kg",
        t.kp,
        t.kd,
        t.kb,
        t.kbd,
        t.substeps,
        t.foot_damp,
        t.mu,
        t.k_com,
        t.gain_ramp_ticks,
        rig::BODY_MASS
    );
}

fn sq(q: Quat) -> SQuat {
    SQuat {
        w: q.w as f64,
        x: q.x as f64,
        y: q.y as f64,
        z: q.z as f64,
    }
}
fn sv(v: Vec3) -> SVec3 {
    SVec3::new(v.x as f64, v.y as f64, v.z as f64)
}
pub fn bv(v: SVec3) -> Vec3 {
    Vec3::new(v.x as f32, v.y as f32, v.z as f32)
}

/// Spawn a standing physical body at `at` (floor offset): Avian bodies + spherical joints + voxel skin,
/// and register it with sim::api as Physical. Returns the handle.
pub fn spawn_body(
    commands: &mut Commands,
    sim: &mut Sim,
    segs: &[Seg],
    at: Vec3,
    style: Option<crate::voxel::Style>,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) -> Physical {
    let mut links = Vec::with_capacity(segs.len());
    for (i, s) in segs.iter().enumerate() {
        let e = commands
            .spawn((
                Name::new(s.name),
                Link { index: i },
                RigidBody::Dynamic,
                s.collider(),
                Mass(s.mass),
                AngularInertia::new_with_local_frame(s.inertia(), s.inertia_frame()),
                MaxAngularSpeed(sim.tuning.w_cap as f32),
                Friction::new(0.8),
                rig::body_layers(),
                Transform::from_translation(at + s.centre()),
                Visibility::default(),
            ))
            .id();
        if matches!(s.role, JointRole::Knee(_) | JointRole::Ankle(_)) {
            commands.entity(e).insert(SweptCcd::NON_LINEAR);
        }
        if let Some(style) = style {
            crate::voxel::attach_skin(commands, e, s, style, meshes, mats);
        }
        links.push(e);
    }
    let mut joints = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let Some(p) = s.parent else { continue };
        let a1 = s.anchor - segs[p].centre();
        let a2 = s.anchor - s.centre();
        let swing = rig::swing_limit(s.role, s.name);
        joints.push(
            commands
                .spawn((
                    SphericalJoint::new(links[p], links[i])
                        .with_local_anchor1(a1)
                        .with_local_anchor2(a2)
                        .with_swing_limits(0.0, swing)
                        .with_twist_limits(-0.4, 0.4),
                    JointCollisionDisabled,
                ))
                .id(),
        );
    }
    let id = sim.spawn_body(BodySpec {
        rig: rig::profile(segs),
        root: Iso {
            pos: sv(at + segs[0].anchor),
            rot: SQuat::IDENTITY,
        },
        mode: Mode::Physical,
    });
    sim.possess(id);
    let mut feet = [None, None];
    for (i, s) in segs.iter().enumerate() {
        if let JointRole::Ankle(side) = s.role {
            feet[side as usize] = Some(i);
        }
    }
    Physical {
        id,
        links,
        parent: segs.iter().map(|s| s.parent).collect(),
        anchor_local: segs.iter().map(|s| s.anchor - s.centre()).collect(),
        feet,
        names: segs.iter().map(|s| s.name).collect(),
        masses: segs.iter().map(|s| s.mass).collect(),
        joints,
        model: String::new(),
        source: None,
    }
}

/// Despawn a body's Avian entities (skins are children) and remove it from sim.
pub fn despawn_body(commands: &mut Commands, sim: &mut Sim, p: &Physical) {
    for &j in &p.joints {
        commands.entity(j).despawn();
    }
    // Despawning a link also despawns its voxel skin child (and drops its mesh/material handles).
    for &e in &p.links {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
    // Remove it from sim completely (no orphaned sim bodies after a swap).
    sim.despawn(p.id);
}

/// Avian's measured joint states for one body, in JointId order (world frame; pos = joint anchor).
pub fn measure(
    p: &Physical,
    q: &Query<(&Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
) -> Option<Vec<MeasuredJoint>> {
    p.links
        .iter()
        .enumerate()
        .map(|(i, &e)| {
            let (pos, rot, v, w) = q.get(e).ok()?;
            Some(MeasuredJoint {
                q: sq(rot.0),
                w: sv(w.0),
                v: sv(v.0),
                pos: sv(pos.0 + rot.0 * p.anchor_local[i]),
            })
        })
        .collect()
}

/// Apply one tick of sim output to Avian: foot_force, grab ext_forces and the launch impulse.
/// (Joint torques: substep_motor.)
pub fn apply(p: &Physical, st: &BodyState, forces: &mut Query<Forces>) {
    // Joint motor torques are applied per Avian substep (substep_motor), not here.
    // External forces (grab link): world force at that joint's anchor (+F enemy ankle, -F player hand).
    for (j, fv) in &st.ext_forces {
        let (i, fv) = (j.0 as usize, bv(*fv));
        let (Some(&e), Some(&_a)) = (p.links.get(i), p.anchor_local.get(i)) else {
            continue;
        };
        let at = st.joints.get(i).map(|x| bv(x.pos));
        if let (Ok(mut f), Some(at)) = (forces.get_mut(e), at) {
            if fv.is_finite() {
                f.apply_force_at_point(fv, at);
            }
        }
    }
    // Launch (one tick): velocity v on every segment, as impulse m_i * v.
    if let Some(v) = st.launch_impulse.map(bv) {
        for (i, &e) in p.links.iter().enumerate() {
            if let Ok(mut f) = forces.get_mut(e) {
                f.apply_linear_impulse(v * p.masses[i]);
            }
        }
    }
    for s in 0..2 {
        let (Some(i), ff) = (p.feet[s], bv(st.foot_force[s])) else {
            continue;
        };
        if ff.is_finite() && ff != Vec3::ZERO {
            if let Ok(mut f) = forces.get_mut(p.links[i]) {
                f.apply_force(ff);
            }
        }
    }
}

/// The per-tick drive: sync_measured for every body, one sim step, then torques/forces into Avian.
fn drive(
    mut sim: ResMut<SimRes>,
    mut bodies: ResMut<Bodies>,
    act: Res<crate::input::Actions>,
    bench: Option<Res<crate::comparison::Settings>>,
    mut set: ParamSet<(
        Query<(&Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
        Query<Forces>,
    )>,
) {
    if bench.is_some() {
        return;
    }
    let bodies = &mut *bodies;
    let launched = act.launched_for.is_some();
    for p in &bodies.list {
        if let Some(m) = measure(p, &set.p0()) {
            sim.sim.sync_measured(p.id, &m);
        }
    }
    let mut forces = set.p1();
    sim.sim.step();
    bodies.last.clear();
    let n = bodies.list.len();
    bodies.procs.resize_with(n, Default::default);
    bodies.boosts.resize_with(n, Default::default);
    bodies.gaits.resize_with(n, Default::default);
    bodies.v_want.resize(n, Vec3::ZERO);
    bodies.heading_want.resize(n, None);
    bodies.rest_h.resize(n, None);
    bodies.age.resize(n, 0);
    bodies.settle.resize_with(n, Default::default);
    bodies.limp.resize(n, false);
    bodies.pending.resize(n, None);
    bodies.getup.resize(n, None);
    let tick = bodies.ticks;
    for k in 0..n {
        let p = &bodies.list[k];
        let st = sim.sim.read_state(p.id);
        apply(p, &st, &mut forces);
        bodies.age[k] += 1;
        if bodies.rest_h[k].is_none() && bodies.age[k] >= REST_LOCK && st.mode == BodyMode::Grounded
        {
            bodies.rest_h[k] = Some(st.com.y);
        }
        // Targets for the next tick, from this tick's state: gait while Grounded (and not in a jump),
        // anim::proc for jump / air / recovery / ragdoll / grab.
        // Get-up (Recovering sub-state): after a fall the body goes limp (anim limp_joints: weight 0, no
        // targets); once anim's body_settled says it has come to rest for 0.5 s, call recover ONLY (sim ramps
        // the motor weight 0 -> 1 over 36 ticks) and run getup_joints from the measured pose until getup_done.
        use guildbound_sim::anim::proc::{
            GetupState, body_settled, getup_done, getup_joints, limp_joints, lying_from_up,
            measured_from_joints,
        };
        let lying = lying_from_up(st.joints[0].q_world);
        let free = !matches!(st.mode, BodyMode::Grabbing | BodyMode::Grabbed);
        if bodies.getup[k].is_none() && free && !(k == 0 && launched) {
            if !bodies.limp[k] && lying.is_some() {
                limp_joints(&mut sim.sim, p.id);
                bodies.limp[k] = true;
                bodies.settle[k] = Default::default();
            }
            if bodies.limp[k] && body_settled(&mut bodies.settle[k], &st, SETTLE_TICKS) {
                sim.sim.recover(p.id);
                bodies.limp[k] = false;
                bodies.procs[k] = Default::default();
                if let Some(kind) = lying {
                    let q: Vec<_> = st.joints.iter().map(|j| j.q_local).collect();
                    bodies.getup[k] = Some(GetupState::new(tick, kind, &measured_from_joints(&q)));
                }
            }
        }
        if let Some(g) = bodies.getup[k] {
            if getup_done(&g, tick) || !free {
                bodies.getup[k] = None;
            }
        }
        let t = if bodies.limp[k] {
            sim.sim.set_gait(p.id, None);
            Vec::new()
        } else if let Some(g) = bodies.getup[k] {
            sim.sim.set_gait(p.id, None);
            getup_joints(&g, tick)
        } else if st.mode == BodyMode::Grounded && bodies.procs[k].jump.is_none() {
            if let Some(source) = &p.source {
                let heading = *bodies.heading_want[k]
                    .get_or_insert_with(|| yaw_of(st.joints[0].q_world.normalize()));
                sim.sim.set_gait(
                    p.id,
                    Some(GaitDrive {
                        heading,
                        swing: None,
                        swing_q: None,
                    }),
                );
                source.targets(
                    bodies.age[k].saturating_sub(1),
                    bodies.v_want[k].length(),
                    &p.parent,
                )
            } else {
                let t = gait_targets(
                    &mut bodies.gaits[k],
                    p,
                    &st,
                    bodies.v_want[k],
                    bodies.heading_want[k],
                    bodies.rest_h[k],
                    &mut bodies.pending[k],
                    tick,
                );
                // Latch the heading on the first gait tick so a standing body holds it (no slow yaw spin).
                let heading = *bodies.heading_want[k]
                    .get_or_insert_with(|| yaw_of(st.joints[0].q_world.normalize()));
                let q_pw = SQuat::from_rotvec(SVec3::new(0.0, heading, 0.0));
                let sw = guildbound_sim::anim::gait::swing_thigh_world(&bodies.gaits[k], q_pw);
                let drive = GaitDrive {
                    heading,
                    swing: bodies.gaits[k].swing,
                    swing_q: sw
                        .filter(|x| Some(x.0) == bodies.gaits[k].swing)
                        .map(|x| x.1),
                };
                sim.sim.set_gait(p.id, Some(drive));
                t
            }
        } else {
            sim.sim.set_gait(p.id, None);
            bodies.gaits[k] = Default::default();
            bodies.pending[k] = None;
            guildbound_sim::anim::proc::proc_from_state_joints(
                &mut bodies.procs[k],
                &st,
                rig::BODY_MASS as f64,
                tick,
            )
        };
        sim.sim.set_target(p.id, &t);
        guildbound_sim::anim::ability::apply_fade(
            &mut sim.sim,
            p.id,
            &bodies.boosts[k],
            None,
            tick,
        );
        bodies.last.push(st);
    }
    bodies.ticks += 1;
}

fn yaw_of(q: SQuat) -> f64 {
    let f = q.rotate(SVec3::new(0.0, 0.0, 1.0));
    f.x.atan2(f.z)
}
fn flat(v: SVec3) -> SVec3 {
    SVec3::new(v.x, 0.0, v.z)
}

/// One Grounded tick of the walking controller: phys_gait (lean + foot placement) -> anim::gait (pose) -> targets.
fn gait_targets(
    gs: &mut GaitState,
    p: &Physical,
    st: &BodyState,
    v_want: Vec3,
    heading_want: Option<f64>,
    rest_h: Option<f64>,
    pending: &mut Option<usize>,
    tick: u64,
) -> Vec<guildbound_sim::api::Target> {
    let pel = st.joints[0];
    let q_pelvis = pel.q_world.normalize();
    let feet_world = [0, 1].map(|s| {
        p.feet[s]
            .and_then(|i| st.joints.get(i))
            .map(|j| j.pos)
            .unwrap_or(pel.pos)
    });
    let feet_down = st.feet_down;
    let heading = yaw_of(q_pelvis);
    // +side = the left foot's side (pelvis +x; anim places B_L_* at +HIP_W), as phys_gait expects for foot 0.
    let side = flat(q_pelvis.rotate(SVec3::new(1.0, 0.0, 0.0)));
    let heading_right = if side.length() > 1e-6 {
        side * (1.0 / side.length())
    } else {
        SVec3::new(1.0, 0.0, 0.0)
    };
    let v_want = sv(v_want);
    // Gait start from a standstill: phys_gait only sends a step on a touchdown (or once the capture point leaves
    // the support), so with both feet planted and a move wanted, treat the foot further along v_want as the one
    // that just landed; the other (rear) foot then lifts. Physics' own step rule does the rest.
    let start = (pending.is_none()
        && gs.swing.is_none()
        && feet_down == [true, true]
        && v_want.length() > 0.05)
        .then(|| {
            let d = flat(v_want);
            if (feet_world[0] - feet_world[1]).dot(d) >= 0.0 {
                0
            } else {
                1
            }
        });
    let just_landed = gs.just_landed(feet_down).or(start);
    let cmd = gait_cmd(
        st.com,
        st.com_v,
        v_want,
        rest_h.unwrap_or(st.com.y),
        feet_world,
        feet_down,
        gs.swing.is_some(),
        just_landed,
        st.foot_share,
        *pending,
        heading_right,
        0.0,
        GaitParams::default(),
    );
    *pending = cmd.pending;
    let inp = GaitInput {
        v_want,
        v: flat(st.com_v),
        heading_want: heading_want.unwrap_or(heading),
        heading,
        lean: cmd.lean,
        lean_dir: cmd.lean_dir,
        step_event: cmd.step.map(|e| (e.foot, e.target)),
        pelvis_pos: pel.pos,
        q_pelvis,
        feet_world,
        feet_down,
    };
    gait_joints(gs, &inp, tick)
}

/// Per Avian substep (h = 1/(120·substeps)): rebuild each body's measured state from the live solver
/// bodies, ask sim::api for this substep's joint torques (Stable PD with dt = h, the tick's held residual on
/// top) and apply them as angular velocity changes: +τ on joint i's body, −τ on its parent, Δω = I⁻¹·τ·h.
fn substep_motor(
    sim: Res<SimRes>,
    bodies: Res<Bodies>,
    time: Res<Time>,
    mut q: Query<(&Position, &Rotation, &mut SolverBody, &SolverBodyInertia)>,
) {
    let h = time.delta_secs_f64();
    if h <= 0.0 {
        return;
    }
    for p in &bodies.list {
        let m: Option<Vec<MeasuredJoint>> = p
            .links
            .iter()
            .enumerate()
            .map(|(i, &e)| {
                let (pos, rot, sb, _) = q.get(e).ok()?;
                let r = (sb.delta_rotation.0 * rot.0).normalize();
                let c = pos.0 + sb.delta_position;
                Some(MeasuredJoint {
                    q: sq(r),
                    w: sv(sb.angular_velocity),
                    v: sv(sb.linear_velocity),
                    pos: sv(c + r * p.anchor_local[i]),
                })
            })
            .collect();
        let Some(m) = m else { continue };
        let tau = sim.sim.motor_substep(p.id, &m, h);
        for (i, t) in tau.iter().enumerate() {
            let t = bv(*t);
            if t == Vec3::ZERO || !t.is_finite() {
                continue;
            }
            let mut kick = |e: Entity, t: Vec3| {
                if let Ok((_, _, mut sb, inr)) = q.get_mut(e) {
                    let dw = inr.effective_inv_angular_inertia() * (t * h as f32);
                    sb.angular_velocity += dw;
                }
            };
            kick(p.links[i], t);
            if let Some(par) = p.parent[i] {
                kick(p.links[par], -t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn source_ragdoll_runs_through_the_actual_physics_bridge() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            TransformPlugin,
            PhysicsPlugins::default(),
        ))
        .insert_resource(Time::<Fixed>::from_hz(crate::TICK_HZ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / crate::TICK_HZ,
        )))
        .insert_resource(SubstepCount(12))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<crate::input::Actions>()
        .add_plugins(plugin);
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 1.0, 20.0),
            Friction::new(0.8),
            rig::floor_layers(),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ));
        app.finish();
        app.cleanup();
        for _ in 0..4800 {
            app.update();
        }
        let bodies = app.world().resource::<Bodies>();
        assert!(bodies.ticks >= 4790);
        for (i, state) in bodies.last.iter().enumerate() {
            println!(
                "SOURCE_BRIDGE body={i} ticks={} com={:?} mode={:?} failed={}",
                bodies.ticks, state.com, state.mode, state.stand_failed
            );
            assert!(state.com.is_finite() && state.com_v.is_finite());
            assert_eq!(state.mode, BodyMode::Grounded);
            assert!(
                state.com.y > 0.6 && !state.stand_failed,
                "source idle failed to stand"
            );
            for j in &state.joints {
                assert!(j.pos.is_finite() && j.w.is_finite());
                assert!(j.pos.y > -0.1, "joint tunnelled through floor");
            }
        }
    }
}

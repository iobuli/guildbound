//! Three-lane animation, passive ragdoll and driven-ragdoll experiment.
use avian3d::prelude::*;
use bevy::{prelude::*, ui::RelativeCursorPosition};
use guildbound_sim::{
    anim::target::target_from,
    api::{BodyState, GaitDrive},
    math::Quat as SQuat,
};

use crate::{
    body::{self, Bodies, SimRes},
    rig::Seg,
    terrain_motion::{self as terrain, Alignment, FootLock, Frame},
};

const LANES: [f32; 3] = [-3.2, 0.0, 3.2];

#[derive(Resource)]
pub struct Settings {
    pub physics: f32,
    pub muscles: f32,
    pub playback: f32,
    pub speed: f32,
    pub assist: f32,
    pub paused: bool,
    reset: bool,
    push: bool,
    dragging: Option<usize>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            physics: 1.0,
            muscles: 1.0,
            playback: 1.0,
            speed: 1.5,
            assist: 1.0,
            paused: false,
            reset: false,
            push: false,
            dragging: None,
        }
    }
}

#[derive(Resource)]
struct Bench {
    alignment: Alignment,
    segments: Vec<Seg>,
    seconds: f32,
    reference: Vec<Entity>,
    mixed: Vec<Entity>,
    locks: [[FootLock; 2]; 2],
    frame: Option<Frame>,
    trails: [std::collections::VecDeque<Vec3>; 4],
}

#[derive(Component)]
struct Preview;
#[derive(Component)]
struct Slider(usize);
#[derive(Component)]
struct Fill(usize);
#[derive(Component)]
struct Value(usize);
#[derive(Component)]
struct Control(usize);
#[derive(Component)]
struct Stats;

pub fn plugin(app: &mut App) {
    app.init_resource::<Settings>()
        .init_resource::<crate::input::Actions>()
        .add_systems(Startup, spawn)
        .add_systems(FixedPostUpdate, tick.before(PhysicsSystems::First))
        .add_systems(Update, (controls, paint, ovals).chain());
}

fn spawn(
    mut commands: Commands,
    settings: Res<Settings>,
    mut sim: ResMut<SimRes>,
    mut bodies: ResMut<Bodies>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let segments = terrain::neutral_segments();
    let alignment = Alignment::new(&segments);
    println!(
        "[terrain] neutral capsule rigs; baked numeric tracks; terrain probes and contact IK active; no source character loaded"
    );
    sim.sim.tuning.k_com = 0.0;
    let reference = preview(&mut commands, &segments, LANES[0], &mut meshes, &mut mats);
    let mixed = preview(&mut commands, &segments, LANES[2], &mut meshes, &mut mats);
    spawn_physics(
        &mut commands,
        &mut sim,
        &mut bodies,
        &segments,
        &alignment,
        settings.speed,
        &mut meshes,
        &mut mats,
    );
    commands.insert_resource(Bench {
        alignment,
        segments,
        seconds: 0.0,
        reference,
        mixed,
        locks: [[FootLock::default(); 2]; 2],
        frame: None,
        trails: std::array::from_fn(|_| default()),
    });

    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            top: px(15.0),
            left: px(20.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(5.0),
            ..default()
        },))
        .with_children(|ui| {
            ui.spawn(label("TERRAIN / FOOT TRACKS", 23.0));
            ui.spawn(label(
                "Neutral test rigs. Baked tracks, no source character loaded.",
                14.0,
            ));
            ui.spawn((Stats, label("", 13.0)));
        });
    commands.spawn(Node { position_type: PositionType::Absolute, right: px(18.0), top: px(15.0), width: px(290.0),
        padding: UiRect::all(px(14.0)), flex_direction: FlexDirection::Column, row_gap: px(8.0), ..default() })
        .insert(BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.92)))
        .with_children(|ui| {
            for (i, title) in ["Physics blend", "Motor strength", "Playback speed", "Tracking assist"].iter().enumerate() {
                ui.spawn((Value(i), label(title, 16.0)));
                ui.spawn((Button, Slider(i), RelativeCursorPosition::default(),
                    Node { width: percent(100.0), height: px(22.0), ..default() },
                    BackgroundColor(Color::srgb(0.15, 0.19, 0.23))))
                    .with_child((Fill(i), Node { width: percent(50.0), height: percent(100.0), ..default() },
                        BackgroundColor(Color::srgb(0.15, 0.75, 0.65))));
            }
            ui.spawn(label("Green: planted targets. Amber: swing.\nCyan: actual feet. Lines trace the floor.\nTracking assist applies physical spring forces.", 12.0));
            for (i, title) in ["Idle", "Walk", "Run", "Pause / resume clip", "Push both ragdolls", "Reset all (R)"].iter().enumerate() {
                ui.spawn((Button, Control(i), Node { width: percent(100.0), padding: UiRect::all(px(7.0)), ..default() },
                    BackgroundColor(Color::srgb(0.13, 0.20, 0.26))))
                    .with_child(label(title, 15.0));
            }
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(24.0),
            left: percent(3.0),
            width: percent(94.0),
            justify_content: JustifyContent::SpaceAround,
            ..default()
        })
        .with_children(|ui| {
            for (title, note) in [
                ("ALIGNED TRACKS", "Terrain IK / no rigid bodies"),
                ("RAGDOLL", "Gravity + terrain / no motors"),
                ("PHYSICAL TRACKING", "Motors + optional spring assist"),
            ] {
                ui.spawn(Node {
                    width: percent(31.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|ui| {
                    ui.spawn(label(title, 20.0));
                    ui.spawn(label(note, 13.0));
                });
            }
        });
}

fn label(text: &str, size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: bevy::text::FontSize::Px(size),
            ..default()
        },
        TextColor(Color::WHITE),
    )
}

fn preview(
    commands: &mut Commands,
    segments: &[Seg],
    x: f32,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) -> Vec<Entity> {
    segments
        .iter()
        .map(|s| {
            let e = commands
                .spawn((
                    Preview,
                    Transform::from_translation(s.centre() + Vec3::X * x),
                    Visibility::default(),
                ))
                .id();
            terrain::attach(commands, e, s, Color::srgb(0.25, 0.65, 0.8), meshes, mats);
            e
        })
        .collect()
}

fn spawn_physics(
    commands: &mut Commands,
    sim: &mut SimRes,
    bodies: &mut Bodies,
    segments: &[Seg],
    alignment: &Alignment,
    speed: f32,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    for (i, x) in [LANES[1], LANES[2]].iter().enumerate() {
        let (root, _) = Alignment::route(0.0, 0.0, *x);
        let at = Vec3::new(root.x, terrain::height(root.x, root.z), root.z);
        let mut p = body::spawn_body(commands, &mut sim.sim, segments, at, None, meshes, mats);
        let mut locks = [FootLock::default(); 2];
        let frame = alignment.frame(0.0, 0.0, speed, *x, &mut locks, 1.0 / 120.0);
        let initial = pose(segments, &frame.local, frame.root, frame.rotation);
        for (j, &e) in p.links.iter().enumerate() {
            commands.entity(e).insert(initial[j]);
        }
        for (j, &e) in p.links.iter().enumerate() {
            terrain::attach(
                commands,
                e,
                &segments[j],
                Color::srgb(0.75, 0.55, 0.28),
                meshes,
                mats,
            );
        }
        p.model = if i == 0 {
            "passive ragdoll"
        } else {
            "combined"
        }
        .into();
        if i == 0 {
            sim.sim.launch(p.id, guildbound_sim::math::Vec3::ZERO);
        } else {
            for &e in &p.links {
                commands.entity(e).insert(Visibility::Hidden);
            }
        }
        bodies.list.push(p);
    }
}

fn bq(q: SQuat) -> Quat {
    Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32).normalize()
}

fn pose(segments: &[Seg], local: &[Quat], root: Vec3, root_rot: Quat) -> Vec<Transform> {
    let mut anchors = vec![Vec3::ZERO; segments.len()];
    let mut rotations = vec![Quat::IDENTITY; segments.len()];
    segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if let Some(p) = s.parent {
                anchors[i] = anchors[p] + rotations[p] * (s.anchor - segments[p].anchor);
                rotations[i] = (rotations[p] * local[i]).normalize();
            } else {
                anchors[i] = root;
                rotations[i] = root_rot;
            }
            Transform::from_translation(anchors[i] + rotations[i] * (s.centre() - s.anchor))
                .with_rotation(rotations[i])
        })
        .collect()
}

fn mixed_pose(
    segments: &[Seg],
    reference: &[Quat],
    root_reference: Vec3,
    rotation_reference: Quat,
    measured: &BodyState,
    weight: f32,
) -> Vec<Transform> {
    let w = weight.clamp(0.0, 1.0);
    let local: Vec<_> = reference
        .iter()
        .enumerate()
        .map(|(i, &q)| q.slerp(bq(measured.joints[i].q_local), w))
        .collect();
    let root = root_reference.lerp(body::bv(measured.joints[0].pos), w);
    pose(
        segments,
        &local,
        root,
        rotation_reference.slerp(bq(measured.joints[0].q_world), w),
    )
}

fn tick(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    mut settings: ResMut<Settings>,
    mut bodies: ResMut<Bodies>,
    mut sim: ResMut<SimRes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut set: ParamSet<(
        Query<(&Position, &Rotation, &LinearVelocity, &AngularVelocity)>,
        Query<Forces>,
        Query<&mut Transform, With<Preview>>,
    )>,
) {
    if std::mem::take(&mut settings.reset) {
        for p in std::mem::take(&mut bodies.list) {
            body::despawn_body(&mut commands, &mut sim.sim, &p);
        }
        bodies.last.clear();
        bodies.ticks = 0;
        bench.seconds = 0.0;
        settings.push = false;
        bench.locks = [[FootLock::default(); 2]; 2];
        bench.frame = None;
        for trail in &mut bench.trails {
            trail.clear();
        }
        spawn_physics(
            &mut commands,
            &mut sim,
            &mut bodies,
            &bench.segments,
            &bench.alignment,
            settings.speed,
            &mut meshes,
            &mut mats,
        );
        return;
    }
    if bodies.list.len() != 2 {
        return;
    }
    let rate = if settings.paused {
        0.0
    } else {
        settings.playback
    };
    let mut left_locks = bench.locks[0];
    let mut right_locks = bench.locks[1];
    let frame = bench.alignment.frame(
        bench.seconds,
        bench.seconds,
        settings.speed,
        LANES[2],
        &mut right_locks,
        rate / 120.0,
    );
    let left = bench.alignment.frame(
        bench.seconds,
        bench.seconds,
        settings.speed,
        LANES[0],
        &mut left_locks,
        rate / 120.0,
    );
    let mut future_locks = right_locks;
    let future = bench.alignment.frame(
        bench.seconds + rate / 120.0,
        bench.seconds + rate / 120.0,
        settings.speed,
        LANES[2],
        &mut future_locks,
        rate / 120.0,
    );
    bench.locks = [left_locks, right_locks];
    let sq = |q: Quat| SQuat {
        x: q.x as f64,
        y: q.y as f64,
        z: q.z as f64,
        w: q.w as f64,
    };
    let targets: Vec<_> = frame
        .local
        .iter()
        .enumerate()
        .map(|(i, &q)| target_from(None, sq(q), Some(sq(future.local[i]))).to_api(i as u16))
        .collect();
    sim.sim.set_motor_weight(bodies.list[0].id, None, 0.0);
    sim.sim
        .set_motor_weight(bodies.list[1].id, None, settings.muscles as f64);
    let heading = (frame.rotation * Vec3::Z)
        .x
        .atan2((frame.rotation * Vec3::Z).z) as f64;
    sim.sim.set_gait(
        bodies.list[1].id,
        Some(GaitDrive {
            heading,
            swing: None,
            swing_q: None,
        }),
    );
    sim.sim.set_target(bodies.list[1].id, &targets);
    for p in &bodies.list {
        if let Some(m) = body::measure(p, &set.p0()) {
            let heights = std::array::from_fn(|s| {
                p.feet[s]
                    .and_then(|i| terrain::trace(body::bv(m[i].pos) + Vec3::Y, 4.0))
                    .map(|h| h.y as f64)
            });
            sim.sim.set_support_heights(p.id, Some(heights));
            sim.sim.sync_measured(p.id, &m);
        }
    }
    sim.sim.step();
    bodies.last.clear();
    let push = std::mem::take(&mut settings.push);
    let wanted = pose(&bench.segments, &frame.local, frame.root, frame.rotation);
    let next = pose(&bench.segments, &future.local, future.root, future.rotation);
    for (i, p) in bodies.list.iter().enumerate() {
        let mut state = sim.sim.read_state(p.id);
        for f in &mut state.foot_force {
            *f = *f * if i == 0 { 0.0 } else { settings.muscles as f64 };
        }
        body::apply(p, &state, &mut set.p1());
        if i == 1 && settings.assist > 0.0 && settings.muscles > 0.0 {
            let supported = state.feet_down.iter().any(|&down| down);
            if supported {
                for (j, &e) in p.links.iter().enumerate() {
                    let measured = {
                        let q = set.p0();
                        q.get(e).ok().map(|(pos, _, v, _)| (pos.0, v.0))
                    };
                    if let Some((pos, v)) = measured {
                        let velocity = (next[j].translation - wanted[j].translation) * 120.0;
                        let force = ((wanted[j].translation - pos) * 225.0
                            + (velocity - v) * 30.0
                            + Vec3::Y * 9.81)
                            * p.masses[j]
                            * settings.assist
                            * settings.muscles;
                        if let Ok(mut f) = set.p1().get_mut(e) {
                            f.apply_force(force.clamp_length_max(p.masses[j] * 9.81 * 4.0));
                        }
                    }
                }
            }
        }
        if push {
            for (j, &e) in p.links.iter().enumerate() {
                if let Ok(mut f) = set.p1().get_mut(e) {
                    f.apply_linear_impulse(Vec3::X * (1.5 * p.masses[j]));
                }
            }
        }
    }
    bodies.last = bodies
        .list
        .iter()
        .map(|p| sim.sim.read_state(p.id))
        .collect();
    let animated = pose(&bench.segments, &left.local, left.root, left.rotation);
    let mixed = mixed_pose(
        &bench.segments,
        &frame.local,
        frame.root,
        frame.rotation,
        &bodies.last[1],
        settings.physics,
    );
    for (entities, transforms) in [(&bench.reference, animated), (&bench.mixed, mixed)] {
        for (&e, transform) in entities.iter().zip(transforms) {
            if let Ok(mut t) = set.p2().get_mut(e) {
                *t = transform;
            }
        }
    }
    bodies.ticks += 1;
    for s in 0..2 {
        for (index, point) in [
            (s, frame.feet[s]),
            (
                s + 2,
                body::bv(bodies.last[1].joints[if s == 0 { 11 } else { 12 }].pos),
            ),
        ] {
            let trail = &mut bench.trails[index];
            if trail.back().is_none_or(|p| p.distance(point) > 0.003) {
                trail.push_back(point);
            }
            if trail.len() > 360 {
                trail.pop_front();
            }
        }
    }
    bench.frame = Some(frame);
    bench.seconds += rate / 120.0;
}

fn slider_values(s: &Settings) -> [f32; 4] {
    [s.physics, s.muscles, s.playback / 2.0, s.assist]
}

fn controls(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    sliders: Query<(&Slider, &Interaction, &RelativeCursorPosition)>,
    buttons: Query<(&Control, &Interaction), Changed<Interaction>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        settings.reset = true;
    }
    if !mouse.pressed(MouseButton::Left) {
        settings.dragging = None;
    }
    for (slider, interaction, cursor) in &sliders {
        if mouse.pressed(MouseButton::Left)
            && mouse.just_pressed(MouseButton::Left)
            && *interaction == Interaction::Pressed
        {
            settings.dragging = Some(slider.0);
        }
        if settings.dragging == Some(slider.0) {
            if let Some(p) = cursor.normalized {
                let v = (p.x + 0.5).clamp(0.0, 1.0);
                match slider.0 {
                    0 => settings.physics = v,
                    1 => settings.muscles = v,
                    2 => settings.playback = v * 2.0,
                    _ => settings.assist = v,
                }
            }
        }
    }
    for (button, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button.0 {
            0..=2 => {
                settings.speed = [0.0, 1.5, 4.0][button.0];
                settings.reset = true;
            }
            3 => settings.paused = !settings.paused,
            4 => settings.push = true,
            _ => settings.reset = true,
        }
    }
}

fn paint(
    settings: Res<Settings>,
    bench: Res<Bench>,
    bodies: Res<Bodies>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut labels: Query<(&Value, &mut Text), Without<Stats>>,
    mut stats: Query<&mut Text, (With<Stats>, Without<Value>)>,
    mut buttons: Query<(&Control, &mut BackgroundColor)>,
) {
    let values = slider_values(&settings);
    for (fill, mut node) in &mut fills {
        node.width = percent(values[fill.0] * 100.0);
    }
    for (label, mut text) in &mut labels {
        text.0 = match label.0 {
            0 => format!("Physics blend   {:.0}%", settings.physics * 100.0),
            1 => format!("Motor strength   {:.0}%", settings.muscles * 100.0),
            2 => format!("Playback speed   {:.2}x", settings.playback),
            _ => format!("Tracking assist   {:.0}%", settings.assist * 100.0),
        };
    }
    for (button, mut color) in &mut buttons {
        let active = match button.0 {
            0 => settings.speed == 0.0,
            1 => settings.speed == 1.5,
            2 => settings.speed == 4.0,
            3 => settings.paused,
            _ => false,
        };
        color.0 = if active {
            Color::srgb(0.08, 0.42, 0.36)
        } else {
            Color::srgb(0.13, 0.20, 0.26)
        };
    }
    if let Ok(mut text) = stats.single_mut() {
        text.0 = format!(
            "Baked numeric tracks  |  time {:.2}s{}\nTarget lengths fitted once. Foot probes follow the terrain.",
            bench.seconds,
            if settings.paused { " (paused)" } else { "" }
        );
        if let Some(state) = bodies.last.get(1) {
            text.0 += &format!(
                "\nPhysical height {:.2} m  |  speed {:.2} m/s",
                state.com.y - terrain::height(state.com.x as f32, state.com.z as f32) as f64,
                state.com_v.length()
            );
        }
    }
}

fn ovals(bench: Res<Bench>, bodies: Res<Bodies>, mut gizmos: Gizmos) {
    for (i, trail) in bench.trails.iter().enumerate() {
        gizmos.linestrip(
            trail.iter().copied(),
            if i < 2 {
                Color::srgb(0.3, 0.9, 0.3)
            } else {
                Color::srgb(0.0, 0.8, 1.0)
            },
        );
    }
    for (i, x) in LANES.iter().enumerate() {
        let color = [
            Color::srgb(0.2, 0.8, 1.0),
            Color::srgb(1.0, 0.65, 0.2),
            Color::srgb(0.2, 1.0, 0.65),
        ][i];
        let points = (0..=80).map(|n| {
            let a = n as f32 / 80.0 * std::f32::consts::TAU;
            let xx = *x + a.cos() * 1.35;
            let z = a.sin() * 1.0;
            Vec3::new(xx, terrain::height(xx, z) + 0.015, z)
        });
        gizmos.linestrip(points, color);
    }
    if let Some(frame) = &bench.frame {
        for side in 0..2 {
            let raw = frame.raw[side];
            let from = raw + Vec3::Y * 0.6;
            if let Some(hit) = terrain::trace(from, 2.0) {
                gizmos.line(from, hit, Color::srgb(0.95, 0.8, 0.15));
                gizmos.line(hit, hit + terrain::normal(hit.x, hit.z) * 0.2, Color::WHITE);
            }
            let foot = frame.feet[side];
            let color = if frame.contact[side] > 0.65 {
                Color::srgb(0.1, 1.0, 0.3)
            } else {
                Color::srgb(1.0, 0.55, 0.05)
            };
            gizmos.sphere(Isometry3d::from_translation(foot), 0.045, color);
            let lock = bench.locks[1][side];
            if lock.planted {
                gizmos.cross(
                    Isometry3d::from_translation(lock.anchor),
                    0.08,
                    Color::srgb(0.1, 1.0, 0.3),
                );
            }
            if let Some(state) = bodies.last.get(1) {
                let index = if side == 0 { 11 } else { 12 };
                let actual = body::bv(state.joints[index].pos);
                gizmos.sphere(
                    Isometry3d::from_translation(actual),
                    0.035,
                    Color::srgb(0.0, 0.9, 1.0),
                );
                gizmos.line(actual, foot, Color::srgb(0.0, 0.9, 1.0));
            }
        }
    }
    for state in &bodies.last {
        for index in [11, 12] {
            let actual = body::bv(state.joints[index].pos);
            if let Some(hit) = terrain::trace(actual + Vec3::Y * 0.4, 2.0) {
                gizmos.line(actual + Vec3::Y * 0.4, hit, Color::srgb(0.0, 0.65, 1.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig;

    #[test]
    fn blend_zero_ignores_physical_displacement_and_one_follows_it() {
        let segments = rig::segments();
        let reference = vec![Quat::IDENTITY; segments.len()];
        let mut measured = BodyState::default();
        measured.joints = vec![Default::default(); segments.len()];
        measured.joints[0].pos = guildbound_sim::math::Vec3::new(8.0, 0.3, 2.0);
        let a = mixed_pose(
            &segments,
            &reference,
            segments[0].anchor + Vec3::X * LANES[2],
            Quat::IDENTITY,
            &measured,
            0.0,
        );
        let p = mixed_pose(
            &segments,
            &reference,
            segments[0].anchor + Vec3::X * LANES[2],
            Quat::IDENTITY,
            &measured,
            1.0,
        );
        let offset = segments[0].centre() - segments[0].anchor;
        assert!((a[0].translation - (segments[0].centre() + Vec3::X * LANES[2])).length() < 1e-5);
        assert!((p[0].translation - (body::bv(measured.joints[0].pos) + offset)).length() < 1e-5);
    }

    #[test]
    fn slider_drag_reaches_both_endpoints_and_stops_on_release() {
        let mut app = App::new();
        app.init_resource::<Settings>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, controls);
        let slider = app
            .world_mut()
            .spawn((
                Slider(0),
                Interaction::Pressed,
                RelativeCursorPosition {
                    cursor_over: true,
                    normalized: Some(Vec2::new(-0.5, 0.0)),
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<Settings>().physics, 0.0);
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(slider)
            .unwrap()
            .normalized = Some(Vec2::new(0.5, 0.0));
        app.update();
        assert_eq!(app.world().resource::<Settings>().physics, 1.0);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .get_mut::<RelativeCursorPosition>(slider)
            .unwrap()
            .normalized = Some(Vec2::new(-0.5, 0.0));
        app.update();
        assert_eq!(app.world().resource::<Settings>().physics, 1.0);
    }

    #[test]
    fn animation_has_no_physics_and_zero_motors_leave_both_ragdolls_passive() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
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
        .insert_resource(Settings {
            muscles: 0.0,
            physics: 0.0,
            ..default()
        })
        .init_resource::<crate::input::Actions>()
        .add_plugins(body::plugin)
        .add_systems(Startup, spawn)
        .add_systems(FixedPostUpdate, tick.before(PhysicsSystems::First));
        app.add_systems(
            Startup,
            |mut commands: Commands,
             mut meshes: ResMut<Assets<Mesh>>,
             mut mats: ResMut<Assets<StandardMaterial>>| {
                terrain::spawn_ground(&mut commands, &mut meshes, &mut mats);
            },
        );
        app.finish();
        app.cleanup();
        for _ in 0..480 {
            app.update();
        }
        let bench = app.world().resource::<Bench>();
        for &e in bench.reference.iter().chain(&bench.mixed) {
            assert!(app.world().get::<RigidBody>(e).is_none());
            assert!(app.world().get::<Collider>(e).is_none());
        }
        for side in 0..2 {
            let index = if side == 0 { 11 } else { 12 };
            let rendered = app.world().get::<Transform>(bench.mixed[index]).unwrap();
            let ankle = rendered.translation
                - rendered.rotation
                    * (bench.segments[index].centre() - bench.segments[index].anchor);
            assert!(
                ankle.distance(bench.frame.as_ref().unwrap().feet[side]) < 1e-4,
                "final rig foot missed IK target"
            );
        }
        let bodies = app.world().resource::<Bodies>();
        assert_eq!(bodies.list.len(), 2);
        assert!(
            bodies.list.iter().all(|p| p.source.is_none()),
            "test scene loaded a source character"
        );
        for state in &bodies.last {
            assert!(state.com.is_finite());
            assert!(state.com.y < 0.6, "zero-motor body was held up");
            assert!(state.joints.iter().all(|j| j.torque.length() < 1e-8));
        }
        let old_ids: Vec<_> = bodies.list.iter().map(|p| p.id).collect();
        app.world_mut().resource_mut::<Settings>().reset = true;
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Bodies>().list.len(), 2);
        for id in old_ids {
            assert!(
                app.world()
                    .resource::<SimRes>()
                    .sim
                    .read_state(id)
                    .joints
                    .is_empty()
            );
        }
        {
            let mut settings = app.world_mut().resource_mut::<Settings>();
            settings.reset = true;
            settings.muscles = 1.0;
            settings.assist = 1.0;
            settings.physics = 1.0;
        }
        app.update();
        app.update();
        let start = app.world().resource::<Bodies>().last[1].com;
        for _ in 0..1200 {
            app.update();
        }
        let end = &app.world().resource::<Bodies>().last[1];
        let clearance = end.com.y - terrain::height(end.com.x as f32, end.com.z as f32) as f64;
        println!(
            "TERRAIN_TRACKING start={start:?} end={:?} clearance={clearance}",
            end.com
        );
        assert!(clearance > 0.55, "assisted terrain tracking fell");
        assert!(
            (end.com - start).length() > 0.2,
            "physical body did not travel"
        );
        {
            let mut settings = app.world_mut().resource_mut::<Settings>();
            settings.reset = true;
            settings.speed = 4.0;
        }
        app.update();
        app.update();
        for _ in 0..1200 {
            app.update();
        }
        let run = &app.world().resource::<Bodies>().last[1];
        let clearance = run.com.y - terrain::height(run.com.x as f32, run.com.z as f32) as f64;
        println!("TERRAIN_RUN com={:?} clearance={clearance}", run.com);
        assert!(clearance > 0.55, "assisted run tracking fell");
    }
}

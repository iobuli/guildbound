//! Joint-relation monitor. Every frame, after `compose` and transform propagation, it measures
//! how each character's knees, feet, elbows and hands relate to each other and to the world,
//! and logs every broken relation to `staging/relations.log` (rate-limited per character,
//! relation and side), so any playtest leaves a record. F4 shows it: limbs drawn green when
//! sound and red when broken, hand targets, and a panel for the player.
//!
//! Relations (thresholds below):
//! - knee: folded toward the back of the body, or popped to the other side between frames;
//! - foot: heel / ball sunk into the ground, floating or sliding while planted, toes or knee
//!   inside world geometry;
//! - elbow: folded inwards, or popped between frames;
//! - hand: away from its fully weighted `HandTargets` point, inside a prop;
//! - body: an arm inside the torso / head / opposite thigh, or the legs crossing
//!   (`motion::body` capsules, the same ones `compose` resolves).
use super::select::HandTargets;
use super::{Action, Mover, Phys, Player};
use crate::motion::bank::PoseBank;
use crate::motion::body::Body;
use crate::motion::{Rig, RigBone};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::transform::TransformSystems;
use std::collections::{HashMap, VecDeque};
use std::io::Write;

/// Bend smaller than this (deg) has no meaningful side.
const MIN_FLEX: f32 = 10.0;
/// Bend side against the natural side by more than this (cos) is folded the wrong way.
const AGAINST: f32 = -0.3;
/// Bend side turning by more than this (cos) in one frame is a pop.
const POP: f32 = -0.5;
const SINK: f32 = -0.03;
const FLOAT: f32 = 0.05;
/// Feet slower than this (m/s) are planted; planted feet faster than `SLIDE` slide.
const PLANTED: f32 = 0.3;
const SLIDE: f32 = 0.6;
/// Ground gap (m) under which a foot counts as down.
const DOWN: f32 = 0.02;
const HAND_MISS: f32 = 0.05;
/// Body capsule overlap (m) that counts as self-clipping.
const SELF_CLIP: f32 = 0.01;
/// Hand weights above this are expected to arrive.
const HAND_FULL: f32 = 0.95;
/// Seconds between log lines for the same character, relation and side.
const LOG_EVERY: f32 = 0.25;
/// Window (s) of the panel's broken-relation counts.
const RECENT: f32 = 5.0;
/// Natural elbow direction in model space for the left arm (x mirrored for the right).
const ELBOW_OUT: Vec3 = Vec3::new(0.5, -0.6, -0.6);

const LOG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../staging/relations.log");

pub fn plugin(app: &mut App) {
    app.init_resource::<Monitor>()
        .add_systems(Startup, spawn_panel)
        .add_systems(Update, toggle)
        .add_systems(PostUpdate, measure.after(TransformSystems::Propagate));
}

#[derive(Resource, Default)]
pub struct Monitor {
    /// Draw limbs and the panel (measuring and logging always run).
    pub on: bool,
    log: Option<std::io::BufWriter<std::fs::File>>,
    opened: bool,
    last: HashMap<(Entity, &'static str, usize), f32>,
    /// Player's broken relations in the last `RECENT` seconds.
    recent: VecDeque<(f32, &'static str)>,
}

impl Monitor {
    /// Append a line to the log (other systems use it for events, e.g. graded landings).
    pub fn write(&mut self, line: &str) {
        if !self.opened {
            self.opened = true;
            let path = std::path::Path::new(LOG_PATH);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::File::create(path) {
                Ok(f) => self.log = Some(std::io::BufWriter::new(f)),
                Err(e) => warn!("[monitor] cannot write {}: {e}", path.display()),
            }
            if let Some(log) = &mut self.log {
                let _ = writeln!(log, "# t character relation side value mode action speed");
            }
        }
        if let Some(log) = &mut self.log {
            let _ = writeln!(log, "{line}");
        }
    }
}

/// Per-character memory between frames.
#[derive(Default)]
struct Memory {
    knee_side: [Option<Vec3>; 2],
    elbow_side: [Option<Vec3>; 2],
    foot: [Option<Vec3>; 2],
}

/// Bend of a limb `root → mid → end`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    /// Flexion (deg): 0 = straight.
    pub flex: f32,
    /// Unit direction of `mid` off the root→end axis (zero when straight).
    pub side: Vec3,
    /// `side · natural` (natural projected off the axis).
    pub against: f32,
}

pub fn bend(root: Vec3, mid: Vec3, end: Vec3, natural: Vec3) -> Bend {
    let (u, v) = (root - mid, end - mid);
    let interior = u.angle_between(v);
    let flex = if interior.is_finite() { 180.0 - interior.to_degrees() } else { 0.0 };
    let axis = (end - root).normalize_or(Vec3::NEG_Y);
    let off = |w: Vec3| w - axis * w.dot(axis);
    let side = off(mid - root).normalize_or_zero();
    Bend {
        flex,
        side,
        against: side.dot(off(natural).normalize_or_zero()),
    }
}

/// Wrong-way fold or a pop since `prev`, as a relation name.
fn bend_fault(b: Bend, prev: Option<Vec3>, folded: &'static str, popped: &'static str) -> Option<&'static str> {
    if b.flex < MIN_FLEX {
        return None;
    }
    if b.against < AGAINST {
        return Some(folded);
    }
    prev.filter(|p| p.dot(b.side) < POP).map(|_| popped)
}

#[derive(Component)]
struct Panel;

fn spawn_panel(mut commands: Commands) {
    commands.spawn((
        Panel,
        Node {
            position_type: PositionType::Absolute,
            top: px(8.0),
            left: px(8.0),
            padding: UiRect::axes(px(10.0), px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.75)),
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.95, 0.88)),
        Visibility::Hidden,
    ));
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut monitor: ResMut<Monitor>, mut panel: Query<&mut Visibility, With<Panel>>) {
    if keys.just_pressed(KeyCode::F4) {
        monitor.on = !monitor.on;
        for mut vis in &mut panel {
            vis.set_if_neq(if monitor.on { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn measure(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    spatial: SpatialQuery,
    mut monitor: ResMut<Monitor>,
    characters: Query<(
        Entity,
        &Rig,
        &Mover,
        &Transform,
        Option<&Name>,
        Option<&Action>,
        Option<&HandTargets>,
        Has<Player>,
    )>,
    bones_q: Query<&GlobalTransform, With<RigBone>>,
    mut panel: Query<&mut Text, With<Panel>>,
    mut gizmos: Gizmos,
    mut memory: Local<HashMap<Entity, Memory>>,
) {
    let Some(bank) = bank else { return };
    let b = bank.bones;
    let rest = bank.skeleton.fk(&bank.skeleton.rest_pose(), b.pelvis);
    let ankle_rest = [rest[b.foot[0]].translation.y, rest[b.foot[1]].translation.y];
    let ball_rest = [rest[b.ball[0]].translation.y, rest[b.ball[1]].translation.y];
    let now = time.elapsed_secs();
    let dt = time.delta_secs().max(1e-4);
    let world = SpatialQueryFilter::from_mask(Phys::World);
    let ground = |p: Vec3| {
        let origin = Vec3::new(p.x, p.y + 0.5, p.z);
        spatial.cast_ray(origin, Dir3::NEG_Y, 2.5, true, &world).map(|hit| origin.y - hit.distance)
    };
    let inside = |p: Vec3| !spatial.point_intersections(p, &world).is_empty();
    memory.retain(|e, _| characters.contains(*e));
    let mut panel_text = None;

    for (entity, rig, mover, root, name, action, hands, player) in &characters {
        let pos = |i: usize| {
            rig.bones
                .get(i)
                .and_then(|&e| bones_q.get(e).ok())
                .map(|g| g.translation())
                .filter(|p| p.is_finite())
        };
        let mem = memory.entry(entity).or_default();
        let mut faults: Vec<(&'static str, usize, f32)> = Vec::new();
        let mut rows = [[0.0_f32; 6]; 2];
        let forward = root.rotation * Vec3::Z;
        let speed = mover.velocity.with_y(0.0).length();

        for side in 0..2 {
            // Legs.
            let (Some(hip), Some(knee), Some(ankle), Some(ball)) =
                (pos(b.thigh[side]), pos(b.calf[side]), pos(b.foot[side]), pos(b.ball[side]))
            else {
                continue;
            };
            let k = bend(hip, knee, ankle, forward);
            let mut leg_bad = false;
            if let Some(f) = bend_fault(k, mem.knee_side[side], "knee_backward", "knee_pop") {
                faults.push((f, side, k.flex));
                leg_bad = true;
            }
            mem.knee_side[side] = (k.flex >= MIN_FLEX).then_some(k.side);
            let heel_gap = ground(ankle).map(|g| ankle.y - ankle_rest[side] - g);
            let ball_gap = ground(ball).map(|g| ball.y - ball_rest[side] - g);
            let foot_speed = mem.foot[side].map_or(0.0, |p| (ankle - p).with_y(0.0).length() / dt);
            mem.foot[side] = Some(ankle);
            let low = match (heel_gap, ball_gap) {
                (Some(h), Some(t)) => Some(h.min(t)),
                (h, t) => h.or(t),
            };
            if let Some(low) = low {
                if low < SINK {
                    faults.push(("foot_sink", side, low));
                    leg_bad = true;
                }
                if mover.grounded && foot_speed < PLANTED && speed < PLANTED && low > FLOAT {
                    faults.push(("foot_float", side, low));
                    leg_bad = true;
                }
                if mover.grounded && low < DOWN && foot_speed > SLIDE.max(speed * 0.5) {
                    faults.push(("foot_slide", side, foot_speed));
                }
            }
            if inside(ball) {
                faults.push(("toe_inside", side, 0.0));
                leg_bad = true;
            }
            if inside(knee) {
                faults.push(("knee_inside", side, 0.0));
                leg_bad = true;
            }
            rows[side][0] = k.flex;
            rows[side][2] = heel_gap.unwrap_or(f32::NAN);
            rows[side][3] = ball_gap.unwrap_or(f32::NAN);

            // Arms.
            let (Some(shoulder), Some(elbow), Some(hand)) =
                (pos(b.upperarm[side]), pos(b.lowerarm[side]), pos(b.hand[side]))
            else {
                continue;
            };
            let out = if side == 0 { ELBOW_OUT } else { ELBOW_OUT * Vec3::new(-1.0, 1.0, 1.0) };
            let e = bend(shoulder, elbow, hand, root.rotation * out);
            let mut arm_bad = false;
            if let Some(f) = bend_fault(e, mem.elbow_side[side], "elbow_inward", "elbow_pop") {
                faults.push((f, side, e.flex));
                arm_bad = true;
            }
            mem.elbow_side[side] = (e.flex >= MIN_FLEX).then_some(e.side);
            let mut miss = f32::NAN;
            if let Some((target, w)) = hands.and_then(|h| h.hands[side]) {
                miss = hand.distance(target);
                if w >= HAND_FULL && miss > HAND_MISS {
                    faults.push(("hand_miss", side, miss));
                    arm_bad = true;
                }
                if monitor.on {
                    gizmos.sphere(Isometry3d::from_translation(target), 0.03, Color::srgb(1.0, 0.85, 0.2));
                }
            }
            if inside(hand) {
                faults.push(("hand_inside", side, 0.0));
                arm_bad = true;
            }
            rows[side][1] = e.flex;
            rows[side][4] = miss;

            if monitor.on {
                let ok = Color::srgb(0.3, 1.0, 0.45);
                let bad = Color::srgb(1.0, 0.2, 0.2);
                let leg = if leg_bad { bad } else { ok };
                gizmos.linestrip([hip, knee, ankle, ball], leg);
                gizmos.linestrip([shoulder, elbow, hand], if arm_bad { bad } else { ok });
            }
        }

        // Self-clipping left over after `body::resolve` (springs, IK and hand targets act after it).
        let body = Body::new(&|i| pos(i).unwrap_or(Vec3::NAN), &b, 1.0);
        for side in 0..2 {
            if let Some(hit) = body.arm_overlap(side).filter(|o| o.depth > SELF_CLIP) {
                faults.push(("arm_in_body", side, hit.depth));
            }
        }
        if let Some(hit) = body.leg_overlap().filter(|o| o.depth > SELF_CLIP) {
            faults.push(("legs_crossed", 0, hit.depth));
        }

        let who = name.map_or_else(|| format!("{entity}"), |n| n.as_str().to_string());
        let kind = action.map_or_else(|| "-".to_string(), |a| format!("{:?}", a.kind));
        for &(relation, side, value) in &faults {
            let key = (entity, relation, side);
            if monitor.last.get(&key).is_some_and(|t| now - t < LOG_EVERY) {
                continue;
            }
            monitor.last.insert(key, now);
            let lr = if side == 0 { "L" } else { "R" };
            let line = format!("{now:.2} {who} {relation} {lr} {value:.3} {:?} {kind} {speed:.2}", mover.mode);
            monitor.write(&line);
            if player {
                monitor.recent.push_back((now, relation));
            }
        }

        if player && monitor.on {
            let pelvis = pos(b.pelvis).map_or(f32::NAN, |p| p.y);
            let low_foot = [pos(b.foot[0]), pos(b.foot[1])]
                .into_iter()
                .flatten()
                .map(|p| p.y)
                .fold(f32::INFINITY, f32::min);
            let cm = |x: f32| if x.is_finite() { format!("{:+5.1}", x * 100.0) } else { "   - ".to_string() };
            let [l, r] = rows;
            panel_text = Some(format!(
                "JOINT MONITOR (F4)   log: staging/relations.log\n\
                 knee flex  L {:5.1}  R {:5.1} deg\n\
                 elbow flex L {:5.1}  R {:5.1} deg\n\
                 heel gap   L {}  R {} cm\n\
                 ball gap   L {}  R {} cm\n\
                 hand miss  L {}  R {} cm\n\
                 pelvis over low foot {:.2} m   {:?}  {kind}",
                l[0],
                r[0],
                l[1],
                r[1],
                cm(l[2]),
                cm(r[2]),
                cm(l[3]),
                cm(r[3]),
                cm(l[4]),
                cm(r[4]),
                pelvis - low_foot,
                mover.mode,
            ));
        }
    }

    while monitor.recent.front().is_some_and(|(t, _)| now - t > RECENT) {
        monitor.recent.pop_front();
    }
    if let Some(log) = &mut monitor.log {
        let _ = log.flush();
    }
    if let Some(mut text) = panel_text {
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for (_, r) in &monitor.recent {
            match counts.iter_mut().find(|(n, _)| n == r) {
                Some((_, c)) => *c += 1,
                None => counts.push((r, 1)),
            }
        }
        text.push_str(&format!("\nbroken, last {RECENT:.0} s:"));
        if counts.is_empty() {
            text.push_str(" none");
        }
        for (r, c) in counts {
            text.push_str(&format!("  {r} x{c}"));
        }
        for mut t in &mut panel {
            t.0 = text.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bend_reads_flexion_side_and_folds() {
        let forward = Vec3::Z;
        // Knee 10 cm in front of the hip–ankle line: bent forward.
        let k = bend(Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, 0.5, 0.1), Vec3::new(0.0, 0.1, 0.0), forward);
        assert!(k.flex > 20.0 && k.against > 0.9);
        assert_eq!(bend_fault(k, None, "back", "pop"), None);
        // Knee behind the line: folded backwards.
        let back = bend(Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, 0.5, -0.1), Vec3::new(0.0, 0.1, 0.0), forward);
        assert_eq!(bend_fault(back, None, "back", "pop"), Some("back"));
        // Same side last frame is fine; the opposite side is a pop.
        let side = Vec3::new(1.0, 0.0, 0.0);
        let x = bend(Vec3::ZERO, Vec3::new(0.1, -0.4, 0.0), Vec3::new(0.0, -0.8, 0.0), side);
        assert_eq!(bend_fault(x, Some(-side), "back", "pop"), Some("pop"));
        assert_eq!(bend_fault(x, Some(side), "back", "pop"), None);
        // Straight limbs have no side.
        let straight = bend(Vec3::ZERO, Vec3::new(0.0, -0.4, 0.0), Vec3::new(0.0, -0.8, 0.0), forward);
        assert!(straight.flex < 1e-3);
        assert_eq!(bend_fault(straight, Some(-forward), "back", "pop"), None);
    }
}

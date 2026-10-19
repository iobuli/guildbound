//! Player input → `Intent` (keyboard + mouse + gamepad). Move direction is relative to the
//! camera yaw (`camera::CameraRig`). Controls: WASD move, Alt walk, Shift sprint, C crouch,
//! Space jump/climb, Ctrl dodge, LMB primary (tap/hold), RMB secondary (block/aim), F interact,
//! Q art (Battlewill), E spell (magicules), MB5 (mouse forward) next spell, Tab lock-on nearest
//! enemy, R reset player. Weapons come from the chest (`armory`), not from number keys.
//!
//! Gamepad: left stick move (small tilt walks), right stick camera, A jump, B dodge, X interact,
//! Y crouch, RB/RT primary, LB/LT secondary, L3 sprint (latched until the stick is released),
//! R3 lock-on, d-pad up art, d-pad right spell, d-pad left next spell, Select reset.
//! Mouse buttons only count while the cursor is captured (`camera::CursorCapture`), so clicking
//! UI or re-capturing the cursor never attacks. Crouch is a toggle latched here: `Intent.crouch`
//! is the latched state; sprinting or jumping stands back up.
use super::camera::{CameraRig, CursorCapture, stick_deadzone};
use super::controller::Teleport;
use super::world::PlayerSpawn;
use super::{Fighter, Intent, Npc, Phys, PlaySet, Player};
use avian3d::prelude::*;
use bevy::prelude::*;

/// `Intent.look` is this far along the camera view.
const LOOK_DISTANCE: f32 = 20.0;
/// Crosshair ray length (m) and the nearest aim point (m from the camera).
const AIM_RANGE: f32 = 120.0;
const LOOK_MIN: f32 = 2.0;
/// Lock-on acquire range (m) and half-angle of the acquire cone around the view (deg).
const LOCK_RANGE: f32 = 15.0;
const LOCK_CONE_DEG: f32 = 75.0;
/// A held lock breaks beyond this range.
const LOCK_BREAK_RANGE: f32 = 22.0;
/// Analog triggers count as pressed above this.
const TRIGGER_THRESHOLD: f32 = 0.35;

pub fn plugin(app: &mut App) {
    app.init_resource::<Latches>()
        .add_systems(Update, (player_input, scripted).chain().in_set(PlaySet::Input));
}

/// One scripted press: key, game time it starts (s), how long it is held (s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    pub key: char,
    pub at: f32,
    pub hold: f32,
}

/// Parse `GB_PRESS`: comma-separated `KEY@seconds[+hold]`, e.g. `F@3,B@5+1.5,W@7+2`.
pub fn parse_presses(spec: &str) -> Vec<Press> {
    spec.split(',')
        .filter_map(|item| {
            let (key, rest) = item.trim().split_once('@')?;
            let key = key.trim().chars().next()?.to_ascii_uppercase();
            let (at, hold) = match rest.split_once('+') {
                Some((a, h)) => (a.trim().parse().ok()?, h.trim().parse().ok()?),
                None => (rest.trim().parse().ok()?, 0.0),
            };
            Some(Press { key, at, hold })
        })
        .collect()
}

/// Smoke-test hook `GB_PRESS` (see `parse_presses`): after the real input, the player's `Intent`
/// gets the scripted keys. F interact, X draw / sheathe, G grab, L light attack, H heavy (held),
/// B block (held), Q art / E spell (held: aimed, cast on release), W / S / A / D walk (camera-free: world +Z / −Z / +X / −X). Edge flags fire on
/// the first frame at or after `at`; holds last `hold` s.
fn scripted(time: Res<Time>, mut presses: Local<Option<Vec<Press>>>, mut players: Query<&mut Intent, With<Player>>) {
    let list = presses.get_or_insert_with(|| std::env::var("GB_PRESS").map(|s| parse_presses(&s)).unwrap_or_default());
    if list.is_empty() {
        return;
    }
    let (now, dt) = (time.elapsed_secs(), time.delta_secs());
    for mut intent in &mut players {
        for p in list.iter() {
            let edge = now >= p.at && now - dt < p.at;
            let held = now >= p.at && now < p.at + p.hold.max(dt);
            match p.key {
                'F' if edge => intent.interact_pressed = true,
                'X' if edge => intent.sheathe_pressed = true,
                'G' if edge => intent.grab_pressed = true,
                'L' if edge => {
                    intent.primary_pressed = true;
                    intent.primary_held = true;
                }
                'L' if now - dt < p.at + 2.0 * dt && now >= p.at + dt => intent.primary_released = true,
                'H' if held => {
                    intent.primary_pressed |= edge;
                    intent.primary_held = true;
                }
                'H' if now >= p.at + p.hold && now - dt < p.at + p.hold => intent.primary_released = true,
                'B' if held => {
                    intent.secondary_pressed |= edge;
                    intent.secondary_held = true;
                }
                'Q' if held => {
                    intent.art_pressed |= edge;
                    intent.art_held = true;
                }
                'E' if held => {
                    intent.spell_pressed |= edge;
                    intent.spell_held = true;
                }
                'W' if held => intent.move_dir = Vec3::Z * 0.6,
                'S' if held => intent.move_dir = Vec3::NEG_Z * 0.6,
                'A' if held => intent.move_dir = Vec3::X * 0.6,
                'D' if held => intent.move_dir = Vec3::NEG_X * 0.6,
                _ => {}
            }
        }
    }
}

/// Toggle / edge state that outlives a frame.
#[derive(Resource, Default)]
struct Latches {
    crouch: bool,
    pad_sprint: bool,
    pad_primary: bool,
    pad_secondary: bool,
}

/// Camera-relative world move vector from a stick-style input (x right, y forward), length ≤ 1.
pub fn camera_relative(stick: Vec2, camera_yaw: f32) -> Vec3 {
    if !stick.is_finite() || !camera_yaw.is_finite() {
        return Vec3::ZERO;
    }
    let forward = Vec3::new(camera_yaw.sin(), 0.0, camera_yaw.cos());
    let right = forward.cross(Vec3::Y);
    (forward * stick.y + right * stick.x).clamp_length_max(1.0)
}

/// Best lock-on candidate: within `LOCK_RANGE`, inside the view cone, scored by distance and
/// angle. `candidates` = (entity, position).
pub fn pick_lock_target(
    from: Vec3,
    view_forward: Vec3,
    candidates: impl IntoIterator<Item = (Entity, Vec3)>,
) -> Option<Entity> {
    let cone = LOCK_CONE_DEG.to_radians().cos();
    let view = view_forward.with_y(0.0).normalize_or_zero();
    let mut best: Option<(f32, Entity)> = None;
    for (entity, pos) in candidates {
        let to = (pos - from).with_y(0.0);
        let dist = to.length();
        if !dist.is_finite() || dist > LOCK_RANGE {
            continue;
        }
        let dot = if dist > 1e-3 { view.dot(to / dist) } else { 1.0 };
        if dot < cone {
            continue;
        }
        let score = dist * (2.0 - dot);
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, entity));
        }
    }
    best.map(|(_, e)| e)
}

#[allow(clippy::too_many_arguments)]
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    capture: Res<CursorCapture>,
    mut rig: ResMut<CameraRig>,
    spawn: Option<Res<PlayerSpawn>>,
    mut latches: ResMut<Latches>,
    mut commands: Commands,
    mut players: Query<(Entity, &Transform, &mut Intent), With<Player>>,
    npcs: Query<(Entity, &Transform, Option<&Fighter>), (With<Npc>, Without<Player>)>,
    tweak: Option<Res<crate::motion::tweak::Tweak>>,
    talk: (Option<Res<super::talk::TalkFocus>>, Option<Res<super::talk::Talking>>, Option<Res<super::class::ClassChoice>>),
    spatial: SpatialQuery,
) {
    let Some((entity, transform, mut intent)) = players.iter_mut().next() else {
        return;
    };
    // The pose tweak tool (F2) or a conversation has the keyboard.
    // Talking, or choosing a class at the start: the panel has the keyboard.
    let talking = talk.1.as_ref().is_some_and(|t| t.active()) || talk.2.as_ref().is_some_and(|c| c.open);
    if tweak.is_some_and(|t| t.on) || talking {
        *intent = Intent {
            look: intent.look,
            ..default()
        };
        return;
    }
    let mouse_live = capture.captured && !capture.just_captured;

    // --- Movement stick (keyboard digital, gamepad analog; the larger wins).
    let axis = |neg: KeyCode, pos: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let mut stick = Vec2::new(axis(KeyCode::KeyA, KeyCode::KeyD), axis(KeyCode::KeyS, KeyCode::KeyW))
        .normalize_or_zero();
    let mut walk = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let mut sprint = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let mut jump_pressed = keys.just_pressed(KeyCode::Space);
    let mut jump_held = keys.pressed(KeyCode::Space);
    let mut dodge = keys.any_just_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let mut crouch_toggle = keys.just_pressed(KeyCode::KeyC);
    let mut interact = keys.just_pressed(KeyCode::KeyF);
    let sheathe = keys.just_pressed(KeyCode::KeyX);
    let grab = keys.just_pressed(KeyCode::KeyG);
    let mut art = keys.just_pressed(KeyCode::KeyQ);
    let mut spell = keys.just_pressed(KeyCode::KeyE);
    let mut art_held = keys.pressed(KeyCode::KeyQ);
    let mut spell_held = keys.pressed(KeyCode::KeyE);
    let mut next_spell = mouse.just_pressed(MouseButton::Forward);
    let mut lock_toggle = keys.just_pressed(KeyCode::Tab);
    let mut reset = keys.just_pressed(KeyCode::KeyR);
    let mut primary_pressed = mouse_live && mouse.just_pressed(MouseButton::Left);
    let mut primary_held = mouse_live && mouse.pressed(MouseButton::Left);
    let mut primary_released = mouse_live && mouse.just_released(MouseButton::Left);
    let mut secondary_pressed = mouse_live && mouse.just_pressed(MouseButton::Right);
    let mut secondary_held = mouse_live && mouse.pressed(MouseButton::Right);

    let mut pad_primary = false;
    let mut pad_secondary = false;
    for pad in &gamepads {
        let analog = stick_deadzone(pad.left_stick());
        if analog.length_squared() > stick.length_squared() {
            stick = analog;
            // Analog magnitude drives walk/run; the controller walks below half tilt.
        }
        jump_pressed |= pad.just_pressed(GamepadButton::South);
        jump_held |= pad.pressed(GamepadButton::South);
        dodge |= pad.just_pressed(GamepadButton::East);
        interact |= pad.just_pressed(GamepadButton::West);
        crouch_toggle |= pad.just_pressed(GamepadButton::North);
        lock_toggle |= pad.just_pressed(GamepadButton::RightThumb);
        reset |= pad.just_pressed(GamepadButton::Select);
        if pad.just_pressed(GamepadButton::LeftThumb) {
            latches.pad_sprint = true;
        }
        pad_primary |= pad.pressed(GamepadButton::RightTrigger)
            || pad.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > TRIGGER_THRESHOLD;
        pad_secondary |= pad.pressed(GamepadButton::LeftTrigger)
            || pad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0) > TRIGGER_THRESHOLD;
        art |= pad.just_pressed(GamepadButton::DPadUp);
        spell |= pad.just_pressed(GamepadButton::DPadRight);
        art_held |= pad.pressed(GamepadButton::DPadUp);
        spell_held |= pad.pressed(GamepadButton::DPadRight);
        next_spell |= pad.just_pressed(GamepadButton::DPadLeft);
    }
    // Gamepad button edges for the analog-capable buttons.
    primary_pressed |= pad_primary && !latches.pad_primary;
    primary_released |= !pad_primary && latches.pad_primary;
    primary_held |= pad_primary;
    secondary_pressed |= pad_secondary && !latches.pad_secondary;
    secondary_held |= pad_secondary;
    latches.pad_primary = pad_primary;
    latches.pad_secondary = pad_secondary;
    if stick.length() < 0.3 {
        latches.pad_sprint = false;
    }
    sprint |= latches.pad_sprint;
    if sprint {
        walk = false;
    }

    // --- Crouch latch.
    if crouch_toggle {
        latches.crouch = !latches.crouch;
    }
    if (sprint && stick.length() > 0.1) || jump_pressed {
        latches.crouch = false;
    }

    // --- Lock-on (keep, validate, toggle).
    let root = transform.translation;
    let alive = |e: Entity| {
        npcs.get(e).ok().filter(|(_, t, f)| {
            !f.is_some_and(|f| f.dead) && t.translation.distance(root) <= LOCK_BREAK_RANGE
        })
    };
    let mut target = intent.target.filter(|t| alive(*t).is_some());
    if lock_toggle {
        target = if target.is_some() {
            None
        } else {
            pick_lock_target(
                root,
                rig.flat_forward(),
                npcs.iter()
                    .filter(|(_, _, f)| !f.is_some_and(|f| f.dead))
                    .map(|(e, t, _)| (e, t.translation)),
            )
        };
    }

    // --- Reset to spawn.
    if reset {
        let (feet, yaw) = spawn.as_deref().map_or((Vec3::ZERO, 0.0), |s| (s.feet, s.yaw));
        commands.entity(entity).insert(Teleport { feet, yaw });
        rig.yaw = yaw;
        target = None;
        latches.crouch = false;
        latches.pad_sprint = false;
    }

    // The aim point is what the crosshair covers (world or a body), so shots from the chest
    // converge on it at any range; open sky falls back to `AIM_RANGE` out.
    let filter = SpatialQueryFilter::from_mask([Phys::World, Phys::Character]).with_excluded_entities([entity]);
    let forward = rig.forward();
    let look = Dir3::new(forward)
        .ok()
        .and_then(|dir| spatial.cast_ray(rig.eye, dir, AIM_RANGE, true, &filter))
        .map_or(rig.eye + forward * AIM_RANGE, |hit| rig.eye + forward * hit.distance.max(LOOK_MIN));
    *intent = Intent {
        move_dir: camera_relative(stick, rig.yaw),
        look: if look.is_finite() { look } else { root + Vec3::Z * LOOK_DISTANCE },
        walk,
        sprint,
        crouch: latches.crouch,
        jump_pressed,
        jump_held,
        dodge_pressed: dodge,
        primary_pressed,
        primary_held,
        primary_released,
        secondary_held,
        secondary_pressed,
        // F talks to a talkable in front of you rather than using an interactable.
        interact_pressed: interact && talk.0.as_ref().is_none_or(|f| f.0.is_none()),
        talk_pressed: interact && talk.0.as_ref().is_some_and(|f| f.0.is_some()),
        art_pressed: art,
        spell_pressed: spell,
        art_held,
        spell_held,
        next_spell,
        target,
        sheathe_pressed: sheathe,
        grab_pressed: grab,
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_scripted_presses() {
        let p = super::parse_presses("F@3, b@5+1.5,W@7+2,junk,X@");
        assert_eq!(p.len(), 3);
        assert_eq!((p[0].key, p[0].at, p[0].hold), ('F', 3.0, 0.0));
        assert_eq!((p[1].key, p[1].at, p[1].hold), ('B', 5.0, 1.5));
        assert_eq!(p[2].key, 'W');
    }

    use super::*;

    #[test]
    fn move_dir_follows_camera_yaw() {
        // yaw 0 looks along +Z; W moves +Z, D moves screen-right (−X).
        assert!(camera_relative(Vec2::Y, 0.0).abs_diff_eq(Vec3::Z, 1e-6));
        assert!(camera_relative(Vec2::X, 0.0).abs_diff_eq(Vec3::NEG_X, 1e-6));
        // camera turned to look along +X: W moves +X
        let d = camera_relative(Vec2::Y, std::f32::consts::FRAC_PI_2);
        assert!(d.abs_diff_eq(Vec3::X, 1e-5));
        // never longer than 1, NaN-safe
        assert!(camera_relative(Vec2::new(1.0, 1.0), 0.3).length() <= 1.0 + 1e-6);
        assert_eq!(camera_relative(Vec2::NAN, 0.0), Vec3::ZERO);
    }

    #[test]
    fn lock_on_prefers_close_targets_in_front() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let c = Entity::from_raw_u32(3).unwrap();
        let picked = pick_lock_target(
            Vec3::ZERO,
            Vec3::Z,
            [
                (a, Vec3::new(0.0, 0.0, 10.0)),
                (b, Vec3::new(0.5, 0.0, 4.0)),
                (c, Vec3::new(0.0, 0.0, -3.0)), // behind
            ],
        );
        assert_eq!(picked, Some(b));
        assert_eq!(pick_lock_target(Vec3::ZERO, Vec3::Z, [(c, Vec3::new(0.0, 0.0, -3.0))]), None);
        assert_eq!(pick_lock_target(Vec3::ZERO, Vec3::Z, [(a, Vec3::new(0.0, 0.0, 30.0))]), None);
    }
}

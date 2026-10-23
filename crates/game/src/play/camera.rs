//! Third-person orbit camera (`CameraRig` resource: yaw, pitch, distance, smoothed focus).
//! Mouse/right-stick orbit, wheel zoom, follows the player's INTERPOLATED Transform, lock-on
//! framing between player and target, over-shoulder offset while aiming, small shake on hits
//! (`CameraShake` resource other modules can add trauma to). Runs in PostUpdate after
//! TransformSystems::Propagate is NOT needed: read root `Transform` directly.
//!
//! Conventions: `CameraRig.yaw` uses the same convention as `Mover.yaw` (view forward on the
//! ground = (sin yaw, 0, cos yaw)); `pitch` > 0 looks down. The cursor is captured (hidden,
//! locked/confined) while the window is focused; Esc releases it, a click on the world (not on
//! UI) re-captures it, F1 / F2 (tuning / pose tweak pages) toggle a free-cursor UI mode.
use super::moveset::{Kind, Movesets};
use super::{HALF_HEIGHT, Intent, Loadout, Phys, PlaySet, Player};
use avian3d::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use super::armory::ArmoryOpen;
use super::hud::stats_page::StatsOpen;
use super::inventory::BagOpen;
use super::tuning::TuningPanel;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use std::f32::consts::{PI, TAU};

/// Zoom limits (m).
pub const MIN_DISTANCE: f32 = 2.5;
pub const MAX_DISTANCE: f32 = 9.0;
const DEFAULT_DISTANCE: f32 = 5.8;
/// Pitch limits (rad, + looks down).
const PITCH_MIN: f32 = -0.7;
const PITCH_MAX: f32 = 1.2;
const DEFAULT_PITCH: f32 = 0.22;
/// The camera frames the chest (≈1.35 m above the soles), not the capsule centre.
const CHEST_ABOVE_ROOT: f32 = 1.35 - HALF_HEIGHT;
/// Mouse look (rad per pixel), right stick (rad/s), wheel (m per notch).
const MOUSE_SENSITIVITY: f32 = 0.0022;
const STICK_YAW_SPEED: f32 = 2.8;
const STICK_PITCH_SPEED: f32 = 1.8;
const STICK_DEADZONE: f32 = 0.15;
const ZOOM_STEP: f32 = 0.6;
/// Critically damped follow times (s): horizontal is tight, vertical softer so jumps breathe.
const FOLLOW_TIME_H: f32 = 0.06;
const FOLLOW_TIME_V: f32 = 0.16;
/// Focus jumps larger than this (teleport / respawn) snap instead of swooping.
const SNAP_JUMP: f32 = 6.0;
/// Field of view (deg) normally and fully aimed.
const FOV_DEG: f32 = 60.0;
const AIM_FOV_DEG: f32 = 42.0;
/// Over-shoulder pivot offset while aiming (m, camera right / up) and boom length.
const AIM_SHOULDER_RIGHT: f32 = 0.65;
const AIM_SHOULDER_UP: f32 = 0.12;
const AIM_DISTANCE: f32 = 2.2;
/// Aim / lock-on blend rates (1/s, exponential).
const AIM_RATE: f32 = 12.0;
const LOCK_RATE: f32 = 6.0;
/// Lock-on: how hard the yaw is steered toward the target (1/s) and framing limits.
const LOCK_YAW_RATE: f32 = 7.0;
const LOCK_MAX_SHIFT: f32 = 3.0;
const LOCK_EXTRA_DISTANCE: f32 = 0.25;
const LOCK_MAX_EXTRA: f32 = 3.0;
const LOCK_MAX_RANGE: f32 = 30.0;
/// Collision probe radius (m) and how fast the boom eases back out after a pull-in (1/s).
const PROBE_RADIUS: f32 = 0.3;
const BOOM_RECOVER_RATE: f32 = 4.0;
const BOOM_MIN: f32 = 0.35;
/// Shake: trauma lost per second; maximum angles (rad) and offset (m) at trauma 1.
const SHAKE_DECAY: f32 = 1.6;
const SHAKE_YAW: f32 = 0.045;
const SHAKE_PITCH: f32 = 0.045;
const SHAKE_ROLL: f32 = 0.018;
const SHAKE_OFFSET: f32 = 0.08;

pub fn plugin(app: &mut App) {
    app.insert_resource(CameraRig::default())
        .init_resource::<CameraShake>()
        .insert_resource(CursorCapture::from_env())
        .init_resource::<FollowState>()
        .add_systems(
            Update,
            (
                capture_cursor.after(PageToggles),
                orbit,
            )
                .chain()
                .before(PlaySet::Input),
        )
        // Avian's interpolation eases root Transforms in `RunFixedMainLoop` (after the fixed loop,
        // before `Update`), so by PostUpdate every root holds its smooth render pose, including
        // anything gameplay changed during Update. Placing the camera here, before propagation,
        // means its GlobalTransform is current for this frame's render: no one-frame lag and no
        // 120 Hz stair-stepping from reading physics `Position`.
        .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
}

/// Marker for the gameplay camera (spawned by `world`, driven here).
#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Clone, Debug)]
pub struct CameraRig {
    /// View yaw (rad); ground forward = (sin yaw, 0, cos yaw), like `Mover.yaw`.
    pub yaw: f32,
    /// Rad, + looks down.
    pub pitch: f32,
    /// Wanted boom length (zoom), `MIN_DISTANCE..MAX_DISTANCE`.
    pub distance: f32,
    /// Smoothed world focus point (player chest, shifted toward a lock-on target).
    pub focus: Vec3,
    /// 0..1 over-shoulder aim blend.
    pub aim_blend: f32,
    /// 0..1 lock-on framing blend.
    pub lock_blend: f32,
    /// Unshaken eye position from the last update (`input` aims `Intent.look` from it).
    pub eye: Vec3,
    /// Over-shoulder side (+1 right, -1 left), toggled with B.
    pub shoulder: f32,
}

/// `GB_ZOOM=<m>`: the boom length at start (smoke stills; down to `SMOKE_MIN_DISTANCE` for a face
/// close-up, nearer than the wheel can go). Unset: `DEFAULT_DISTANCE`.
fn start_distance() -> f32 {
    std::env::var("GB_ZOOM")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|d| d.is_finite())
        .map_or(DEFAULT_DISTANCE, |d| d.clamp(SMOKE_MIN_DISTANCE, MAX_DISTANCE))
}

/// `GB_CAM=side|front|back|orbit[:<deg per s>]` (recordings): the view turns to a fixed angle from the player's facing, so a
/// kick or a dive reads in profile instead of from behind. `side` puts the player facing screen-right (as in the pose
/// sheets), `front` has it face the camera, `orbit` starts at `side` and circles at the given speed (default 40 deg/s).
/// Unset: the usual free orbit.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CamMode {
    Side,
    Front,
    Back,
    Orbit(f32),
}

fn cam_mode() -> Option<CamMode> {
    static MODE: std::sync::OnceLock<Option<CamMode>> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| {
        let v = std::env::var("GB_CAM").ok()?;
        let (name, arg) = v.split_once(':').map_or((v.as_str(), None), |(a, b)| (a, Some(b)));
        match name.trim() {
            "side" => Some(CamMode::Side),
            "front" => Some(CamMode::Front),
            "back" => Some(CamMode::Back),
            "orbit" => Some(CamMode::Orbit(arg.and_then(|a| a.trim().parse::<f32>().ok()).filter(|d| d.is_finite()).unwrap_or(40.0).to_radians())),
            _ => None,
        }
    })
}

/// View yaw for a `GB_CAM` mode: `facing` is the player's `Mover.yaw`, `t` the elapsed seconds.
fn cam_yaw(mode: CamMode, facing: f32, t: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match mode {
        CamMode::Side => facing + FRAC_PI_2,
        CamMode::Front => facing + PI,
        CamMode::Back => facing,
        CamMode::Orbit(rate) => facing + FRAC_PI_2 + rate * t,
    }
}

/// Nearest `GB_ZOOM` boom (m).
const SMOKE_MIN_DISTANCE: f32 = 0.8;

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: DEFAULT_PITCH,
            distance: start_distance(),
            focus: Vec3::ZERO,
            aim_blend: 0.0,
            lock_blend: 0.0,
            eye: Vec3::new(0.0, 2.0, -DEFAULT_DISTANCE),
            shoulder: 1.0,
        }
    }
}

impl CameraRig {
    /// Unit view direction.
    pub fn forward(&self) -> Vec3 {
        view_forward(self.yaw, self.pitch)
    }
    /// View direction flattened onto the ground.
    pub fn flat_forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
    /// Screen right on the ground (character left is +X, so yaw 0 → −X).
    pub fn flat_right(&self) -> Vec3 {
        self.flat_forward().cross(Vec3::Y)
    }
}

/// Camera shake trauma 0..1; offsets scale with trauma² and it decays over ~0.6 s.
#[derive(Resource, Default, Clone, Debug)]
pub struct CameraShake {
    pub trauma: f32,
}

impl CameraShake {
    pub fn add(&mut self, trauma: f32) {
        if trauma.is_finite() && trauma > 0.0 {
            self.trauma = (self.trauma.max(0.0) + trauma).min(1.0);
        }
    }
}

/// Systems that open or close UI pages (tuning, bag); the cursor is decided after them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageToggles;

/// Cursor capture state. Mouse look and mouse buttons only act while `captured`.
#[derive(Resource, Clone, Debug)]
pub struct CursorCapture {
    /// The player wants the cursor captured (cleared by Esc, set by clicking the world).
    pub wanted: bool,
    /// The cursor is captured right now.
    pub captured: bool,
    /// Captured this frame (the capturing click must not also attack).
    pub just_captured: bool,
    /// A UI page is open (F1 tuning, F2 pose tweak, I bag): keep the cursor free. Derived each
    /// frame from the pages' own state, never toggled, so pages can't fall out of step.
    pub ui_mode: bool,
}

impl CursorCapture {
    /// Smoke tests (`GB_AUTOQUIT`) never grab the user's mouse.
    fn from_env() -> Self {
        Self {
            wanted: std::env::var_os("GB_AUTOQUIT").is_none(),
            captured: false,
            just_captured: false,
            ui_mode: false,
        }
    }
}

/// Private follow state: spring velocity, collision-limited boom, smoothed zoom, fov, shake clock.
#[derive(Resource, Default)]
pub(crate) struct FollowState {
    initialized: bool,
    focus_velocity: Vec3,
    boom: f32,
    zoom: f32,
    shake_time: f32,
    shoulder: f32,
    /// Last known lock-on target chest (keeps the framing while blending out).
    last_target: Option<Vec3>,
}

/// View direction for a yaw/pitch pair (pitch + looks down).
pub fn view_forward(yaw: f32, pitch: f32) -> Vec3 {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    Vec3::new(sy * cp, -sp, cy * cp)
}

/// Wrap an angle into (−π, π].
pub fn wrap_angle(a: f32) -> f32 {
    if !a.is_finite() {
        return 0.0;
    }
    let w = (a + PI).rem_euclid(TAU) - PI;
    if w <= -PI { w + TAU } else { w }
}

/// Critically damped spring toward `target` (Game Programming Gems 4, "SmoothDamp").
/// Frame-rate independent; never overshoots for a static target.
pub fn smooth_damp(current: Vec3, target: Vec3, velocity: &mut Vec3, smooth_time: f32, dt: f32) -> Vec3 {
    if !current.is_finite() || !velocity.is_finite() {
        *velocity = Vec3::ZERO;
        return target;
    }
    let smooth_time = smooth_time.max(1e-4);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * decay;
    let mut out = target + (change + temp) * decay;
    // Guard against overshoot on large steps.
    if (target - current).dot(out - target) > 0.0 {
        out = target;
        *velocity = Vec3::ZERO;
    }
    out
}

/// Exponential approach factor for a rate (1/s) over `dt`.
fn blend(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Deadzone + rescale of an analog stick.
pub fn stick_deadzone(raw: Vec2) -> Vec2 {
    if !raw.is_finite() {
        return Vec2::ZERO;
    }
    let len = raw.length();
    if len <= STICK_DEADZONE {
        return Vec2::ZERO;
    }
    raw / len * ((len - STICK_DEADZONE) / (1.0 - STICK_DEADZONE)).min(1.0)
}

/// Smooth pseudo-noise in −1..1 for shake (sum of incommensurate sines).
fn wobble(t: f32, seed: f32) -> f32 {
    0.6 * (t * 13.1 + seed).sin() + 0.4 * (t * 23.7 + seed * 1.93).sin()
}

/// Whether the equipped moveset aims over the shoulder (ranged / magic).
fn aims_over_shoulder(loadout: Option<&Loadout>, movesets: Option<&Movesets>) -> bool {
    let Some(loadout) = loadout else {
        return true;
    };
    let name = loadout.equipped();
    if let Some(set) = movesets.and_then(|m| m.get(name)) {
        return matches!(set.kind, Kind::Ranged | Kind::Magic);
    }
    matches!(
        name,
        "bow" | "crossbow" | "knives" | "fire" | "frost" | "storm" | "arcane"
    )
}

/// The only writer of the cursor's grab and visibility.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capture_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    ui: Query<&Interaction>,
    mut capture: ResMut<CursorCapture>,
    mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    bag: Option<Res<BagOpen>>,
    armory: Option<Res<ArmoryOpen>>,
    stats: Option<Res<StatsOpen>>,
    tuning: Option<Res<TuningPanel>>,
    talking: Option<Res<super::talk::Talking>>,
    paused: Option<Res<super::pause::Paused>>,
    mut tweak_page: Local<bool>,
) {
    capture.just_captured = false;
    let Ok((window, mut cursor)) = windows.single_mut() else {
        return;
    };
    // F2 (pose tweak) has no page state of its own yet: it frees the cursor while toggled on.
    if keys.just_pressed(KeyCode::F2) {
        *tweak_page = !*tweak_page;
    }
    if keys.just_pressed(KeyCode::Escape) {
        *tweak_page = false;
    }
    // Any open page frees the cursor; closing the last one takes it back.
    let ui_mode = bag.is_some_and(|b| b.0)
        || armory.is_some_and(|a| a.0.is_some())
        || stats.is_some_and(|s| s.0.is_some())
        || tuning.is_some_and(|t| t.open)
        || talking.is_some_and(|t| t.active())
        || paused.is_some_and(|p| p.0)
        || *tweak_page;
    if capture.ui_mode && !ui_mode {
        capture.wanted = true;
    }
    capture.ui_mode = ui_mode;
    if keys.just_pressed(KeyCode::Escape) {
        capture.wanted = false;
    }
    let over_ui = ui.iter().any(|i| *i != Interaction::None);
    if !capture.wanted
        && !capture.ui_mode
        && window.focused
        && mouse.just_pressed(MouseButton::Left)
        && !over_ui
    {
        capture.wanted = true;
    }
    let grab = capture.wanted && window.focused && !capture.ui_mode;
    if grab != capture.captured {
        capture.captured = grab;
        capture.just_captured = grab;
        // Locked falls back to Confined on platforms without pointer lock (Windows); raw mouse
        // deltas keep flowing either way.
        cursor.grab_mode = if grab {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        cursor.visible = !grab;
    }
}

fn orbit(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    capture: Res<CursorCapture>,
    gamepads: Query<&Gamepad>,
    settings: Option<Res<super::pause::Settings>>,
    player: Query<&super::Mover, With<super::Player>>,
    mut rig: ResMut<CameraRig>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let mut d_yaw = 0.0;
    let mut d_pitch = 0.0;
    if capture.captured && !capture.just_captured {
        if keys.just_pressed(KeyCode::KeyB) { rig.shoulder = -rig.shoulder; }
        let d = motion.delta;
        if d.is_finite() {
            let k = MOUSE_SENSITIVITY * settings.as_ref().map_or(1.0, |s| s.mouse) * (1.0 - 0.5 * rig.aim_blend);
            d_yaw -= d.x * k;
            d_pitch += d.y * k;
        }
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        if notches.is_finite() && notches != 0.0 {
            rig.distance = (rig.distance - notches * ZOOM_STEP).clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    }
    for pad in &gamepads {
        let s = stick_deadzone(pad.right_stick());
        let precision = 1.0 - 0.5 * rig.aim_blend;
        d_yaw -= s.x * STICK_YAW_SPEED * dt * precision;
        d_pitch -= s.y * STICK_PITCH_SPEED * dt * precision;
    }
    rig.yaw = wrap_angle(rig.yaw + d_yaw);
    rig.pitch = (rig.pitch + d_pitch).clamp(PITCH_MIN, PITCH_MAX);
    // `GB_CAM`: hold the chosen angle from the player's facing, level enough to see a kick's line.
    if let (Some(mode), Ok(mover)) = (cam_mode(), player.single())
        && mover.yaw.is_finite()
    {
        rig.yaw = wrap_angle(cam_yaw(mode, mover.yaw, time.elapsed_secs()));
        rig.pitch = 0.12;
    }
    if !rig.distance.is_finite() {
        rig.distance = DEFAULT_DISTANCE;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn follow(
    time: Res<Time>,
    real: Res<Time<Real>>,
    mut rig: ResMut<CameraRig>,
    mut shake: ResMut<CameraShake>,
    mut state: ResMut<FollowState>,
    movesets: Option<Res<Movesets>>,
    spatial: SpatialQuery,
    players: Query<(&Transform, &Intent, Option<&Loadout>), (With<Player>, Without<MainCamera>)>,
    roots: Query<&Transform, Without<MainCamera>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let real_dt = real.delta_secs().clamp(0.0, 0.1);
    let Some((player, intent, loadout)) = players.iter().next() else {
        return;
    };
    if !player.translation.is_finite() {
        return;
    }
    let chest = player.translation + Vec3::Y * CHEST_ABOVE_ROOT;

    // Lock-on framing target.
    let target = intent
        .target
        .and_then(|t| roots.get(t).ok())
        .map(|t| t.translation + Vec3::Y * CHEST_ABOVE_ROOT)
        .filter(|t| t.is_finite() && t.distance(chest) < LOCK_MAX_RANGE);
    if target.is_some() {
        state.last_target = target;
    }
    let lock_goal = if target.is_some() { 1.0 } else { 0.0 };
    rig.lock_blend += (lock_goal - rig.lock_blend) * blend(LOCK_RATE, dt);
    if rig.lock_blend < 1e-3 {
        state.last_target = None;
    }

    // Aim (over-shoulder) blend.
    let aiming = (intent.secondary_held && aims_over_shoulder(loadout, movesets.as_deref())) || intent.art_held || intent.spell_held;
    let aim_goal = if aiming { 1.0 } else { 0.0 };
    rig.aim_blend += (aim_goal - rig.aim_blend) * blend(AIM_RATE, dt);

    // Focus goal: chest, shifted toward the target while locked.
    let mut focus_goal = chest;
    let mut lock_extra = 0.0;
    if let Some(t) = state.last_target {
        let to = t - chest;
        let shift = (to * 0.5).clamp_length_max(LOCK_MAX_SHIFT);
        let w = rig.lock_blend * (1.0 - rig.aim_blend);
        focus_goal += shift * w;
        let sep = Vec2::new(to.x, to.z).length();
        lock_extra = (sep * LOCK_EXTRA_DISTANCE).min(LOCK_MAX_EXTRA) * w;
        // Steer the view behind the player toward the target.
        if target.is_some() && sep > 0.5 && rig.aim_blend < 0.5 {
            let wanted = to.x.atan2(to.z) + 0.32;
            rig.yaw = wrap_angle(rig.yaw + wrap_angle(wanted - rig.yaw) * blend(LOCK_YAW_RATE, dt));
        }
    }

    // Critically damped follow (separate horizontal / vertical times).
    if !state.initialized || !rig.focus.is_finite() || rig.focus.distance(focus_goal) > SNAP_JUMP {
        rig.focus = focus_goal;
        state.focus_velocity = Vec3::ZERO;
        state.zoom = rig.distance;
        state.boom = rig.distance;
        state.initialized = true;
    } else {
        let mut vh = state.focus_velocity.with_y(0.0);
        let mut vv = Vec3::Y * state.focus_velocity.y;
        let h = smooth_damp(rig.focus.with_y(0.0), focus_goal.with_y(0.0), &mut vh, FOLLOW_TIME_H, dt);
        let v = smooth_damp(Vec3::Y * rig.focus.y, Vec3::Y * focus_goal.y, &mut vv, FOLLOW_TIME_V, dt);
        rig.focus = Vec3::new(h.x, v.y, h.z);
        state.focus_velocity = Vec3::new(vh.x, vv.y, vh.z);
    }

    // Zoom and boom length.
    state.zoom += (rig.distance - state.zoom) * blend(10.0, dt);
    let boom_goal = (state.zoom + lock_extra).lerp(AIM_DISTANCE, rig.aim_blend);

    // Pivot (over-shoulder offset) and collision pull-in against the world.
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    let probe = Collider::sphere(PROBE_RADIUS);
    let cast = |from: Vec3, to: Vec3| -> Vec3 {
        let delta = to - from;
        let Ok((dir, len)) = Dir3::new_and_length(delta) else {
            return to;
        };
        match spatial.cast_shape(
            &probe,
            from,
            Quat::IDENTITY,
            dir,
            &ShapeCastConfig::from_max_distance(len),
            &filter,
        ) {
            Some(hit) => from + dir * (hit.distance - 0.02).max(0.0),
            None => to,
        }
    };
    state.shoulder += (rig.shoulder - state.shoulder) * blend(AIM_RATE, dt);
    let shoulder = rig.flat_right() * AIM_SHOULDER_RIGHT * rig.aim_blend * state.shoulder
        + Vec3::Y * AIM_SHOULDER_UP * rig.aim_blend;
    let pivot = cast(rig.focus, rig.focus + shoulder);
    let forward = rig.forward();
    let allowed = cast(pivot, pivot - forward * boom_goal).distance(pivot);
    if allowed < state.boom || !state.boom.is_finite() {
        state.boom = allowed; // pull in instantly so nothing clips
    } else {
        state.boom += (allowed - state.boom) * blend(BOOM_RECOVER_RATE, dt);
    }
    state.boom = state.boom.max(BOOM_MIN.min(allowed.max(0.05)));
    let eye = pivot - forward * state.boom;
    rig.eye = eye;

    // Shake (real time, so it plays through hitstop).
    shake.trauma = (shake.trauma - SHAKE_DECAY * real_dt).clamp(0.0, 1.0);
    if !shake.trauma.is_finite() {
        shake.trauma = 0.0;
    }
    state.shake_time = (state.shake_time + real_dt) % 1000.0;
    let amount = shake.trauma * shake.trauma;
    let t = state.shake_time;
    let shake_rot = Quat::from_euler(
        EulerRot::YXZ,
        SHAKE_YAW * amount * wobble(t, 1.3),
        SHAKE_PITCH * amount * wobble(t, 7.9),
        SHAKE_ROLL * amount * wobble(t, 4.1),
    );
    let shake_offset = Vec3::new(wobble(t, 2.2), wobble(t, 5.5), wobble(t, 9.7)) * SHAKE_OFFSET * amount;

    let fov = FOV_DEG.lerp(AIM_FOV_DEG, rig.aim_blend).to_radians();
    for (mut transform, mut projection) in &mut cameras {
        let base = Transform::from_translation(eye).looking_to(forward, Vec3::Y);
        transform.translation = cast(eye, eye + base.rotation * shake_offset);
        transform.rotation = (base.rotation * shake_rot).normalize();
        if let Projection::Perspective(p) = &mut *projection
            && (p.fov - fov).abs() > 1e-4
        {
            p.fov = fov;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cam_yaws_put_the_player_in_profile_front_and_back() {
        use super::{CamMode, cam_yaw};
        use std::f32::consts::{FRAC_PI_2, PI};
        assert!((cam_yaw(CamMode::Side, 0.3, 5.0) - (0.3 + FRAC_PI_2)).abs() < 1e-6);
        assert!((cam_yaw(CamMode::Front, 0.3, 5.0) - (0.3 + PI)).abs() < 1e-6);
        assert!((cam_yaw(CamMode::Back, 0.3, 5.0) - 0.3).abs() < 1e-6);
        assert!((cam_yaw(CamMode::Orbit(1.0), 0.0, 2.0) - (FRAC_PI_2 + 2.0)).abs() < 1e-6);
    }

    use super::*;

    #[test]
    fn view_conventions_match_mover() {
        let rig = CameraRig::default();
        assert!(rig.flat_forward().abs_diff_eq(Vec3::Z, 1e-6));
        // character left is +X, so screen right at yaw 0 is −X
        assert!(rig.flat_right().abs_diff_eq(Vec3::NEG_X, 1e-6));
        let down = view_forward(0.0, 0.5);
        assert!(down.y < 0.0 && (down.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn wrap_angle_stays_in_range() {
        for a in [-10.0, -PI, -0.1, 0.0, 3.0, PI, 7.0, 100.0] {
            let w = wrap_angle(a);
            assert!(w > -PI - 1e-5 && w <= PI + 1e-5, "{a} → {w}");
            assert!(((w - a) / TAU).round() * TAU - (w - a) < 1e-3);
        }
        assert_eq!(wrap_angle(f32::NAN), 0.0);
    }

    #[test]
    fn smooth_damp_converges_without_overshoot() {
        let mut v = Vec3::ZERO;
        let mut x = Vec3::ZERO;
        let target = Vec3::new(3.0, 0.0, 0.0);
        for _ in 0..240 {
            x = smooth_damp(x, target, &mut v, 0.1, 1.0 / 120.0);
            assert!(x.x <= target.x + 1e-5);
        }
        assert!(x.distance(target) < 1e-3);
        // a huge step still lands on the target, not past it
        let mut v = Vec3::ZERO;
        let y = smooth_damp(Vec3::ZERO, target, &mut v, 0.05, 1.0);
        assert!(y.x <= target.x + 1e-5);
    }

    #[test]
    fn stick_deadzone_rescales() {
        assert_eq!(stick_deadzone(Vec2::new(0.1, 0.0)), Vec2::ZERO);
        assert!((stick_deadzone(Vec2::new(1.0, 0.0)).x - 1.0).abs() < 1e-6);
        assert!(stick_deadzone(Vec2::new(0.5, 0.0)).x < 0.5);
        assert_eq!(stick_deadzone(Vec2::NAN), Vec2::ZERO);
    }

    #[test]
    fn shake_add_clamps_and_ignores_garbage() {
        let mut s = CameraShake::default();
        s.add(0.7);
        s.add(0.7);
        assert_eq!(s.trauma, 1.0);
        s.add(f32::NAN);
        s.add(-1.0);
        assert_eq!(s.trauma, 1.0);
    }

    #[test]
    fn ranged_loadouts_aim_over_shoulder() {
        let l = |name: &str| Loadout {
            slots: vec![name.to_string()],
            current: 0,
        };
        assert!(aims_over_shoulder(Some(&l("bow")), None));
        assert!(aims_over_shoulder(Some(&l("frost")), None));
        assert!(!aims_over_shoulder(Some(&l("katana")), None));
    }
}

//! Athorina: one controllable model, one impact wall, and a motion browser.
pub mod controller;
pub mod athorina;
use avian3d::prelude::*;
use bevy::prelude::*;
pub fn plugin(app: &mut App) {
    app.configure_sets(Update, (PlaySet::Input, PlaySet::Think, PlaySet::Act, PlaySet::Select).chain())
        .init_resource::<Feel>()
        .add_plugins((controller::plugin, athorina::plugin));
}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlaySet {
    Input,
    Think,
    Act,
    Select,
    /// FixedUpdate only.
    Move,
}

#[derive(PhysicsLayer, Default, Clone, Copy, Debug)]
pub enum Phys {
    #[default]
    World,
    Character,
    /// Ragdoll segment bodies (collide with World and each other, not with capsules).
    Ragdoll,
    Projectile,
    /// Sensors: water volumes, interaction triggers.
    Trigger,
    /// Loose physics pieces (felled trees): collide with World and each other only, so
    /// characters and ground probes ignore them.
    Debris,
}

/// Capsule dimensions shared by spawn, controller and select (UAL2 mannequin ≈ 1.75 m).
/// The capsule floats `STEP_CLEARANCE` above the soles: the controller holds the body up on a
/// ground probe, so stairs, kerbs and rocks lower than that never touch the collider.
pub const CAPSULE_RADIUS: f32 = 0.3;
pub const STEP_CLEARANCE: f32 = 0.35;
pub const CAPSULE_LENGTH: f32 = 1.75 - STEP_CLEARANCE - 2.0 * CAPSULE_RADIUS;
/// Distance from capsule centre to the soles.
pub const HALF_HEIGHT: f32 = STEP_CLEARANCE + CAPSULE_RADIUS + CAPSULE_LENGTH * 0.5;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Npc;

/// One-frame player intent consumed by the capsule controller and combat preview.
#[derive(Component, Clone, Debug, Default)]
pub struct Intent {
    pub move_dir: Vec3,
    pub walk: bool,
    pub sprint: bool,
    pub crouch: bool,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub primary_pressed: bool,
    pub secondary_pressed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveMode {
    #[default]
    Ground,
    Air,
    Swim,
    Climb,
    /// Hanging from a ledge.
    Ledge,
    /// Physics owns the body (`ragdoll`).
    Ragdoll,
}

/// Kinematic movement state; written by `controller` (and `traverse` for climb/swim).
#[derive(Component, Clone, Debug)]
pub struct Mover {
    pub mode: MoveMode,
    /// World velocity (m/s).
    pub velocity: Vec3,
    /// Smoothed world acceleration (m/s²) → `ContactRequest.accel`.
    pub accel: Vec3,
    /// Facing yaw (rad); forward = (sin yaw, 0, cos yaw).
    pub yaw: f32,
    pub yaw_rate: f32,
    pub grounded: bool,
    pub ground_normal: Vec3,
    /// Seconds since leaving the ground.
    pub air_time: f32,
    /// Landing impact 0..1, decays (drives squash + land key).
    pub landing: f32,
    /// Locomotion cycle phase 0..1. Advanced by `select` (travelled distance / blended track
    /// stride), read by `select` only.
    pub phase: f32,
    pub crouch: bool,
    pub sprinting: bool,
    /// Knockback / external velocity, decays in `controller`.
    pub external: Vec3,
    /// Velocity requested by the current action this frame (lunges, dodges, root motion).
    /// Reset to zero by `controller` after use.
    pub root_motion: Vec3,
    /// 0..1 scale on player-driven movement (actions lock movement).
    pub move_scale: f32,
    /// When set, the controller turns toward this yaw instead of the move direction.
    pub face: Option<f32>,
    /// Running total (m) of how far the drawn root has moved vertically while easing over steps.
    /// Written by `controller` only; `contact` uses its per-frame change to keep the pelvis still
    /// in the world, so the visible body height has one smoother.
    pub step_ease: f32,
    /// Downward speed (m/s) at the last touchdown. Written by `controller` only.
    pub impact: f32,
    /// Horizontal speed (m/s) when the body last left the ground (jump or run-off). Written by
    /// `controller` only; `select` picks the running-jump poses from it.
    pub takeoff_speed: f32,
}

impl Default for Mover {
    fn default() -> Self {
        Self {
            mode: MoveMode::Ground,
            velocity: Vec3::ZERO,
            accel: Vec3::ZERO,
            yaw: 0.0,
            yaw_rate: 0.0,
            grounded: true,
            ground_normal: Vec3::Y,
            air_time: 0.0,
            landing: 0.0,
            phase: 0.0,
            crouch: false,
            sprinting: false,
            external: Vec3::ZERO,
            root_motion: Vec3::ZERO,
            move_scale: 1.0,
            face: None,
            step_ease: 0.0,
            impact: 0.0,
            takeoff_speed: 0.0,
        }
    }
}

impl Mover {
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
    }
}

#[derive(Resource, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Feel {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    /// Ground acceleration (m/s²) and braking (m/s²).
    pub accel: f32,
    pub brake: f32,
    /// Turn rate (deg/s).
    pub turn_rate: f32,
    pub jump_speed: f32,
    pub gravity: f32,
    pub air_control: f32,
    pub coyote_time: f32,
    /// Hitstop duration (s) on a landed hit.
    pub hitstop: f32,
    pub knockback_scale: f32,
    /// Knockback speed above which the target ragdolls.
    pub ragdoll_threshold: f32,
}

impl Default for Feel {
    fn default() -> Self {
        Self {
            walk_speed: 1.6,
            run_speed: 4.2,
            sprint_speed: 6.8,
            accel: 28.0,
            brake: 34.0,
            turn_rate: 720.0,
            jump_speed: 7.2,
            gravity: 20.0,
            air_control: 0.5,
            coyote_time: 0.12,
            hitstop: 0.07,
            knockback_scale: 1.0,
            ragdoll_threshold: 7.0,
        }
    }
}

//! Gameplay: world, characters, movement, combat, ranged, magic, traversal, interaction, AI.
//!
//! Data flow per frame:
//! - `Update / PlaySet::Input`  – `input` (player) writes `Intent`.
//! - `Update / PlaySet::Think`  – `ai` (NPCs) writes `Intent`.
//! - `Update / PlaySet::Act`    – `combat`, `ranged`, `magic`, `interact`, `traverse` run their
//!                                state machines on `Action`, emit `Hit` messages, request root
//!                                motion through `Mover.root_motion`.
//! - `Update / PlaySet::Select` – `select` turns `Mover` + `Action` + `Loadout` into
//!                                `MotionLayers` (the only producer of `MotionLayers.layers`).
//! - `FixedUpdate / PlaySet::Move` – `controller` moves capsules with Avian `MoveAndSlide`.
//! - `PostUpdate` – `motion::compose` writes bones; `camera` follows the player.
//!
//! Action ownership: a system may START an action only when `Action::is_free()` (or inside its
//! own cancel window), and may only ADVANCE/END actions whose `owner` is itself. `Owner::Hit`
//! (combat damage reactions) may interrupt anything.

pub mod ai;
pub mod armory;
pub mod armor;
pub mod audio;
pub mod camera;
pub mod combat;
pub mod contracts;
pub mod controller;
pub mod deeds;
pub mod doors;
pub mod fx;
pub mod grass;
pub mod aim;
pub mod debug;
pub mod senses;
pub mod skin;
pub mod looks;
pub mod ground;
pub mod ruins;
pub mod horse;
pub mod boss;
pub mod waypoint;
pub mod class;
pub mod wind_aura;
pub mod beam;
pub mod hall;
pub mod hud;
pub mod input;
pub mod insignia;
pub mod interact;
pub mod inventory;
pub mod items;
pub mod magic;
pub mod monitor;
pub mod moveset;
pub mod occupations;
pub mod parkour;
pub mod ragdoll;
pub mod ranged;
pub mod record;
pub mod select;
pub mod sheath;
pub mod skills;
pub mod sky;
pub mod stats;
pub mod talk;
pub mod guild;
pub mod surfaces;
pub mod traverse;
pub mod tools;
pub mod traits;
pub mod trees;
pub mod tuning;
pub mod vfx;
pub mod voxel;
pub mod world;

use crate::motion::{self, Mask, ModelRequest};
use avian3d::prelude::*;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.configure_sets(
        Update,
        (PlaySet::Input, PlaySet::Think, PlaySet::Act, PlaySet::Select).chain(),
    )
    .init_resource::<Feel>()
    .init_resource::<InteractPrompt>()
    .add_message::<Hit>()
    .add_message::<StaminaSpend>()
    .add_plugins((
        world::plugin,
        input::plugin,
        camera::plugin,
        controller::plugin,
        select::plugin,
        moveset::plugin,
        combat::plugin,
        ragdoll::plugin,
        ranged::plugin,
        magic::plugin,
    ))
    .add_plugins((
        traverse::plugin,
        interact::plugin,
        ai::plugin,
        tuning::plugin,
        hud::plugin,
        monitor::plugin,
        parkour::plugin,
        fx::plugin,
        tools::plugin,
        trees::plugin,
        items::plugin,
        inventory::plugin,
        grass::plugin,
        armor::plugin,
        audio::plugin,
    ))
    .add_plugins((stats::plugin, armory::plugin, vfx::plugin, deeds::plugin, skills::plugin, record::plugin, traits::plugin, insignia::plugin, contracts::plugin, occupations::plugin, sky::plugin, hall::plugin, talk::plugin, guild::plugin, beam::plugin))
    .add_plugins((aim::plugin, wind_aura::plugin, debug::plugin, senses::plugin, skin::plugin, looks::plugin, ground::plugin, horse::plugin, boss::plugin, waypoint::plugin, class::plugin))
    .add_plugins((doors::plugin, sheath::plugin,));
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

/// What a character wants this frame. Written by `input` (player) or `ai` (NPCs).
/// Edge flags (`*_pressed`) are true for exactly one frame.
#[derive(Component, Clone, Debug, Default)]
pub struct Intent {
    /// World-space horizontal move direction, length 0..1.
    pub move_dir: Vec3,
    /// World-space point/direction the character attends to (camera aim / AI target).
    pub look: Vec3,
    pub walk: bool,
    pub sprint: bool,
    pub crouch: bool,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub dodge_pressed: bool,
    /// LMB / RB.
    pub primary_pressed: bool,
    pub primary_held: bool,
    pub primary_released: bool,
    /// RMB / LB: block (melee) or aim (ranged/magic).
    pub secondary_held: bool,
    pub secondary_pressed: bool,
    pub interact_pressed: bool,
    /// F with a `talk::Talkable` in focus: talk instead of interacting.
    pub talk_pressed: bool,
    /// Q / d-pad up: the bound art (Battlewill). E / d-pad right: the bound spell (magicules).
    pub art_pressed: bool,
    pub spell_pressed: bool,
    /// MB5 / d-pad left: bind the next known spell to the spell slot.
    pub next_spell: bool,
    /// Lock-on target, if any.
    pub target: Option<Entity>,
    /// X: draw / sheathe the weapon (`sheath`).
    pub sheathe_pressed: bool,
    /// G: command grab (`combat`).
    pub grab_pressed: bool,
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

#[derive(Component, Clone, Debug)]
pub struct Fighter {
    pub team: u8,
    pub hp: f32,
    pub max_hp: f32,
    pub poise: f32,
    pub max_poise: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub dead: bool,
}

impl Fighter {
    pub fn new(team: u8, hp: f32) -> Self {
        Self {
            team,
            hp,
            max_hp: hp,
            poise: 50.0,
            max_poise: 50.0,
            stamina: 100.0,
            max_stamina: 100.0,
            dead: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionKind {
    #[default]
    None,
    Attack,
    Block,
    Parry,
    Stagger,
    Knockdown,
    GetUp,
    Dead,
    Dodge,
    Aim,
    Shoot,
    Reload,
    Cast,
    Channel,
    Interact,
    /// Climbing up onto a ledge or a waist-high obstacle on a scripted path (`traverse`).
    Mantle,
    /// Landing roll that carries momentum (`parkour`).
    Roll,
    /// Heavy landing: deep crouch, brief lock (`parkour`).
    HardLand,
    /// Walking a balance beam (`beam`): steered along the beam, swaying, arms out.
    Balance,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Owner {
    #[default]
    None,
    Move,
    Combat,
    Ranged,
    Magic,
    Interact,
    Traverse,
    Hit,
    /// `ragdoll`'s get-up after physics hands the body back.
    Ragdoll,
}

/// One clip-driven layer requested by an action. `select` converts it to a `motion::Layer`.
#[derive(Clone, Debug)]
pub struct ActionLayer {
    pub track: String,
    /// Seconds in the track.
    pub time: f32,
    pub looping: bool,
    pub mask: Mask,
    pub weight: f32,
    pub style: Option<motion::timing::Style>,
}

impl ActionLayer {
    pub fn new(track: impl Into<String>, time: f32, mask: Mask) -> Self {
        Self {
            track: track.into(),
            time,
            looping: false,
            mask,
            weight: 1.0,
            style: None,
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Action {
    pub kind: ActionKind,
    pub owner: Owner,
    /// Seconds since the action started (owner advances it).
    pub elapsed: f32,
    /// Main clip layer (owner updates `time` every frame).
    pub layer: Option<ActionLayer>,
    /// Optional second layer (e.g. upper-body aim on top of a lower-body step).
    pub overlay: Option<ActionLayer>,
    /// Owner-defined index (attack in chain, spell id, interactable entity bits...).
    pub index: usize,
    /// Buffered input for the next chain step.
    pub buffered: bool,
}

impl Action {
    pub fn is_free(&self) -> bool {
        self.kind == ActionKind::None
    }
    pub fn start(&mut self, kind: ActionKind, owner: Owner, layer: Option<ActionLayer>) {
        *self = Action {
            kind,
            owner,
            layer,
            ..default()
        };
    }
    pub fn clear(&mut self) {
        *self = Action::default();
    }
}

/// Moveset names (see `moveset::Movesets`) a character can switch between; `current` is equipped.
#[derive(Component, Clone, Debug)]
pub struct Loadout {
    pub slots: Vec<String>,
    pub current: usize,
}

impl Loadout {
    pub fn equipped(&self) -> &str {
        self.slots.get(self.current).map_or("unarmed", String::as_str)
    }
}

/// Damage event. Produced by melee hit tests (`combat`), projectiles (`ranged`, `magic`);
/// resolved ONLY by `combat` (block/parry, hp/poise, reactions, knockback, ragdoll trigger).
#[derive(Message, Clone, Debug)]
pub struct Hit {
    pub attacker: Entity,
    pub target: Entity,
    /// World contact point.
    pub point: Vec3,
    /// World direction of the blow (normalized, horizontal-ish).
    pub direction: Vec3,
    pub damage: f32,
    pub poise: f32,
    /// Knockback speed (m/s) before scaling by relative velocity.
    pub knockback: f32,
    /// Closing speed between attacker limb and target (m/s); damage scales with it.
    pub speed: f32,
    pub kind: HitKind,
}

/// Stamina spent by a module that does not own `Fighter` (`traverse`: climbing, swimming
/// fast). `combat`, the only writer of `Fighter.stamina`, applies it; spenders read the
/// stamina back to decide when the character is out (it can't go below 0).
#[derive(Message, Clone, Copy, Debug)]
pub struct StaminaSpend {
    pub who: Entity,
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Melee,
    Projectile,
    Spell,
}

/// Something the player can use with E. Spawned by `world`, driven by `interact`.
#[derive(Component, Clone, Debug)]
pub struct Interactable {
    /// e.g. "chest", "crop", "tree", "well", "table", "chair", "anvil", "lever", "torch".
    pub kind: String,
    pub prompt: String,
    /// Track played while interacting (bank name), its window and loop flag.
    pub track: String,
    pub duration: f32,
    pub looping: bool,
    /// Where the character stands, relative to the interactable, and the yaw it faces.
    pub stand: Vec3,
    pub face_yaw: f32,
    /// Optional hand targets, relative to the interactable, or to its `grip` part when set.
    pub hands: [Option<Vec3>; 2],
    /// Name of a moving child part (lever arm, chest lid) the hands hold on to.
    pub grip: Option<String>,
    pub range: f32,
    /// Tool the user must hold (`tools::Tool::name`, e.g. "axe"); without it E refuses.
    pub requires: Option<String>,
    /// Seconds into the clip where the use lands: one-shots apply their effect there (default
    /// 45 % of the clip), loops land every cycle (`interact::UseBeat`).
    pub beat: Option<f32>,
    /// Clip seconds between which the hand targets hold fully (they ramp in and out around
    /// them); default 35–70 % of a one-shot.
    pub reach: Option<(f32, f32)>,
    /// Stand this far (m) from the root on the side the user comes from, facing it (trees),
    /// instead of at `stand`.
    pub radial: Option<f32>,
}

/// Climbable surface marker (walls, cliffs, ledges). `ledge` = top edge height in world space.
#[derive(Component, Clone, Debug)]
pub struct Climbable {
    pub ledge: f32,
}

/// Water volume (axis-aligned box from its Transform + half extents); surface = top.
#[derive(Component, Clone, Debug)]
pub struct Water {
    pub half_extents: Vec3,
}

/// Text for the HUD's interaction prompt (set by `interact`, read by `hud`).
#[derive(Resource, Default, Clone, Debug)]
pub struct InteractPrompt(pub Option<String>);

/// Gameplay feel knobs (edited live by `tuning`, saved to `profiles/feel.json`).
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
            jump_speed: 5.6,
            gravity: 20.0,
            air_control: 0.35,
            coyote_time: 0.12,
            hitstop: 0.07,
            knockback_scale: 1.0,
            ragdoll_threshold: 7.0,
        }
    }
}

/// Spawn a character root (kinematic capsule) with all gameplay components and request its
/// model. `position` is the SOLE position (feet on ground).
pub fn spawn_character(
    commands: &mut Commands,
    name: &str,
    feet: Vec3,
    yaw: f32,
    fighter: Fighter,
    loadout: Loadout,
    tint: Option<Color>,
) -> Entity {
    commands
        .spawn((
            Name::new(name.to_string()),
            RigidBody::Kinematic,
            Collider::capsule(CAPSULE_RADIUS, CAPSULE_LENGTH),
            CollisionLayers::new(Phys::Character, [Phys::World, Phys::Character, Phys::Projectile]),
            Transform::from_translation(feet + Vec3::Y * HALF_HEIGHT)
                .with_rotation(Quat::from_rotation_y(yaw)),
            Mover {
                yaw,
                ..default()
            },
            Intent::default(),
            Action::default(),
            fighter,
            loadout,
            ModelRequest {
                path: motion::MODEL_GLB.to_string(),
                offset: Vec3::new(0.0, -HALF_HEIGHT, 0.0),
                tint,
            },
        ))
        .id()
}

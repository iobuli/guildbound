//! Weapons ride on the body when they are not in use: a greatsword or an axe across the back
//! (grip over the right shoulder), a katana at the left hip, a dagger at the right hip. Every
//! melee fighter gets a `Sheath`.
//!
//! Drawing: an attack, a guard, an art, a lock-on or X starts `Drawing`: the weapon hand reaches
//! for the grip (`HandTargets`), and at `DRAW_SWAP` (before any attack's active frames) the
//! weapon moves from its mount bone to the hand, where `combat::arm` places it again. Sheathing
//! (X, `SHEATHE_AFTER` seconds out of the fight, walking up to use something (`interact::Using`),
//! or taking up a tool; climbing puts it away at once) runs the other way: the hand
//! carries the weapon back and it is re-parented to the mount at `SHEATHE_SWAP`. A tool in the
//! hand sheathes at once (no reach), and nothing is drawn while one is held.
//!
//! Mounts come from `WeaponDef.carry` (model space at rest), else `default_carry` by moveset
//! name. The mount is fixed on its bone (`spine_03` for "back", the pelvis for "hip"), so the
//! weapon follows the body's motion.
//!
//! Ownership: the weapon's parent and transform while `!in_hand()` (here; `combat::arm` while in
//! hand); the weapon hand's `HandTargets` during a reach (only when no other owner holds one).
use super::combat::{Weapon, WeaponSlot};
use super::moveset::{Carry, Kind, Movesets, WeaponDef};
use super::select::HandTargets;
use super::tools::Holding;
use super::{Action, Fighter, Intent, Loadout, PlaySet};
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;

/// Seconds without attacking, guarding or a lock-on before a fighter sheathes.
pub const SHEATHE_AFTER: f32 = 8.0;
/// Draw: total time and the moment the weapon changes hands (s).
pub const DRAW_TIME: f32 = 0.42;
pub const DRAW_SWAP: f32 = 0.2;
/// Sheathe: total time and the moment the weapon reaches its mount (s).
pub const SHEATHE_TIME: f32 = 0.6;
pub const SHEATHE_SWAP: f32 = 0.36;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Drawn,
    Drawing,
    Sheathing,
    Sheathed,
}

#[derive(Component, Clone, Debug, Default)]
pub struct Sheath {
    pub state: State,
    /// Seconds into the current draw / sheathe; seconds idle while drawn.
    t: f32,
    idle: f32,
    /// The moveset this state belongs to (a new weapon starts drawn).
    set: String,
    /// This module inserted the `HandTargets` for the reach.
    reaching: bool,
}

impl Sheath {
    /// Whether the weapon belongs in the hand right now.
    pub fn in_hand(&self) -> bool {
        match self.state {
            State::Drawn => true,
            State::Sheathed => false,
            State::Drawing => self.t >= DRAW_SWAP,
            State::Sheathing => self.t < SHEATHE_SWAP,
        }
    }
}

/// `sheath`'s systems (after `combat`); order against this.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SheathSet;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, Sheath>()
        .add_systems(Update, (decide, mount).chain().in_set(PlaySet::Act).in_set(SheathSet).after(super::combat::CombatSet));
}

/// Default mount for a moveset without `carry`.
pub fn default_carry(set: &str) -> Option<Carry> {
    let c = |at: &str, grip: [f32; 3], blade: [f32; 3], face: [f32; 3]| Some(Carry { at: at.into(), grip, blade, face });
    match set {
        // Across the back: grip above the right shoulder, the blade down to the left hip.
        "greatsword" => c("back", [-0.2, 1.52, -0.17], [0.42, -0.9, 0.0], [0.0, 0.0, -1.0]),
        "axe" => c("back", [-0.18, 1.45, -0.18], [0.35, -0.93, 0.0], [0.0, 0.0, -1.0]),
        // Left hip, grip forward for the right hand's cross draw, blade down and back.
        "katana" => c("hip", [0.16, 0.98, 0.12], [0.06, -0.32, -0.95], [1.0, 0.0, 0.0]),
        // Right hip, point down.
        "dagger" => c("hip", [-0.17, 0.95, 0.04], [0.0, -0.85, -0.52], [-1.0, 0.0, 0.0]),
        _ => None,
    }
}

/// The weapon root's model-space transform for `carry` (scale = the weapon's own).
pub fn carry_transform(carry: &Carry, scale: f32) -> Transform {
    let y = Vec3::from(carry.blade).normalize_or(Vec3::NEG_Y);
    let face = Vec3::from(carry.face);
    let x = y.cross(face).normalize_or(y.any_orthonormal_vector());
    let z = x.cross(y);
    Transform {
        translation: Vec3::from(carry.grip),
        rotation: Quat::from_mat3(&Mat3::from_cols(x, y, z)),
        scale: Vec3::splat(scale),
    }
}

/// The mount under a bone at `bone_rest` (model space, rest pose) for a weapon placed at
/// `model` (model space): its local transform under that bone.
pub fn under_bone(bone_rest: &Transform, model: &Transform) -> Transform {
    Transform::from_matrix((bone_rest.compute_affine().inverse() * model.compute_affine()).into())
}

fn carry_for(def: &WeaponDef, set: &str) -> Option<Carry> {
    def.carry.clone().or_else(|| default_carry(set))
}

/// State changes from intent.
#[allow(clippy::type_complexity)]
fn decide(
    time: Res<Time>,
    movesets: Res<Movesets>,
    mut fighters: Query<(&mut Sheath, &Intent, &Action, &Loadout, Has<Holding>, Option<&WeaponSlot>, Has<super::interact::Using>, Option<&super::Mover>)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (mut sheath, intent, action, loadout, holding, slot, using, mover) in &mut fighters {
        // Both hands are on the wall: the blade goes home at once, as for a tool.
        let climbing = mover.is_some_and(|m| matches!(m.mode, super::MoveMode::Climb | super::MoveMode::Ledge));
        let holding = holding || climbing;
        let set = loadout.equipped();
        let melee = movesets
            .get(set)
            .is_some_and(|m| m.kind == Kind::Melee && m.weapon.as_ref().is_some_and(|d| carry_for(d, set).is_some()));
        if !melee || slot.is_none() {
            if sheath.state != State::Drawn || sheath.set != set {
                *sheath = Sheath { set: set.to_string(), ..default() };
            }
            continue;
        }
        if sheath.set != set {
            *sheath = Sheath { set: set.to_string(), ..default() };
        }
        let wants_fight = intent.primary_pressed
            || intent.primary_held
            || intent.secondary_held
            || intent.secondary_pressed
            || intent.art_pressed
            || intent.grab_pressed
            || intent.target.is_some();
        sheath.t += dt;
        match sheath.state {
            State::Drawn => {
                sheath.idle = if wants_fight || !action.is_free() { 0.0 } else { sheath.idle + dt };
                if holding {
                    sheath.state = State::Sheathed;
                } else if (intent.sheathe_pressed && action.is_free()) || sheath.idle > SHEATHE_AFTER || using {
                    sheath.state = State::Sheathing;
                    sheath.t = 0.0;
                }
            }
            State::Sheathed => {
                if !holding && !using && (wants_fight || intent.sheathe_pressed) {
                    sheath.state = State::Drawing;
                    sheath.t = 0.0;
                }
            }
            State::Drawing => {
                if sheath.t >= DRAW_TIME {
                    sheath.state = State::Drawn;
                    sheath.idle = 0.0;
                }
            }
            State::Sheathing => {
                if holding {
                    sheath.state = State::Sheathed;
                } else if wants_fight && !using && sheath.t < SHEATHE_SWAP {
                    // Changed its mind before the blade went home.
                    sheath.state = State::Drawn;
                    sheath.idle = 0.0;
                } else if sheath.t >= SHEATHE_TIME {
                    sheath.state = State::Sheathed;
                }
            }
        }
    }
}

/// Parent the weapon to its mount (or back to the hand) and drive the reach.
#[allow(clippy::type_complexity)]
fn mount(
    mut commands: Commands,
    movesets: Res<Movesets>,
    bank: Option<Res<PoseBank>>,
    mut rest: Local<Option<(usize, Vec<Transform>)>>,
    mut fighters: Query<(Entity, &mut Sheath, &Loadout, &Rig, &WeaponSlot, Option<&HandTargets>, Has<Holding>)>,
    weapons: Query<&ChildOf, With<Weapon>>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    let skeleton = &bank.skeleton;
    if rest.as_ref().is_none_or(|r| r.0 != skeleton.len()) {
        *rest = Some((skeleton.len(), skeleton.fk(&skeleton.rest_pose(), bank.bones.pelvis)));
    }
    let rest = &rest.as_ref().unwrap().1;
    for (entity, mut sheath, loadout, rig, slot, hands, holding) in &mut fighters {
        let set = loadout.equipped();
        let Some(def) = movesets.get(set).and_then(|m| m.weapon.as_ref()) else { continue };
        let Some(carry) = carry_for(def, set) else { continue };
        let bone_index = if carry.at == "back" { bank.bones.spine[2] } else { bank.bones.pelvis };
        let side = usize::from(def.hand != "l");
        let bone = rig.bones.get(bone_index).copied().filter(|b| *b != Entity::PLACEHOLDER);
        let hand = rig.bones.get(bank.bones.hand[side]).copied().filter(|b| *b != Entity::PLACEHOLDER);
        let (Some(bone), Some(hand)) = (bone, hand) else { continue };
        let local = under_bone(&rest[bone_index], &carry_transform(&carry, def.scale.max(0.01)));
        let parent = weapons.get(slot.weapon).ok().map(|c| c.parent());

        if sheath.in_hand() {
            if parent != Some(hand) {
                let scale = globals.get(hand).map_or(1.0, |g| g.compute_transform().scale.abs().max_element());
                commands.entity(slot.weapon).insert((ChildOf(hand), super::combat::grip(def, scale)));
            }
        } else {
            if parent != Some(bone) {
                commands.entity(slot.weapon).insert(ChildOf(bone));
            }
            commands.entity(slot.weapon).insert((local, Visibility::Inherited));
        }

        // The reach: the weapon hand goes to the mount's grip and back.
        let reach = match sheath.state {
            State::Drawing => Some(bump(sheath.t, DRAW_SWAP, DRAW_TIME)),
            State::Sheathing => Some(bump(sheath.t, SHEATHE_SWAP, SHEATHE_TIME)),
            _ => None,
        };
        match reach {
            Some(w) if !holding && (sheath.reaching || hands.is_none()) => {
                if let Ok(g) = globals.get(bone) {
                    let target = g.transform_point(local.translation);
                    let mut targets = [None, None];
                    targets[side] = Some((target, w));
                    commands.entity(entity).insert(HandTargets { hands: targets, ..default() });
                    sheath.reaching = true;
                }
            }
            _ => {
                if sheath.reaching {
                    commands.entity(entity).remove::<HandTargets>();
                    sheath.reaching = false;
                }
            }
        }
    }
}

/// Hand weight for a reach peaking at `swap` within `total` seconds.
fn bump(t: f32, swap: f32, total: f32) -> f32 {
    let s = |u: f32| u.clamp(0.0, 1.0) * u.clamp(0.0, 1.0) * (3.0 - 2.0 * u.clamp(0.0, 1.0));
    if t < swap { s(t / swap.max(1e-3)) } else { 1.0 - s((t - swap) / (total - swap).max(1e-3)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mounts_point_the_blade_where_it_belongs() {
        let back = carry_transform(&default_carry("greatsword").unwrap(), 1.0);
        let blade = back.rotation * Vec3::Y;
        assert!(blade.y < -0.8 && blade.x > 0.3, "greatsword hangs down to the left hip: {blade}");
        assert!(back.translation.z < -0.1, "on the back");
        let hip = carry_transform(&default_carry("katana").unwrap(), 1.0);
        assert!(hip.translation.x > 0.1, "katana on the left hip");
        assert!((hip.rotation * Vec3::Y).z < -0.8, "katana points back");
        assert!(default_carry("unarmed").is_none());
    }

    #[test]
    fn a_mount_under_a_bone_lands_back_on_the_model_transform() {
        let bone = Transform::from_xyz(0.0, 1.35, -0.02).with_rotation(Quat::from_rotation_z(0.4)).with_scale(Vec3::splat(0.01));
        let want = carry_transform(&default_carry("axe").unwrap(), 1.0);
        let got = bone.mul_transform(under_bone(&bone, &want));
        assert!(got.translation.distance(want.translation) < 1e-4);
        assert!(got.rotation.angle_between(want.rotation) < 1e-3);
        assert!((got.scale - Vec3::ONE).abs().max_element() < 1e-4);
    }

    #[test]
    fn the_weapon_changes_hands_at_the_swap() {
        let mut s = Sheath { state: State::Drawing, ..default() };
        assert!(!s.in_hand());
        s.t = DRAW_SWAP;
        assert!(s.in_hand());
        s = Sheath { state: State::Sheathing, ..default() };
        assert!(s.in_hand());
        s.t = SHEATHE_SWAP;
        assert!(!s.in_hand());
        assert!(DRAW_SWAP < 0.3, "drawn before the quickest attack's active frames");
        assert!((bump(DRAW_SWAP, DRAW_SWAP, DRAW_TIME) - 1.0).abs() < 1e-6);
        assert!(bump(DRAW_TIME, DRAW_SWAP, DRAW_TIME) < 1e-6);
    }
}

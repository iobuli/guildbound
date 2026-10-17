//! Hand tools (the axe). Owns `Holding` on characters, `ToolItem` on the tool and where the
//! tool sits: on its home interactable (the table) or under a hand bone.
//!
//! Picking up (`interact::UseEffect` on the table, at the clip's reach): `ual1/PickUp_Table`
//! reaches with the LEFT hand while the right shoulder pulls back out of range, so the left hand
//! takes the axe by the middle of the haft, the clip brings it across to the right side of the
//! chest, and the right hand reaches up (a hand target added to interact's `HandTargets`) and
//! takes the end of the haft. Putting down reverses the table step. Every change of parent keeps
//! the axe where it is in the world and then settles it into place over `SETTLE_TIME`, so it
//! never jumps between hands.
//!
//! Grips follow the clips: in idle / walk / run the thumb axis of the hand points forward and up,
//! so the carry grip holds the haft along it (head forward, edge down along the knuckles).
//! `ual2/TreeChopping_Loop` swings the right hand like a knife-hand chop leading with the pinky
//! edge, so the chop grip lays the haft along the fingers (tipped toward the thumb) with the edge
//! on the pinky side. The two blend over `GRIP_TIME` when a chop starts or ends.
use super::fx::{Sfx, SfxKind};
use super::interact::{Phase, PropState, UseEffect, Using};
use super::select::HandTargets;
use super::{Interactable, PlaySet};
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Axe,
}

impl Tool {
    /// Name used by `Interactable.requires`.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Axe => "axe",
        }
    }
}

/// On a character: the tool it holds and in which hand (0 left, 1 right).
#[derive(Component, Clone, Copy, Debug)]
pub struct Holding {
    pub tool: Tool,
    pub item: Entity,
    pub hand: usize,
    /// 0 = carry grip, 1 = chop grip.
    grip: f32,
}

/// On a tool entity: what it is, the interactable it rests on, its local transform there and
/// who holds it.
#[derive(Component, Clone, Copy, Debug)]
pub struct ToolItem {
    pub tool: Tool,
    pub home: Entity,
    pub rest: Transform,
    holder: Option<Entity>,
}

impl ToolItem {
    pub fn new(tool: Tool, home: Entity, rest: Transform) -> Self {
        Self {
            tool,
            home,
            rest,
            holder: None,
        }
    }
}

/// A tool just changed parent: ease from `from` (its local transform under the new parent that
/// kept it still in the world) to where it belongs.
#[derive(Component, Clone, Copy, Debug)]
struct Settle {
    from: Transform,
    t: f32,
}

/// Woodcutting size of the 1.2 m `battleaxe.glb`.
pub const AXE_SCALE: f32 = 0.75;
/// Mesh-space points (the axe's frame: head toward −Z, edge toward −X): where the right fist
/// holds the haft (near the end), where the left fist holds it (mid-haft, so the right hand can
/// take the end), and the middle of the cutting edge.
pub const AXE_GRIP: Vec3 = Vec3::new(0.0, 0.0, 0.17);
const AXE_GRIP_LEFT: Vec3 = Vec3::new(0.0, 0.0, -0.12);
const AXE_EDGE: Vec3 = Vec3::new(-0.27, 0.0, -0.8);
/// Fist centre in `hand_r` space (fingers +Y, thumb side +Z, palm −X), m. The left hand is the
/// mirror (palm +X).
const FIST: Vec3 = Vec3::new(-0.025, 0.075, 0.0);
/// Chop grip: haft along the fingers tipped this far (rad) toward the thumb side.
const CHOP_TILT: f32 = 30.0 * std::f32::consts::PI / 180.0;
/// Carry ↔ chop grip blend time and parent-change settle time (s).
const GRIP_TIME: f32 = 0.15;
const SETTLE_TIME: f32 = 0.22;
/// Pickup hand-over, clip seconds: the right hand reaches the haft fully over `HANDOVER`
/// (ramping `HANDOVER_RAMP` around it) and takes the axe at `HANDOVER_AT`.
const HANDOVER: (f32, f32) = (0.44, 0.54);
const HANDOVER_RAMP: f32 = 0.1;
const HANDOVER_AT: f32 = 0.5;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (take_and_return, hand_over, hold, place_items)
            .chain()
            .in_set(PlaySet::Act)
            .after(super::interact::drive),
    );
}

/// Carry grip: mesh −Z (head) → hand +Z (thumb side), mesh −X (edge) → hand +Y.
fn carry_rotation() -> Quat {
    Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_Y, Vec3::NEG_X, Vec3::NEG_Z))
}

/// Chop grip: mesh −Z (head) → hand +Y (fingers), mesh −X (edge) → hand −Z (pinky side), then
/// tipped toward the thumb.
fn chop_rotation() -> Quat {
    Quat::from_rotation_x(CHOP_TILT) * Quat::from_mat3(&Mat3::from_cols(Vec3::Z, Vec3::NEG_X, Vec3::NEG_Y))
}

/// The axe's transform under hand bone `hand` (0 left, 1 right) for grip blend `g` (0 carry,
/// 1 chop; the left hand only carries), given the bone's global scale: the grip point sits in
/// the fist.
pub fn grip_transform(g: f32, hand_scale: f32, hand: usize) -> Transform {
    let g = if g.is_finite() && hand == 1 { g.clamp(0.0, 1.0) } else { 0.0 };
    let s = if hand_scale.is_finite() && hand_scale > 1e-3 { hand_scale } else { 1.0 };
    let rotation = carry_rotation().slerp(chop_rotation(), g).normalize();
    let (fist, grip) = if hand == 0 {
        (FIST * Vec3::new(-1.0, 1.0, 1.0), AXE_GRIP_LEFT)
    } else {
        (FIST, AXE_GRIP)
    };
    Transform {
        translation: (fist - rotation * (grip * AXE_SCALE)) / s,
        rotation,
        scale: Vec3::splat(AXE_SCALE / s),
    }
}

/// World point of the axe's cutting edge.
pub fn blade(axe: &GlobalTransform) -> Vec3 {
    axe.transform_point(AXE_EDGE)
}

/// The hand bone entity (0 left, 1 right) of a bound rig.
fn hand_bone(rig: &Rig, bank: &PoseBank, hand: usize) -> Option<Entity> {
    let bone = *bank.bones.hand.get(hand)?;
    rig.bones.get(bone).copied().filter(|e| *e != Entity::PLACEHOLDER)
}

/// Local transform under `parent` that keeps `item` where it is in the world.
fn kept(item: Option<&GlobalTransform>, parent: Option<&GlobalTransform>) -> Option<Transform> {
    let local = parent?.affine().inverse() * item?.affine();
    let t = Transform::from_matrix(local.into());
    (t.translation.is_finite() && t.rotation.is_finite() && t.scale.is_finite()).then_some(t)
}

/// Re-parent `item` under `parent` without moving it, then let it settle.
fn reparent(commands: &mut Commands, item: Entity, parent: Entity, globals: &Query<&GlobalTransform>) {
    let mut e = commands.entity(item);
    e.insert(ChildOf(parent));
    if let Some(from) = kept(globals.get(item).ok(), globals.get(parent).ok()) {
        e.insert((from, Settle { from, t: 0.0 }));
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn take_and_return(
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    mut effects: MessageReader<UseEffect>,
    mut sounds: MessageWriter<Sfx>,
    users: Query<(&Rig, Option<&Holding>, &Transform)>,
    mut homes: Query<(&Interactable, &mut PropState)>,
    mut items: Query<(Entity, &mut ToolItem)>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else {
        effects.clear();
        return;
    };
    for effect in effects.read() {
        let Ok((it, mut prop)) = homes.get_mut(effect.target) else {
            continue;
        };
        if it.kind != "table" {
            continue;
        }
        let Ok((rig, holding, tf)) = users.get(effect.user) else {
            continue;
        };
        match holding {
            None if !prop.on => {
                let Some(hand) = hand_bone(rig, &bank, 0) else { continue };
                let Some((item, mut tool)) = items.iter_mut().find(|(_, t)| t.home == effect.target) else {
                    continue;
                };
                reparent(&mut commands, item, hand, &globals);
                tool.holder = Some(effect.user);
                commands.entity(effect.user).insert(Holding {
                    tool: tool.tool,
                    item,
                    hand: 0,
                    grip: 0.0,
                });
                prop.on = true;
                sounds.write(Sfx {
                    kind: SfxKind::PickUp,
                    at: tf.translation,
                    volume: 0.7,
                });
            }
            Some(held) if prop.on => {
                let Ok((_, mut tool)) = items.get_mut(held.item) else { continue };
                reparent(&mut commands, held.item, tool.home, &globals);
                tool.holder = None;
                commands.entity(effect.user).remove::<Holding>();
                prop.on = false;
                sounds.write(Sfx {
                    kind: SfxKind::PutDown,
                    at: tf.translation,
                    volume: 0.7,
                });
            }
            _ => {}
        }
    }
}

/// Window weight: 0 outside, 1 between `from` and `until`, smooth ramps of `ramp` around.
fn window(t: f32, from: f32, until: f32, ramp: f32) -> f32 {
    let smooth = |a: f32, b: f32, x: f32| {
        let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    smooth(from - ramp, from, t) * (1.0 - smooth(until, until + ramp, t))
}

/// Left hand → right hand. During the table pickup the right hand reaches for the end of the
/// haft and takes it at `HANDOVER_AT`; any other use that needs the tool (a chop) takes it at
/// once.
#[allow(clippy::type_complexity)]
fn hand_over(
    mut commands: Commands,
    bank: Option<Res<PoseBank>>,
    mut users: Query<(&Rig, &mut Holding, Option<&Using>, Option<&mut HandTargets>)>,
    targets: Query<&Interactable>,
    globals: Query<&GlobalTransform>,
) {
    let Some(bank) = bank else { return };
    for (rig, mut holding, using, hands) in &mut users {
        if holding.hand != 0 {
            continue;
        }
        let kind = using.and_then(|u| targets.get(u.target).ok()).map(|it| it.kind.as_str());
        let at_table = kind == Some("table") && using.is_some_and(|u| u.phase == Phase::Use);
        let t = using.map_or(0.0, |u| u.t);
        if at_table && t < HANDOVER_AT {
            if let Some(mut hands) = hands
                && let Ok(axe) = globals.get(holding.item)
            {
                let w = window(t, HANDOVER.0, HANDOVER.1, HANDOVER_RAMP);
                hands.hands[1] = Some((axe.transform_point(AXE_GRIP), w));
            }
            continue;
        }
        if !(at_table || kind == Some("tree")) {
            continue;
        }
        let Some(right) = hand_bone(rig, &bank, 1) else { continue };
        reparent(&mut commands, holding.item, right, &globals);
        holding.hand = 1;
    }
}

/// Blend each held tool between the carry and chop grips.
fn hold(time: Res<Time>, mut holders: Query<(&mut Holding, Option<&Using>)>, targets: Query<&Interactable>) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (mut holding, using) in &mut holders {
        let chopping = using.is_some_and(|u| {
            u.phase == Phase::Use && targets.get(u.target).is_ok_and(|it| it.kind == "tree")
        });
        let target = if chopping { 1.0 } else { 0.0 };
        let step = dt / GRIP_TIME;
        holding.grip = (holding.grip + (target - holding.grip).clamp(-step, step)).clamp(0.0, 1.0);
    }
}

/// Place every tool: in its holder's grip or at rest on its home, easing out of a parent change.
#[allow(clippy::type_complexity)]
fn place_items(
    mut commands: Commands,
    time: Res<Time>,
    holders: Query<&Holding>,
    mut items: Query<(Entity, &ToolItem, &mut Transform, &ChildOf, Option<&mut Settle>)>,
    globals: Query<&GlobalTransform, Without<ToolItem>>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, tool, mut tf, parent, settle) in &mut items {
        let target = match tool.holder.and_then(|h| holders.get(h).ok()) {
            Some(holding) => {
                let scale = globals
                    .get(parent.parent())
                    .map_or(1.0, |g| g.compute_transform().scale.abs().max_element());
                let g = holding.grip;
                grip_transform(g * g * (3.0 - 2.0 * g), scale, holding.hand)
            }
            None => tool.rest,
        };
        *tf = match settle {
            Some(mut s) => {
                s.t += dt / SETTLE_TIME;
                if s.t >= 1.0 {
                    commands.entity(entity).remove::<Settle>();
                    target
                } else {
                    let u = s.t * s.t * (3.0 - 2.0 * s.t);
                    Transform {
                        translation: s.from.translation.lerp(target.translation, u),
                        rotation: s.from.rotation.slerp(target.rotation, u),
                        scale: s.from.scale.lerp(target.scale, u),
                    }
                }
            }
            None => target,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grips_hold_the_haft_in_the_fist() {
        for g in [0.0, 0.5, 1.0] {
            let t = grip_transform(g, 1.0, 1);
            assert!(t.transform_point(AXE_GRIP).abs_diff_eq(FIST, 1e-5), "grip {g}");
            assert!((t.rotation.length() - 1.0).abs() < 1e-5);
        }
        let left = grip_transform(0.0, 1.0, 0);
        assert!(left.transform_point(AXE_GRIP_LEFT).abs_diff_eq(Vec3::new(0.025, 0.075, 0.0), 1e-5));
        let head = |g: f32| {
            let t = grip_transform(g, 1.0, 1);
            (t.transform_point(Vec3::new(0.0, 0.0, -0.8)) - t.transform_point(AXE_GRIP)).normalize()
        };
        assert!(head(0.0).abs_diff_eq(Vec3::Z, 1e-5), "carry: head out of the thumb side");
        let tilt = Vec3::new(0.0, CHOP_TILT.cos(), CHOP_TILT.sin());
        assert!(head(1.0).abs_diff_eq(tilt, 1e-5), "chop: head along the fingers");
        // Edge leads: toward the knuckles when carrying, the pinky side when chopping.
        let edge = |g: f32| grip_transform(g, 1.0, 1).rotation * Vec3::NEG_X;
        assert!(edge(0.0).abs_diff_eq(Vec3::Y, 1e-5));
        assert!(edge(1.0).dot(Vec3::NEG_Z) > 0.85);
        // The left hand only carries; a scaled bone gets a compensating local transform.
        assert!(grip_transform(1.0, 1.0, 0).rotation.abs_diff_eq(carry_rotation(), 1e-5));
        assert!((grip_transform(0.0, 2.0, 1).scale.x - AXE_SCALE / 2.0).abs() < 1e-6);
    }

    #[test]
    fn kept_transform_leaves_the_item_in_place() {
        let parent = GlobalTransform::from(
            Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_y(0.8)),
        );
        let item = GlobalTransform::from(
            Transform::from_xyz(-0.5, 1.0, 0.2)
                .with_rotation(Quat::from_rotation_x(0.3))
                .with_scale(Vec3::splat(AXE_SCALE)),
        );
        let local = kept(Some(&item), Some(&parent)).unwrap();
        let back = parent.mul_transform(local);
        assert!(back.translation().abs_diff_eq(item.translation(), 1e-5));
        assert!(back.rotation().abs_diff_eq(item.rotation(), 1e-5));
        assert!(kept(None, Some(&parent)).is_none());
    }

    #[test]
    fn handover_window() {
        assert_eq!(window(0.2, HANDOVER.0, HANDOVER.1, HANDOVER_RAMP), 0.0);
        assert_eq!(window(0.48, HANDOVER.0, HANDOVER.1, HANDOVER_RAMP), 1.0);
        assert!(HANDOVER_AT > HANDOVER.0 && HANDOVER_AT <= HANDOVER.1, "take it while the hand is there");
    }
}

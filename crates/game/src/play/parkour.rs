//! Parkour landings (`Update / PlaySet::Act`, actions owned by `Owner::Move`).
//! Every touchdown is graded by the downward speed (`Mover.impact`) and the horizontal speed:
//! - soft: nothing here; `select` plays the Jump_Land overlay weighted by `Mover.landing`;
//! - roll: hard enough while moving fast → `ActionKind::Roll`: the roll clip from its tuck,
//!   root motion carrying the landing speed along the travel direction and easing toward run
//!   speed; input is locked until the get-up, where jump / dodge / attack cancel and the stick
//!   takes over smoothly;
//! - hard: heavy while slow → `ActionKind::HardLand`: a deep landing crouch, locked for a time
//!   that grows with the impact; jump cancels its second half.
//! Rolls and hard landings are `land` deeds (tagged `Roll` / `Hard`; Breakfall).
//! Every landing above `LOG_IMPACT` is logged to `staging/relations.log`. The constants are
//! body-mechanics defaults until the reference breakdown (`docs/reference/parkour_jump.md`)
//! calibrates them.
use super::controller::yaw_of;
use super::deeds::Deed;
use super::monitor::Monitor;
use super::{Action, ActionKind, ActionLayer, Feel, Intent, MoveMode, Mover, Owner, PlaySet};
use crate::motion::Mask;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;

const ROLL: &str = "ual1/Roll";
const HARD: &str = "ual2/NinjaJump_Land";
/// Roll from this downward speed (m/s, about a 1.2 m drop) when moving at least `ROLL_SPEED`.
const ROLL_IMPACT: f32 = 7.0;
const ROLL_SPEED: f32 = 3.0;
/// Hard landing from this downward speed (m/s, about a 2 m drop) when too slow to roll.
const HARD_IMPACT: f32 = 9.0;
/// Roll clip: start (s, past the dive, at the tuck), playback rate, get-up from this share.
const ROLL_OFFSET: f32 = 0.3;
const ROLL_RATE: f32 = 1.6;
const ROLL_GET_UP: f32 = 0.7;
/// Speed the roll eases toward by its end, as a share of `Feel.run_speed`.
const ROLL_EXIT: f32 = 0.85;
/// Hard landing: clip start (s), rate, lock (s) at `HARD_IMPACT` plus per extra m/s, cap.
const HARD_OFFSET: f32 = 0.0;
const HARD_RATE: f32 = 1.0;
const HARD_LOCK: f32 = 0.35;
const HARD_LOCK_PER: f32 = 0.04;
const HARD_LOCK_MAX: f32 = 0.8;
/// Share of a hard landing after which jump cancels it.
const HARD_CANCEL: f32 = 0.5;
const LOG_IMPACT: f32 = 2.0;

pub fn plugin(app: &mut App) {
    app.register_required_components::<Mover, Landing>()
        .add_systems(Update, land.in_set(PlaySet::Act));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    Soft,
    Roll,
    Hard,
}

/// Landing grade from the downward speed at touchdown and the horizontal speed (m/s).
pub fn grade(impact: f32, speed: f32) -> Grade {
    if !(impact.is_finite() && speed.is_finite()) {
        Grade::Soft
    } else if impact >= ROLL_IMPACT && speed >= ROLL_SPEED {
        Grade::Roll
    } else if impact >= HARD_IMPACT {
        Grade::Hard
    } else {
        Grade::Soft
    }
}

/// Per-character landing state.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Landing {
    airborne: bool,
    /// Active roll: travel direction and entry speed. Both kinds: duration (s).
    dir: Vec3,
    speed: f32,
    duration: f32,
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Hand the body back to the player / AI.
fn release(mover: &mut Mover, action: &mut Action) {
    action.clear();
    mover.move_scale = 1.0;
    mover.face = None;
    mover.root_motion = Vec3::ZERO;
}

fn ours(action: &Action) -> bool {
    action.owner == Owner::Move && matches!(action.kind, ActionKind::Roll | ActionKind::HardLand)
}

#[allow(clippy::type_complexity)]
fn land(
    time: Res<Time>,
    feel: Res<Feel>,
    bank: Res<PoseBank>,
    mut monitor: ResMut<Monitor>,
    mut deeds: MessageWriter<Deed>,
    mut characters: Query<(Entity, Option<&Name>, &mut Mover, &Intent, &mut Action, &mut Landing)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let now = time.elapsed_secs();
    for (entity, name, mut mover, intent, mut action, mut st) in &mut characters {
        let touchdown = st.airborne && mover.mode == MoveMode::Ground;
        st.airborne = mover.mode == MoveMode::Air;

        if touchdown && (action.is_free() || ours(&action)) {
            let velocity = mover.velocity.with_y(0.0);
            let speed = if velocity.is_finite() { velocity.length() } else { 0.0 };
            let impact = mover.impact;
            let g = grade(impact, speed);
            if impact >= LOG_IMPACT {
                let who = name.map_or("?", |n| n.as_str());
                let takeoff = mover.takeoff_speed;
                monitor.write(&format!(
                    "{now:.2} {who} land_{g:?} - {impact:.3} Ground takeoff={takeoff:.2} {speed:.2}"
                ));
            }
            if g != Grade::Soft {
                deeds.write(Deed::new(entity, "land", format!("{g:?}"), 1.0));
            }
            match g {
                Grade::Roll if bank.has(ROLL) => {
                    st.dir = velocity.normalize_or(mover.forward());
                    st.speed = speed;
                    st.duration = ((bank.duration(ROLL) - ROLL_OFFSET) / ROLL_RATE).max(0.2);
                    action.start(ActionKind::Roll, Owner::Move, Some(ActionLayer::new(ROLL, ROLL_OFFSET, Mask::Full)));
                }
                Grade::Hard if bank.has(HARD) => {
                    st.dir = Vec3::ZERO;
                    st.speed = 0.0;
                    st.duration = (HARD_LOCK + (impact - HARD_IMPACT).max(0.0) * HARD_LOCK_PER).min(HARD_LOCK_MAX);
                    action.start(ActionKind::HardLand, Owner::Move, Some(ActionLayer::new(HARD, HARD_OFFSET, Mask::Full)));
                }
                _ => {}
            }
        }

        if !ours(&action) {
            continue;
        }
        if mover.mode != MoveMode::Ground {
            // Rolled off an edge: let the air take over.
            release(&mut mover, &mut action);
            continue;
        }
        action.elapsed += dt;
        let u = action.elapsed / st.duration.max(1e-3);
        let elapsed = action.elapsed;
        match action.kind {
            ActionKind::Roll => {
                if let Some(layer) = &mut action.layer {
                    layer.time = ROLL_OFFSET + elapsed * ROLL_RATE;
                }
                let exit = feel.run_speed.max(0.0) * ROLL_EXIT;
                let speed = st.speed + (exit - st.speed) * smooth01(u);
                // Get-up: the stick takes over while the carried speed fades out.
                let hand = smooth01((u - ROLL_GET_UP) / (1.0 - ROLL_GET_UP));
                mover.move_scale = hand;
                mover.root_motion = st.dir * speed * (1.0 - hand);
                mover.face = if hand < 0.5 { yaw_of(st.dir) } else { None };
                let cancel = u >= ROLL_GET_UP && (intent.jump_pressed || intent.dodge_pressed || intent.primary_pressed);
                if cancel || u >= 1.0 {
                    release(&mut mover, &mut action);
                }
            }
            ActionKind::HardLand => {
                if let Some(layer) = &mut action.layer {
                    layer.time = HARD_OFFSET + elapsed * HARD_RATE;
                }
                mover.move_scale = 0.0;
                mover.root_motion = Vec3::ZERO;
                if (u >= HARD_CANCEL && intent.jump_pressed) || u >= 1.0 {
                    release(&mut mover, &mut action);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades() {
        assert_eq!(grade(5.6, 6.8), Grade::Soft, "a jump on flat ground");
        assert_eq!(grade(7.7, 4.2), Grade::Roll, "running off the 1.5 m platform");
        assert_eq!(grade(7.7, 0.5), Grade::Soft);
        assert_eq!(grade(9.6, 0.5), Grade::Hard, "jumping down from the platform while still");
        assert_eq!(grade(f32::NAN, 5.0), Grade::Soft);
    }
}

//! Duel instructor and quick switching between the existing sword and Sparking libraries.
use super::combat::Equip;
use super::hud::log::{HudLog, LogKind};
use super::magic::{Energy, Recovery};
use super::talk::{Choice, Chose, Dialogue, Role, TalkStarted, Talking};
use super::{Action, Fighter, Intent, Loadout, Mover, Player, PlaySet};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, switch_style.in_set(PlaySet::Input))
        .add_systems(Update, face_player.in_set(PlaySet::Think))
        .add_systems(Update, respond.after(super::guild::respond));
}

fn face_player(
    player: Query<(Entity, &Transform), With<Player>>,
    mut trainers: Query<(&super::talk::Talkable, &Transform, &mut Mover, &mut Intent), Without<Player>>,
) {
    let Ok((entity, player)) = player.single() else { return };
    for (talkable, tf, mut mover, mut intent) in &mut trainers {
        if talkable.role != Role::Trainer { continue; }
        let to = player.translation - tf.translation;
        if to.length_squared() < 64.0 {
            mover.face = super::controller::yaw_of(to);
            intent.target = Some(entity);
            intent.look = player.translation + Vec3::Y * 0.5;
        } else {
            mover.face = None;
            intent.target = None;
        }
    }
}

fn switch_style(
    keys: Res<ButtonInput<KeyCode>>,
    talking: Res<Talking>,
    pause: Res<super::pause::Paused>,
    players: Query<(Entity, &Loadout, &Action), With<Player>>,
    mut equip: MessageWriter<Equip>,
    mut log: MessageWriter<HudLog>,
) {
    if !keys.just_pressed(KeyCode::KeyZ) || talking.active() || pause.0 { return; }
    let Ok((who, loadout, action)) = players.single() else { return };
    if !action.is_free() { return; }
    let set = if loadout.slots.get(loadout.current).is_some_and(|s| s == "katana") { "sparking" } else { "katana" };
    equip.write(Equip { who, set: set.to_string() });
    log.write(HudLog::new(LogKind::System, format!("Style: {set} | Z switches sword / fists")).key(7010));
}

fn menu() -> Vec<Choice> {
    [
        ("Equip sword style", "trainer_sword"),
        ("Equip Sparking fists", "trainer_fists"),
        ("Restore my health and energy", "trainer_restore"),
        ("Explain magic and jumping", "trainer_help"),
        ("Leave", "trainer_leave"),
    ].into_iter().map(|(label, id)| Choice { label: label.to_string(), id }).collect()
}

fn respond(
    mut started: MessageReader<TalkStarted>,
    mut chose: MessageReader<Chose>,
    mut dialogue: ResMut<Dialogue>,
    mut end: MessageWriter<super::talk::EndTalk>,
    mut equip: MessageWriter<Equip>,
    mut players: Query<(Entity, &mut Fighter, &mut Energy, &mut Recovery), With<Player>>,
) {
    for _ in started.read().filter(|e| e.role == Role::Trainer) {
        *dialogue = Dialogue {
            line: "I'm Rook. Train your sword, ki and footwork here. Z switches sword and fists. What do you need?".into(),
            options: menu(),
        };
    }
    for pick in chose.read().filter(|e| e.role == Role::Trainer) {
        let Ok((who, mut fighter, mut energy, mut recovery)) = players.single_mut() else { continue };
        dialogue.line = match pick.id {
            "trainer_sword" | "trainer_fists" => {
                let set = if pick.id == "trainer_sword" { "katana" } else { "sparking" };
                equip.write(Equip { who, set: set.into() });
                format!("{set} equipped. Tap attacks to chain; hold for a heavy. X sheathes your sword.")
            }
            "trainer_restore" => {
                if !fighter.dead {
                    fighter.hp = fighter.max_hp;
                    fighter.stamina = fighter.max_stamina;
                    fighter.poise = fighter.max_poise;
                    energy.battlewill = energy.max_battlewill;
                    energy.magicules = energy.max_magicules;
                    *recovery = Recovery::default();
                }
                "Ready for another round. Health, stamina and both energy pools restored.".into()
            }
            "trainer_help" => "Tap Q/E for a quick cast; hold up to 1.5s for power, then release. Charge respects your available energy. Guard, dodge or jump cancels. T cycles spells; Shift+T cycles arts. Hold Space for height; release early to hop. Steer in the air and keep your run-up momentum.".into(),
            "trainer_leave" => { end.write_default(); String::new() }
            _ => continue,
        };
        dialogue.options = menu();
    }
}

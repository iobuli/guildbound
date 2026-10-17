//! The hotbar: the player's bound art (Q) and spell (E) from `magic::Grimoire`, the weapon in hand
//! (weapons come from the chest, `armory`), and the bag button at the far right. Ability slots
//! show a three-letter code, `★` once the ability is a skill, and dim while its pool can't pay
//! for it; their tooltips give the name, cost and mastery. Clicking Q or E opens the stats page on
//! its Arts & Magic tab (bind there); the bag button sends `inventory::ToggleBag`.
use super::MARGIN;
use super::stats_page::{StatsTab, ToggleStats};
use crate::play::inventory::{BagOpen, ToggleBag};
use crate::play::magic::{Energy, Grimoire, Pool, SKILL_COST, ability};
use crate::play::{Loadout, Player};
use bevy::prelude::*;
use gbui::prelude::*;

pub const SLOT: f32 = 50.0;
const GAP: f32 = 4.0;
/// Space before the weapon slot and before the bag button.
const BREAK: f32 = 16.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (update, buttons));
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Art,
    Spell,
    Weapon,
    Bag,
}

/// Three-letter terminal code for a weapon or ability name.
pub fn code(name: &str) -> String {
    match name {
        "unarmed" => "FST".into(),
        "katana" => "KTN".into(),
        "greatsword" => "GSW".into(),
        "dagger" => "DGR".into(),
        "crossbow" => "XBW".into(),
        "knives" => "KNV".into(),
        other => {
            let words: Vec<&str> = other.split([' ', '_']).filter(|w| !w.is_empty()).collect();
            let s: String = if words.len() >= 2 {
                words[0].chars().take(2).chain(words[1].chars().take(1)).collect()
            } else {
                other.chars().filter(|c| c.is_alphanumeric()).take(3).collect()
            };
            s.to_uppercase()
        }
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(MARGIN),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                column_gap: px(GAP),
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((Kind::Art, slot("slot", SLOT, "Q"), Tooltip(String::new()), Tweens::default()));
                bar.spawn((Kind::Spell, slot("slot", SLOT, "E"), Tooltip(String::new()), Tweens::default()));
                bar.spawn(Node {
                    width: px(BREAK - GAP),
                    ..default()
                });
                bar.spawn((Kind::Weapon, slot("slot", SLOT, ""), Tooltip(String::new()), Tweens::default()));
                bar.spawn(Node {
                    width: px(BREAK - GAP),
                    ..default()
                });
                bar.spawn((Kind::Bag, slot("slot.bag", SLOT, "I"), Tooltip("Bag  [I]".into()), Tweens::default()));
            });
        });
}

#[allow(clippy::type_complexity)]
fn update(
    player: Query<(&Loadout, Option<&Grimoire>, Option<&Energy>), With<Player>>,
    open: Res<BagOpen>,
    mut slots: Query<(&Kind, &Children, &mut UiFlags, &mut Tooltip, &mut Tweens)>,
    mut labels: Query<&mut Text, (With<SlotLabel>, Without<SlotCount>)>,
    mut counts: Query<&mut Text, (With<SlotCount>, Without<SlotLabel>)>,
    mut last: Local<(String, String, String)>,
) {
    let Ok((loadout, grimoire, energy)) = player.single() else { return };
    let g = grimoire.cloned().unwrap_or_default();
    let e = energy.cloned().unwrap_or_default();
    let now = (g.art.to_string(), g.spell.to_string(), loadout.equipped().to_string());
    for (kind, children, mut flags, mut tip, mut tweens) in &mut slots {
        let (label, count, tooltip, dim, checked, changed) = match kind {
            Kind::Art | Kind::Spell => {
                let id = if *kind == Kind::Art { g.art } else { g.spell };
                let Some(a) = ability(id) else { continue };
                let skill = g.is_skill(id);
                let cost = a.cost * if skill { SKILL_COST } else { 1.0 };
                let have = match a.pool {
                    Pool::Battlewill => e.battlewill,
                    Pool::Magicules => e.magicules,
                };
                let pool = if a.pool == Pool::Battlewill { "BW" } else { "MP" };
                let tip = format!(
                    "{}{}  //  {cost:.0} {pool}  //  mastery {:.0}%",
                    a.name,
                    if skill { " [SKILL]" } else { "" },
                    g.mastery(id) * 100.0
                );
                let changed = if *kind == Kind::Art { last.0 != now.0 } else { last.1 != now.1 };
                (code(a.name), if skill { "★".to_string() } else { String::new() }, tip, have < cost, false, changed)
            }
            Kind::Weapon => {
                let name = loadout.equipped();
                (code(name), String::new(), format!("{name}  (weapons: the chest)"), false, true, last.2 != now.2)
            }
            Kind::Bag => ("BAG".to_string(), String::new(), "Bag  [I]".to_string(), false, open.0, false),
        };
        if flags.disabled != dim {
            flags.disabled = dim;
        }
        if flags.checked != checked {
            flags.checked = checked;
        }
        if tip.0 != tooltip {
            tip.0 = tooltip;
        }
        if changed && !last.2.is_empty() {
            tweens.play(Tween::scale(1.14, 1.0, 0.22).ease(Easing::OutBack));
        }
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0 != label
            {
                text.0 = label.clone();
            }
            if let Ok(mut text) = counts.get_mut(child)
                && text.0 != count
            {
                text.0 = count.clone();
            }
        }
    }
    *last = now;
}

fn buttons(
    slots: Query<(&Kind, Ref<Interaction>)>,
    mut bag: MessageWriter<ToggleBag>,
    mut stats: MessageWriter<ToggleStats>,
) {
    for (kind, interaction) in &slots {
        if !(interaction.is_changed() && *interaction == Interaction::Pressed) {
            continue;
        }
        match kind {
            Kind::Bag => {
                bag.write(ToggleBag);
            }
            Kind::Art | Kind::Spell => {
                stats.write(ToggleStats(Some(StatsTab::Arts)));
            }
            Kind::Weapon => {}
        }
    }
}

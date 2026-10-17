//! The hotbar: the player's `Loadout` slots (keys 1–9, 0) as terminal codes, the equipped one
//! checked (and popped when it changes), and the bag button at the far right. Display only: the
//! keys reach `Intent.slot` through `input`. The bag button sends `inventory::ToggleBag` (the
//! cursor must be free: Esc) and shows checked while the bag is open.
use super::MARGIN;
use crate::play::inventory::{BagOpen, ToggleBag};
use crate::play::{Loadout, Player};
use bevy::prelude::*;
use gbui::prelude::*;

pub const SLOTS: usize = 10;
pub const SLOT: f32 = 50.0;
const GAP: f32 = 4.0;
/// Space between the last slot and the bag button.
const BAG_GAP: f32 = 16.0;
/// Width of the whole bar (slots, gap, bag button).
pub const WIDTH: f32 = SLOTS as f32 * SLOT + (SLOTS - 1) as f32 * GAP + BAG_GAP + SLOT;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, (update, bag_button));
}

#[derive(Component)]
struct HotbarSlot(usize);

#[derive(Component)]
struct BagButton;

fn key_name(i: usize) -> String {
    if i == 9 { "0".to_string() } else { (i + 1).to_string() }
}

/// Three-letter terminal code for a loadout entry.
pub fn code(name: &str) -> String {
    match name {
        "unarmed" => "FST".into(),
        "katana" => "KTN".into(),
        "greatsword" => "GSW".into(),
        "dagger" => "DGR".into(),
        "crossbow" => "XBW".into(),
        "knives" => "KNV".into(),
        "fire" => "FIR".into(),
        "frost" => "FRS".into(),
        other => other.chars().filter(|c| c.is_alphanumeric()).take(3).collect::<String>().to_uppercase(),
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
                for i in 0..SLOTS {
                    bar.spawn((
                        HotbarSlot(i),
                        slot("slot", SLOT, &key_name(i)),
                        Tooltip(String::new()),
                        Tweens::default(),
                    ));
                }
                bar.spawn(Node {
                    width: px(BAG_GAP - GAP),
                    ..default()
                });
                bar.spawn((BagButton, slot("slot.bag", SLOT, "I"), Tooltip("Bag  [I]".into())));
            });
        });
}

#[allow(clippy::type_complexity)]
fn update(
    player: Query<&Loadout, With<Player>>,
    mut slots: Query<(&HotbarSlot, &Children, &mut UiFlags, &mut Tooltip, &mut Tweens)>,
    mut labels: Query<&mut Text, With<SlotLabel>>,
    mut last: Local<Option<usize>>,
) {
    let Ok(loadout) = player.single() else { return };
    let changed = *last != Some(loadout.current);
    for (slot, children, mut flags, mut tip, mut tweens) in &mut slots {
        let name = loadout.slots.get(slot.0);
        let checked = slot.0 == loadout.current;
        if flags.checked != checked {
            flags.checked = checked;
        }
        let disabled = name.is_none();
        if flags.disabled != disabled {
            flags.disabled = disabled;
        }
        if changed && checked && last.is_some() {
            tweens.play(Tween::scale(1.14, 1.0, 0.22).ease(Easing::OutBack));
        }
        let want_tip = name.map_or(String::new(), |n| format!("{n}  [{}]", key_name(slot.0)));
        if tip.0 != want_tip {
            tip.0 = want_tip;
        }
        let want = name.map_or(String::new(), |n| code(n));
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0 != want
            {
                text.0 = want.clone();
            }
        }
    }
    *last = Some(loadout.current);
}

fn bag_button(
    open: Res<BagOpen>,
    mut button: Query<(Ref<Interaction>, &mut UiFlags, &Children), With<BagButton>>,
    mut labels: Query<&mut Text, With<SlotLabel>>,
    mut toggle: MessageWriter<ToggleBag>,
) {
    for (interaction, mut flags, children) in &mut button {
        if interaction.is_changed() && *interaction == Interaction::Pressed {
            toggle.write(ToggleBag);
        }
        if flags.checked != open.0 {
            flags.checked = open.0;
        }
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0.is_empty()
            {
                text.0 = "BAG".into();
            }
        }
    }
}

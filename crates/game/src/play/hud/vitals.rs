//! Vitals above the hotbar, each a `gbui` bar with a damage trail: HP and stamina (the player's
//! `Fighter`, written by `combat`) on the top row, Battlewill and magicules (`magic::Energy`) under
//! them. A bar is "checked" (themed as a warning) when its value falls under `LOW`.
use super::MARGIN;
use super::hotbar;
use crate::play::magic::Energy;
use crate::play::{Fighter, Player};
use bevy::prelude::*;
use gbui::prelude::*;

/// Fraction under which a bar shows its warning state.
const LOW: f32 = 0.3;
const BAR_HEIGHT: f32 = 11.0;
/// Width of the vitals block (px), and the gap between its rows.
pub const WIDTH: f32 = 560.0;
const ROW_GAP: f32 = 5.0;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(Update, update);
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Vital {
    Hp,
    Stamina,
    Battlewill,
    Magicules,
}

#[derive(Component)]
struct VitalValue(Vital);

fn spawn(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(MARGIN + hotbar::SLOT + 9.0),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: px(WIDTH),
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(14.0),
                row_gap: px(ROW_GAP),
                ..default()
            })
            .with_children(|pair| {
                for (vital, key, class) in [
                    (Vital::Hp, "HP", "bar.hp"),
                    (Vital::Stamina, "ST", "bar.st"),
                    (Vital::Battlewill, "BW", "bar.bw"),
                    (Vital::Magicules, "MP", "bar.mp"),
                ] {
                    pair.spawn(Node {
                        width: px((WIDTH - 14.0) * 0.5),
                        align_items: AlignItems::Center,
                        column_gap: px(6.0),
                        ..default()
                    })
                    .with_children(|g| {
                        g.spawn(label(key, "vital.key"));
                        g.spawn((
                            vital,
                            bar(
                                class,
                                Node {
                                    flex_grow: 1.0,
                                    height: px(BAR_HEIGHT),
                                    ..default()
                                },
                            ),
                            UiFlags::default(),
                        ));
                        g.spawn((
                            VitalValue(vital),
                            label("", "vital.value"),
                            Node {
                                min_width: px(28.0),
                                ..default()
                            },
                            TextLayout::justify(Justify::Right),
                        ));
                    });
                }
            });
        });
}

fn update(
    player: Query<(&Fighter, Option<&Energy>), With<Player>>,
    mut bars: Query<(&Vital, &mut Bar, &mut UiFlags)>,
    mut values: Query<(&VitalValue, &mut Text)>,
) {
    let Ok((f, energy)) = player.single() else { return };
    let e = energy.cloned().unwrap_or_default();
    let read = |v: Vital| match v {
        Vital::Hp => (f.hp, f.max_hp),
        Vital::Stamina => (f.stamina, f.max_stamina),
        Vital::Battlewill => (e.battlewill, e.max_battlewill),
        Vital::Magicules => (e.magicules, e.max_magicules),
    };
    for (vital, mut bar, mut flags) in &mut bars {
        let (now, max) = read(*vital);
        let fraction = if max > 0.0 { (now / max).clamp(0.0, 1.0) } else { 0.0 };
        if bar.value != fraction {
            bar.value = fraction;
        }
        let low = fraction < LOW;
        if flags.checked != low {
            flags.checked = low;
        }
    }
    for (value, mut text) in &mut values {
        let s = format!("{:.0}", read(value.0).0.max(0.0).ceil());
        if text.0 != s {
            text.0 = s;
        }
    }
}

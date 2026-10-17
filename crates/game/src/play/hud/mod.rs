//! HUD: the terminal-style overlay, built from `gbui` widgets and styled by `assets/ui/theme.json`
//! (edit it while the game runs; it reloads). Single-player layout:
//! - bottom left: the round map (`minimap`), the message log above it (`log`);
//! - bottom centre: HP / ST bars (`vitals`) over the hotbar (`hotbar`: the `Loadout` slots on
//!   1–0 and the bag button, which sends `inventory::ToggleBag`);
//! - bottom right: the bag panel (`inventory`, I or the bag button);
//! - centre low: the interaction prompt (`InteractPrompt`), typed out when it changes.
//!
//! `hud` owns these nodes. Other modules reach it through `log::HudLog` messages and the state it
//! reads (`InteractPrompt`, the player's `Fighter` and `Loadout`, `CameraRig`).
use super::{InteractPrompt, PlaySet};
use bevy::prelude::*;
use gbui::prelude::*;

pub mod hotbar;
pub mod log;
pub mod minimap;
pub mod vitals;

/// Gap between HUD blocks and the window edge (px).
pub const MARGIN: f32 = 18.0;
/// The prompt's typing rate (chars/s).
const PROMPT_RATE: f32 = 70.0;

pub fn plugin(app: &mut App) {
    app.add_plugins((log::plugin, vitals::plugin, hotbar::plugin, minimap::plugin))
        .add_systems(Startup, spawn_prompt)
        .add_systems(Update, show_prompt.after(PlaySet::Act));
}

#[derive(Component)]
struct PromptBox;

#[derive(Component)]
struct PromptText;

fn spawn_prompt(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(150.0),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                PromptBox,
                panel(
                    "prompt",
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(10.0),
                        padding: UiRect::axes(px(12.0), px(6.0)),
                        ..default()
                    },
                ),
                Visibility::Hidden,
                Tweens::default(),
            ))
            .with_children(|p| {
                p.spawn((
                    label("E", "prompt.key"),
                    Node {
                        padding: UiRect::axes(px(7.0), px(1.0)),
                        ..default()
                    },
                ));
                p.spawn((PromptText, label("", "prompt.text"), Typewriter::new("", PROMPT_RATE)));
            });
        });
}

fn show_prompt(
    prompt: Res<InteractPrompt>,
    mut boxes: Query<(&mut Visibility, &mut Tweens), With<PromptBox>>,
    mut texts: Query<&mut Typewriter, With<PromptText>>,
) {
    if !prompt.is_changed() {
        return;
    }
    for (mut vis, mut tweens) in &mut boxes {
        if prompt.0.is_none() {
            vis.set_if_neq(Visibility::Hidden);
        } else if *vis == Visibility::Hidden {
            *vis = Visibility::Inherited;
            tweens.play(Tween::scale(0.92, 1.0, 0.2).ease(Easing::OutBack));
            tweens.play(Tween::opacity(0.0, 1.0, 0.12));
        }
    }
    if let Some(label) = &prompt.0 {
        for mut text in &mut texts {
            text.retype(label);
        }
    }
}

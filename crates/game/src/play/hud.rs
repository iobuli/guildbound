//! HUD: player hp/stamina/poise bars, target hp bar, equipped moveset name + slot list,
//! interaction prompt (`InteractPrompt`), action/state debug line, controls hint, damage
//! numbers floating from `Hit` messages.
//!
//! Implemented so far: the interaction prompt ("[E] Chop tree"), low centre.
use super::{InteractPrompt, PlaySet};
use bevy::prelude::*;
use bevy::text::FontSize;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_prompt)
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
            bottom: percent(22.0),
            width: percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                PromptBox,
                Node {
                    padding: UiRect::axes(px(14.0), px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.72)),
                Visibility::Hidden,
            ))
            .with_child((
                PromptText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.9, 0.62)),
            ));
        });
}

fn show_prompt(
    prompt: Res<InteractPrompt>,
    mut boxes: Query<&mut Visibility, With<PromptBox>>,
    mut texts: Query<&mut Text, With<PromptText>>,
) {
    if !prompt.is_changed() {
        return;
    }
    for mut vis in &mut boxes {
        vis.set_if_neq(if prompt.0.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }
    if let Some(label) = &prompt.0 {
        for mut text in &mut texts {
            text.0 = format!("[E]  {label}");
        }
    }
}

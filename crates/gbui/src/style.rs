//! Styling. A `Styled(class)` node takes its look from the `Theme` for its current states, times
//! the opacity it inherits:
//! - states: hovered / pressed from `Interaction`, checked / focused / disabled from `UiFlags`; a
//!   node with neither takes the states of its nearest ancestor that has them (a slot's label
//!   lights up with the slot);
//! - `Tint` overrides the theme's background, text or border colour (an item's colour on a slot);
//! - `Opacity` multiplies every colour of the node and its descendants (fades, blinking).
//!
//! `apply` is the only writer of `BackgroundColor`, `BackgroundGradient`, `BorderColor`,
//! `BoxShadow` (glow), `TextColor`, `TextShadow`, `TextFont` (face and size), and of `Node`'s
//! border width and radius when the class sets them, on `Styled` entities. Change those through
//! the theme, `Tint`, `UiFlags` or `Opacity`, never directly. Glass overlays are materials and
//! don't fade with `Opacity`.
use crate::theme::{States, Style, Theme, UiFonts};
use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use std::borrow::Cow;
use std::collections::HashSet;

/// Text shadow offset (px).
const SHADOW_OFFSET: Vec2 = Vec2::new(1.0, 1.0);

#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[require(BackgroundColor, BorderColor)]
pub struct Styled(pub Cow<'static, str>);

impl Styled {
    pub fn new(class: impl Into<Cow<'static, str>>) -> Self {
        Self(class.into())
    }
}

/// Per-node colour overrides on top of the theme.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Tint {
    pub bg: Option<Color>,
    pub text: Option<Color>,
    pub border: Option<Color>,
}

/// States the owner sets (hover and press come from `Interaction`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiFlags {
    pub checked: bool,
    pub focused: bool,
    pub disabled: bool,
}

/// 0..1, multiplied down the hierarchy.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Opacity(pub f32);

impl Default for Opacity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Marks a node that draws the theme's glass surface (`glass::attach` gives it the material).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Glass;

pub fn states(flags: Option<&UiFlags>, interaction: Option<&Interaction>) -> States {
    let mut s = 0;
    if let Some(f) = flags {
        s |= if f.checked { States::CHECKED } else { 0 };
        s |= if f.focused { States::FOCUSED } else { 0 };
        s |= if f.disabled { States::DISABLED } else { 0 };
    }
    match interaction {
        Some(Interaction::Pressed) => s |= States::PRESSED | States::HOVERED,
        Some(Interaction::Hovered) => s |= States::HOVERED,
        _ => {}
    }
    States(s)
}

/// States of the nearest ancestor (from `entity`'s parent up) that has `UiFlags` or `Interaction`.
#[allow(clippy::type_complexity)]
pub fn inherited_states(
    entity: Entity,
    parents: &Query<&ChildOf>,
    stateful: &Query<(Option<&UiFlags>, Option<&Interaction>), Or<(With<UiFlags>, With<Interaction>)>>,
) -> States {
    let mut at = parents.get(entity).ok().map(ChildOf::parent);
    for _ in 0..16 {
        let Some(e) = at else { break };
        if let Ok((flags, interaction)) = stateful.get(e) {
            return states(flags, interaction);
        }
        at = parents.get(e).ok().map(ChildOf::parent);
    }
    States::default()
}

/// Product of `Opacity` from `entity` up to its root.
pub fn inherited_opacity(entity: Entity, parents: &Query<&ChildOf>, opacities: &Query<&Opacity>) -> f32 {
    let mut o = 1.0;
    let mut at = Some(entity);
    for _ in 0..64 {
        let Some(e) = at else { break };
        if let Ok(op) = opacities.get(e) {
            o *= op.0.clamp(0.0, 1.0);
        }
        at = parents.get(e).ok().map(ChildOf::parent);
    }
    o
}

fn faded(c: Color, o: f32) -> Color {
    c.with_alpha(c.alpha() * o)
}

type Looks<'a> = (
    Option<&'a mut Node>,
    &'a mut BackgroundColor,
    &'a mut BorderColor,
    Option<&'a mut BackgroundGradient>,
    Option<&'a mut BoxShadow>,
    Option<&'a mut TextColor>,
    Option<&'a mut TextFont>,
    Option<&'a mut TextShadow>,
);

#[allow(clippy::type_complexity)]
pub fn apply(
    mut commands: Commands,
    theme: Res<Theme>,
    fonts: Option<Res<UiFonts>>,
    parents: Query<&ChildOf>,
    opacities: Query<&Opacity>,
    stateful: Query<(Option<&UiFlags>, Option<&Interaction>), Or<(With<UiFlags>, With<Interaction>)>>,
    mut nodes: Query<(Entity, &Styled, Option<&Tint>, Option<&UiFlags>, Option<&Interaction>, Looks)>,
    mut missing: Local<HashSet<String>>,
) {
    for (entity, styled, tint, flags, interaction, looks) in &mut nodes {
        let (node, mut bg, mut border, gradient, shadow, text, font, text_shadow) = looks;
        let st = if flags.is_some() || interaction.is_some() {
            states(flags, interaction)
        } else {
            inherited_states(entity, &parents, &stateful)
        };
        let style = match theme.resolve(&styled.0, st) {
            Some(s) => s,
            None => {
                if missing.insert(styled.0.to_string()) {
                    warn!("[gbui] the theme has no class '{}'", styled.0);
                }
                Style::default()
            }
        };
        let o = inherited_opacity(entity, &parents, &opacities);
        let tint = tint.copied().unwrap_or_default();

        bg.set_if_neq(BackgroundColor(faded(tint.bg.or(style.bg).unwrap_or(Color::NONE), o)));
        border.set_if_neq(BorderColor::all(faded(tint.border.or(style.border).unwrap_or(Color::NONE), o)));
        if let Some(mut node) = node {
            if let Some(w) = style.border_width {
                let want = UiRect::all(px(w));
                if node.border != want {
                    node.border = want;
                }
            }
            if let Some(r) = style.radius {
                let want = BorderRadius::all(px(r));
                if node.border_radius != want {
                    node.border_radius = want;
                }
            }
        }

        let want_gradient = style.gradient.as_ref().filter(|g| !g.is_empty()).map(|stops| {
            let n = stops.len().max(2) - 1;
            let stops = stops
                .iter()
                .enumerate()
                .map(|(i, c)| ColorStop::new(faded(*c, o), percent(100.0 * i as f32 / n as f32)))
                .collect();
            BackgroundGradient(vec![LinearGradient::to_right(stops).into()])
        });
        match (gradient, want_gradient) {
            (Some(mut have), Some(want)) => {
                if *have != want {
                    *have = want;
                }
            }
            (Some(mut have), None) => {
                if !have.0.is_empty() {
                    have.0.clear();
                }
            }
            (None, Some(want)) => {
                commands.entity(entity).insert(want);
            }
            (None, None) => {}
        }

        let want_glow = style.glow.filter(|c| c.alpha() * o > 0.0).map(|c| {
            BoxShadow(vec![ShadowStyle {
                color: faded(c, o),
                x_offset: Val::ZERO,
                y_offset: Val::ZERO,
                spread_radius: px(style.glow_spread.unwrap_or(0.0)),
                blur_radius: px(style.glow_blur.unwrap_or(12.0)),
            }])
        });
        match (shadow, want_glow) {
            (Some(mut have), Some(want)) => {
                if *have != want {
                    *have = want;
                }
            }
            (Some(mut have), None) => {
                if !have.0.is_empty() {
                    have.0.clear();
                }
            }
            (None, Some(want)) => {
                commands.entity(entity).insert(want);
            }
            (None, None) => {}
        }

        if let Some(mut color) = text {
            color.set_if_neq(TextColor(faded(tint.text.or(style.text).unwrap_or(Color::WHITE), o)));
            let want = style.shadow.map(|c| TextShadow {
                offset: SHADOW_OFFSET,
                color: faded(c, o),
            });
            match (text_shadow, want) {
                (Some(mut have), Some(want)) => {
                    have.set_if_neq(want);
                }
                (Some(mut have), None) => {
                    if have.color != Color::NONE {
                        have.color = Color::NONE;
                    }
                }
                (None, Some(want)) => {
                    commands.entity(entity).insert(want);
                }
                (None, None) => {}
            }
        }
        if let Some(mut font) = font {
            if let Some(size) = style.size {
                let want = FontSize::Px(size);
                if font.font_size != want {
                    font.font_size = want;
                }
            }
            if let Some(fonts) = &fonts {
                let face = if style.bold == Some(true) { &fonts.bold } else { &fonts.regular };
                let want = FontSource::Handle(face.clone());
                if font.font != want {
                    font.font = want;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_and_flags_map_to_states() {
        assert_eq!(states(None, None), States(0));
        assert_eq!(states(None, Some(&Interaction::Pressed)), States(States::PRESSED | States::HOVERED));
        let flags = UiFlags { checked: true, disabled: true, ..default() };
        let s = states(Some(&flags), Some(&Interaction::Hovered));
        assert!(s.has(States::CHECKED) && s.has(States::DISABLED) && s.has(States::HOVERED) && !s.has(States::PRESSED));
    }

    #[test]
    fn opacity_multiplies_down_the_hierarchy() {
        let mut world = World::new();
        let root = world.spawn(Opacity(0.5)).id();
        let mid = world.spawn((ChildOf(root),)).id();
        let leaf = world.spawn((ChildOf(mid), Opacity(0.5))).id();
        let mut system = bevy::ecs::system::SystemState::<(Query<&ChildOf>, Query<&Opacity>)>::new(&mut world);
        let (parents, opacities) = system.get(&world).unwrap();
        assert!((inherited_opacity(leaf, &parents, &opacities) - 0.25).abs() < 1e-6);
        assert!((inherited_opacity(mid, &parents, &opacities) - 0.5).abs() < 1e-6);
    }
}

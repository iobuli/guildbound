//! Widget builders. Each returns a bundle; add content with `with_children`.
//! - `panel(class, node)`: themed box with a glass overlay and four corner brackets (class
//!   `bracket`).
//! - `label(text, class)`.
//! - `bar(class, node)`: `Bar` with a fill (`{class}.fill`), a damage trail behind it
//!   (`{class}.trail`) that waits `trail_delay` then drains, and ten segment ticks (`bar.tick`).
//!   Set `Bar::value`; `drive_bars` does the rest.
//! - `slot(class, size, key)`: square button with a key hint (`{class}.key`), a centred label
//!   (`{class}.label`, marker `SlotLabel`) and a count (`{class}.count`, `SlotCount`); check it with
//!   `UiFlags::checked`.
//! - `Tooltip(text)` on any node with `Interaction`: while hovered, one shared tooltip panel
//!   (`tooltip` / `tooltip.text`) types the text next to the cursor.
use crate::anim::{Easing, Tween, Tweens};
use crate::style::{Glass, Opacity, Styled, UiFlags};
use crate::term::Typewriter;
use bevy::ecs::spawn::SpawnIter;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::borrow::Cow;

/// Corner bracket length and thickness (px).
const BRACKET: f32 = 9.0;
const BRACKET_W: f32 = 2.0;
/// Tooltip offset from the cursor (px) and typing rate (chars/s).
const TIP_OFFSET: Vec2 = Vec2::new(16.0, 18.0);
const TIP_RATE: f32 = 140.0;
/// The shown fill eases toward `Bar::value` at this rate (1/s).
const BAR_EASE: f32 = 14.0;

pub fn label(text: impl Into<String>, class: impl Into<Cow<'static, str>>) -> impl Bundle {
    (Text::new(text), TextFont::default(), Styled::new(class))
}

fn inset() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0.0),
        right: px(0.0),
        top: px(0.0),
        bottom: px(0.0),
        ..default()
    }
}

fn bracket(left: bool, top: bool) -> impl Bundle {
    let (h, v) = (px(-1.0), px(BRACKET_W));
    let zero = px(0.0);
    (
        Node {
            position_type: PositionType::Absolute,
            width: px(BRACKET),
            height: px(BRACKET),
            left: if left { h } else { Val::Auto },
            right: if left { Val::Auto } else { h },
            top: if top { h } else { Val::Auto },
            bottom: if top { Val::Auto } else { h },
            border: UiRect {
                left: if left { v } else { zero },
                right: if left { zero } else { v },
                top: if top { v } else { zero },
                bottom: if top { zero } else { v },
            },
            ..default()
        },
        Styled::new("bracket"),
    )
}

pub fn panel(class: impl Into<Cow<'static, str>>, node: Node) -> impl Bundle {
    (
        node,
        Styled::new(class),
        children![
            (Glass, inset()),
            bracket(true, true),
            bracket(false, true),
            bracket(true, false),
            bracket(false, false),
        ],
    )
}

#[derive(Component, Clone, Debug)]
pub struct Bar {
    /// 0..1, set by the owner.
    pub value: f32,
    /// What the fill shows (eases toward `value`).
    pub shown: f32,
    /// The damage trail: holds after a drop, then drains toward `value`.
    pub trail: f32,
    pub hold: f32,
    pub trail_delay: f32,
    /// Fraction per second.
    pub trail_rate: f32,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            value: 1.0,
            shown: 1.0,
            trail: 1.0,
            hold: 0.0,
            trail_delay: 0.45,
            trail_rate: 0.6,
        }
    }
}

impl Bar {
    /// One step of the fill and trail toward `value`.
    pub fn step(&mut self, dt: f32) {
        self.value = self.value.clamp(0.0, 1.0);
        self.shown += (self.value - self.shown) * (1.0 - (-BAR_EASE * dt).exp());
        if (self.value - self.shown).abs() < 1e-3 {
            self.shown = self.value;
        }
        if self.shown >= self.trail {
            self.trail = self.shown;
            self.hold = 0.0;
        } else {
            self.hold += dt;
            if self.hold >= self.trail_delay {
                self.trail = (self.trail - self.trail_rate * dt).max(self.shown);
            }
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct BarFill;

#[derive(Component, Clone, Copy, Debug)]
pub struct BarTrail;

fn fill_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0.0),
        top: px(0.0),
        bottom: px(0.0),
        width: percent(100.0),
        ..default()
    }
}

pub fn bar(class: &str, node: Node) -> impl Bundle {
    (
        node,
        Styled::new(class.to_string()),
        Bar::default(),
        children![
            (BarTrail, fill_node(), Styled::new(format!("{class}.trail"))),
            (BarFill, fill_node(), Styled::new(format!("{class}.fill"))),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    ..inset()
                },
                Children::spawn(SpawnIter((0..10).map(|i| {
                    (
                        Node {
                            width: percent(10.0),
                            height: percent(100.0),
                            border: UiRect::right(px(if i < 9 { 1.0 } else { 0.0 })),
                            ..default()
                        },
                        Styled::new("bar.tick"),
                    )
                }))),
            ),
        ],
    )
}

#[allow(clippy::type_complexity)]
pub fn drive_bars(
    time: Res<Time<Real>>,
    mut bars: Query<(&mut Bar, &Children)>,
    mut fills: Query<&mut Node, (With<BarFill>, Without<BarTrail>)>,
    mut trails: Query<&mut Node, (With<BarTrail>, Without<BarFill>)>,
) {
    let dt = time.delta_secs().min(0.1);
    for (mut bar, children) in &mut bars {
        bar.step(dt);
        for child in children.iter() {
            if let Ok(mut n) = fills.get_mut(child) {
                let w = percent(bar.shown * 100.0);
                if n.width != w {
                    n.width = w;
                }
            }
            if let Ok(mut n) = trails.get_mut(child) {
                let w = percent(bar.trail * 100.0);
                if n.width != w {
                    n.width = w;
                }
            }
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct SlotKey;

#[derive(Component, Clone, Copy, Debug)]
pub struct SlotLabel;

#[derive(Component, Clone, Copy, Debug)]
pub struct SlotCount;

pub fn slot(class: &str, size: f32, key: &str) -> impl Bundle {
    (
        Button,
        Node {
            width: px(size),
            height: px(size),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        Styled::new(class.to_string()),
        UiFlags::default(),
        children![
            (
                SlotKey,
                label(key.to_string(), format!("{class}.key")),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(3.0),
                    top: px(1.0),
                    ..default()
                },
            ),
            (SlotLabel, label("", format!("{class}.label")), crate::style::Tint::default()),
            (
                SlotCount,
                label("", format!("{class}.count")),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(3.0),
                    bottom: px(1.0),
                    ..default()
                },
            ),
        ],
    )
}

/// Hover text for a node with `Interaction`.
#[derive(Component, Clone, Debug)]
pub struct Tooltip(pub String);

#[derive(Component)]
pub struct TooltipBox;

#[derive(Component)]
pub struct TooltipText;

pub fn spawn_tooltip(mut commands: Commands) {
    commands
        .spawn((
            TooltipBox,
            panel(
                "tooltip",
                Node {
                    position_type: PositionType::Absolute,
                    padding: UiRect::axes(px(9.0), px(5.0)),
                    max_width: px(340.0),
                    ..default()
                },
            ),
            Visibility::Hidden,
            GlobalZIndex(100),
            Opacity(0.0),
            Tweens::default(),
        ))
        .with_child((TooltipText, label("", "tooltip.text"), Typewriter::new("", TIP_RATE)));
}

#[allow(clippy::type_complexity)]
pub fn tooltips(
    windows: Query<&Window, With<PrimaryWindow>>,
    hovered: Query<(&Interaction, &Tooltip)>,
    mut boxes: Query<(&mut Node, &mut Visibility, &mut Tweens, &ComputedNode), With<TooltipBox>>,
    mut texts: Query<&mut Typewriter, With<TooltipText>>,
) {
    let Ok((mut node, mut vis, mut tweens, computed)) = boxes.single_mut() else { return };
    let tip = hovered.iter().find(|(i, _)| **i != Interaction::None).map(|(_, t)| t.0.as_str());
    let cursor = windows.single().ok().and_then(|w| w.cursor_position().map(|c| (c, w.size())));
    let (Some(tip), Some((cursor, screen))) = (tip, cursor) else {
        vis.set_if_neq(Visibility::Hidden);
        return;
    };
    if *vis == Visibility::Hidden {
        vis.set_if_neq(Visibility::Inherited);
        tweens.play(Tween::opacity(0.0, 1.0, 0.12).ease(Easing::OutQuad));
    }
    if let Ok(mut tw) = texts.single_mut() {
        tw.retype(tip);
    }
    let size = computed.size * computed.inverse_scale_factor;
    let mut at = cursor + TIP_OFFSET;
    if at.x + size.x > screen.x - 4.0 {
        at.x = cursor.x - TIP_OFFSET.x - size.x;
    }
    if at.y + size.y > screen.y - 4.0 {
        at.y = cursor.y - TIP_OFFSET.y - size.y;
    }
    let (left, top) = (px(at.x.max(0.0)), px(at.y.max(0.0)));
    if node.left != left || node.top != top {
        node.left = left;
        node.top = top;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_trail_holds_then_drains_to_the_value() {
        let mut b = Bar::default();
        b.value = 0.6;
        for _ in 0..30 {
            b.step(0.01); // 0.3 s: fill has caught up, trail still holds
        }
        assert!((b.shown - 0.6).abs() < 0.02, "fill {}", b.shown);
        assert_eq!(b.trail, 1.0);
        for _ in 0..100 {
            b.step(0.01);
        }
        assert!(b.trail < 1.0 && b.trail >= b.shown);
        for _ in 0..200 {
            b.step(0.01);
        }
        assert!((b.trail - b.shown).abs() < 1e-6, "drained");
        b.value = 0.9; // healing: the trail follows the fill up at once
        for _ in 0..100 {
            b.step(0.01);
        }
        assert_eq!(b.trail, b.shown);
        b.value = 7.0;
        b.step(0.01);
        assert_eq!(b.value, 1.0, "clamped");
    }
}

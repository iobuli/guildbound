//! The hotbar: the player's bound art (Q) and spell (E) from `magic::Grimoire`, the weapon in hand
//! (weapons come from the chest, `armory`), and the bag button at the far right. Ability slots
//! show a three-letter code, `★` once the ability is mastered, and dim while its pool can't pay
//! for it; their tooltips give the name, cost and mastery. Clicking Q or E opens the stats page on
//! its Arts & Magic tab (bind there); the bag button sends `inventory::ToggleBag`.
//!
//! The Q / E slots are image-layer skins (`gbui::layers`, `assets/ui/skins/ability_{bw,mp}`): glass,
//! ring track, ten pool ticks lit by how much of the cost the pool covers (`cover`), twelve cast
//! wedges lit by the cast's progress (`progress`, from `magic::Casting::progress`), frame, glow and
//! the mastered star; this module only sets the states `casting`, `dim`, `mastered` and the two
//! drives, the skin's JSON decides how each layer looks in each state. The glyph (`gbui::icons`,
//! sheet `abilities`) sits on top. The weapon slot shows its glyph from the `bag` sheet. A slot
//! whose glyph is missing falls back to the three-letter code.
use super::MARGIN;
use super::stats_page::{StatsTab, ToggleStats};
use crate::play::inventory::{BagOpen, ToggleBag};
use crate::play::magic::{Casting, Energy, Grimoire, MASTERED_COST, Pool, ability};
use crate::play::{Loadout, Player};
use bevy::prelude::*;
use gbui::prelude::*;

pub const SLOT: f32 = 50.0;
const GAP: f32 = 4.0;
/// Space before the weapon slot and before the bag button.
const BREAK: f32 = 16.0;
/// Glyph size inside a slot.
const GLYPH: f32 = 34.0;

/// The glyph layer of a slot.
#[derive(Component)]
struct Glyph;

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
                for (kind, key, skin) in [(Kind::Art, "Q", "ability_bw"), (Kind::Spell, "E", "ability_mp")] {
                    bar.spawn((kind, slot("slot.bare", SLOT, key), Skinned::new(skin), Tooltip(String::new()), Tweens::default()))
                        .with_children(|s| {
                            s.spawn(glyph("abilities"));
                        });
                }
                bar.spawn(Node {
                    width: px(BREAK - GAP),
                    ..default()
                });
                bar.spawn((Kind::Weapon, slot("slot", SLOT, ""), Tooltip(String::new()), Tweens::default()))
                    .with_children(|s| {
                        s.spawn(glyph("bag"));
                    });
                bar.spawn(Node {
                    width: px(BREAK - GAP),
                    ..default()
                });
                bar.spawn((Kind::Bag, slot("slot.bag", SLOT, "I"), Tooltip("Bag  [I]".into()), Tweens::default()));
            });
        });
}

fn glyph(sheet: &str) -> impl Bundle {
    (
        Glyph,
        Icon::new(sheet, ""),
        ImageNode::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px((SLOT - GLYPH) / 2.0),
            top: px((SLOT - GLYPH) / 2.0 - 1.0),
            width: px(GLYPH),
            height: px(GLYPH),
            ..default()
        },
    )
}

/// Glyph colour per slot (the glyphs ship white): arts in Battlewill gold, spells in Magicules
/// violet, the weapon in the HUD accent.
fn glyph_color(kind: Kind) -> Color {
    match kind {
        Kind::Art => Color::srgb_u8(0xFF, 0xC8, 0x57),
        Kind::Spell => Color::srgb_u8(0xB7, 0x9C, 0xFF),
        _ => Color::srgb_u8(0x2E, 0xE6, 0xC5),
    }
}

/// The `bag` sheet's sprite for a weapon (the axe weapon is the battleaxe; the tool is `axe`).
fn weapon_glyph(name: &str) -> &str {
    if name == "axe" { "axe_battle" } else { name }
}

#[allow(clippy::type_complexity)]
fn update(
    player: Query<
        (&Loadout, Option<&Grimoire>, Option<&Energy>, Option<&crate::play::guild::Purse>, Option<&Casting>),
        With<Player>,
    >,
    open: Res<BagOpen>,
    sheets: Res<IconSheets>,
    mut slots: Query<(&Kind, &Children, &mut UiFlags, &mut Tooltip, &mut Tweens, Option<&mut Skinned>)>,
    mut labels: Query<&mut Text, (With<SlotLabel>, Without<SlotCount>)>,
    mut counts: Query<&mut Text, (With<SlotCount>, Without<SlotLabel>)>,
    mut glyphs: Query<&mut Icon, With<Glyph>>,
    mut last: Local<(String, String, String)>,
) {
    let Ok((loadout, grimoire, energy, purse, casting)) = player.single() else { return };
    let cast = casting.map(|c| c.progress());
    let g = grimoire.cloned().unwrap_or_default();
    let e = energy.cloned().unwrap_or_default();
    let now = (g.art.to_string(), g.spell.to_string(), loadout.equipped().to_string());
    for (kind, children, mut flags, mut tip, mut tweens, skinned) in &mut slots {
        // Per-slot layer state: (glyph sheet, sprite, tint), pool coverage, cast progress.
        let mut layer: Option<(&str, String, IconTint)> = None;
        let mut cover = None;
        let mut casting_u = 0.0;
        let (label, count, tooltip, dim, checked, changed) = match kind {
            Kind::Art | Kind::Spell => {
                let id = if *kind == Kind::Art { g.art } else { g.spell };
                let Some(a) = ability(id) else { continue };
                let mastered = g.is_mastered(id);
                let cost = a.cost * if mastered { MASTERED_COST } else { 1.0 };
                let have = match a.pool {
                    Pool::Battlewill => e.battlewill,
                    Pool::Magicules => e.magicules,
                };
                let pool = if a.pool == Pool::Battlewill { "BW" } else { "MP" };
                let tip = format!(
                    "{}{}  //  {cost:.0} {pool}  //  mastery {:.0}%",
                    a.name,
                    if mastered { " [MASTERED]" } else { "" },
                    g.mastery(id) * 100.0
                );
                let changed = if *kind == Kind::Art { last.0 != now.0 } else { last.1 != now.1 };
                let tint = if have < cost {
                    IconTint::Dim
                } else if mastered {
                    IconTint::Mastered
                } else {
                    IconTint::Full
                };
                layer = Some(("abilities", id.to_string(), tint));
                cover = Some((have / cost.max(1e-3)).clamp(0.0, 1.0));
                if let Some((casting_id, u)) = cast
                    && casting_id == id
                {
                    casting_u = u;
                }
                (code(a.name), if mastered { "★".to_string() } else { String::new() }, tip, have < cost, false, changed)
            }
            Kind::Weapon => {
                let name = loadout.equipped();
                layer = Some(("bag", weapon_glyph(name).to_string(), IconTint::Full));
                (code(name), String::new(), format!("{name}  (weapons: the chest)"), false, true, last.2 != now.2)
            }
            Kind::Bag => {
                let gold = purse.map_or(0, |p| p.gold);
                ("BAG".to_string(), format!("{gold}g"), format!("Bag  [I]  //  {gold} gold"), false, open.0, false)
            }
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
        if let (Some(mut skin), Some(c)) = (skinned, cover) {
            let tint = layer.as_ref().map(|l| l.2);
            skin.set("casting", casting_u > 0.0);
            skin.set("dim", tint == Some(IconTint::Dim));
            skin.set("mastered", tint == Some(IconTint::Mastered));
            skin.drive("progress", casting_u);
            skin.drive("cover", c);
        }
        // The code label only stands in while the slot has no glyph to show.
        let drawn = layer.as_ref().is_some_and(|(sheet, name, _)| sheets.cell(sheet, name).is_some());
        let label = if drawn { String::new() } else { label };
        for child in children.iter() {
            if let Some((sheet, name, tint)) = &layer
                && let Ok(mut icon) = glyphs.get_mut(child)
                && (icon.sheet != *sheet || icon.name != *name || icon.tint != *tint || icon.color != glyph_color(*kind))
            {
                icon.sheet = sheet.to_string();
                icon.name = name.clone();
                icon.tint = *tint;
                icon.color = glyph_color(*kind);
            }
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

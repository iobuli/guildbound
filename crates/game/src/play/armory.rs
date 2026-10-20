//! The weapon chest. Weapons are not on the hotbar: opening the courtyard chest (F) shows the
//! weapon rack, a `gbui` panel with one slot per unarmed / melee / ranged moveset
//! (`moveset::Movesets`).
//! Clicking a slot asks `combat` to wield it (`combat::Equip`, applied once the player is free);
//! the wielded one shows checked, and the footer types the hovered weapon's light / heavy damage.
//! The rack closes with Esc, when the chest is closed again, or when the player walks off.
//! `ArmoryOpen` (written here only) frees the cursor through `camera`.
use super::camera::PageToggles;
use super::combat::Equip;
use super::interact::{PropState, UseEffect};
use super::moveset::{Kind, Moveset, Movesets, VERSUS_PREFIX};
use super::{Interactable, Loadout, Player};
use bevy::prelude::*;
use gbui::prelude::*;

const TILE: f32 = 64.0;
const GAP: f32 = 6.0;
const COLUMNS: usize = 4;
/// Most rack slots shown.
const RACK: usize = 12;
/// The rack closes when the player is farther than this from the chest (m).
const REACH: f32 = 3.5;
const FOOTER_RATE: f32 = 110.0;

pub fn plugin(app: &mut App) {
    app.init_resource::<ArmoryOpen>()
        .add_systems(Startup, spawn_ui)
        .add_systems(Update, (open_on_chest, close.in_set(PageToggles), refresh, pick, hover).chain());
}

/// The chest the rack belongs to, while it is open.
#[derive(Resource, Default)]
pub struct ArmoryOpen(pub Option<Entity>);

#[derive(Component)]
struct RackRoot;

/// Rack slot `i` shows the i-th weapon of `weapons`.
#[derive(Component)]
struct RackSlot(usize);

#[derive(Component)]
struct RackFooter;

/// Weapon movesets in rack order: unarmed first, then by name.
pub fn weapons(sets: &Movesets) -> Vec<&Moveset> {
    let mut v: Vec<&Moveset> = sets
        .0
        .values()
        .filter(|m| matches!(m.kind, Kind::Unarmed | Kind::Melee | Kind::Ranged) && !m.name.starts_with(VERSUS_PREFIX))
        .collect();
    v.sort_by(|a, b| (a.kind != Kind::Unarmed, &a.name).cmp(&(b.kind != Kind::Unarmed, &b.name)));
    v.truncate(RACK);
    v
}

fn code(name: &str) -> String {
    super::hud::hotbar::code(name)
}

fn spawn_ui(mut commands: Commands) {
    let width = COLUMNS as f32 * (TILE + GAP) - GAP;
    commands
        .spawn((
            RackRoot,
            panel(
                "bag",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    right: px(super::hud::MARGIN * 4.0),
                    top: percent(28.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(12.0)),
                    row_gap: px(8.0),
                    ..default()
                },
            ),
            Tweens::default(),
            GlobalZIndex(40),
            Interaction::default(),
        ))
        .with_children(|panel| {
            panel.spawn(label("▌WEAPON RACK", "title"));
            panel
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    width: px(width),
                    column_gap: px(GAP),
                    row_gap: px(GAP),
                    ..default()
                })
                .with_children(|grid| {
                    for i in 0..RACK {
                        grid.spawn((RackSlot(i), slot("slot", TILE, ""), Tooltip(String::new())));
                    }
                });
            panel.spawn((
                RackFooter,
                label("", "text.dim"),
                Typewriter::new("", FOOTER_RATE),
                Node {
                    width: px(width),
                    min_height: px(32.0),
                    ..default()
                },
            ));
        });
}

/// The player opening a chest opens the rack.
fn open_on_chest(
    mut effects: MessageReader<UseEffect>,
    players: Query<(), With<Player>>,
    chests: Query<(&Interactable, &PropState)>,
    mut open: ResMut<ArmoryOpen>,
    mut roots: Query<(&mut Node, &mut Tweens), With<RackRoot>>,
) {
    for e in effects.read() {
        let Ok((it, prop)) = chests.get(e.target) else { continue };
        if it.kind != "chest" || !players.contains(e.user) || !prop.on {
            continue;
        }
        open.0 = Some(e.target);
        for (mut node, mut tweens) in &mut roots {
            node.display = Display::Flex;
            tweens.play(Tween::opacity(0.0, 1.0, 0.14));
            tweens.play(Tween::offset(Vec2::new(12.0, 0.0), Vec2::ZERO, 0.18));
        }
    }
}

/// Esc, a closed chest or walking away closes the rack.
fn close(
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<ArmoryOpen>,
    chests: Query<(&GlobalTransform, &PropState)>,
    players: Query<&GlobalTransform, With<Player>>,
    mut roots: Query<&mut Node, With<RackRoot>>,
) {
    let Some(chest) = open.0 else { return };
    let gone = chests.get(chest).map_or(true, |(tf, prop)| {
        let far = players
            .iter()
            .next()
            .is_none_or(|p| p.translation().distance(tf.translation()) > REACH);
        !prop.on || far
    });
    if keys.just_pressed(KeyCode::Escape) || gone {
        open.0 = None;
        for mut node in &mut roots {
            node.display = Display::None;
        }
    }
}

/// Slot labels and checks follow the weapons and the wielded one.
#[allow(clippy::type_complexity)]
fn refresh(
    open: Res<ArmoryOpen>,
    sets: Res<Movesets>,
    player: Query<&Loadout, With<Player>>,
    mut slots: Query<(&RackSlot, &Children, &mut UiFlags, &mut Tooltip, &mut Visibility)>,
    mut labels: Query<&mut Text, With<SlotLabel>>,
) {
    if open.0.is_none() {
        return;
    }
    let rack = weapons(&sets);
    let wielded = player.iter().next().map(|l| l.equipped().to_string());
    for (slot, children, mut flags, mut tip, mut vis) in &mut slots {
        let set = rack.get(slot.0);
        vis.set_if_neq(if set.is_some() { Visibility::Inherited } else { Visibility::Hidden });
        let checked = set.is_some_and(|s| Some(&s.name) == wielded.as_ref());
        if flags.checked != checked {
            flags.checked = checked;
        }
        let want_tip = set.map_or(String::new(), |s| format!("Wield {}", s.name));
        if tip.0 != want_tip {
            tip.0 = want_tip;
        }
        let want = set.map_or(String::new(), |s| code(&s.name));
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0 != want
            {
                text.0 = want.clone();
            }
        }
    }
}

/// A click wields the weapon.
fn pick(
    open: Res<ArmoryOpen>,
    sets: Res<Movesets>,
    player: Query<Entity, With<Player>>,
    slots: Query<(&RackSlot, Ref<Interaction>)>,
    mut equip: MessageWriter<Equip>,
) {
    if open.0.is_none() {
        return;
    }
    let Some(who) = player.iter().next() else { return };
    let rack = weapons(&sets);
    for (slot, interaction) in &slots {
        if interaction.is_changed()
            && *interaction == Interaction::Pressed
            && let Some(set) = rack.get(slot.0)
        {
            equip.write(Equip {
                who,
                set: set.name.clone(),
            });
        }
    }
}

/// One line about a weapon: its attacks' damage.
pub fn describe(set: &Moveset) -> String {
    if let Some(p) = &set.projectile {
        return format!(
            "{}  //  {} {:.0} dmg{}  every {:.2} s",
            set.name.to_uppercase(),
            p.kind,
            p.damage,
            if p.count > 1 { format!(" ×{}", p.count) } else { String::new() },
            set.cooldown
        );
    }
    let list = |v: &[super::moveset::Attack]| {
        v.iter().map(|a| format!("{:.0}", a.damage)).collect::<Vec<_>>().join("/")
    };
    let mut s = format!("{}  //  light {}", set.name.to_uppercase(), list(&set.light));
    if !set.heavy.is_empty() {
        s += &format!("  heavy {}", list(&set.heavy));
    }
    s
}

fn hover(
    open: Res<ArmoryOpen>,
    sets: Res<Movesets>,
    slots: Query<(&RackSlot, &Interaction)>,
    mut footer: Query<&mut Typewriter, With<RackFooter>>,
) {
    if open.0.is_none() {
        return;
    }
    let rack = weapons(&sets);
    let hovered = slots.iter().find(|(_, i)| **i != Interaction::None).and_then(|(s, _)| rack.get(s.0));
    let line = hovered.map_or_else(|| "Pick a weapon to wield.  [Esc] close".to_string(), |s| describe(s));
    for mut tw in &mut footer {
        tw.retype(&line);
    }
}

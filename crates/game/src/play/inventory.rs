//! The bag: `Inventory` on the player (required by `Player`), `SLOTS` stacks. `inventory` alone
//! changes it: it applies `items::ItemCollected` and `SpendItems` (the guild buying), answers `items::ItemRejected` with whatever
//! doesn't fit, and reports every change in the HUD log ("+3 Log", one line per kind that counts
//! up while pickups keep coming; "Bag full", also when `items::BagFull` says something in reach
//! has no room). I, or a `ToggleBag` message (the hotbar's bag button), opens the bag: a `gbui`
//! panel at the bottom right with a 5 × 4 grid of slots (item codes in the item's colour, counts)
//! and a footer that types out the hovered item's description. `camera` releases the cursor while
//! it is open (`BagOpen`, written here only).
use super::Player;
use super::camera::PageToggles;
use super::hud::MARGIN;
use super::hud::log::{HudLog, LogKind};
use super::items::{BagFull, ItemCollected, ItemKind, ItemRejected};
use bevy::prelude::*;
use gbui::prelude::*;

pub const SLOTS: usize = 20;
const COLUMNS: usize = 5;
const TILE: f32 = 56.0;
const GAP: f32 = 5.0;
/// Pickups of one kind within this many seconds of each other count up on one log line.
const MERGE: f32 = 1.5;
/// Footer typing rate (chars/s).
const FOOTER_RATE: f32 = 110.0;
/// Log key for "Bag full" (item kinds use their index).
const FULL_KEY: u64 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    pub kind: ItemKind,
    pub count: u32,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    pub slots: [Option<Stack>; SLOTS],
}

impl Default for Inventory {
    fn default() -> Self {
        Self { slots: [None; SLOTS] }
    }
}

impl Inventory {
    /// A bag rebuilt from a save (`savegame`): `(slot, kind, count)`; slots out of range, empty
    /// stacks and duplicate slots are skipped, counts capped at the stack size.
    pub fn restored(stacks: &[(usize, ItemKind, u32)]) -> Self {
        let mut inv = Self::default();
        for &(slot, kind, count) in stacks {
            if slot < SLOTS && count > 0 && inv.slots[slot].is_none() {
                inv.slots[slot] = Some(Stack { kind, count: count.min(kind.stack()) });
            }
        }
        inv
    }
    /// Put `n` of `kind` in, topping up stacks of that kind before taking empty slots. Returns
    /// how many did not fit.
    pub fn add(&mut self, kind: ItemKind, n: u32) -> u32 {
        let max = kind.stack();
        let mut left = n;
        for stack in self.slots.iter_mut().flatten().filter(|s| s.kind == kind) {
            let take = left.min(max.saturating_sub(stack.count));
            stack.count += take;
            left -= take;
        }
        for slot in self.slots.iter_mut().filter(|s| s.is_none()) {
            if left == 0 {
                break;
            }
            let take = left.min(max);
            *slot = Some(Stack { kind, count: take });
            left -= take;
        }
        left
    }

    /// Whether `n` of `kind` would all fit.
    pub fn fits(&self, kind: ItemKind, n: u32) -> bool {
        let max = kind.stack();
        let room: u32 = self
            .slots
            .iter()
            .map(|s| match s {
                None => max,
                Some(s) if s.kind == kind => max.saturating_sub(s.count),
                Some(_) => 0,
            })
            .sum();
        room >= n
    }

    pub fn count(&self, kind: ItemKind) -> u32 {
        self.slots.iter().flatten().filter(|s| s.kind == kind).map(|s| s.count).sum()
    }

    /// Take `n` of `kind` out (from the last stacks first); false and unchanged if there are
    /// fewer.
    pub fn remove(&mut self, kind: ItemKind, n: u32) -> bool {
        if self.count(kind) < n {
            return false;
        }
        let mut left = n;
        for slot in self.slots.iter_mut().rev() {
            if left == 0 {
                break;
            }
            if let Some(stack) = slot
                && stack.kind == kind
            {
                let take = left.min(stack.count);
                stack.count -= take;
                left -= take;
                if stack.count == 0 {
                    *slot = None;
                }
            }
        }
        true
    }
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Player, Inventory>()
        .init_resource::<BagOpen>()
        .add_message::<ToggleBag>()
        .add_message::<SpendItems>()
        .add_systems(Startup, spawn_ui)
        .add_systems(Update, (collect, spend, toggle.in_set(PageToggles), refresh, hover).chain());
}

/// Take items out of a bag (sold at the guild desk); nothing happens unless all of them are there.
#[derive(Message, Clone, Copy, Debug)]
pub struct SpendItems {
    pub who: Entity,
    pub kind: ItemKind,
    pub count: u32,
}

fn spend(mut spends: MessageReader<SpendItems>, mut bags: Query<&mut Inventory>, mut log: MessageWriter<HudLog>) {
    for s in spends.read() {
        if let Ok(mut inv) = bags.get_mut(s.who)
            && s.count > 0
            && inv.remove(s.kind, s.count)
        {
            log.write(HudLog::new(LogKind::Item, format!("-{} {}", s.count, s.kind.name())));
        }
    }
}

/// Whether the bag panel is open.
#[derive(Resource, Default)]
pub struct BagOpen(pub bool);

/// Open the bag if it is closed, close it if open (the hotbar's bag button).
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct ToggleBag;

#[derive(Component)]
struct BagRoot;

#[derive(Component)]
struct BagSlot(usize);

#[derive(Component)]
struct BagUsed;

#[derive(Component)]
struct BagFooter;

/// Three-letter code shown on a slot.
fn code(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Log => "LOG",
        ItemKind::Stick => "STK",
        ItemKind::Stone => "STN",
        ItemKind::Apple => "APL",
    }
}

fn spawn_ui(mut commands: Commands) {
    let width = COLUMNS as f32 * (TILE + GAP) - GAP;
    commands
        .spawn((
            BagRoot,
            panel(
                "bag",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    right: px(MARGIN),
                    bottom: px(MARGIN),
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
            panel
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|head| {
                    head.spawn(label("▌BAG", "title"));
                    head.spawn((BagUsed, label("", "text.dim")));
                });
            panel
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    width: px(width),
                    column_gap: px(GAP),
                    row_gap: px(GAP),
                    ..default()
                })
                .with_children(|grid| {
                    for i in 0..SLOTS {
                        grid.spawn((BagSlot(i), slot("slot", TILE, "")));
                    }
                });
            panel.spawn((
                BagFooter,
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

/// Pickups counting up on one log line: kind (None = "Bag full"), running count, seconds since
/// the last one.
struct Recent {
    kind: Option<ItemKind>,
    count: u32,
    age: f32,
}

/// Add `count` to the frame's news for `kind` (None = "Bag full").
fn note(news: &mut Vec<(Option<ItemKind>, u32)>, kind: Option<ItemKind>, count: u32) {
    match news.iter_mut().find(|(k, _)| *k == kind) {
        Some((_, n)) => *n += count,
        None => news.push((kind, count)),
    }
}

/// Apply collected items to the bag; refuse the rest; log the change.
#[allow(clippy::too_many_arguments)]
fn collect(
    time: Res<Time<Real>>,
    mut collected: MessageReader<ItemCollected>,
    mut full: MessageReader<BagFull>,
    mut rejected: MessageWriter<ItemRejected>,
    mut log: MessageWriter<HudLog>,
    mut bags: Query<&mut Inventory>,
    players: Query<(), With<Player>>,
    mut recent: Local<Vec<Recent>>,
) {
    let dt = time.delta_secs().min(0.1);
    recent.retain_mut(|r| {
        r.age += dt;
        r.age < MERGE
    });
    // Everything that reached the player this frame, per kind: a burst of pickups landing
    // together (a felled tree's logs) makes one line, not one each.
    let mut news: Vec<(Option<ItemKind>, u32)> = Vec::new();
    for c in collected.read() {
        let Ok(mut bag) = bags.get_mut(c.who) else { continue };
        let left = bag.add(c.kind, c.count);
        if left > 0 {
            rejected.write(ItemRejected {
                who: c.who,
                kind: c.kind,
                count: left,
            });
        }
        if players.contains(c.who) {
            if c.count > left {
                note(&mut news, Some(c.kind), c.count - left);
            }
            if left > 0 {
                note(&mut news, None, 0);
            }
        }
    }
    for f in full.read() {
        if players.contains(f.who) {
            note(&mut news, None, 0);
        }
    }
    for (kind, count) in news {
        let total = match recent.iter_mut().find(|r| r.kind == kind) {
            Some(r) => {
                r.count += count;
                r.age = 0.0;
                r.count
            }
            None => {
                recent.push(Recent { kind, count, age: 0.0 });
                count
            }
        };
        log.write(match kind {
            Some(k) => HudLog::new(LogKind::Item, format!("+{total} {}", k.name())).key(k as u64),
            None => HudLog::new(LogKind::Warn, "Bag full").key(FULL_KEY),
        });
    }
}

/// I or `ToggleBag` toggles the bag, Escape closes it (the camera frees the cursor while it is
/// open). Opening fades and lifts the panel in.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggles: MessageReader<ToggleBag>,
    mut open: ResMut<BagOpen>,
    mut roots: Query<(&mut Node, &mut Tweens), With<BagRoot>>,
) {
    let was = open.0;
    let flips = toggles.read().count() + usize::from(keys.just_pressed(KeyCode::KeyI));
    if flips % 2 == 1 {
        open.0 = !open.0;
    }
    if keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
    }
    if open.0 != was {
        for (mut node, mut tweens) in &mut roots {
            node.display = if open.0 { Display::Flex } else { Display::None };
            if open.0 {
                tweens.play(Tween::opacity(0.0, 1.0, 0.14));
                tweens.play(Tween::offset(Vec2::new(0.0, 12.0), Vec2::ZERO, 0.18));
            }
        }
    }
}

/// Slots mirror the player's bag while it is open.
#[allow(clippy::type_complexity)]
fn refresh(
    open: Res<BagOpen>,
    bags: Query<&Inventory, With<Player>>,
    slots: Query<(&BagSlot, &Children)>,
    mut labels: Query<(&mut Text, &mut Tint), (With<SlotLabel>, Without<SlotCount>)>,
    mut counts: Query<&mut Text, (With<SlotCount>, Without<SlotLabel>)>,
    mut used: Query<&mut Text, (With<BagUsed>, Without<SlotLabel>, Without<SlotCount>)>,
) {
    if !open.0 {
        return;
    }
    let Ok(bag) = bags.single() else { return };
    for (slot, children) in &slots {
        let stack = bag.slots[slot.0];
        for child in children.iter() {
            if let Ok((mut text, mut tint)) = labels.get_mut(child) {
                let s = stack.map_or("", |s| code(s.kind));
                if text.0 != s {
                    text.0 = s.to_string();
                }
                tint.set_if_neq(Tint {
                    text: stack.map(|s| item_text_color(s.kind)),
                    ..default()
                });
            }
            if let Ok(mut text) = counts.get_mut(child) {
                let s = stack.map_or(String::new(), |s| s.count.to_string());
                if text.0 != s {
                    text.0 = s;
                }
            }
        }
    }
    let s = format!("{}/{SLOTS}", bag.slots.iter().flatten().count());
    for mut text in &mut used {
        if text.0 != s {
            text.0 = s.clone();
        }
    }
}

/// The item colour lifted toward white so it reads on the dark glass.
fn item_text_color(kind: ItemKind) -> Color {
    let c = kind.color().to_srgba();
    Color::srgb(0.45 + 0.55 * c.red, 0.45 + 0.55 * c.green, 0.45 + 0.55 * c.blue)
}

/// The footer types out the hovered slot.
fn hover(
    open: Res<BagOpen>,
    bags: Query<&Inventory, With<Player>>,
    slots: Query<(&BagSlot, &Interaction)>,
    mut footer: Query<&mut Typewriter, With<BagFooter>>,
) {
    if !open.0 {
        return;
    }
    let Ok(bag) = bags.single() else { return };
    let hovered = slots
        .iter()
        .find(|(_, i)| **i != Interaction::None)
        .and_then(|(s, _)| bag.slots[s.0]);
    let line = match hovered {
        Some(s) => format!("{} x{}  // {}", s.kind.name(), s.count, s.kind.description()),
        None => {
            let total: u32 = bag.slots.iter().flatten().map(|s| s.count).sum();
            format!("{total} items. Hover a slot to read it.  [I] close")
        }
    };
    for mut tw in &mut footer {
        tw.retype(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_tops_up_then_fills_and_reports_overflow() {
        let mut bag = Inventory::default();
        assert_eq!(bag.add(ItemKind::Log, 25), 0);
        assert_eq!(bag.slots[0], Some(Stack { kind: ItemKind::Log, count: 20 }));
        assert_eq!(bag.slots[1], Some(Stack { kind: ItemKind::Log, count: 5 }));
        assert_eq!(bag.add(ItemKind::Log, 3), 0);
        assert_eq!(bag.slots[1].unwrap().count, 8, "tops up before a new slot");
        assert_eq!(bag.count(ItemKind::Log), 28);
        // Fill every other slot with stones; then the bag refuses what doesn't fit.
        assert_eq!(bag.add(ItemKind::Stone, 50 * 18), 0);
        assert!(!bag.fits(ItemKind::Apple, 1));
        assert!(bag.fits(ItemKind::Log, 12) && !bag.fits(ItemKind::Log, 13));
        assert_eq!(bag.add(ItemKind::Log, 15), 3);
        assert_eq!(bag.add(ItemKind::Apple, 2), 2);
    }

    #[test]
    fn remove_takes_from_the_back_and_clears_empty_slots() {
        let mut bag = Inventory::default();
        bag.add(ItemKind::Stick, 60);
        bag.add(ItemKind::Apple, 1);
        assert!(!bag.remove(ItemKind::Stick, 61), "not enough: unchanged");
        assert_eq!(bag.count(ItemKind::Stick), 60);
        assert!(bag.remove(ItemKind::Stick, 15));
        assert_eq!(bag.slots[0].unwrap().count, 45);
        assert!(bag.slots[1].is_none(), "the emptied stack frees its slot");
        assert!(bag.remove(ItemKind::Apple, 1));
        assert_eq!(bag.count(ItemKind::Apple), 0);
    }
}

//! The stats page: a `gbui` panel with two tabs, opened by the STATS button on the minimap's rim
//! (a dot lights on it while attribute points wait), K, or the hotbar's Q / E slots.
//! - STATS: level, experience, unspent points; the six attributes with a + button each (sends
//!   `stats::SpendPoint`); the derived values (`stats::Derived`).
//! - ARTS & MAGIC: every art (Q, Battlewill) and spell (E, magicules) with its cost and mastery;
//!   known ones bind to their key on click (`magic::Bind`), skills are tagged, locked ones show
//!   the level that teaches them.
//! The footer types out whatever is hovered. `StatsOpen` (written here only) frees the cursor
//! through `camera`.
use super::MARGIN;
use super::minimap;
use crate::play::camera::PageToggles;
use crate::play::magic::{ABILITIES, Bind, Grimoire, Pool, SKILL_COST};
use crate::play::stats::{Attr, Derived, LevelUp, POINTS_PER_LEVEL, SpendPoint, Stats};
use super::log::{HudLog, LogKind};
use crate::play::Player;
use bevy::prelude::*;
use gbui::prelude::*;

const WIDTH: f32 = 430.0;
const ROW: f32 = 24.0;
const FOOTER_RATE: f32 = 110.0;
/// The map button: size (px) and where on the rim it sits (rad from east, counter-clockwise).
const BUTTON: Vec2 = Vec2::new(52.0, 20.0);
const BUTTON_ANGLE: f32 = 0.78;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StatsTab {
    #[default]
    Stats,
    Arts,
}

/// The open tab, if the page is open.
#[derive(Resource, Default)]
pub struct StatsOpen(pub Option<StatsTab>);

/// Toggle the page (onto a tab).
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct ToggleStats(pub Option<StatsTab>);

pub fn plugin(app: &mut App) {
    app.init_resource::<StatsOpen>()
        .add_message::<ToggleStats>()
        .add_systems(Startup, spawn)
        .add_systems(Update, (toggle.in_set(PageToggles), clicks, refresh, hover, level_up).chain());
}

#[derive(Component)]
struct Root;
#[derive(Component)]
struct MapButton;
#[derive(Component)]
struct MapDot;
#[derive(Component)]
struct Tab(StatsTab);
#[derive(Component)]
struct Page(StatsTab);
#[derive(Component)]
struct Close;
#[derive(Component)]
struct Header;
#[derive(Component)]
struct AttrRow(Attr);
#[derive(Component)]
struct AttrValue(Attr);
#[derive(Component)]
struct Plus(Attr);
#[derive(Component)]
struct DerivedText;
#[derive(Component)]
struct AbilityRow(usize);
#[derive(Component)]
struct AbilityText(usize);
#[derive(Component)]
struct AbilityTag(usize);
#[derive(Component)]
struct Footer;

fn button(class: &'static str, node: Node) -> impl Bundle {
    (Button, node, Styled::new(class), UiFlags::default())
}

fn spawn(mut commands: Commands) {
    // The rim button, over the minimap's top-right edge.
    let r = minimap::SIZE * 0.5;
    let centre = Vec2::new(MARGIN + r, MARGIN + minimap::READOUT + r);
    let at = centre + Vec2::new(BUTTON_ANGLE.cos(), BUTTON_ANGLE.sin()) * r;
    commands
        .spawn((
            MapButton,
            button(
                "mapbutton",
                Node {
                    position_type: PositionType::Absolute,
                    left: px(at.x - BUTTON.x * 0.5),
                    bottom: px(at.y - BUTTON.y * 0.5),
                    width: px(BUTTON.x),
                    height: px(BUTTON.y),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ),
            Tooltip("Stats, arts & magic  [K]".into()),
            GlobalZIndex(30),
            Tweens::default(),
        ))
        .with_children(|b| {
            b.spawn(label("STATS", "mapbutton.text"));
            b.spawn((
                MapDot,
                Node {
                    position_type: PositionType::Absolute,
                    right: px(-4.0),
                    top: px(-4.0),
                    width: px(8.0),
                    height: px(8.0),
                    ..default()
                },
                Styled::new("mapbutton.dot"),
                Visibility::Hidden,
            ));
        });

    commands
        .spawn((
            Root,
            panel(
                "stats",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: px(MARGIN + minimap::SIZE + 24.0),
                    bottom: px(MARGIN + 110.0),
                    width: px(WIDTH),
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
        .with_children(|p| {
            // Title row: name, tabs, close.
            p.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(label("▌STATUS", "title"));
                row.spawn(Node {
                    column_gap: px(6.0),
                    ..default()
                })
                .with_children(|tabs| {
                    for (tab, text) in [(StatsTab::Stats, "STATS"), (StatsTab::Arts, "ARTS & MAGIC")] {
                        tabs.spawn((
                            Tab(tab),
                            button(
                                "tab",
                                Node {
                                    padding: UiRect::axes(px(9.0), px(3.0)),
                                    ..default()
                                },
                            ),
                        ))
                        .with_child(label(text, "tab.text"));
                    }
                    tabs.spawn((
                        Close,
                        button(
                            "tab",
                            Node {
                                padding: UiRect::axes(px(7.0), px(3.0)),
                                ..default()
                            },
                        ),
                        Tooltip("Close  [K / Esc]".into()),
                    ))
                    .with_child(label("×", "tab.text"));
                });
            });
            p.spawn((Header, label("", "text.accent")));

            // STATS page.
            p.spawn((
                Page(StatsTab::Stats),
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2.0),
                    ..default()
                },
            ))
            .with_children(|page| {
                for a in Attr::ALL {
                    page.spawn((
                        AttrRow(a),
                        button(
                            "row",
                            Node {
                                height: px(ROW),
                                align_items: AlignItems::Center,
                                column_gap: px(10.0),
                                padding: UiRect::horizontal(px(6.0)),
                                ..default()
                            },
                        ),
                    ))
                    .with_children(|r| {
                        r.spawn((label(a.code(), "row.tag"), Node { width: px(34.0), ..default() }));
                        r.spawn((label(a.name(), "row.text"), Node { flex_grow: 1.0, ..default() }));
                        r.spawn((AttrValue(a), label("", "row.text"), Node { width: px(30.0), ..default() }));
                        r.spawn((
                            Plus(a),
                            button(
                                "plus",
                                Node {
                                    width: px(20.0),
                                    height: px(18.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                            ),
                        ))
                        .with_child(label("+", "plus.text"));
                    });
                }
                page.spawn((
                    DerivedText,
                    label("", "text.dim"),
                    Node {
                        margin: UiRect::top(px(6.0)),
                        ..default()
                    },
                ));
            });

            // ARTS & MAGIC page.
            p.spawn((
                Page(StatsTab::Arts),
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2.0),
                    ..default()
                },
            ))
            .with_children(|page| {
                for (heading, art) in [("ARTS  [Q]  //  Battlewill", true), ("SPELLS  [E]  //  Magicules", false)] {
                    page.spawn((
                        label(heading, "text.dim"),
                        Node {
                            margin: UiRect::top(px(4.0)),
                            ..default()
                        },
                    ));
                    for (i, a) in ABILITIES.iter().enumerate().filter(|(_, a)| a.is_art() == art) {
                        let _ = a;
                        page.spawn((
                            AbilityRow(i),
                            button(
                                "row",
                                Node {
                                    height: px(ROW),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    padding: UiRect::horizontal(px(6.0)),
                                    ..default()
                                },
                            ),
                        ))
                        .with_children(|r| {
                            r.spawn((AbilityText(i), label("", "row.text")));
                            r.spawn((AbilityTag(i), label("", "row.tag")));
                        });
                    }
                }
            });

            p.spawn((
                Footer,
                label("", "text.dim"),
                Typewriter::new("", FOOTER_RATE),
                Node {
                    width: px(WIDTH - 24.0),
                    min_height: px(34.0),
                    ..default()
                },
            ));
        });
}

/// K, Esc, `ToggleStats` (map button, hotbar) and the close / tab buttons.
#[allow(clippy::type_complexity)]
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggles: MessageReader<ToggleStats>,
    mut open: ResMut<StatsOpen>,
    map: Query<Ref<Interaction>, With<MapButton>>,
    close: Query<Ref<Interaction>, With<Close>>,
    tabs: Query<(&Tab, Ref<Interaction>)>,
    mut roots: Query<(&mut Node, &mut Tweens), (With<Root>, Without<Page>)>,
    mut pages: Query<(&Page, &mut Node), Without<Root>>,
    mut tab_flags: Query<(&Tab, &mut UiFlags), Without<MapButton>>,
    mut map_flags: Query<&mut UiFlags, With<MapButton>>,
) {
    let was = open.0;
    let pressed = |i: &Ref<Interaction>| i.is_changed() && **i == Interaction::Pressed;
    let mut flip = |want: Option<StatsTab>| {
        open.0 = match (open.0, want) {
            (Some(now), Some(w)) if now != w => Some(w),
            (Some(_), _) => None,
            (None, w) => Some(w.unwrap_or_default()),
        };
    };
    for t in toggles.read() {
        flip(t.0);
    }
    if keys.just_pressed(KeyCode::KeyK) || map.iter().any(|i| pressed(&i)) {
        flip(None);
    }
    if keys.just_pressed(KeyCode::Escape) || close.iter().any(|i| pressed(&i)) {
        open.0 = None;
    }
    for (tab, i) in &tabs {
        if pressed(&i) && open.0.is_some() {
            open.0 = Some(tab.0);
        }
    }
    if open.0 == was {
        return;
    }
    for (mut node, mut tweens) in &mut roots {
        node.display = if open.0.is_some() { Display::Flex } else { Display::None };
        if was.is_none() && open.0.is_some() {
            tweens.play(Tween::opacity(0.0, 1.0, 0.14));
            tweens.play(Tween::offset(Vec2::new(-12.0, 0.0), Vec2::ZERO, 0.18));
        }
    }
    for (page, mut node) in &mut pages {
        node.display = if Some(page.0) == open.0 { Display::Flex } else { Display::None };
    }
    for (tab, mut flags) in &mut tab_flags {
        flags.checked = Some(tab.0) == open.0;
    }
    for mut flags in &mut map_flags {
        flags.checked = open.0.is_some();
    }
}

/// + buttons spend points; ability rows bind.
#[allow(clippy::type_complexity)]
fn clicks(
    open: Res<StatsOpen>,
    player: Query<Entity, With<Player>>,
    plus: Query<(&Plus, Ref<Interaction>)>,
    rows: Query<(&AbilityRow, Ref<Interaction>)>,
    mut spend: MessageWriter<SpendPoint>,
    mut bind: MessageWriter<Bind>,
) {
    if open.0.is_none() {
        return;
    }
    let Some(who) = player.iter().next() else { return };
    for (p, i) in &plus {
        if i.is_changed() && *i == Interaction::Pressed {
            spend.write(SpendPoint { who, attr: p.0 });
        }
    }
    for (r, i) in &rows {
        if i.is_changed() && *i == Interaction::Pressed {
            bind.write(Bind { who, id: ABILITIES[r.0].id });
        }
    }
}

#[allow(clippy::type_complexity)]
fn refresh(
    open: Res<StatsOpen>,
    player: Query<(&Stats, Option<&Derived>, Option<&Grimoire>), With<Player>>,
    mut texts: ParamSet<(
        Query<(&AttrValue, &mut Text)>,
        Query<&mut Text, With<Header>>,
        Query<&mut Text, With<DerivedText>>,
        Query<(&AbilityText, &mut Text)>,
        Query<(&AbilityTag, &mut Text)>,
    )>,
    mut plus: Query<(&Plus, &mut UiFlags), Without<AbilityRow>>,
    mut rows: Query<(&AbilityRow, &mut UiFlags), Without<Plus>>,
    mut dot: Query<&mut Visibility, With<MapDot>>,
) {
    let Ok((s, d, g)) = player.single() else { return };
    for mut v in &mut dot {
        v.set_if_neq(if s.points > 0 { Visibility::Inherited } else { Visibility::Hidden });
    }
    if open.0.is_none() {
        return;
    }
    let set = |t: &mut Text, v: String| {
        if t.0 != v {
            t.0 = v;
        }
    };
    for (a, mut t) in &mut texts.p0() {
        set(&mut t, s.get(a.0).to_string());
    }
    for mut t in &mut texts.p1() {
        set(
            &mut t,
            format!("LEVEL {}   XP {}/{}   POINTS {}", s.level, s.xp, s.xp_to_next(), s.points),
        );
    }
    if let Some(d) = d {
        for mut t in &mut texts.p2() {
            set(
                &mut t,
                format!(
                    "HP {:.0}  ST {:.0}  POISE {:.0}  BW {:.0}  MP {:.0}\nMIGHT ×{:.2}  SPELL ×{:.2}  CAST ×{:.2}  REGEN ×{:.2}",
                    d.max_hp, d.max_stamina, d.max_poise, d.max_battlewill, d.max_mana - g.map_or(0.0, |g| g.debt),
                    d.might, d.spell, d.cast_speed, d.regen
                ),
            );
        }
    }
    for (p, mut flags) in &mut plus {
        let off = s.points == 0 || s.get(p.0) >= crate::play::stats::MAX_ATTR;
        if flags.disabled != off {
            flags.disabled = off;
        }
    }
    let g = g.cloned().unwrap_or_default();
    for (r, mut flags) in &mut rows {
        let a = &ABILITIES[r.0];
        let bound = g.art == a.id || g.spell == a.id;
        let locked = a.level > s.level;
        if flags.checked != bound {
            flags.checked = bound;
        }
        if flags.disabled != locked {
            flags.disabled = locked;
        }
    }
    for (r, mut t) in &mut texts.p3() {
        let a = &ABILITIES[r.0];
        let pool = if a.pool == Pool::Battlewill { "BW" } else { "MP" };
        let cost = a.cost * if g.is_skill(a.id) { SKILL_COST } else { 1.0 };
        let line = if a.level > s.level {
            format!("{}", a.name)
        } else {
            format!("{}   {cost:.0} {pool}   {:.0}%", a.name, g.mastery(a.id) * 100.0)
        };
        set(&mut t, line);
    }
    for (r, mut t) in &mut texts.p4() {
        let a = &ABILITIES[r.0];
        let tag = if a.level > s.level {
            format!("LV {}", a.level)
        } else if g.is_skill(a.id) {
            "SKILL".to_string()
        } else if g.art == a.id {
            "[Q]".to_string()
        } else if g.spell == a.id {
            "[E]".to_string()
        } else {
            String::new()
        };
        set(&mut t, tag);
    }
}

/// The footer explains what the cursor is over.
fn hover(
    open: Res<StatsOpen>,
    attrs: Query<(&AttrRow, &Interaction)>,
    rows: Query<(&AbilityRow, &Interaction)>,
    mut footer: Query<&mut Typewriter, With<Footer>>,
) {
    let Some(tab) = open.0 else { return };
    let line = attrs
        .iter()
        .find(|(_, i)| **i != Interaction::None)
        .map(|(a, _)| format!("{}: {}", a.0.name(), a.0.description()))
        .or_else(|| {
            rows.iter().find(|(_, i)| **i != Interaction::None).map(|(r, _)| {
                let a = &ABILITIES[r.0];
                format!("{} (from level {}): {} Click to bind.", a.name, a.level, a.about)
            })
        })
        .unwrap_or_else(|| match tab {
            StatsTab::Stats => "Levels come from defeating foes. Spend points with +.".to_string(),
            StatsTab::Arts => {
                "Landing an ability grows its mastery; mastered, it becomes a Skill (cheaper, faster, stronger) at a permanent cost of magicules."
                    .to_string()
            }
        });
    for mut tw in &mut footer {
        tw.retype(&line);
    }
}

/// A level gained: a log line and a pulse on the map button.
fn level_up(
    mut ups: MessageReader<LevelUp>,
    players: Query<(), With<Player>>,
    mut log: MessageWriter<HudLog>,
    mut button: Query<&mut Tweens, With<MapButton>>,
) {
    for up in ups.read() {
        if !players.contains(up.who) {
            continue;
        }
        log.write(HudLog::new(
            LogKind::Item,
            format!("LEVEL {}  //  +{POINTS_PER_LEVEL} points  [K]", up.level),
        ));
        for mut t in &mut button {
            t.play(Tween::scale(1.35, 1.0, 0.5).ease(Easing::OutBack));
        }
    }
}

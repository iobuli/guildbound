//! The stats page: a `gbui` panel with four tabs, opened by the STATS button on the minimap's rim
//! (a dot lights on it while attribute points wait), K, or the hotbar's Q / E slots.
//! - STATS: level, experience, unspent points; the six attributes with a + button each (sends
//!   `stats::SpendPoint`); the derived values (`stats::Derived`).
//! - ARTS & MAGIC: every art (Q, Battlewill) and spell (E, magicules) with its cost and mastery;
//!   discovered ones bind to their key on click (`magic::Bind`), mastered ones are tagged,
//!   undiscovered ones show the skill levels that make them discoverable.
//! - SKILLS: every discovered skill (`skills::Sheet`) with its exact level and XP to the next, and
//!   the undiscovered skills the character is closest to finding (affinity toward the pity floor),
//!   and the titles earned (`skills::TitleBook`).
//! - WORK: the three open contracts (`contracts::Board`) with exact progress.
//! The footer types out whatever is hovered. Smoke: `GB_STATS=stats|arts|skills|contracts` opens that tab
//! 1 s in (for headless screenshots). `StatsOpen` (written here only) frees the cursor
//! through `camera`.
use super::MARGIN;
use super::minimap;
use crate::play::camera::PageToggles;
use crate::play::magic::{ABILITIES, Ability, Bind, Grimoire, MASTERED_COST, Pool};
use crate::play::contracts::{self, Board, ContractBook};
use crate::play::traits::{SetTrait, Slot, TraitBook, Traits};
use crate::play::skills::{Sheet, SkillBook, SkillDef, TitleBook, level_for_xp, xp_for_level};
use crate::play::stats::{Attr, Derived, LevelUp, POINTS_PER_LEVEL, SpendPoint, Stats};
use super::log::{HudLog, LogKind};
use crate::play::Player;
use bevy::prelude::*;
use gbui::prelude::*;

const WIDTH: f32 = 470.0;
const ROW: f32 = 24.0;
const SKILL_ROW: f32 = 21.0;
/// Extra SKILLS rows beyond the book at startup.
const SPARE_ROWS: usize = 8;
/// Undiscovered skills hinted at on the SKILLS tab.
const HINTS: usize = 3;
const FOOTER_RATE: f32 = 110.0;
/// The map button: size (px) and where on the rim it sits (rad from east, counter-clockwise).
const BUTTON: Vec2 = Vec2::new(52.0, 20.0);
const BUTTON_ANGLE: f32 = 0.78;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StatsTab {
    #[default]
    Stats,
    Arts,
    Skills,
    Contracts,
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
        .add_systems(Update, smoke.before(PageToggles))
        .add_systems(
            Update,
            (toggle.in_set(PageToggles), clicks, refresh, refresh_skills, refresh_madlib, refresh_contracts, hover, level_up).chain(),
        );
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
struct SkillRow(usize);
#[derive(Component)]
struct SkillText(usize);
#[derive(Component)]
struct SkillTag(usize);
#[derive(Component)]
struct SkillHint;
#[derive(Component)]
struct ContractName(usize);
#[derive(Component)]
struct ContractInfo(usize);
#[derive(Component)]
struct ContractsDone;
#[derive(Component)]
struct MadlibWord(Slot);
#[derive(Component)]
struct MadlibText(Slot);
#[derive(Component)]
struct Footer;

fn button(class: &'static str, node: Node) -> impl Bundle {
    (Button, node, Styled::new(class), UiFlags::default())
}

fn spawn(mut commands: Commands, book: Res<SkillBook>) {
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
                    for (tab, text) in [(StatsTab::Stats, "STATS"), (StatsTab::Arts, "ARTS & MAGIC"), (StatsTab::Skills, "SKILLS"), (StatsTab::Contracts, "WORK")] {
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
                // The madlib: who this character is (click a word to change it).
                page.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::Center,
                    column_gap: px(5.0),
                    row_gap: px(3.0),
                    margin: UiRect::top(px(6.0)),
                    ..default()
                })
                .with_children(|m| {
                    let word = |m: &mut ChildSpawnerCommands, slot: Slot| {
                        m.spawn((
                            MadlibWord(slot),
                            button(
                                "tab",
                                Node {
                                    padding: UiRect::axes(px(6.0), px(1.0)),
                                    ..default()
                                },
                            ),
                            Tooltip("Click for another word".into()),
                        ))
                        .with_child((MadlibText(slot), label("…", "tab.text")));
                    };
                    m.spawn(label("I'm a", "text.dim"));
                    word(m, Slot::Temper);
                    m.spawn(label("fighter who walks", "text.dim"));
                    word(m, Slot::Walk);
                    m.spawn(label("and fights", "text.dim"));
                    word(m, Slot::Fight);
                });
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
                    for (i, _) in ABILITIES.iter().enumerate().filter(|(_, a)| a.is_art() == art) {
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

            // SKILLS page.
            p.spawn((
                Page(StatsTab::Skills),
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(1.0),
                    ..default()
                },
            ))
            .with_children(|page| {
                // Spare rows for skills added by a hot reload.
                for i in 0..book.0.len() + SPARE_ROWS {
                    page.spawn((
                        SkillRow(i),
                        button(
                            "row",
                            Node {
                                display: Display::None,
                                height: px(SKILL_ROW),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::horizontal(px(6.0)),
                                ..default()
                            },
                        ),
                    ))
                    .with_children(|r| {
                        r.spawn((SkillText(i), label("", "row.text")));
                        r.spawn((SkillTag(i), label("", "row.tag")));
                    });
                }
                page.spawn((
                    SkillHint,
                    label("", "text.dim"),
                    Node {
                        margin: UiRect::top(px(6.0)),
                        ..default()
                    },
                ));
            });

            // CONTRACTS page.
            p.spawn((
                Page(StatsTab::Contracts),
                Node {
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6.0),
                    ..default()
                },
            ))
            .with_children(|page| {
                for i in 0..contracts::OPEN {
                    page.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(1.0),
                        padding: UiRect::horizontal(px(6.0)),
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((ContractName(i), label("", "row.text")));
                        c.spawn((ContractInfo(i), label("", "text.dim")));
                    });
                }
                page.spawn((ContractsDone, label("", "text.accent")));
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

/// `GB_STATS=<tab>`: open the page on that tab once, 1 s in.
fn smoke(time: Res<Time>, mut done: Local<bool>, mut toggles: MessageWriter<ToggleStats>) {
    if *done || time.elapsed_secs() < 1.0 {
        return;
    }
    *done = true;
    let tab = match std::env::var("GB_STATS").unwrap_or_default().trim() {
        "stats" => StatsTab::Stats,
        "arts" => StatsTab::Arts,
        "skills" => StatsTab::Skills,
        "contracts" | "work" => StatsTab::Contracts,
        _ => return,
    };
    toggles.write(ToggleStats(Some(tab)));
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
    words: Query<(&MadlibWord, Ref<Interaction>)>,
    mut spend: MessageWriter<SpendPoint>,
    mut bind: MessageWriter<Bind>,
    mut traits: MessageWriter<SetTrait>,
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
    for (w, i) in &words {
        if i.is_changed() && *i == Interaction::Pressed {
            traits.write(SetTrait { who, slot: w.0 });
        }
    }
}

#[allow(clippy::type_complexity)]
fn refresh(
    open: Res<StatsOpen>,
    book: Res<SkillBook>,
    player: Query<(&Stats, Option<&Derived>, Option<&Grimoire>, Option<&Sheet>), With<Player>>,
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
    let Ok((s, d, g, sheet)) = player.single() else { return };
    let found = |a: &Ability| sheet.is_some_and(|sh| sh.has_ability(a.id));
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
        let locked = !found(a);
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
        let cost = a.cost * if g.is_mastered(a.id) { MASTERED_COST } else { 1.0 };
        let line = if !found(a) {
            "· · ·".to_string()
        } else {
            format!("{}   {cost:.0} {pool}   {:.0}%", a.name, g.mastery(a.id) * 100.0)
        };
        set(&mut t, line);
    }
    for (r, mut t) in &mut texts.p4() {
        let a = &ABILITIES[r.0];
        let tag = if !found(a) {
            requirement(&book, a, sheet)
        } else if g.is_mastered(a.id) {
            "MASTERED".to_string()
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

/// What an undiscovered ability waits on: "Aura Arts 5" (met requirements are dropped; all met
/// reads "SENSED").
fn requirement(book: &SkillBook, a: &Ability, sheet: Option<&Sheet>) -> String {
    let left: Vec<String> = book
        .requires(a.id)
        .into_iter()
        .filter(|(s, lvl)| sheet.map_or(0, |sh| sh.level(&s.id)) < *lvl)
        .map(|(s, lvl)| format!("{} {lvl}", short(&s.name)))
        .collect();
    if left.is_empty() { "SENSED".to_string() } else { left.join(" + ") }
}

/// "Fire Manipulation" → "Fire".
fn short(name: &str) -> &str {
    name.strip_suffix(" Manipulation").unwrap_or(name)
}

/// One line about a skill: family, bonus per level, abilities it opens.
fn describe(s: &SkillDef) -> String {
    let b = &s.bonus;
    let mut per = Vec::new();
    for (v, what, pct) in [
        (b.max_hp, "HP", false),
        (b.max_stamina, "ST", false),
        (b.max_battlewill, "BW", false),
        (b.max_mana, "MP", false),
        (b.might, "might", true),
        (b.spell, "spell", true),
        (b.cast_speed, "cast", true),
    ] {
        if v != 0.0 {
            per.push(if pct { format!("+{:.1}% {what}", v * 100.0) } else { format!("+{v:.1} {what}") });
        }
    }
    let opens: Vec<String> = s
        .unlocks
        .iter()
        .filter_map(|u| ABILITIES.iter().find(|a| a.id == u.ability).map(|a| format!("{} at {}", a.name, u.level)))
        .collect();
    let mut line = format!("{} ({}): {} per level.", s.name, s.family, if per.is_empty() { "—".into() } else { per.join(", ") });
    if !opens.is_empty() {
        line += &format!(" Opens {}.", opens.join(", "));
    }
    line
}

/// The SKILLS tab: discovered skills with exact numbers, the nearest undiscovered ones.
#[allow(clippy::type_complexity)]
fn refresh_skills(
    open: Res<StatsOpen>,
    book: Res<SkillBook>,
    titles: Res<TitleBook>,
    player: Query<&Sheet, With<Player>>,
    mut rows: Query<(&SkillRow, &mut Node)>,
    mut texts: ParamSet<(
        Query<(&SkillText, &mut Text)>,
        Query<(&SkillTag, &mut Text)>,
        Query<&mut Text, With<SkillHint>>,
    )>,
) {
    if open.0 != Some(StatsTab::Skills) {
        return;
    }
    let Ok(sheet) = player.single() else { return };
    let set = |t: &mut Text, v: String| {
        if t.0 != v {
            t.0 = v;
        }
    };
    for (r, mut node) in &mut rows {
        let show = book.0.get(r.0).is_some_and(|s| sheet.knows(&s.id));
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (r, mut t) in &mut texts.p0() {
        let Some(s) = book.0.get(r.0) else { continue };
        set(&mut t, format!("{}   LV {}", s.name, sheet.level(&s.id)));
    }
    for (r, mut t) in &mut texts.p1() {
        let Some(s) = book.0.get(r.0) else { continue };
        let xp = sheet.xp.get(&s.id).copied().unwrap_or(0.0);
        let level = level_for_xp(xp);
        let tag = if level >= 99 {
            format!("{xp:.0} XP  MAX")
        } else {
            format!("{xp:.0} / {:.0} XP", xp_for_level(level + 1))
        };
        set(&mut t, tag);
    }
    let mut near: Vec<(&SkillDef, f32)> = book
        .0
        .iter()
        .filter(|s| !sheet.knows(&s.id))
        .filter_map(|s| sheet.affinity.get(&s.id).map(|a| (s, (a / s.discovery.pity).min(1.0))))
        .filter(|(_, f)| *f > 0.0)
        .collect();
    near.sort_by(|a, b| b.1.total_cmp(&a.1));
    let known = sheet.xp.len();
    let mut hint = if known == 0 {
        "No skills yet: they are found by doing.".to_string()
    } else {
        format!("{known} of {} skills discovered.", book.0.len())
    };
    if !sheet.titles.is_empty() {
        let names: Vec<&str> = sheet.titles.iter().map(|id| titles.get(id).map_or(id.as_str(), |t| t.name.as_str())).collect();
        hint += &format!("\nTitles: {}", names.join(", "));
    }
    if !near.is_empty() {
        let list: Vec<String> = near.iter().take(HINTS).map(|(s, f)| format!("{} {:.0}%", s.name, f * 100.0)).collect();
        hint += &format!("\nSensing: {}", list.join("  //  "));
    }
    for mut t in &mut texts.p2() {
        set(&mut t, hint.clone());
    }
}

/// The WORK tab: open contracts with exact progress.
#[allow(clippy::type_complexity)]
fn refresh_contracts(
    open: Res<StatsOpen>,
    book: Res<ContractBook>,
    board: Res<Board>,
    mut texts: ParamSet<(
        Query<(&ContractName, &mut Text)>,
        Query<(&ContractInfo, &mut Text)>,
        Query<&mut Text, With<ContractsDone>>,
    )>,
) {
    if open.0 != Some(StatsTab::Contracts) {
        return;
    }
    let set = |t: &mut Text, v: String| {
        if t.0 != v {
            t.0 = v;
        }
    };
    let get = |i: usize| board.open.get(i).and_then(|o| book.0.get(o.index).map(|d| (o, d)));
    for (c, mut t) in &mut texts.p0() {
        let line = get(c.0).map_or(String::new(), |(_, d)| {
            let from = d.issuer.as_ref().map_or(String::new(), |who| format!("   ·   from the {who}"));
            format!("{}   +{} XP{from}", d.name, d.reward.xp)
        });
        set(&mut t, line);
    }
    for (c, mut t) in &mut texts.p1() {
        let line = get(c.0).map_or(String::new(), |(o, d)| {
            let steps: Vec<String> = d
                .objectives
                .iter()
                .zip(&o.progress)
                .map(|(obj, p)| format!("{} {:.0}/{:.0}", obj.kind.replace('_', " "), p.floor(), obj.count))
                .collect();
            format!("{}\n{}", d.about, steps.join("   "))
        });
        set(&mut t, line);
    }
    for mut t in &mut texts.p2() {
        let rank = board.rank();
        let next = contracts::RANKS
            .get(rank + 1)
            .map_or(String::new(), |(l, n)| format!("   ·   {} more to {l}", n.saturating_sub(board.done)));
        set(&mut t, format!("Guild rank {}   ·   {} done{next}", contracts::RANKS[rank].0, board.done));
    }
}

/// The madlib's words follow the player's traits.
fn refresh_madlib(
    open: Res<StatsOpen>,
    book: Res<TraitBook>,
    player: Query<Option<&Traits>, With<Player>>,
    mut texts: Query<(&MadlibText, &mut Text)>,
) {
    if open.0 != Some(StatsTab::Stats) {
        return;
    }
    let Ok(t) = player.single() else { return };
    for (m, mut text) in &mut texts {
        let want = book.word(t, m.0).unwrap_or("…");
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

/// The footer explains what the cursor is over.
#[allow(clippy::type_complexity)]
fn hover(
    open: Res<StatsOpen>,
    book: Res<SkillBook>,
    player: Query<&Sheet, With<Player>>,
    attrs: Query<(&AttrRow, &Interaction)>,
    rows: Query<(&AbilityRow, &Interaction)>,
    skills: Query<(&SkillRow, &Interaction)>,
    mut footer: Query<&mut Typewriter, With<Footer>>,
) {
    let Some(tab) = open.0 else { return };
    let sheet = player.single().ok();
    let line = attrs
        .iter()
        .find(|(_, i)| **i != Interaction::None)
        .map(|(a, _)| format!("{}: {}", a.0.name(), a.0.description()))
        .or_else(|| {
            rows.iter().find(|(_, i)| **i != Interaction::None).map(|(r, _)| {
                let a = &ABILITIES[r.0];
                if sheet.is_some_and(|s| s.has_ability(a.id)) {
                    format!("{}: {} Click to bind.", a.name, a.about)
                } else {
                    format!(
                        "Undiscovered. Needs {}; then keep training it and it will come to you.",
                        requirement(&book, a, sheet)
                    )
                }
            })
        })
        .or_else(|| {
            skills
                .iter()
                .find(|(_, i)| **i != Interaction::None)
                .and_then(|(r, _)| book.0.get(r.0))
                .map(describe)
        })
        .unwrap_or_else(|| match tab {
            StatsTab::Stats => {
                "Levels come from defeating foes. Spend points with +. Click the words of the sentence to choose how you carry yourself.".to_string()
            }
            StatsTab::Arts => {
                "Arts and spells are discovered by training their skills. Landing one grows its mastery; mastered, it is cheaper, faster and stronger, at a permanent cost of magicules."
                    .to_string()
            }
            StatsTab::Contracts => "Posted work. Progress counts by itself as you go about it; a finished contract pays experience and a new one is posted.".to_string(),
            StatsTab::Skills => {
                "Skills are found and raised by doing: fighting, casting, swimming, climbing, felling. They never fade.".to_string()
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

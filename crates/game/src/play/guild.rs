//! The Guild: its people and the front desk (`assets/data/guild.json`, `GuildData`). The hall
//! itself is `hall`; talking is `talk`.
//! - **People** (non-combat `occupations::Civilian`s, `talk::Talkable`): Mira the receptionist
//!   (behind the desk), Captain Varro (the training ground, drilling at the dummies), and two
//!   adventurers, Kael and Sena, who sit at the lodging table, chat (log lines you hear when
//!   near), read the bounties and sleep in the beds at night.
//! - **Gold** (`Purse` on the player, written here only), paid at the desk.
//! - **Mira's desk** (`desk_menu`, pure): turn in finished contracts (`contracts::TurnIn` →
//!   gold and XP), the **Guild Card** (appraisal of you: rank, level, stats, skills, titles,
//!   honours), selling gathered items at `prices` (`inventory::SpendItems`), claiming
//!   **bounties** for kills since the last report (`kill` deeds tagged with the victim, priced by
//!   name), applying for the next rank once eligible, and renting a bed (sleep: `sky::SetHour`
//!   to dawn and `Rested`, which `combat` and `magic` turn into full hp, stamina and pools).
//! - **The rank exam**: once an application is in, Varro takes you into the duel ring. During the
//!   bout he leaves `Civilian`, fights on the NPC brain (`ai::Autopilot`, team `DUEL_TEAM`) at a
//!   strength set by the rank, and both of you are `combat::Sparring`: no blow takes either below
//!   `exam.yield_share` of max hp, and reaching it (or leaving the ring) yields. Win: Mira issues
//!   the card (`contracts::RankUp`). Lose: rest and try again later.
//! The guild writes `Purse`, `Guild`, `Dialogue` (for `talk`) and, during a bout only, Varro's
//! `Fighter.team`, `Civilian` / `Autopilot` and both fighters' `Sparring`.
use super::ai::{Autopilot, Difficulty};
use super::combat::{Revive, Sparring, Yielded};
use super::contracts::{Board, ContractBook, Honours, HonourBook, Paid, RANKS, RankUp, TurnIn};
use super::controller::Teleport;
use super::deeds::Deed;
use super::hall::{CAPTAIN_POST, DESK_FACE, DESK_STAND, RING_CENTER, RING_RADIUS};
use super::hud::log::{HudLog, LogKind};
use super::interact::Using;
use super::inventory::{Inventory, SpendItems};
use super::items::ItemKind;
use super::monitor::Monitor;
use super::occupations::{Civilian, Occupation};
use super::skills::{Sheet, SkillBook, TitleBook};
use super::stats::{Attr, Stats};
use super::talk::{Choice, Chose, Dialogue, EndTalk, Role, TalkStarted, Talkable, Talking};
use super::{Fighter, Interactable, Loadout, Player, spawn_character};
use bevy::prelude::*;
use gbui::prelude::*;
use std::collections::HashMap;

/// Varro's team while he duels (no one else is on it).
const DUEL_TEAM: u8 = 7;
/// Leaving the ring by this much (m) for this long (s) yields.
const RING_SLACK: f32 = 2.0;
const OUT_TIME: f32 = 3.0;
/// Seconds of sitting on a paid bed before sleep.
const SLEEP_AFTER: f32 = 1.5;
/// Chatter: seconds between lines at one table, how near another sitter must be (m), how near the
/// player must be to hear it (m).
const CHATTER_EVERY: f32 = 8.0;
const CHATTER_PAIR: f32 = 3.5;
const HEAR: f32 = 10.0;

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct ExamDef {
    pub captain_level_per_rank: u32,
    pub yield_share: f32,
    /// Game seconds before a failed exam can be retried.
    #[serde(default = "retry_default")]
    pub retry_seconds: f32,
}

fn retry_default() -> f32 {
    60.0
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Lines {
    pub receptionist_greet: Vec<String>,
    pub captain_greet: Vec<String>,
    pub adventurer_greet: Vec<String>,
    pub chatter: Vec<String>,
}

#[derive(Resource, Clone, Debug, Default, serde::Deserialize)]
pub struct GuildData {
    pub prices: HashMap<String, u32>,
    pub bounties: HashMap<String, u32>,
    pub bed_cost: u32,
    pub exam: ExamDef,
    pub lines: Lines,
}

impl GuildData {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let d: Self = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(k) = d.prices.keys().find(|k| !ItemKind::ALL.iter().any(|i| format!("{i:?}") == **k)) {
            return Err(format!("guild.json: no item kind '{k}'"));
        }
        if !(d.exam.yield_share > 0.0 && d.exam.yield_share < 0.9) {
            return Err("guild.json: exam.yield_share".into());
        }
        Ok(d)
    }
    pub fn price(&self, kind: ItemKind) -> u32 {
        self.prices.get(&format!("{kind:?}")).copied().unwrap_or(0)
    }
}

pub fn guild_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/guild.json")
}

/// The player's gold (written by `guild` only).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Purse {
    pub gold: u32,
}

/// Fully rested (a night in a guild bed, the end of a bout): `combat` refills hp / stamina /
/// poise, `magic` the pools.
#[derive(Message, Clone, Copy, Debug)]
pub struct Rested {
    pub who: Entity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Exam {
    #[default]
    None,
    /// Applied for this rank (index into `RANKS`); Varro will test.
    Scheduled(usize),
    /// Passed; Mira issues the card.
    Passed(usize),
}

/// A bout in progress.
#[derive(Clone, Copy, Debug)]
pub struct Duel {
    pub captain: Entity,
    pub to: usize,
    out: [f32; 2],
}

/// The guild's memory of the player (written by `guild` only).
#[derive(Resource, Clone, Debug, Default)]
pub struct Guild {
    /// Kills not yet reported, by victim name.
    pub bounties: HashMap<String, u32>,
    pub bed_paid: bool,
    pub exam: Exam,
    /// Game time before which a failed exam can't be retried.
    pub retry_at: f32,
    pub duel: Option<Duel>,
    pub card_open: bool,
    rng: u32,
}

impl Guild {
    fn roll(&mut self, n: usize) -> usize {
        let mut x = self.rng.max(0x9E37_79B9);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as usize) % n.max(1)
    }
}

/// What the desk knows when it builds Mira's options.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeskView {
    pub ready: usize,
    pub ready_gold: u32,
    pub ready_xp: u32,
    pub items_value: u32,
    pub bounty_value: u32,
    pub held: usize,
    pub eligible: usize,
    pub exam: Exam,
    pub gold: u32,
    pub bed_cost: u32,
    pub bed_paid: bool,
}

fn choice(label: impl Into<String>, id: &'static str) -> Choice {
    Choice { label: label.into(), id }
}

/// Mira's options (at most `talk::OPTIONS`).
pub fn desk_menu(v: &DeskView) -> Vec<Choice> {
    let mut o = Vec::new();
    if v.ready > 0 {
        o.push(choice(format!("Turn in {} contract(s)  (+{} gold, +{} XP)", v.ready, v.ready_gold, v.ready_xp), "turn_in"));
    }
    o.push(choice("Appraise me  (Guild Card)", "card"));
    o.push(if v.items_value > 0 {
        choice(format!("Sell what I gathered  ({} gold)", v.items_value), "sell")
    } else {
        choice("Appraise my items", "items")
    });
    o.push(if v.bounty_value > 0 {
        choice(format!("Claim bounties  ({} gold)", v.bounty_value), "bounty")
    } else {
        choice("Any bounties for me?", "bounties")
    });
    match v.exam {
        Exam::Passed(to) => o.push(choice(format!("Collect my rank {} card", RANKS[to].0), "collect")),
        Exam::None if v.eligible > v.held => o.push(choice(format!("Apply for rank {}", RANKS[v.held + 1].0), "apply")),
        _ => {}
    }
    if !v.bed_paid {
        o.push(choice(format!("Rent a bed  ({} gold)", v.bed_cost), "bed"));
    }
    o.push(choice("Leave", "leave"));
    o.truncate(super::talk::OPTIONS);
    o
}

/// Varro's options.
pub fn captain_menu(exam: Exam, can_retry: bool) -> Vec<Choice> {
    let mut o = Vec::new();
    if let Exam::Scheduled(to) = exam
        && can_retry
    {
        o.push(choice(format!("I'm ready for my rank {} exam", RANKS[to].0), "exam"));
    }
    o.push(choice("Any advice?", "advice"));
    o.push(choice("Leave", "leave"));
    o
}

/// An adventurer's options.
pub fn adventurer_menu() -> Vec<Choice> {
    vec![choice("What's the news?", "news"), choice("Leave", "leave")]
}

/// What the bag is worth at the desk.
pub fn bag_value(data: &GuildData, inv: &Inventory) -> u32 {
    ItemKind::ALL.iter().map(|k| inv.count(*k) * data.price(*k)).sum()
}

/// What the unreported kills are worth.
pub fn bounty_value(data: &GuildData, ledger: &HashMap<String, u32>) -> u32 {
    ledger.iter().map(|(who, n)| data.bounties.get(who).copied().unwrap_or(0) * n).sum()
}

pub fn plugin(app: &mut App) {
    let data = GuildData::load(&guild_path()).unwrap_or_else(|e| {
        error!("[guild] {e}");
        GuildData::default()
    });
    app.insert_resource(data)
        .init_resource::<Guild>()
        .register_required_components::<Player, Purse>()
        .add_message::<Rested>()
        .add_systems(Startup, (spawn_people, spawn_card))
        .add_systems(
            Update,
            (smoke, ledger, paid, respond, sleep, duel, chatter, card).chain().after(super::PlaySet::Input),
        );
}

/// Mira, Varro and two adventurers.
fn spawn_people(mut commands: Commands) {
    struct Person {
        name: &'static str,
        title: &'static str,
        role: Role,
        job: &'static str,
        at: Vec3,
        yaw: f32,
        weapon: &'static str,
        tint: Color,
    }
    let people = [
        Person { name: "Mira", title: "Guild receptionist", role: Role::Receptionist, job: "receptionist", at: DESK_STAND, yaw: DESK_FACE, weapon: "unarmed", tint: Color::srgb(0.86, 0.66, 0.52) },
        Person { name: "Varro", title: "Guild Martial Captain", role: Role::Captain, job: "captain", at: CAPTAIN_POST, yaw: -std::f32::consts::FRAC_PI_2, weapon: "katana", tint: Color::srgb(0.45, 0.47, 0.52) },
        Person { name: "Kael", title: "Adventurer, rank D", role: Role::Adventurer, job: "adventurer", at: Vec3::new(26.6, 0.0, -7.4), yaw: 0.0, weapon: "axe", tint: Color::srgb(0.55, 0.36, 0.22) },
        Person { name: "Sena", title: "Adventurer, rank E", role: Role::Adventurer, job: "adventurer", at: Vec3::new(28.0, 0.0, -4.6), yaw: std::f32::consts::PI, weapon: "dagger", tint: Color::srgb(0.40, 0.52, 0.60) },
    ];
    for p in people {
        let e = spawn_character(
            &mut commands,
            p.name,
            p.at,
            p.yaw,
            Fighter::new(0, 100.0),
            Loadout {
                slots: vec![p.weapon.to_string()],
                current: 0,
            },
            Some(p.tint),
        );
        commands.entity(e).insert((
            Civilian,
            Occupation::new(p.job),
            Talkable {
                name: p.name,
                role: p.role,
                title: p.title,
            },
            Revive {
                after: 8.0,
                home: Some((p.at, p.yaw)),
            },
        ));
        if p.role == Role::Captain {
            let mut sheet = Sheet::default();
            sheet.grant("swordsmanship", super::skills::xp_for_level(40));
            sheet.grant("defence", super::skills::xp_for_level(30));
            commands.entity(e).insert((sheet, Stats::preset(8, &[Attr::Strength, Attr::Dexterity, Attr::Vigor])));
        }
    }
}

/// Smoke hooks: `GB_GOLD=<n>` fills the purse; `GB_CARD=1` opens the Guild Card; `GB_EXAM=1`
/// schedules the next rank's exam and starts the bout at 2.5 s (with `GB_FIGHT` the player duels
/// on autopilot).
#[allow(clippy::too_many_arguments)]
fn smoke(
    time: Res<Time>,
    data: Res<GuildData>,
    mut guild: ResMut<Guild>,
    mut done: Local<(bool, bool)>,
    mut commands: Commands,
    mut purses: Query<&mut Purse, With<Player>>,
    players: Query<Entity, With<Player>>,
    mut captains: Query<(Entity, &Talkable, &mut Fighter)>,
    board: Res<Board>,
    mut log: MessageWriter<HudLog>,
) {
    let t = time.elapsed_secs();
    if !done.0 && t > 1.0 {
        done.0 = true;
        if let Some(g) = std::env::var("GB_GOLD").ok().and_then(|s| s.trim().parse().ok())
            && let Ok(mut p) = purses.single_mut()
        {
            p.gold = g;
        }
        if std::env::var("GB_CARD").is_ok_and(|v| v.trim() == "1") {
            guild.card_open = true;
        }
    }
    if !done.1 && t > 2.5 {
        done.1 = true;
        if std::env::var("GB_EXAM").is_ok_and(|v| v.trim() == "1")
            && let Ok(player) = players.single()
            && let Some((captain, _, mut f)) = captains.iter_mut().find(|(_, t, _)| t.role == Role::Captain)
        {
            let to = (board.rank() + 1).min(RANKS.len() - 1);
            guild.exam = Exam::Scheduled(to);
            start_duel(&mut commands, &mut guild, &data, captain, &mut f, player, to, &mut log);
        }
    }
}

/// Kills the player makes are remembered for the bounty desk.
fn ledger(data: Res<GuildData>, mut guild: ResMut<Guild>, mut deeds: MessageReader<Deed>, players: Query<Entity, With<Player>>) {
    let player = players.iter().next();
    for d in deeds.read() {
        if Some(d.who) == player && d.kind == "kill" && data.bounties.contains_key(&d.tag) {
            *guild.bounties.entry(d.tag.clone()).or_insert(0) += 1;
        }
    }
}

/// Turn-in payouts arrive as gold.
fn paid(mut paid: MessageReader<Paid>, mut purses: Query<&mut Purse>, mut log: MessageWriter<HudLog>) {
    for p in paid.read() {
        if let Ok(mut purse) = purses.get_mut(p.who) {
            purse.gold += p.gold;
            log.write(HudLog::new(LogKind::Item, format!("+{} gold, +{} XP for {} contract(s)", p.gold, p.xp, p.count)));
        }
    }
}

/// Talks open menus; picks do their work and rewrite the dialogue.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn respond(
    time: Res<Time>,
    data: Res<GuildData>,
    book: Res<ContractBook>,
    board: Res<Board>,
    mut guild: ResMut<Guild>,
    mut dialogue: ResMut<Dialogue>,
    mut started: MessageReader<TalkStarted>,
    mut chose: MessageReader<Chose>,
    mut players: Query<(Entity, &Inventory, &mut Purse), With<Player>>,
    mut captains: Query<&mut Fighter, Without<Player>>,
    mut turn_in: MessageWriter<TurnIn>,
    mut rank_up: MessageWriter<RankUp>,
    mut spend: MessageWriter<SpendItems>,
    mut end: MessageWriter<EndTalk>,
    mut log: MessageWriter<HudLog>,
    mut commands: Commands,
) {
    let Ok((player, inv, mut purse)) = players.single_mut() else { return };
    let now = time.elapsed_secs();
    let view = |guild: &Guild, purse: &Purse, board_ready: bool, items: bool, bounties: bool| {
        let ready: Vec<&super::contracts::ContractDef> = if board_ready {
            board.ready.iter().filter_map(|i| book.0.get(*i)).collect()
        } else {
            Vec::new()
        };
        DeskView {
            ready: ready.len(),
            ready_gold: ready.iter().map(|d| d.reward.gold).sum(),
            ready_xp: ready.iter().map(|d| d.reward.xp).sum(),
            items_value: if items { bag_value(&data, inv) } else { 0 },
            bounty_value: if bounties { bounty_value(&data, &guild.bounties) } else { 0 },
            held: board.rank(),
            eligible: board.eligible(),
            exam: guild.exam,
            gold: purse.gold,
            bed_cost: data.bed_cost,
            bed_paid: guild.bed_paid,
        }
    };
    let greet = |guild: &mut Guild, lines: &[String]| lines.get(guild.roll(lines.len())).cloned().unwrap_or_default();
    for s in started.read() {
        let (line, options) = match s.role {
            Role::Receptionist => {
                let mut line = greet(&mut guild, &data.lines.receptionist_greet);
                if let Exam::Scheduled(to) = guild.exam {
                    line = format!("Your rank {} exam is booked: Captain Varro is in the training ground.", RANKS[to].0);
                }
                (line, desk_menu(&view(&guild, &purse, true, true, true)))
            }
            Role::Captain => {
                let line = match guild.exam {
                    Exam::Scheduled(to) if now >= guild.retry_at => format!("Mira tells me you want rank {}. Step into the ring when you're ready.", RANKS[to].0),
                    Exam::Scheduled(_) => "Not yet. Rest, then come back to me.".to_string(),
                    Exam::Passed(_) => "You passed. Go and get your card from Mira.".to_string(),
                    Exam::None => greet(&mut guild, &data.lines.captain_greet),
                };
                (line, captain_menu(guild.exam, now >= guild.retry_at))
            }
            Role::Adventurer => (greet(&mut guild, &data.lines.adventurer_greet), adventurer_menu()),
        };
        *dialogue = Dialogue { line, options };
    }
    for c in chose.read() {
        let mut close = false;
        let line = match c.id {
            "turn_in" => {
                let v = view(&guild, &purse, true, false, false);
                turn_in.write(TurnIn { who: player });
                let mut options = desk_menu(&DeskView { ready: 0, ..view(&guild, &purse, false, true, true) });
                options.retain(|o| o.id != "turn_in");
                dialogue.options = options;
                format!("{} contract(s) signed off. {} gold, and the guild remembers.", v.ready, v.ready_gold)
            }
            "card" => {
                guild.card_open = !guild.card_open;
                "Here's your card. The Guild's eye sees what you are.".to_string()
            }
            "items" => "Nothing in your bag the guild buys. Logs, stones, sticks, apples: bring those.".to_string(),
            "sell" => {
                let mut total = 0;
                let mut parts = Vec::new();
                for k in ItemKind::ALL {
                    let n = inv.count(k);
                    let price = data.price(k);
                    if n > 0 && price > 0 {
                        spend.write(SpendItems { who: player, kind: k, count: n });
                        total += n * price;
                        parts.push(format!("{n} {} at {price}", k.name()));
                    }
                }
                purse.gold += total;
                log.write(HudLog::new(LogKind::Item, format!("+{total} gold")));
                format!("{}. That's {total} gold.", parts.join(", "))
            }
            "bounties" => "No bounties on your name yet. Bring down the Rival, the Hunter or the courtyard brawler and tell me.".to_string(),
            "bounty" => {
                let value = bounty_value(&data, &guild.bounties);
                let list: Vec<String> = guild.bounties.iter().map(|(w, n)| format!("{n}× {w}")).collect();
                guild.bounties.clear();
                purse.gold += value;
                log.write(HudLog::new(LogKind::Item, format!("+{value} gold (bounties)")));
                format!("{}. {value} gold, as posted.", list.join(", "))
            }
            "apply" => {
                let to = (board.rank() + 1).min(RANKS.len() - 1);
                guild.exam = Exam::Scheduled(to);
                format!("Application for rank {} accepted. Captain Varro will test you in the training ground.", RANKS[to].0)
            }
            "collect" => {
                if let Exam::Passed(to) = guild.exam {
                    rank_up.write(RankUp { to });
                    guild.exam = Exam::None;
                    guild.card_open = true;
                    format!("Rank {}. Congratulations. Wear it well.", RANKS[to].0)
                } else {
                    String::new()
                }
            }
            "bed" => {
                if purse.gold >= data.bed_cost {
                    purse.gold -= data.bed_cost;
                    guild.bed_paid = true;
                    "The bed's yours tonight. Any of the three by the north wall.".to_string()
                } else {
                    format!("A bed is {} gold. You're short.", data.bed_cost)
                }
            }
            "exam" => {
                if let (Exam::Scheduled(to), Ok(mut f)) = (guild.exam, captains.get_mut(c.npc)) {
                    close = true;
                    start_duel(&mut commands, &mut guild, &data, c.npc, &mut f, player, to, &mut log);
                }
                String::new()
            }
            "advice" => greet(&mut guild, &data.lines.captain_greet),
            "news" => greet(&mut guild, &data.lines.adventurer_greet),
            _ => String::new(),
        };
        if close {
            end.write(EndTalk);
            continue;
        }
        if !line.is_empty() {
            dialogue.line = line;
        }
        if c.role == Role::Receptionist && c.id != "turn_in" {
            dialogue.options = desk_menu(&view(&guild, &purse, true, c.id != "sell", c.id != "bounty"));
            if c.id == "collect" || c.id == "apply" {
                dialogue.options.retain(|o| o.id != "apply" && o.id != "collect");
            }
        }
        if c.role == Role::Captain {
            dialogue.options = captain_menu(guild.exam, now >= guild.retry_at);
        }
    }
}

/// Into the ring: Varro fights (on the brain, scaled to the rank), both spar to a yield.
#[allow(clippy::too_many_arguments)]
fn start_duel(
    commands: &mut Commands,
    guild: &mut Guild,
    data: &GuildData,
    captain: Entity,
    fighter: &mut Fighter,
    player: Entity,
    to: usize,
    log: &mut MessageWriter<HudLog>,
) {
    let floor = data.exam.yield_share;
    let level = 2 + data.exam.captain_level_per_rank * to as u32;
    fighter.team = DUEL_TEAM;
    let east = RING_CENTER + Vec3::X * (RING_RADIUS * 0.6);
    let west = RING_CENTER - Vec3::X * (RING_RADIUS * 0.6);
    commands.entity(captain).remove::<Civilian>().insert((
        Autopilot,
        Sparring { floor },
        Stats::preset(level, &[Attr::Strength, Attr::Dexterity, Attr::Vigor]),
        Difficulty {
            aggression: (0.45 + 0.08 * to as f32).min(0.95),
            parry: (0.15 + 0.06 * to as f32).min(0.7),
            ..default()
        },
        Teleport {
            feet: east,
            yaw: -std::f32::consts::FRAC_PI_2,
        },
    ));
    commands.entity(player).insert((
        Sparring { floor },
        Teleport {
            feet: west,
            yaw: std::f32::consts::FRAC_PI_2,
        },
    ));
    guild.duel = Some(Duel { captain, to, out: [0.0; 2] });
    log.write(HudLog::new(LogKind::Warn, format!("Varro: Rank {} exam. Guard up. Begin!", RANKS[to].0)));
    info!("[guild] exam {} begins", RANKS[to].0);
}

/// The bout: a yield or leaving the ring ends it; then everyone is whole again.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn duel(
    time: Res<Time>,
    data: Res<GuildData>,
    mut guild: ResMut<Guild>,
    mut commands: Commands,
    mut fighters: Query<(&Transform, &mut Fighter, Has<Yielded>)>,
    players: Query<Entity, With<Player>>,
    mut rested: MessageWriter<Rested>,
    mut monitor: ResMut<Monitor>,
    mut log: MessageWriter<HudLog>,
) {
    let Some(mut d) = guild.duel else { return };
    let Ok(player) = players.single() else { return };
    let dt = time.delta_secs();
    let mut lost = [false; 2];
    let mut ring_out = false;
    for (k, e) in [player, d.captain].into_iter().enumerate() {
        let Ok((tf, f, yielded)) = fighters.get(e) else {
            lost[k] = true;
            continue;
        };
        let out = Vec2::new(tf.translation.x - RING_CENTER.x, tf.translation.z - RING_CENTER.z).length() > RING_RADIUS + RING_SLACK;
        d.out[k] = if out { d.out[k] + dt } else { 0.0 };
        let left = d.out[k] > OUT_TIME;
        ring_out |= left && !yielded;
        lost[k] = yielded || f.dead || left;
    }
    if !(lost[0] || lost[1]) {
        guild.duel = Some(d);
        return;
    }
    // The bout is over.
    let won = lost[1] && !lost[0];
    for e in [player, d.captain] {
        commands.entity(e).try_remove::<(Sparring, Yielded)>();
        rested.write(Rested { who: e });
    }
    commands.entity(d.captain).try_remove::<Autopilot>().try_insert(Civilian);
    if let Ok((_, mut f, _)) = fighters.get_mut(d.captain) {
        f.team = 0;
    }
    let now = time.elapsed_secs();
    let how = if ring_out { "ring_out" } else { "yield" };
    monitor.write(&format!("{now:.2} Varro duel_end - {} Ground to={} by={how}", if won { 1.0 } else { 0.0 }, RANKS[d.to].0));
    if won {
        guild.exam = Exam::Passed(d.to);
        let why = if ring_out { "Varro steps out of the ring." } else { "Varro yields." };
        log.write(HudLog::new(LogKind::Item, format!("{why} Varro: Well fought. Take this to Mira for your card.")));
    } else {
        guild.retry_at = now + data.exam.retry_seconds;
        let why = if ring_out { "You left the ring." } else { "You yield." };
        log.write(HudLog::new(LogKind::Warn, format!("{why} Varro: Not yet. Rest, then come back to me.")));
    }
    info!("[guild] exam {} {} ({how})", RANKS[d.to].0, if won { "passed" } else { "failed" });
    guild.duel = None;
}

/// A paid bed: sit on it a moment and sleep until dawn, whole again.
#[allow(clippy::too_many_arguments)]
fn sleep(
    time: Res<Time>,
    mut guild: ResMut<Guild>,
    players: Query<(Entity, Option<&Using>), With<Player>>,
    beds: Query<&Interactable>,
    mut sat: Local<f32>,
    mut hour: MessageWriter<super::sky::SetHour>,
    mut rested: MessageWriter<Rested>,
    mut log: MessageWriter<HudLog>,
) {
    let Ok((player, using)) = players.single() else { return };
    let on_bed = using.is_some_and(|u| beds.get(u.target).is_ok_and(|it| it.kind == "bed"));
    *sat = if on_bed { *sat + time.delta_secs() } else { 0.0 };
    if on_bed && *sat > SLEEP_AFTER {
        *sat = f32::NEG_INFINITY;
        if guild.bed_paid {
            guild.bed_paid = false;
            hour.write(super::sky::SetHour(6.0));
            rested.write(Rested { who: player });
            log.write(HudLog::new(LogKind::Item, "You sleep until dawn. Rested."));
        } else {
            log.write(HudLog::new(LogKind::System, "Rent a bed from Mira to sleep here."));
        }
    }
}

/// Adventurers sitting together talk; you hear it when near.
#[allow(clippy::type_complexity)]
fn chatter(
    time: Res<Time>,
    data: Res<GuildData>,
    mut guild: ResMut<Guild>,
    sitters: Query<(&Transform, &Talkable, &Using), With<Civilian>>,
    chairs: Query<&Interactable>,
    players: Query<&Transform, With<Player>>,
    mut wait: Local<f32>,
    mut log: MessageWriter<HudLog>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 || data.lines.chatter.is_empty() {
        return;
    }
    *wait = CHATTER_EVERY;
    let seated: Vec<(&Transform, &Talkable)> = sitters
        .iter()
        .filter(|(_, _, u)| chairs.get(u.target).is_ok_and(|it| it.kind == "chair"))
        .map(|(t, k, _)| (t, k))
        .collect();
    let Some((tf, who)) = seated.iter().find(|(a, _)| {
        seated.iter().any(|(b, _)| {
            let d = a.translation.distance(b.translation);
            d > 0.1 && d < CHATTER_PAIR
        })
    }) else {
        return;
    };
    let near = players.single().is_ok_and(|p| p.translation.distance(tf.translation) < HEAR);
    if near {
        let i = guild.roll(data.lines.chatter.len());
        super::talk::say(&mut log, who.name, &data.lines.chatter[i]);
    }
}

#[derive(Component)]
struct Card;
#[derive(Component)]
struct CardText;

fn spawn_card(mut commands: Commands) {
    commands
        .spawn((
            Card,
            panel(
                "stats",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    right: px(40.0),
                    top: percent(14.0),
                    width: px(360.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(14.0)),
                    row_gap: px(6.0),
                    ..default()
                },
            ),
            GlobalZIndex(55),
        ))
        .with_children(|p| {
            p.spawn(label("▌GUILD CARD", "title"));
            p.spawn((CardText, label("", "row.text")));
            p.spawn(label("[Esc] close", "text.dim"));
        });
}

/// The Guild Card: what the guild's appraisal reads off you.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn card_text(
    rank: usize,
    eligible: usize,
    done: u32,
    stats: &Stats,
    sheet: &Sheet,
    skills: &SkillBook,
    titles: &TitleBook,
    honours: Option<&Honours>,
    honour_book: &HonourBook,
    gold: u32,
) -> String {
    let mut s = format!("RANK {}   ·   LEVEL {}   ·   {gold} gold\n", RANKS[rank].0, stats.level);
    let attrs: Vec<String> = Attr::ALL.iter().map(|a| format!("{} {}", a.code(), stats.get(*a))).collect();
    s += &attrs.join("  ");
    let mut known: Vec<(&str, u32)> = skills.0.iter().filter(|d| sheet.knows(&d.id)).map(|d| (d.name.as_str(), sheet.level(&d.id))).collect();
    known.sort_by(|a, b| b.1.cmp(&a.1));
    if !known.is_empty() {
        let top: Vec<String> = known.iter().take(4).map(|(n, l)| format!("{n} {l}")).collect();
        s += &format!("\nSkills: {}", top.join(", "));
    }
    if !sheet.titles.is_empty() {
        let names: Vec<&str> = sheet.titles.iter().map(|id| titles.get(id).map_or(id.as_str(), |t| t.name.as_str())).collect();
        s += &format!("\nTitles: {}", names.join(", "));
    }
    if let Some(h) = honours.filter(|h| !h.0.is_empty()) {
        let names: Vec<&str> = h.0.iter().map(|id| honour_book.get(id).map_or(id.as_str(), |d| d.name.as_str())).collect();
        s += &format!("\nHonours: {}", names.join(", "));
    }
    s += &format!("\nContracts completed: {done}");
    if eligible > rank {
        s += &format!("\nEligible for rank {}", RANKS[eligible].0);
    }
    s
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn card(
    keys: Res<ButtonInput<KeyCode>>,
    mut guild: ResMut<Guild>,
    talking: Res<Talking>,
    board: Res<Board>,
    skills: Res<SkillBook>,
    titles: Res<TitleBook>,
    honour_book: Res<HonourBook>,
    player: Query<(&Stats, &Sheet, Option<&Honours>, &Purse), With<Player>>,
    mut root: Query<&mut Node, With<Card>>,
    mut text: Query<&mut Text, With<CardText>>,
) {
    if guild.card_open && (keys.just_pressed(KeyCode::Escape) || (talking.is_changed() && !talking.active() && talking.blend > 0.0)) {
        guild.card_open = false;
    }
    for mut node in &mut root {
        let want = if guild.card_open { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    if !guild.card_open {
        return;
    }
    let Ok((stats, sheet, honours, purse)) = player.single() else { return };
    let line = card_text(board.rank(), board.eligible(), board.done, stats, sheet, &skills, &titles, honours, &honour_book, purse.gold);
    for mut t in &mut text {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> GuildData {
        GuildData::load(&guild_path()).expect("guild.json")
    }

    #[test]
    fn data_loads_and_prices_items() {
        let d = data();
        let mut inv = Inventory::default();
        inv.add(ItemKind::Log, 3);
        inv.add(ItemKind::Apple, 2);
        assert_eq!(bag_value(&d, &inv), 3 * d.price(ItemKind::Log) + 2 * d.price(ItemKind::Apple));
        let ledger = HashMap::from([("Rival".to_string(), 2), ("Nobody".to_string(), 5)]);
        assert_eq!(bounty_value(&d, &ledger), 2 * d.bounties["Rival"]);
    }

    #[test]
    fn the_desk_offers_what_applies() {
        let base = DeskView {
            bed_cost: 10,
            ..default()
        };
        let ids = |v: &DeskView| desk_menu(v).into_iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids(&base), ["card", "items", "bounties", "bed", "leave"]);
        let busy = DeskView {
            ready: 2,
            items_value: 9,
            bounty_value: 40,
            eligible: 1,
            ..base
        };
        assert_eq!(ids(&busy), ["turn_in", "card", "sell", "bounty", "apply", "bed", "leave"]);
        let passed = DeskView {
            exam: Exam::Passed(1),
            bed_paid: true,
            ..base
        };
        assert!(ids(&passed).contains(&"collect") && !ids(&passed).contains(&"bed"));
        // An exam already booked: no second application.
        let booked = DeskView {
            exam: Exam::Scheduled(1),
            eligible: 2,
            ..base
        };
        assert!(!ids(&booked).contains(&"apply"));
        assert!(desk_menu(&busy).len() <= super::super::talk::OPTIONS);
    }

    #[test]
    fn the_captain_only_tests_a_booked_exam() {
        let ids = |e: Exam, r: bool| captain_menu(e, r).into_iter().map(|c| c.id).collect::<Vec<_>>();
        assert!(!ids(Exam::None, true).contains(&"exam"));
        assert!(ids(Exam::Scheduled(1), true).contains(&"exam"));
        assert!(!ids(Exam::Scheduled(1), false).contains(&"exam"), "retry wait");
    }
}

//! Save and load (E19): the player's progress in one JSON file.
//!
//! - **Saves** on F5 (quicksave), when the player wakes rested (a guild bed: `guild::Rested`) and
//!   when the game closes (`AppExit`). Not while dead.
//! - **Loads** once at start, as soon as the player and the guild board exist, if the file is
//!   there. `GB_NEW=1` starts fresh (the file is kept until the next save).
//! - **Smoke runs never touch it**: with `GB_RECORD`, `GB_SPAWN`, `GB_AUTOQUIT` or `GB_SCREENSHOT`
//!   set, nothing is loaded or saved, so a recording neither starts from nor overwrites a real
//!   save, unless `GB_SAVE` names a file explicitly (headless save/load checks).
//! - **Where**: `GB_SAVE=<path>`, else `%APPDATA%\Guildbound\save.json` (Windows) or
//!   `$XDG_DATA_HOME/guildbound/save.json` / `~/.local/share/guildbound/save.json`. Written to a
//!   temporary file first, then renamed over the old one, so a crash mid-write keeps the last good
//!   save.
//!
//! What is kept: where the player stands and faces; level, attributes, unspent points, XP; skill
//! XP, abilities, deed counts, titles; gold; the bag; art / spell bindings and mastery; honours and
//! holdings; the guild board (open contracts and their progress, contracts done, ready to turn in,
//! rank held); the bounty targets brought down (rumours); the hour. Health, stamina and pools come back full (a load is a fresh morning).
//! Every write on load goes through the owning module's restore function (`Stats::restored`,
//! `Sheet::restore`, `Board::restored`, `Inventory::restored`, `Grimoire::restore`) or its message
//! (`sky::SetHour`); `Purse`, `Honours`, `Standing` and `Guild.hunted` are plain data set once
//! here at start.
//! Also kept (agent-1): the class picked (on load the class panel stays shut and the outfit goes
//! back on, without granting the kit twice), the weapons carried and the one in hand, and the
//! order's path and rank (`discipline::Mastery`). All `#[serde(default)]`, so older saves load.
//! The pause menu saves through `SaveNow` and starts a new game through `SaveState::forget`.
use super::class::{Class, ClassChoice};
use super::combat::Equip;
use super::discipline::{Mastery, Path as OrderPath};
use super::contracts::{Board, ContractBook, Honours, Open, Standing};
use super::guild::{Guild, Purse, Rested};
use super::hud::log::{HudLog, LogKind};
use super::inventory::Inventory;
use super::items::ItemKind;
use super::magic::Grimoire;
use super::skills::Sheet;
use super::sky::{Clock, SetHour};
use super::stats::Stats;
use super::{Fighter, Loadout, Mover, Player};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Bumped when the format changes incompatibly; older files are refused, not misread.
pub const VERSION: u32 = 1;
/// HUD log key for save / load lines.
const LOG_KEY: u64 = 7301;

/// The whole save file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SaveFile {
    pub version: u32,
    pub hour: f32,
    pub player: PlayerSave,
    pub guild: GuildSave,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct PlayerSave {
    pub at: [f32; 3],
    pub yaw: f32,
    pub level: u32,
    pub attrs: [u32; 6],
    pub points: u32,
    pub xp: u32,
    pub gold: u32,
    pub skills: BTreeMap<String, f64>,
    pub abilities: Vec<String>,
    pub deeds: BTreeMap<String, f32>,
    pub titles: Vec<String>,
    /// Bag: (slot, item kind, count).
    pub bag: Vec<(usize, String, u32)>,
    pub art: String,
    pub spell: String,
    pub mastery: BTreeMap<String, f32>,
    pub debt: f32,
    pub honours: Vec<String>,
    pub holdings: Vec<String>,
    /// The class picked (`class::Kit::name`); `None` in older saves.
    #[serde(default)]
    pub class: Option<String>,
    /// Weapons carried (`Loadout::slots`) and the one in hand.
    #[serde(default)]
    pub weapons: Vec<String>,
    #[serde(default)]
    pub equipped: usize,
    /// The order's path and rank (`discipline::Mastery`), if any.
    #[serde(default)]
    pub order: Option<(String, u8)>,
}

/// One open contract: template index, progress per objective, dawns left (timed ones).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct OpenSave {
    pub index: usize,
    pub progress: Vec<f32>,
    #[serde(default)]
    pub days_left: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct GuildSave {
    /// Open contracts.
    pub open: Vec<OpenSave>,
    pub next: usize,
    pub done: u32,
    pub ready: Vec<usize>,
    pub held: usize,
    /// Bounty targets brought down, by name (rumours).
    #[serde(default)]
    pub hunted: BTreeMap<String, u32>,
}

/// The player's saved pieces, borrowed for `capture`.
pub struct PlayerParts<'a> {
    pub tf: &'a Transform,
    pub yaw: f32,
    pub stats: &'a Stats,
    pub sheet: &'a Sheet,
    pub gold: u32,
    pub bag: &'a Inventory,
    pub grimoire: &'a Grimoire,
    pub honours: &'a Honours,
    pub standing: &'a Standing,
    pub class: Option<Class>,
    pub loadout: Option<&'a Loadout>,
    pub mastery: Option<&'a Mastery>,
}

fn order_name(p: OrderPath) -> &'static str {
    match p {
        OrderPath::None => "none",
        OrderPath::Monk => "monk",
        OrderPath::Blade => "blade",
        OrderPath::Arcanist => "arcanist",
        OrderPath::Hunter => "hunter",
    }
}

fn order_from(s: &str) -> OrderPath {
    [OrderPath::Monk, OrderPath::Blade, OrderPath::Arcanist, OrderPath::Hunter].into_iter().find(|p| order_name(*p) == s).unwrap_or_default()
}

/// Save right away (the pause menu's Save now and Save and quit).
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct SaveNow;

/// Snapshot the player and the guild board (and the bounty tally, `hunted`).
pub fn capture(p: &PlayerParts, board: &Board, hunted: &HashMap<String, u32>, hour: f32) -> SaveFile {
    let t = p.tf.translation;
    SaveFile {
        version: VERSION,
        hour,
        player: PlayerSave {
            at: [t.x, t.y, t.z],
            yaw: p.yaw,
            level: p.stats.level,
            attrs: p.stats.attrs(),
            points: p.stats.points,
            xp: p.stats.xp,
            gold: p.gold,
            skills: p.sheet.xp.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            abilities: p.sheet.abilities.iter().map(|a| a.to_string()).collect(),
            deeds: p.sheet.counts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            titles: p.sheet.titles.clone(),
            bag: p.bag.slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, format!("{:?}", s.kind), s.count))).collect(),
            art: p.grimoire.art.to_string(),
            spell: p.grimoire.spell.to_string(),
            mastery: p.grimoire.mastery.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            debt: p.grimoire.debt,
            honours: p.honours.0.clone(),
            holdings: p.standing.holdings.clone(),
            class: p.class.map(|c| c.kit().name.to_string()),
            weapons: p.loadout.map_or_else(Vec::new, |l| l.slots.clone()),
            equipped: p.loadout.map_or(0, |l| l.current),
            order: p.mastery.filter(|m| m.path != OrderPath::None).map(|m| (order_name(m.path).to_string(), m.rank)),
        },
        guild: GuildSave {
            open: board.open.iter().map(|o| OpenSave { index: o.index, progress: o.progress.clone(), days_left: o.days_left }).collect(),
            next: board.next_offer(),
            done: board.done,
            ready: board.ready.clone(),
            held: board.held,
            hunted: hunted.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        },
    }
}

pub fn to_json(save: &SaveFile) -> Result<String, String> {
    serde_json::to_string_pretty(save).map_err(|e| e.to_string())
}

pub fn from_json(text: &str) -> Result<SaveFile, String> {
    let save: SaveFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if save.version != VERSION {
        return Err(format!("save version {} (this build reads {VERSION})", save.version));
    }
    let a = save.player.at;
    if !(a.iter().all(|v| v.is_finite()) && save.player.yaw.is_finite() && save.hour.is_finite()) {
        return Err("save has a non-finite position or hour".into());
    }
    Ok(save)
}

/// The bag's stacks with their kinds parsed (unknown kinds are dropped).
pub fn bag_stacks(bag: &[(usize, String, u32)]) -> Vec<(usize, ItemKind, u32)> {
    bag.iter()
        .filter_map(|(slot, kind, n)| ItemKind::ALL.iter().find(|k| format!("{k:?}") == *kind).map(|k| (*slot, *k, *n)))
        .collect()
}

/// The guild board a save describes, checked against today's contract book.
pub fn board_from(g: &GuildSave, book: &ContractBook) -> Board {
    let open = g.open.iter().map(|o| Open { index: o.index, progress: o.progress.clone(), days_left: o.days_left }).collect();
    Board::restored(open, g.next, g.done, g.ready.clone(), g.held, book)
}

/// Where the save lives in this run: an explicit `GB_SAVE` always; otherwise the platform's
/// per-user data folder, except in a smoke run (nothing).
pub fn run_path(env: impl Fn(&str) -> Option<String> + Copy) -> Option<PathBuf> {
    if env("GB_SAVE").is_some_and(|v| !v.trim().is_empty()) || !smoke_run(env) { save_path(env) } else { None }
}

/// Where the save lives (`GB_SAVE`, else the platform's per-user data folder).
pub fn save_path(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.trim().is_empty());
    if let Some(p) = set("GB_SAVE") {
        return Some(PathBuf::from(p));
    }
    if let Some(appdata) = set("APPDATA") {
        return Some(Path::new(&appdata).join("Guildbound").join("save.json"));
    }
    if let Some(xdg) = set("XDG_DATA_HOME") {
        return Some(Path::new(&xdg).join("guildbound").join("save.json"));
    }
    set("HOME").map(|h| Path::new(&h).join(".local/share/guildbound/save.json"))
}

/// A smoke run (recording, scripted spawn, autoquit, screenshot): leave the save alone.
pub fn smoke_run(env: impl Fn(&str) -> Option<String>) -> bool {
    ["GB_RECORD", "GB_SPAWN", "GB_AUTOQUIT", "GB_SCREENSHOT"].iter().any(|k| env(k).is_some_and(|v| !v.trim().is_empty()))
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

/// Write `text` to `path` through a temporary file and a rename.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Save / load state.
#[derive(Resource, Debug, Default)]
pub struct SaveState {
    /// Where to save (None: smoke run or no home folder: saving off).
    pub path: Option<PathBuf>,
    /// The start-up load has been tried.
    pub loaded: bool,
}

impl SaveState {
    /// Delete the save and stop saving this run (New game: the next launch starts fresh).
    pub fn forget(&mut self) {
        if let Some(path) = self.path.take() {
            match std::fs::remove_file(&path) {
                Ok(()) => info!("[save] forgot {}", path.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => warn!("[save] {}: {e}", path.display()),
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    let path = run_path(env);
    match &path {
        Some(p) => info!("[save] file {}", p.display()),
        None => info!("[save] off (smoke run or no home folder)"),
    }
    app.insert_resource(SaveState { path, loaded: false })
        .add_message::<SaveNow>()
        // Before the class kit is handed out, so a loaded class is not granted again.
        .add_systems(Update, (load_once, save_on_demand).chain().before(super::class::apply))
        .add_systems(Last, save_on_exit);
}

type PlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static Mover,
        &'static Stats,
        &'static Sheet,
        &'static Purse,
        &'static Inventory,
        &'static Grimoire,
        Option<&'static Honours>,
        Option<&'static Standing>,
        &'static Fighter,
        Option<&'static Loadout>,
        Option<&'static Mastery>,
    ),
    With<Player>,
>;

/// Capture the player now, or why not.
fn snapshot(player: &PlayerQuery, board: &Board, guild: &Guild, clock: &Clock, class: Option<Class>) -> Result<SaveFile, &'static str> {
    let Ok((tf, mover, stats, sheet, purse, bag, grimoire, honours, standing, fighter, loadout, mastery)) = player.single() else {
        return Err("no player");
    };
    if fighter.dead {
        return Err("not while dead");
    }
    let (empty_h, empty_s) = (Honours::default(), Standing::default());
    let parts = PlayerParts {
        tf,
        yaw: mover.yaw,
        stats,
        sheet,
        gold: purse.gold,
        bag,
        grimoire,
        honours: honours.unwrap_or(&empty_h),
        standing: standing.unwrap_or(&empty_s),
        class,
        loadout,
        mastery,
    };
    Ok(capture(&parts, board, &guild.hunted, clock.hour))
}

fn write_save(path: &Path, save: &SaveFile) -> Result<(), String> {
    let text = to_json(save)?;
    write_atomic(path, &text).map_err(|e| format!("{}: {e}", path.display()))
}

/// F5, or the player waking rested: save.
#[allow(clippy::too_many_arguments)]
fn save_on_demand(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut now: MessageReader<SaveNow>,
    choice: Option<Res<ClassChoice>>,
    mut rests: MessageReader<Rested>,
    state: Res<SaveState>,
    player: PlayerQuery,
    me: Query<Entity, With<Player>>,
    board: Option<Res<Board>>,
    guild: Option<Res<Guild>>,
    clock: Option<Res<Clock>>,
    mut log: MessageWriter<HudLog>,
) {
    let me = me.single().ok();
    let rested = rests.read().any(|r| Some(r.who) == me);
    let pressed = keys.is_some_and(|k| k.just_pressed(KeyCode::F5)) | (now.read().count() > 0);
    if !(rested || pressed) || !state.loaded {
        return;
    }
    let (Some(path), Some(board), Some(guild), Some(clock)) = (state.path.as_deref(), board, guild, clock) else {
        if pressed {
            log.write(HudLog::new(LogKind::Warn, "Saving is off in this run").key(LOG_KEY));
        }
        return;
    };
    match snapshot(&player, &board, &guild, &clock, choice.as_ref().and_then(|c| c.picked)).map_err(str::to_string).and_then(|s| write_save(path, &s)) {
        Ok(()) => {
            info!("[save] saved ({})", if pressed { "F5" } else { "rested" });
            log.write(HudLog::new(LogKind::System, "Game saved").key(LOG_KEY));
        }
        Err(e) => {
            warn!("[save] not saved: {e}");
            log.write(HudLog::new(LogKind::Warn, format!("Not saved: {e}")).key(LOG_KEY));
        }
    }
}

/// Closing the game: save.
fn save_on_exit(
    mut exits: MessageReader<AppExit>,
    choice: Option<Res<ClassChoice>>,
    state: Res<SaveState>,
    player: PlayerQuery,
    board: Option<Res<Board>>,
    guild: Option<Res<Guild>>,
    clock: Option<Res<Clock>>,
) {
    if exits.read().next().is_none() || !state.loaded {
        return;
    }
    let (Some(path), Some(board), Some(guild), Some(clock)) = (state.path.as_deref(), board, guild, clock) else { return };
    match snapshot(&player, &board, &guild, &clock, choice.as_ref().and_then(|c| c.picked)).map_err(str::to_string).and_then(|s| write_save(path, &s)) {
        Ok(()) => info!("[save] saved on exit"),
        Err(e) => warn!("[save] not saved on exit: {e}"),
    }
}

/// Once the player and the board exist: load the save, if any, through the owners.
#[allow(clippy::type_complexity)]
fn load_once(
    mut state: ResMut<SaveState>,
    book: Option<Res<ContractBook>>,
    board: Option<ResMut<Board>>,
    mut player: Query<
        (
            &mut Transform,
            &mut Mover,
            &mut Stats,
            &mut Sheet,
            &mut Purse,
            &mut Inventory,
            &mut Grimoire,
            Option<&mut Honours>,
            Option<&mut Standing>,
            Entity,
            Option<&mut Loadout>,
            Option<&super::appearance::Appearance>,
        ),
        With<Player>,
    >,
    mut mastery: Query<&mut Mastery, With<Player>>,
    mut choice: Option<ResMut<ClassChoice>>,
    mut equip: MessageWriter<Equip>,
    mut commands: Commands,
    guild: Option<ResMut<Guild>>,
    mut hours: MessageWriter<SetHour>,
    mut log: MessageWriter<HudLog>,
) {
    if state.loaded {
        return;
    }
    let (Some(book), Some(mut board)) = (book, board) else { return };
    if player.is_empty() {
        return;
    }
    state.loaded = true;
    let Some(path) = state.path.clone() else { return };
    if env("GB_NEW").is_some_and(|v| v.trim() == "1") {
        info!("[save] GB_NEW=1: fresh start");
        return;
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("[save] no save yet: fresh start");
            return;
        }
        Err(e) => {
            warn!("[save] {}: {e}", path.display());
            return;
        }
    };
    let save = match from_json(&text) {
        Ok(s) => s,
        Err(e) => {
            warn!("[save] {}: {e}; starting fresh (the file is left as it is)", path.display());
            log.write(HudLog::new(LogKind::Warn, format!("Save not loaded: {e}")).key(LOG_KEY));
            return;
        }
    };
    let p = &save.player;
    let Ok(row) = player.single_mut() else { return };
    let (mut tf, mut mover, mut stats, mut sheet, mut purse, mut bag, mut grimoire, honours, standing, entity, loadout, worn) = row;
    // The class: the panel stays shut and the outfit goes back on; the kit is not granted again
    // (the saved skills and attributes already hold it).
    if let Some(class) = p.class.as_deref().and_then(Class::from_name) {
        if let Some(c) = choice.as_mut() {
            c.open = false;
            c.picked = Some(class);
            c.applied = true;
        }
        super::appearance::dress_class(&mut commands, entity, worn, &format!("{class:?}").to_lowercase());
    }
    if let (Some(mut loadout), false) = (loadout, p.weapons.is_empty()) {
        loadout.slots = p.weapons.clone();
        loadout.current = p.equipped.min(loadout.slots.len() - 1);
        equip.write(Equip { who: entity, set: loadout.equipped().to_string() });
    }
    if let Some((path, rank)) = &p.order {
        let m = Mastery { path: order_from(path), rank: *rank };
        match mastery.single_mut() {
            Ok(mut old) => *old = m,
            Err(_) => {
                commands.entity(entity).try_insert(m);
            }
        }
    }
    tf.translation = Vec3::from(p.at);
    tf.rotation = Quat::from_rotation_y(p.yaw);
    mover.yaw = p.yaw;
    mover.velocity = Vec3::ZERO;
    *stats = Stats::restored(p.level, p.attrs, p.points, p.xp);
    let skills: HashMap<String, f64> = p.skills.iter().map(|(k, v)| (k.clone(), *v)).collect();
    let deeds: HashMap<String, f32> = p.deeds.iter().map(|(k, v)| (k.clone(), *v)).collect();
    sheet.restore(&skills, &p.abilities, &deeds, &p.titles);
    purse.gold = p.gold;
    *bag = Inventory::restored(&bag_stacks(&p.bag));
    let mastery: HashMap<String, f32> = p.mastery.iter().map(|(k, v)| (k.clone(), *v)).collect();
    grimoire.restore(&p.art, &p.spell, &mastery, p.debt);
    if let Some(mut h) = honours {
        h.0 = p.honours.clone();
    }
    if let Some(mut s) = standing {
        s.holdings = p.holdings.clone();
    }
    *board = board_from(&save.guild, &book);
    if let Some(mut g) = guild {
        g.hunted = save.guild.hunted.iter().map(|(k, v)| (k.clone(), *v)).collect();
    }
    hours.write(SetHour(save.hour));
    info!(
        "[save] loaded: level {}, {} gold, rank {}, {} contracts open, at ({:.1}, {:.1}, {:.1})",
        p.level,
        p.gold,
        board.held,
        board.open.len(),
        p.at[0],
        p.at[1],
        p.at[2]
    );
    log.write(HudLog::new(LogKind::System, format!("Welcome back — level {}, {} gold", p.level, p.gold)).key(LOG_KEY));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> ContractBook {
        ContractBook::load(&super::super::contracts::contracts_path()).expect("contracts.json")
    }

    fn sample() -> SaveFile {
        let stats = Stats::restored(7, [12, 14, 10, 18, 10, 11], 3, 120);
        let mut sheet = Sheet::default();
        sheet.grant("swordsmanship", 1234.5);
        sheet.titles.push("Slime Bane".into());
        let mut bag = Inventory::default();
        bag.add(ItemKind::Apple, 3);
        bag.add(ItemKind::Log, 2);
        let grimoire = Grimoire::default();
        let honours = Honours(vec!["first_charter".into()]);
        let standing = Standing { holdings: vec!["forge".into()] };
        let tf = Transform::from_xyz(30.0, 1.5, -9.0);
        let loadout = Loadout { slots: vec!["unarmed".into(), "katana".into()], current: 1 };
        let parts = PlayerParts {
            tf: &tf,
            yaw: 1.2,
            stats: &stats,
            sheet: &sheet,
            gold: 77,
            bag: &bag,
            grimoire: &grimoire,
            honours: &honours,
            standing: &standing,
            class: Some(Class::Monk),
            loadout: Some(&loadout),
            mastery: None,
        };
        let mut board = Board::default();
        board.fill(&book(), &[]);
        board.done = 4;
        board.held = 1;
        let hunted = HashMap::from([("Rival".to_string(), 2u32)]);
        capture(&parts, &board, &hunted, 15.5)
    }

    #[test]
    fn a_save_round_trips_through_json() {
        let s = sample();
        let back = from_json(&to_json(&s).unwrap()).unwrap();
        assert_eq!(s, back);
        assert_eq!(back.player.gold, 77);
        assert_eq!(back.player.level, 7);
        assert_eq!(back.player.skills.get("swordsmanship"), Some(&1234.5));
        assert_eq!(back.guild.held, 1);
        assert_eq!(back.guild.hunted.get("Rival"), Some(&2));
    }

    #[test]
    fn restoring_rebuilds_what_was_saved() {
        let s = sample();
        let p = &s.player;
        let stats = Stats::restored(p.level, p.attrs, p.points, p.xp);
        assert_eq!((stats.level, stats.attrs(), stats.points, stats.xp), (p.level, p.attrs, p.points, p.xp));
        let bag = Inventory::restored(&bag_stacks(&p.bag));
        let apples: u32 = bag.slots.iter().flatten().filter(|s| s.kind == ItemKind::Apple).map(|s| s.count).sum();
        assert_eq!(apples, 3);
        let mut sheet = Sheet::default();
        let skills: HashMap<String, f64> = p.skills.iter().map(|(k, v)| (k.clone(), *v)).collect();
        sheet.restore(&skills, &p.abilities, &HashMap::new(), &p.titles);
        assert_eq!(sheet.level("swordsmanship"), super::super::skills::level_for_xp(1234.5));
        assert_eq!(sheet.titles, vec!["Slime Bane".to_string()]);
        let board = board_from(&s.guild, &book());
        assert_eq!((board.done, board.held, board.open.len()), (4, 1, s.guild.open.len()));
    }

    #[test]
    fn bad_saves_are_refused_and_stale_parts_dropped() {
        let mut s = sample();
        s.version = VERSION + 1;
        assert!(from_json(&to_json(&s).unwrap()).is_err(), "future version");
        assert!(from_json("{ not json").is_err());
        // A contract whose template no longer exists, an unknown item kind, a slot out of range.
        let mut s = sample();
        s.guild.open.push(OpenSave { index: 10_000, progress: vec![0.0], days_left: Some(1) });
        s.player.bag.push((0, "Dragon".into(), 1));
        s.player.bag.push((9_999, "Apple".into(), 1));
        let board = board_from(&s.guild, &book());
        assert!(board.open.iter().all(|o| o.index < 10_000));
        let bag = Inventory::restored(&bag_stacks(&s.player.bag));
        assert_eq!(bag.slots.iter().flatten().count(), sample().player.bag.len());
    }

    #[test]
    fn where_it_saves_and_when_it_stays_off() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(save_path(env(&[("GB_SAVE", "/tmp/x.json"), ("HOME", "/h")])), Some(PathBuf::from("/tmp/x.json")));
        assert_eq!(save_path(env(&[("APPDATA", "C:/A")])), Some(Path::new("C:/A").join("Guildbound").join("save.json")));
        assert_eq!(save_path(env(&[("HOME", "/h")])), Some(PathBuf::from("/h/.local/share/guildbound/save.json")));
        assert_eq!(save_path(env(&[])), None);
        assert!(smoke_run(env(&[("GB_RECORD", "/tmp/r")])));
        assert!(smoke_run(env(&[("GB_SPAWN", "1,2,3")])));
        assert!(!smoke_run(env(&[("GB_SPAWN", " ")])));
        assert!(!smoke_run(env(&[("HOME", "/h")])));
        // A smoke run saves only to a file it names.
        assert_eq!(run_path(env(&[("GB_RECORD", "/tmp/r"), ("HOME", "/h")])), None);
        assert_eq!(run_path(env(&[("GB_RECORD", "/tmp/r"), ("GB_SAVE", "/tmp/s.json")])), Some(PathBuf::from("/tmp/s.json")));
    }

    #[test]
    fn writes_replace_the_file_whole() {
        let dir = std::env::temp_dir().join(format!("gb_save_test_{}", std::process::id()));
        let path = dir.join("save.json");
        write_atomic(&path, "first").unwrap();
        write_atomic(&path, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

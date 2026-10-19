//! Save and load: the player's progress in one JSON file (`saves/slot1.json` beside `assets/`).
//! - **What is kept**: the chosen class, where the player stands, the hour, gold, level and
//!   attributes, skills, abilities, deed totals and titles, the bag, the weapons carried, the art
//!   and spell bound, honours, the order's rank, the guild board (open contracts and their
//!   progress, contracts done, finished ones waiting, rank held) and unreported bounties.
//! - **Loading**: at start-up, if the file exists, the class panel stays shut and the saved class
//!   is applied as if picked (`class::ClassChoice`); `LOAD_AFTER` s after its kit is in, the saved
//!   values overwrite it and the player is moved (`controller::Teleport`).
//! - **Saving**: every `AUTOSAVE` s of play, after a night in a guild bed (`guild::Rested`), and
//!   when the window closes or the game quits, and on `SaveNow` (the pause menu). Written to a temporary file first, then renamed, so a crash
//!   mid-write never leaves a broken save.
//! - **Smoke runs** (any `GB_*` set) neither load nor save, so test runs never touch the player's
//!   game; `GB_SAVE=<path>` opts one in with its own file. `GB_NEWGAME=1` ignores the save (the
//!   next autosave replaces it).
//!
//! Ownership: reads every saved component, writes them only once, on load.
use super::class::{Class, ClassChoice};
use super::combat::Equip;
use super::contracts::{Board, Honours, Open};
use super::controller::Teleport;
use super::discipline::{Mastery, Path};
use super::guild::{Guild, Purse, Rested};
use super::hud::log::{HudLog, LogKind};
use super::inventory::{Inventory, Stack};
use super::items::ItemKind;
use super::magic::{ABILITIES, Grimoire};
use super::skills::Sheet;
use super::sky::Clock;
use super::stats::Stats;
use super::{HALF_HEIGHT, Loadout, Mover, Player};
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use std::collections::HashMap;
use std::path::PathBuf;

/// Seconds of play between autosaves.
const AUTOSAVE: f32 = 90.0;
/// Seconds after the class kit is applied before the saved values go on (the kit's weapon swap
/// and skill grants have landed by then).
const LOAD_AFTER: f32 = 0.5;
/// Bumped when the file's shape changes; older files are ignored rather than half-read.
const VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SaveFile {
    pub version: u32,
    pub class: Option<String>,
    pub feet: [f32; 3],
    pub yaw: f32,
    pub hour: f32,
    pub gold: u32,
    pub level: u32,
    pub attrs: [u32; 6],
    pub points: u32,
    pub xp: u32,
    pub skills: HashMap<String, f64>,
    pub affinity: HashMap<String, f32>,
    pub abilities: Vec<String>,
    pub counts: HashMap<String, f32>,
    pub titles: Vec<String>,
    /// (item name, count) per bag slot.
    pub bag: Vec<Option<(String, u32)>>,
    pub weapons: Vec<String>,
    pub equipped: usize,
    pub art: String,
    pub spell: String,
    pub ability_mastery: HashMap<String, f32>,
    pub debt: f32,
    pub honours: Vec<String>,
    pub order: Option<(String, u8)>,
    pub contracts_open: Vec<(usize, Vec<f32>)>,
    pub contracts_next: usize,
    pub contracts_done: u32,
    pub contracts_ready: Vec<usize>,
    pub rank: usize,
    pub bounties: HashMap<String, u32>,
}

/// Save right away (the pause menu's Save now, and Save and quit).
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct SaveNow;

/// Where this run saves (None: a smoke run that saves nothing), and the load in progress.
#[derive(Resource, Default)]
pub struct SaveSlot {
    pub path: Option<PathBuf>,
    /// Read at start-up, applied once the class kit is in.
    pending: Option<SaveFile>,
    /// Seconds since the class kit went on (while `pending`).
    wait: f32,
    since_save: f32,
}

impl SaveSlot {
    /// Delete the save and stop saving this run (New game: the next launch starts fresh).
    pub fn forget(&mut self) {
        if let Some(path) = self.path.take() {
            match std::fs::remove_file(&path) {
                Ok(()) => info!("[save] forgot {}", path.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => error!("[save] {}: {e}", path.display()),
            }
        }
        self.pending = None;
    }
}

pub fn default_path() -> PathBuf {
    std::path::Path::new(crate::ASSETS).join("../saves/slot1.json")
}

pub fn plugin(app: &mut App) {
    let smoke = std::env::vars().any(|(k, _)| k.starts_with("GB_") && k != "GB_SAVE" && k != "GB_NEWGAME");
    let path = std::env::var("GB_SAVE").ok().filter(|s| !s.trim().is_empty()).map(PathBuf::from).or((!smoke).then(default_path));
    let fresh = std::env::var_os("GB_NEWGAME").is_some();
    let pending = path.as_deref().filter(|_| !fresh).and_then(|p| match read(p) {
        Ok(f) => f,
        Err(e) => {
            error!("[save] {e}");
            None
        }
    });
    if let Some(f) = &pending {
        info!("[save] continuing from {} (level {}, {} gold)", path.as_deref().map_or(String::new(), |p| p.display().to_string()), f.level, f.gold);
        // The saved class is applied as if picked; the panel stays shut.
        if let Some(class) = f.class.as_deref().and_then(Class::from_name) {
            app.insert_resource(ClassChoice { open: false, picked: Some(class), applied: false });
        }
    }
    app.insert_resource(SaveSlot { path, pending, wait: 0.0, since_save: 0.0 })
        .add_message::<SaveNow>()
        .add_systems(Update, load.after(super::class::apply))
        // Last, so a quit sent this frame is seen before the app stops.
        .add_systems(Last, autosave);
}

/// The save at `path`: `Ok(None)` when there is none (or it is from another version).
pub fn read(path: &std::path::Path) -> Result<Option<SaveFile>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let file: SaveFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((file.version == VERSION).then_some(file))
}

/// Write `file` to `path` (via a temporary file and a rename).
pub fn write(path: &std::path::Path, file: &SaveFile) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn path_name(p: Path) -> &'static str {
    match p {
        Path::None => "none",
        Path::Monk => "monk",
        Path::Blade => "blade",
        Path::Arcanist => "arcanist",
        Path::Hunter => "hunter",
    }
}

fn path_from(s: &str) -> Path {
    [Path::Monk, Path::Blade, Path::Arcanist, Path::Hunter].into_iter().find(|p| path_name(*p) == s).unwrap_or_default()
}

/// The static id of a known ability (`magic::ABILITIES`).
fn ability(id: &str) -> Option<&'static str> {
    ABILITIES.iter().find(|a| a.id == id).map(|a| a.id)
}

/// A deed kind as the `&'static str` `Sheet::counts` keys by. Kinds are a short fixed list, so
/// leaking the few names read from a save is bounded.
fn deed_kind(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// The player components a save reads.
type PlayerParts = (
    &'static Transform,
    &'static Mover,
    &'static Purse,
    &'static Stats,
    &'static Sheet,
    &'static Inventory,
    &'static Loadout,
    &'static Grimoire,
    Option<&'static Honours>,
    Option<&'static Mastery>,
);

#[allow(clippy::type_complexity)]
fn snapshot(
    p: (&Transform, &Mover, &Purse, &Stats, &Sheet, &Inventory, &Loadout, &Grimoire, Option<&Honours>, Option<&Mastery>),
    class: Option<Class>,
    clock: Option<&Clock>,
    board: Option<&Board>,
    guild: Option<&Guild>,
) -> SaveFile {
    let (tf, mover, purse, stats, sheet, bag, loadout, grimoire, honours, mastery) = p;
    let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
    SaveFile {
        version: VERSION,
        class: class.map(|c| c.kit().name.to_string()),
        feet: feet.to_array(),
        yaw: mover.yaw,
        hour: clock.map_or(9.0, |c| c.hour),
        gold: purse.gold,
        level: stats.level,
        attrs: stats.attrs,
        points: stats.points,
        xp: stats.xp,
        skills: sheet.xp.clone(),
        affinity: sheet.affinity.clone(),
        abilities: sheet.abilities.iter().map(|a| a.to_string()).collect(),
        counts: sheet.counts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        titles: sheet.titles.clone(),
        bag: bag.slots.iter().map(|s| s.map(|s| (s.kind.name().to_string(), s.count))).collect(),
        weapons: loadout.slots.clone(),
        equipped: loadout.current,
        art: grimoire.art.to_string(),
        spell: grimoire.spell.to_string(),
        ability_mastery: grimoire.mastery.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        debt: grimoire.debt,
        honours: honours.map_or_else(Vec::new, |h| h.0.clone()),
        order: mastery.filter(|m| m.path != Path::None).map(|m| (path_name(m.path).to_string(), m.rank)),
        contracts_open: board.map_or_else(Vec::new, |b| b.open.iter().map(|o| (o.index, o.progress.clone())).collect()),
        contracts_next: board.map_or(0, |b| b.next),
        contracts_done: board.map_or(0, |b| b.done),
        contracts_ready: board.map_or_else(Vec::new, |b| b.ready.clone()),
        rank: board.map_or(0, |b| b.held),
        bounties: guild.map_or_else(HashMap::new, |g| g.bounties.clone()),
    }
}

/// Put the pending save on the player, once its class kit is in.
#[allow(clippy::type_complexity)]
fn load(
    time: Res<Time>,
    mut slot: ResMut<SaveSlot>,
    choice: Option<Res<ClassChoice>>,
    mut players: Query<
        (Entity, &mut Purse, &mut Stats, &mut Sheet, &mut Inventory, &mut Loadout, &mut Grimoire, Option<&mut Honours>, Option<&mut Mastery>),
        With<Player>,
    >,
    mut clock: Option<ResMut<Clock>>,
    mut board: Option<ResMut<Board>>,
    mut guild: Option<ResMut<Guild>>,
    mut equip: MessageWriter<Equip>,
    mut log: MessageWriter<HudLog>,
    mut commands: Commands,
) {
    if slot.pending.is_none() {
        return;
    }
    // A save without a class (or with one this build doesn't know) loads straight away.
    let kit_in = choice.as_ref().is_none_or(|c| c.picked.is_none() || c.applied);
    if !kit_in {
        return;
    }
    slot.wait += time.delta_secs();
    if slot.wait < LOAD_AFTER {
        return;
    }
    let Ok((entity, mut purse, mut stats, mut sheet, mut bag, mut loadout, mut grimoire, honours, mastery)) = players.single_mut() else { return };
    let Some(f) = slot.pending.take() else { return };
    purse.gold = f.gold;
    *stats = Stats { level: f.level.max(1), attrs: f.attrs, points: f.points, xp: f.xp };
    sheet.xp = f.skills;
    sheet.affinity = f.affinity;
    sheet.abilities = f.abilities.iter().filter_map(|a| ability(a)).collect();
    sheet.counts = f.counts.iter().map(|(k, v)| (deed_kind(k), *v)).collect();
    sheet.titles = f.titles;
    for (slot_out, saved) in bag.slots.iter_mut().zip(f.bag.iter().chain(std::iter::repeat(&None))) {
        *slot_out = saved.as_ref().and_then(|(name, count)| {
            let kind = ItemKind::ALL.into_iter().find(|k| k.name() == name)?;
            Some(Stack { kind, count: *count })
        });
    }
    if !f.weapons.is_empty() {
        loadout.slots = f.weapons;
        loadout.current = f.equipped.min(loadout.slots.len() - 1);
        equip.write(Equip { who: entity, set: loadout.equipped().to_string() });
    }
    if let Some(a) = ability(&f.art) {
        grimoire.art = a;
    }
    if let Some(s) = ability(&f.spell) {
        grimoire.spell = s;
    }
    grimoire.mastery = f.ability_mastery.iter().filter_map(|(k, v)| Some((ability(k)?, *v))).collect();
    grimoire.debt = f.debt;
    match honours {
        Some(mut h) => h.0 = f.honours,
        None if !f.honours.is_empty() => {
            commands.entity(entity).try_insert(Honours(f.honours));
        }
        None => {}
    }
    if let Some((path, rank)) = &f.order {
        let m = Mastery { path: path_from(path), rank: *rank };
        match mastery {
            Some(mut old) => *old = m,
            None => {
                commands.entity(entity).try_insert(m);
            }
        }
    }
    if let Some(c) = clock.as_mut() {
        c.hour = f.hour.rem_euclid(24.0);
    }
    if let Some(b) = board.as_mut() {
        b.open = f.contracts_open.into_iter().map(|(index, progress)| Open { index, progress }).collect();
        b.next = f.contracts_next;
        b.done = f.contracts_done;
        b.ready = f.contracts_ready;
        b.held = f.rank;
    }
    if let Some(g) = guild.as_mut() {
        g.bounties = f.bounties;
    }
    let feet = Vec3::from_array(f.feet);
    if feet.is_finite() {
        commands.entity(entity).try_insert(Teleport { feet, yaw: f.yaw });
    }
    log.write(HudLog::new(LogKind::Info, format!("Welcome back: level {}, {} gold", f.level, f.gold)));
    slot.since_save = 0.0;
}

/// Write the save every `AUTOSAVE` s, after a night's rest, and when the window closes or the
/// game quits.
#[allow(clippy::too_many_arguments)]
fn autosave(
    time: Res<Time>,
    mut slot: ResMut<SaveSlot>,
    choice: Option<Res<ClassChoice>>,
    players: Query<(Entity, PlayerParts), With<Player>>,
    clock: Option<Res<Clock>>,
    board: Option<Res<Board>>,
    guild: Option<Res<Guild>>,
    mut rested: MessageReader<Rested>,
    mut closing: MessageReader<WindowCloseRequested>,
    mut quit: MessageReader<AppExit>,
    mut now: MessageReader<SaveNow>,
) {
    let Some(path) = slot.path.clone() else { return };
    let Ok((me, parts)) = players.single() else { return };
    let slept = rested.read().any(|r| r.who == me);
    let closed = closing.read().count() + quit.read().count() + now.read().count() > 0;
    // Nothing to keep before a class is chosen, or while a save is still being loaded.
    let class = choice.as_ref().and_then(|c| c.picked);
    if slot.pending.is_some() || choice.as_ref().is_some_and(|c| c.open || (c.picked.is_some() && !c.applied)) {
        return;
    }
    slot.since_save += time.delta_secs();
    if !(slept || closed || slot.since_save >= AUTOSAVE) {
        return;
    }
    slot.since_save = 0.0;
    let file = snapshot(parts, class, clock.as_deref(), board.as_deref(), guild.as_deref());
    match write(&path, &file) {
        Ok(()) => info!("[save] saved to {} (level {}, {} gold)", path.display(), file.level, file.gold),
        Err(e) => error!("[save] {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_round_trips_and_other_versions_are_ignored() {
        let dir = std::env::temp_dir().join(format!("gb_save_{}", std::process::id()));
        let path = dir.join("slot.json");
        assert_eq!(read(&path), Ok(None), "no file, no save");
        let mut f = SaveFile { version: VERSION, class: Some("Monk".into()), gold: 42, level: 3, ..default() };
        f.bag = vec![Some(("Apple".into(), 2)), None];
        write(&path, &f).unwrap();
        assert_eq!(read(&path), Ok(Some(f.clone())));
        f.version = VERSION + 1;
        write(&path, &f).unwrap();
        assert_eq!(read(&path), Ok(None));
        let _ = std::fs::remove_dir_all(dir);
    }
}

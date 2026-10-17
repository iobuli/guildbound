//! Skills: use-based aptitudes on top of the six stats (`stats`), discovered and levelled by deeds,
//! never picked from a menu, never lost. Data: `assets/data/skills.json` (`SkillBook`).
//!
//! `progress` turns every `deeds::Deed` (and item pickups, `items::ItemCollected`) into:
//! - **XP** for known skills whose `sources` match (kind + tag), on the RuneScape curve (1–99,
//!   level 99 at 13 034 431 XP), capped at `WINDOW_CAP` per skill per `WINDOW` s so a fight can't
//!   be farmed;
//! - **affinity** for undiscovered ones, and a **discovery** roll each time: chance
//!   `base · a / (a + K)`, certain once affinity reaches `pity`; per-character seeded, so a run
//!   replays the same;
//! - **ability discovery** (`magic::ABILITIES`): once every skill that `unlocks` an ability has
//!   reached its level, matching deeds build that ability's affinity and roll the same way
//!   (`ART_BASE`, `ART_PITY`). Arts and spells are found, not granted.
//! The player is told with a log line and a flourish; levels log too. Skill levels add their
//! row's `bonus` per level to `stats::SkillBonus`, which `stats` folds into `Derived`.
//! Everyone carries a `Sheet` (required by `Fighter`): NPCs progress the same way.
//! `skills.json` hot-reloads (`reload`, checked every second).
//!
//! **Events** are world-unique skills (`unique` rows): one holder at a time (`SkillBook.1`), freed
//! when the holder is gone. Second Wind comes from surviving close calls (`close_calls` writes
//! `survive` deeds), Spellscarred from being struck by spells; when an NPC claims one first, the
//! player hears of it.
//!
//! **Titles** (`assets/data/titles.json`, `TitleBook`): recognition earned automatically once a
//! character's skill levels and lifetime deed counts (`Sheet::counts`) meet a row's `requires`;
//! never lost. Each adds its flat `bonus` to `SkillBonus`; the player gets a log line and a
//! flourish.
use super::deeds::Deed;
use super::hud::log::{HudLog, LogKind};
use super::items::ItemCollected;
use super::magic::{ABILITIES, Ability};
use super::contracts::{HonourBook, Honours};
use super::stats::SkillBonus;
use super::vfx::{self, Element, Vfx};
use super::{Fighter, Player};
use bevy::prelude::*;
use std::collections::HashMap;

/// XP cap per skill per window (s).
const WINDOW: f32 = 30.0;
const WINDOW_CAP: f64 = 600.0;
/// Discovery: affinity half-point, and the same for abilities (base chance, pity).
const K: f32 = 4.0;
const ART_BASE: f32 = 0.15;
const ART_K: f32 = 3.0;
const ART_PITY: f32 = 8.0;
/// Abilities everyone starts knowing.
pub const STARTING: [&str; 2] = ["crescent_edge", "cinder_bolt"];

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SourceDef {
    pub kind: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub xp: f32,
    pub affinity: f32,
}

impl SourceDef {
    pub fn matches(&self, d: &Deed) -> bool {
        self.kind == d.kind && (self.tags.is_empty() || self.tags.iter().any(|t| *t == d.tag))
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct Discovery {
    pub base: f32,
    pub pity: f32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct UnlockDef {
    pub ability: String,
    pub level: u32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub name: String,
    pub family: String,
    pub sources: Vec<SourceDef>,
    pub discovery: Discovery,
    #[serde(default)]
    pub bonus: SkillBonus,
    #[serde(default)]
    pub unlocks: Vec<UnlockDef>,
    /// World-unique (an event skill): only one character may hold it at a time.
    #[serde(default)]
    pub unique: bool,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SkillsFile {
    skills: Vec<SkillDef>,
}

/// Every skill row, and who holds each world-unique one.
#[derive(Resource, Clone, Debug, Default)]
pub struct SkillBook(pub Vec<SkillDef>, pub HashMap<String, Entity>);

impl SkillBook {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: SkillsFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let book = Self(file.skills, HashMap::new());
        book.validate()?;
        Ok(book)
    }
    /// Unique ids, unlocks that name real abilities, sane discovery.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for s in &self.0 {
            if !seen.insert(&s.id) {
                return Err(format!("skill '{}' twice", s.id));
            }
            if !(s.discovery.base > 0.0 && s.discovery.pity > 0.0) {
                return Err(format!("skill '{}': discovery", s.id));
            }
            for u in &s.unlocks {
                if !ABILITIES.iter().any(|a| a.id == u.ability) {
                    return Err(format!("skill '{}' unlocks unknown ability '{}'", s.id, u.ability));
                }
            }
        }
        Ok(())
    }
    pub fn get(&self, id: &str) -> Option<&SkillDef> {
        self.0.iter().find(|s| s.id == id)
    }
    /// A unique skill someone other than `who` holds.
    pub fn taken(&self, id: &str, who: Entity) -> bool {
        self.1.get(id).is_some_and(|h| *h != who)
    }
    /// (skill, level) pairs an ability needs.
    pub fn requires(&self, ability: &str) -> Vec<(&SkillDef, u32)> {
        self.0
            .iter()
            .flat_map(|s| s.unlocks.iter().filter(|u| u.ability == ability).map(move |u| (s, u.level)))
            .collect()
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct TitleRequires {
    #[serde(default)]
    pub skills: HashMap<String, u32>,
    #[serde(default)]
    pub deeds: HashMap<String, f32>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct TitleDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub about: String,
    pub requires: TitleRequires,
    #[serde(default)]
    pub bonus: SkillBonus,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct TitlesFile {
    titles: Vec<TitleDef>,
}

/// Every title row.
#[derive(Resource, Clone, Debug, Default)]
pub struct TitleBook(pub Vec<TitleDef>);

impl TitleBook {
    /// Load and check against the skills (unique ids, real skills, something required).
    pub fn load(path: &std::path::Path, skills: &SkillBook) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: TitlesFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut seen = std::collections::HashSet::new();
        for t in &file.titles {
            if !seen.insert(&t.id) {
                return Err(format!("title '{}' twice", t.id));
            }
            if t.requires.skills.is_empty() && t.requires.deeds.is_empty() {
                return Err(format!("title '{}' requires nothing", t.id));
            }
            if let Some(s) = t.requires.skills.keys().find(|s| skills.get(s).is_none()) {
                return Err(format!("title '{}' needs unknown skill '{s}'", t.id));
            }
        }
        Ok(Self(file.titles))
    }
    pub fn get(&self, id: &str) -> Option<&TitleDef> {
        self.0.iter().find(|t| t.id == id)
    }
}

pub fn titles_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/titles.json")
}

/// Titles `sheet` now qualifies for and lacks; they are added to it.
pub fn earn(titles: &TitleBook, sheet: &mut Sheet) -> Vec<String> {
    let mut new = Vec::new();
    for t in &titles.0 {
        if sheet.titles.contains(&t.id) {
            continue;
        }
        let skills = t.requires.skills.iter().all(|(s, l)| sheet.level(s) >= *l);
        let deeds = t.requires.deeds.iter().all(|(k, n)| sheet.counts.get(k.as_str()).copied().unwrap_or(0.0) >= *n);
        if skills && deeds {
            sheet.titles.push(t.id.clone());
            new.push(t.id.clone());
        }
    }
    new
}

pub fn skills_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/skills.json")
}

/// RuneScape XP needed to reach `level` (1..=99).
pub fn xp_for_level(level: u32) -> f64 {
    let mut points = 0.0f64;
    for l in 1..level.clamp(1, 99) {
        points += (l as f64 + 300.0 * 2f64.powf(l as f64 / 7.0)).floor();
    }
    (points / 4.0).floor()
}

/// Level for `xp` (1..=99).
pub fn level_for_xp(xp: f64) -> u32 {
    let mut level = 1;
    while level < 99 && xp >= xp_for_level(level + 1) {
        level += 1;
    }
    level
}

/// A character's skills, affinities and discovered abilities (written by `skills` only).
#[derive(Component, Clone, Debug)]
pub struct Sheet {
    /// Known skills → XP.
    pub xp: HashMap<String, f64>,
    /// Undiscovered skills → affinity.
    pub affinity: HashMap<String, f32>,
    /// Discovered abilities (`magic::ABILITIES` ids).
    pub abilities: Vec<&'static str>,
    ability_affinity: HashMap<&'static str, f32>,
    /// Lifetime deed totals by kind (titles).
    pub counts: HashMap<&'static str, f32>,
    /// Titles earned, in order.
    pub titles: Vec<String>,
    rng: u32,
    seeded: bool,
    /// Skill → (seconds into the window, XP gained in it).
    window: HashMap<String, (f32, f64)>,
}

impl Default for Sheet {
    fn default() -> Self {
        Self {
            xp: HashMap::new(),
            affinity: HashMap::new(),
            abilities: STARTING.to_vec(),
            ability_affinity: HashMap::new(),
            counts: HashMap::new(),
            titles: Vec::new(),
            rng: 0x2545_F491,
            seeded: false,
            window: HashMap::new(),
        }
    }
}

impl Sheet {
    pub fn knows(&self, skill: &str) -> bool {
        self.xp.contains_key(skill)
    }
    /// Level of a skill (0 when undiscovered).
    pub fn level(&self, skill: &str) -> u32 {
        self.xp.get(skill).map_or(0, |x| level_for_xp(*x))
    }
    pub fn has_ability(&self, id: &str) -> bool {
        self.abilities.contains(&id)
    }
    pub fn seed(&mut self, seed: u32) {
        self.rng = seed.max(1);
        self.seeded = true;
    }
    /// Learn a skill outright at `xp` (presets).
    pub fn grant(&mut self, skill: &str, xp: f64) {
        self.xp.insert(skill.to_string(), xp);
        self.affinity.remove(skill);
    }
    fn roll(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// What one deed changed on a sheet.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub titles: Vec<String>,
    pub discovered: Vec<String>,
    pub levelled: Vec<(String, u32)>,
    pub abilities: Vec<&'static str>,
}

/// Apply one deed to a sheet (pure: the systems wrap it with I/O).
pub fn apply(book: &SkillBook, sheet: &mut Sheet, deed: &Deed, dt_since: f32) -> Outcome {
    let mut out = Outcome::default();
    *sheet.counts.entry(deed.kind).or_insert(0.0) += deed.amount.max(0.0);
    for w in sheet.window.values_mut() {
        w.0 += dt_since;
    }
    let mut touched: Vec<String> = Vec::new();
    for s in &book.0 {
        for src in s.sources.iter().filter(|src| src.matches(deed)) {
            let units = deed.amount.max(0.0);
            touched.push(s.id.clone());
            if let Some(xp) = sheet.xp.get(&s.id).copied() {
                let w = sheet.window.entry(s.id.clone()).or_insert((0.0, 0.0));
                if w.0 > WINDOW {
                    *w = (0.0, 0.0);
                }
                let gain = ((src.xp * units) as f64).min((WINDOW_CAP - w.1).max(0.0));
                w.1 += gain;
                let before = level_for_xp(xp);
                let now = xp + gain;
                sheet.xp.insert(s.id.clone(), now);
                let after = level_for_xp(now);
                if after > before {
                    out.levelled.push((s.id.clone(), after));
                }
            } else if s.unique && book.taken(&s.id, deed.who) {
                continue;
            } else {
                let a = sheet.affinity.entry(s.id.clone()).or_insert(0.0);
                *a += src.affinity * units;
                let a = *a;
                let p = s.discovery.base * a / (a + K);
                if a >= s.discovery.pity || sheet.roll() < p {
                    sheet.grant(&s.id, 0.0);
                    out.discovered.push(s.id.clone());
                }
            }
        }
    }
    // Abilities whose requirements are met grow on deeds that train a requiring skill.
    for a in ABILITIES.iter() {
        if sheet.has_ability(a.id) {
            continue;
        }
        let req = book.requires(a.id);
        if req.is_empty() || !req.iter().all(|(s, lvl)| sheet.level(&s.id) >= *lvl) {
            continue;
        }
        if !req.iter().any(|(s, _)| touched.contains(&s.id)) {
            continue;
        }
        let aff = sheet.ability_affinity.entry(a.id).or_insert(0.0);
        *aff += 1.0;
        let aff = *aff;
        if aff >= ART_PITY || sheet.roll() < ART_BASE * aff / (aff + ART_K) {
            sheet.abilities.push(a.id);
            out.abilities.push(a.id);
        }
    }
    out
}

/// The sum of every known skill's bonus × level, plus every earned title's bonus.
pub fn bonus(book: &SkillBook, titles: &TitleBook, sheet: &Sheet) -> SkillBonus {
    let mut b = SkillBonus::default();
    for s in &book.0 {
        let l = sheet.level(&s.id) as f32;
        if l > 0.0 {
            b.add(&s.bonus, l);
        }
    }
    for t in sheet.titles.iter().filter_map(|id| titles.get(id)) {
        b.add(&t.bonus, 1.0);
    }
    b
}

pub fn plugin(app: &mut App) {
    let book = SkillBook::load(&skills_path()).unwrap_or_else(|e| {
        error!("[skills] {e}");
        SkillBook::default()
    });
    let titles = TitleBook::load(&titles_path(), &book).unwrap_or_else(|e| {
        error!("[skills] {e}");
        TitleBook::default()
    });
    info!("[skills] {} skills, {} titles", book.0.len(), titles.0.len());
    app.insert_resource(book)
        .insert_resource(titles)
        .register_required_components::<Fighter, Sheet>()
        .add_systems(Update, (reload, close_calls, seed, progress, entitle, publish).chain().after(super::PlaySet::Act).before(super::PlaySet::Select));
}

/// How often `skills.json` is checked for edits (real s).
const RELOAD_EVERY: f32 = 1.0;

/// Hot reload: an edited `skills.json` replaces the book once it loads and validates (a bad edit
/// is logged and the old book kept); every sheet then re-publishes its bonus.
fn reload(
    time: Res<Time<Real>>,
    mut book: ResMut<SkillBook>,
    mut titles: ResMut<TitleBook>,
    mut sheets: Query<&mut Sheet>,
    mut log: MessageWriter<HudLog>,
    mut state: Local<(f32, Option<std::time::SystemTime>)>,
) {
    state.0 += time.delta_secs();
    if state.0 < RELOAD_EVERY {
        return;
    }
    state.0 = 0.0;
    let path = skills_path();
    let modified = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let Some(stamp) = modified(&path).max(modified(&titles_path())) else { return };
    let Some(seen) = state.1.replace(stamp) else { return };
    if seen == stamp {
        return;
    }
    match SkillBook::load(&path).and_then(|b| TitleBook::load(&titles_path(), &b).map(|t| (b, t))) {
        Ok((new, new_titles)) => {
            info!("[skills] reloaded: {} skills, {} titles", new.0.len(), new_titles.0.len());
            let claims = std::mem::take(&mut book.1);
            *book = new;
            book.1 = claims;
            *titles = new_titles;
            for mut s in &mut sheets {
                s.set_changed();
            }
            log.write(HudLog::new(LogKind::System, "skills.json / titles.json reloaded").key(7201));
        }
        Err(e) => {
            error!("[skills] {e}");
            log.write(HudLog::new(LogKind::Warn, "skills / titles: error, kept the old ones (see log)").key(7201));
        }
    }
}

/// Give every sheet its own seed (from its entity) before its first deed.
fn seed(mut sheets: Query<(Entity, &mut Sheet), Changed<Sheet>>) {
    for (e, mut s) in &mut sheets {
        if !s.seeded {
            let bits = e.to_bits();
            s.seed((bits as u32) ^ ((bits >> 32) as u32) ^ 0x9E37_79B9);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn progress(
    time: Res<Time>,
    mut book: ResMut<SkillBook>,
    titles: Res<TitleBook>,
    vfx: Option<Res<Vfx>>,
    mut deeds: MessageReader<Deed>,
    mut items: MessageReader<ItemCollected>,
    mut log: MessageWriter<HudLog>,
    mut sheets: Query<(&mut Sheet, Option<&Transform>, Has<Player>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let mut all: Vec<Deed> = deeds.read().cloned().collect();
    for c in items.read() {
        all.push(Deed::new(c.who, "gather", format!("{:?}", c.kind), c.count as f32));
    }
    let mut first = true;
    for deed in &all {
        let Ok((mut sheet, tf, player)) = sheets.get_mut(deed.who) else { continue };
        let mut out = apply(&book, &mut sheet, deed, if first { dt } else { 0.0 });
        out.titles = earn(&titles, &mut sheet);
        first = false;
        let claimed: Vec<String> = out.discovered.iter().filter(|id| book.get(id).is_some_and(|s| s.unique)).cloned().collect();
        for id in &claimed {
            book.1.insert(id.clone(), deed.who);
            if !player {
                let name = book.get(id).map_or(id.clone(), |s| s.name.clone());
                info!("[skills] unique {id} claimed by {:?}", deed.who);
                log.write(HudLog::new(LogKind::Warn, format!("Somewhere, someone has claimed {name}.")));
            }
        }
        if !player {
            continue;
        }
        let name = |id: &str| book.get(id).map_or(id.to_string(), |s| s.name.clone());
        for id in &out.discovered {
            info!("[skills] discovered {id}");
            let unique = if claimed.contains(id) { "  (unique: yours alone)" } else { "" };
            log.write(HudLog::new(LogKind::Item, format!("Discovered: {}{unique}", name(id))));
        }
        for (id, level) in &out.levelled {
            info!("[skills] {id} {level}");
            log.write(HudLog::new(LogKind::Item, format!("{} {level}", name(id))));
        }
        for id in &out.abilities {
            info!("[skills] ability {id}");
            let a: Option<&Ability> = ABILITIES.iter().find(|a| a.id == *id);
            let what = if a.is_some_and(|a| a.is_art()) { "New art" } else { "New spell" };
            log.write(HudLog::new(LogKind::Item, format!("{what}: {}  [K]", a.map_or(*id, |a| a.name))));
        }
        for id in &out.titles {
            info!("[skills] title {id}");
            let name = titles.get(id).map_or(id.as_str(), |t| t.name.as_str());
            log.write(HudLog::new(LogKind::Item, format!("Title earned: {name}  [K]")));
        }
        let flourish = !out.discovered.is_empty() || !out.abilities.is_empty() || !out.titles.is_empty();
        if flourish && let (Some(vfx), Some(tf)) = (vfx.as_deref(), tf) {
            vfx::impact(&mut commands, &mut meshes, &mut materials, vfx, Element::Aura, tf.translation + Vec3::Y * 0.6, 1.2);
        }
    }
}

/// A close call: below `CLOSE_CALL` of max hp, and still alive `SURVIVE` s later → a `survive`
/// deed (Second Wind). Also frees unique skills whose holder is gone.
const CLOSE_CALL: f32 = 0.15;
const SURVIVE: f32 = 8.0;

fn close_calls(
    time: Res<Time>,
    mut book: ResMut<SkillBook>,
    fighters: Query<(Entity, &Fighter)>,
    mut deeds: MessageWriter<Deed>,
    mut timers: Local<HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    for (e, f) in &fighters {
        let low = !f.dead && f.hp > 0.0 && f.hp < CLOSE_CALL * f.max_hp.max(1.0);
        match (timers.get_mut(&e), low) {
            (Some(_), _) if f.dead => {
                timers.remove(&e);
            }
            (Some(t), _) => {
                *t += dt;
                if *t >= SURVIVE {
                    timers.remove(&e);
                    deeds.write(Deed::new(e, "survive", "", 1.0));
                }
            }
            (None, true) => {
                timers.insert(e, 0.0);
            }
            (None, false) => {}
        }
    }
    timers.retain(|e, _| fighters.contains(*e));
    if book.1.values().any(|h| !fighters.contains(*h)) {
        book.1.retain(|_, h| fighters.contains(*h));
    }
}

/// Titles from any change (presets, reloads), not only deeds; quiet (deeds log the player's).
fn entitle(titles: Res<TitleBook>, mut sheets: Query<&mut Sheet, Changed<Sheet>>) {
    for mut sheet in &mut sheets {
        if !earn(&titles, sheet.bypass_change_detection()).is_empty() {
            sheet.set_changed();
        }
    }
}

/// Skill levels → `SkillBonus` for `stats`.
#[allow(clippy::type_complexity)]
fn publish(
    book: Res<SkillBook>,
    titles: Res<TitleBook>,
    honour_book: Option<Res<HonourBook>>,
    mut commands: Commands,
    sheets: Query<(Entity, &Sheet, Option<&Honours>, Option<&SkillBonus>), Or<(Changed<Sheet>, Changed<Honours>)>>,
) {
    for (e, sheet, honours, old) in &sheets {
        let mut b = bonus(&book, &titles, sheet);
        // Honours (`contracts`) add their flat bonus too.
        if let (Some(hb), Some(h)) = (honour_book.as_deref(), honours) {
            for g in h.0.iter().filter_map(|id| hb.get(id)) {
                b.add(&g.grants.bonus, 1.0);
            }
        }
        if old != Some(&b) {
            commands.entity(e).try_insert(b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> SkillBook {
        SkillBook::load(&skills_path()).expect("skills.json")
    }

    #[test]
    fn runescape_curve() {
        assert_eq!(xp_for_level(1), 0.0);
        assert_eq!(xp_for_level(2), 83.0);
        assert_eq!(xp_for_level(10), 1154.0);
        assert_eq!(xp_for_level(99), 13_034_431.0);
        assert_eq!(level_for_xp(0.0), 1);
        assert_eq!(level_for_xp(1154.0), 10);
        assert_eq!(level_for_xp(1e9), 99);
    }

    #[test]
    fn data_loads_and_every_ability_is_reachable() {
        let b = book();
        assert!(b.0.len() >= 10);
        for a in ABILITIES.iter() {
            assert!(
                STARTING.contains(&a.id) || !b.requires(a.id).is_empty(),
                "{} can never be discovered",
                a.id
            );
        }
    }

    #[test]
    fn sources_match_kind_and_tag() {
        let s = SourceDef {
            kind: "hit".into(),
            tags: vec!["katana".into()],
            xp: 1.0,
            affinity: 1.0,
        };
        let e = Entity::PLACEHOLDER;
        assert!(s.matches(&Deed::new(e, "hit", "katana", 1.0)));
        assert!(!s.matches(&Deed::new(e, "hit", "axe", 1.0)));
        assert!(!s.matches(&Deed::new(e, "block", "katana", 1.0)));
    }

    #[test]
    fn deeds_discover_then_level_and_the_same_seed_replays() {
        let b = book();
        let e = Entity::PLACEHOLDER;
        let run = |seed: u32| {
            let mut sheet = Sheet::default();
            sheet.seed(seed);
            let mut found_at = None;
            for i in 0..200 {
                let out = apply(&b, &mut sheet, &Deed::new(e, "swim", "", 1.0), 1.0);
                if out.discovered.contains(&"swimming".to_string()) && found_at.is_none() {
                    found_at = Some(i);
                }
            }
            (found_at, sheet.level("swimming"))
        };
        let (a, la) = run(7);
        let (b2, lb) = run(7);
        assert_eq!((a, la), (b2, lb), "seeded");
        let found = a.expect("discovered");
        assert!(found < 4, "pity 4 guarantees discovery by the 4th swim, got {found}");
        assert!(la > 1, "swimming levelled");
    }

    #[test]
    fn xp_is_capped_per_window() {
        let b = book();
        let e = Entity::PLACEHOLDER;
        let mut sheet = Sheet::default();
        sheet.grant("woodcutting", 0.0);
        for _ in 0..100 {
            apply(&b, &mut sheet, &Deed::new(e, "fell", "", 1.0), 0.0);
        }
        assert!(sheet.xp["woodcutting"] <= WINDOW_CAP + 1e-6);
        apply(&b, &mut sheet, &Deed::new(e, "fell", "", 1.0), WINDOW + 1.0);
        assert!(sheet.xp["woodcutting"] > WINDOW_CAP);
    }

    #[test]
    fn arts_are_discovered_once_requirements_hold() {
        let b = book();
        let e = Entity::PLACEHOLDER;
        let mut sheet = Sheet::default();
        assert!(!sheet.has_ability("flash_step"));
        // Aura Arts below level 2: no flash step however many casts.
        sheet.grant("aura_arts", 0.0);
        for _ in 0..3 {
            apply(&b, &mut sheet, &Deed::new(e, "cast", "crescent_edge", 1.0), 0.0);
        }
        assert!(!sheet.has_ability("flash_step"));
        sheet.grant("aura_arts", xp_for_level(2));
        for _ in 0..(ART_PITY as usize) {
            apply(&b, &mut sheet, &Deed::new(e, "cast", "crescent_edge", 1.0), 0.0);
        }
        assert!(sheet.has_ability("flash_step"), "pity guarantees it");
    }

    #[test]
    fn bonuses_scale_with_level() {
        let b = book();
        let t = TitleBook::default();
        let mut sheet = Sheet::default();
        assert_eq!(bonus(&b, &t, &sheet), SkillBonus::default());
        sheet.grant("swordsmanship", xp_for_level(10));
        assert!(bonus(&b, &t, &sheet).might > 0.0);
    }

    #[test]
    fn unique_skills_have_one_holder() {
        let mut b = book();
        let (a, z) = (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap());
        assert!(b.get("second_wind").is_some_and(|s| s.unique));
        let mut first = Sheet::default();
        let out = apply(&b, &mut first, &Deed::new(a, "survive", "", 1.0), 0.0);
        let out = if out.discovered.is_empty() { apply(&b, &mut first, &Deed::new(a, "survive", "", 1.0), 0.0) } else { out };
        assert!(out.discovered.contains(&"second_wind".to_string()), "pity 2");
        b.1.insert("second_wind".into(), a);
        let mut second = Sheet::default();
        for _ in 0..10 {
            apply(&b, &mut second, &Deed::new(z, "survive", "", 1.0), 0.0);
        }
        assert!(!second.knows("second_wind"), "taken");
        assert!(second.affinity.get("second_wind").is_none());
    }

    #[test]
    fn titles_load_and_are_earned_once() {
        let b = book();
        let t = TitleBook::load(&titles_path(), &b).expect("titles.json");
        assert!(t.0.len() >= 8);
        let e = Entity::PLACEHOLDER;
        let mut sheet = Sheet::default();
        sheet.grant("swordsmanship", xp_for_level(10));
        assert_eq!(earn(&t, &mut sheet), ["blade_adept"]);
        assert!(earn(&t, &mut sheet).is_empty(), "once");
        let before = bonus(&b, &t, &sheet).might;
        for _ in 0..5 {
            apply(&b, &mut sheet, &Deed::new(e, "kill", "", 1.0), 0.0);
        }
        for _ in 0..9 {
            apply(&b, &mut sheet, &Deed::new(e, "parry", "", 1.0), 0.0);
        }
        assert!(!earn(&t, &mut sheet).contains(&"duelist".to_string()), "9 parries are not 10");
        apply(&b, &mut sheet, &Deed::new(e, "parry", "", 1.0), 0.0);
        assert!(earn(&t, &mut sheet).contains(&"duelist".to_string()));
        assert!(bonus(&b, &t, &sheet).max_stamina > 0.0);
        assert!(bonus(&b, &t, &sheet).might >= before);
    }
}

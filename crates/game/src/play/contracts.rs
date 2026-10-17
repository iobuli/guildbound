//! Contracts: posted work for the player (`assets/data/contracts.json`, `ContractBook`). Three are
//! open at a time (`Board`, written here only); each objective is a deed pattern × count, so
//! progress comes from the player's `deeds::Deed`s like everything else. A finished contract waits
//! in `Board.ready` until it is turned in at the guild desk (`TurnIn` → experience through
//! `stats::GrantXp`, gold through `Paid`), and the next template in turn takes its place
//! (never one already open). **Guild rank** (F, E, D, C, B, A, S): contracts completed make you
//! *eligible* (`RANKS`); the rank *held* rises only when the guild issues the card after the
//! exam (`RankUp`, from `guild`); a template is only posted once the rank reaches its `rank`. **Issuers**: a template
//! with an `issuer` is posted only while a character of that name with `Standing` (holdings: the
//! Smith's forge, the Farmer's fields) is about; the WORK tab names who posted it.
//! **Honours** (`assets/data/honours.json`, `HonourBook`): standing above rank and titles, granted
//! to the player once a row's rank, title count, named title and earlier honours hold; each adds
//! its holdings to the player's `Standing` and its bonus (through `skills::publish`); the badge
//! turns gold (`insignia`); every dawn the holdings yield their `daily_xp` (`yields`). `Honours` and `Standing` are written here (the world seeds NPC
//! holdings at spawn). Shown on the stats page's WORK tab, which the courtyard noticeboard
//! (interactable kind "board", F) opens.
use super::deeds::Deed;
use super::hud::log::{HudLog, LogKind};
use super::skills::Sheet;
use super::stats::{GrantXp, SkillBonus};
use super::hud::stats_page::{StatsOpen, StatsTab, ToggleStats};
use super::interact::UseEffect;
use super::{Interactable, Player};
use bevy::prelude::*;

/// Contracts open at once.
pub const OPEN: usize = 3;
/// Guild ranks and the contracts completed to reach each.
pub const RANKS: [(&str, u32); 7] = [("F", 0), ("E", 3), ("D", 6), ("C", 10), ("B", 15), ("A", 21), ("S", 28)];

/// Rank index for `done` contracts.
pub fn rank_for(done: u32) -> usize {
    RANKS.iter().rposition(|(_, n)| done >= *n).unwrap_or(0)
}

/// Index of a rank letter (unknown → None).
pub fn rank_index(letter: &str) -> Option<usize> {
    RANKS.iter().position(|(l, _)| *l == letter)
}

fn rank_f() -> String {
    "F".into()
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Objective {
    pub kind: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub count: f32,
}

impl Objective {
    pub fn matches(&self, d: &Deed) -> bool {
        self.kind == d.kind && (self.tags.is_empty() || self.tags.iter().any(|t| *t == d.tag))
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Reward {
    #[serde(default)]
    pub xp: u32,
    /// Paid at the guild desk on turn-in.
    #[serde(default)]
    pub gold: u32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ContractDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub about: String,
    pub objectives: Vec<Objective>,
    #[serde(default)]
    pub reward: Reward,
    /// Guild rank needed before it is posted.
    #[serde(default = "rank_f")]
    pub rank: String,
    /// Who posts it (a character's name); `None` = the guild.
    #[serde(default)]
    pub issuer: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct HonourRequires {
    pub rank: String,
    #[serde(default)]
    pub titles: usize,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub honours: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct HonourGrants {
    #[serde(default)]
    pub holdings: Vec<String>,
    /// Experience the holdings yield every dawn (`sky::Clock`).
    #[serde(default)]
    pub daily_xp: u32,
    #[serde(default)]
    pub bonus: SkillBonus,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct HonourDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub about: String,
    pub requires: HonourRequires,
    #[serde(default)]
    pub grants: HonourGrants,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct HonoursFile {
    honours: Vec<HonourDef>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct HonourBook(pub Vec<HonourDef>);

impl HonourBook {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: HonoursFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        for h in &file.honours {
            if rank_index(&h.requires.rank).is_none() {
                return Err(format!("honour '{}': unknown rank '{}'", h.id, h.requires.rank));
            }
            if let Some(r) = h.requires.honours.iter().find(|r| !file.honours.iter().any(|o| &o.id == *r)) {
                return Err(format!("honour '{}' needs unknown honour '{r}'", h.id));
            }
        }
        Ok(Self(file.honours))
    }
    pub fn get(&self, id: &str) -> Option<&HonourDef> {
        self.0.iter().find(|h| h.id == id)
    }
}

/// Whether `def` is due at guild rank index `rank` with `titles` and `honours` held.
pub fn qualifies(def: &HonourDef, rank: usize, titles: &[String], honours: &[String]) -> bool {
    let r = &def.requires;
    rank_index(&r.rank).is_some_and(|need| rank >= need)
        && titles.len() >= r.titles
        && r.title.as_ref().is_none_or(|t| titles.contains(t))
        && r.honours.iter().all(|h| honours.contains(h))
}

/// Honours a character holds (written by `contracts` only).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Honours(pub Vec<String>);

pub fn honours_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/honours.json")
}

/// Holdings that let a character post contracts (seeded by `world` at spawn, grown by honours).
#[derive(Component, Clone, Debug, Default)]
pub struct Standing {
    pub holdings: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ContractsFile {
    contracts: Vec<ContractDef>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ContractBook(pub Vec<ContractDef>);

impl ContractBook {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: ContractsFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        for c in &file.contracts {
            if c.objectives.is_empty() || c.objectives.iter().any(|o| !(o.count > 0.0)) {
                return Err(format!("contract '{}': objectives", c.id));
            }
            if rank_index(&c.rank).is_none() {
                return Err(format!("contract '{}': unknown rank '{}'", c.id, c.rank));
            }
        }
        Ok(Self(file.contracts))
    }
}

/// One open contract: its template and progress per objective.
#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    pub index: usize,
    pub progress: Vec<f32>,
}

/// The player's open contracts and how many are done (written by `contracts` only).
#[derive(Resource, Clone, Debug, Default)]
pub struct Board {
    pub open: Vec<Open>,
    /// Next template to offer.
    next: usize,
    /// Contracts completed (eligibility for the next rank).
    pub done: u32,
    /// Finished contracts waiting to be turned in at the guild desk.
    pub ready: Vec<usize>,
    /// The rank held (index into `RANKS`): raised only by `RankUp` after the exam.
    pub held: usize,
}

impl Board {
    /// The rank held (gates postings and honours).
    pub fn rank(&self) -> usize {
        self.held
    }
    /// The highest rank the contracts done qualify for.
    pub fn eligible(&self) -> usize {
        rank_for(self.done)
    }
    /// Pay out everything ready: (gold, xp, contracts).
    pub fn turn_in(&mut self, book: &ContractBook) -> (u32, u32, usize) {
        let defs: Vec<&ContractDef> = self.ready.iter().filter_map(|i| book.0.get(*i)).collect();
        let paid = (defs.iter().map(|d| d.reward.gold).sum(), defs.iter().map(|d| d.reward.xp).sum(), defs.len());
        self.ready.clear();
        paid
    }
    /// Fill up to `OPEN` with the next templates not already open that the rank allows and
    /// whose issuer (if any) is among `issuers`.
    pub fn fill(&mut self, book: &ContractBook, issuers: &[String]) {
        let n = book.0.len();
        let rank = self.rank();
        let mut tries = 0;
        while self.open.len() < OPEN.min(n) && tries < n {
            let i = self.next % n;
            self.next += 1;
            tries += 1;
            let def = &book.0[i];
            let allowed = rank_index(&def.rank).is_some_and(|r| r <= rank)
                && def.issuer.as_ref().is_none_or(|who| issuers.contains(who));
            if allowed && !self.open.iter().any(|o| o.index == i) {
                self.open.push(Open {
                    index: i,
                    progress: vec![0.0; book.0[i].objectives.len()],
                });
            }
        }
    }
    /// Count a deed; returns the templates it finished (removed from the board).
    pub fn record(&mut self, book: &ContractBook, d: &Deed) -> Vec<usize> {
        let mut finished = Vec::new();
        for o in &mut self.open {
            let Some(def) = book.0.get(o.index) else { continue };
            for (k, obj) in def.objectives.iter().enumerate() {
                if obj.matches(d) && let Some(p) = o.progress.get_mut(k) {
                    *p = (*p + d.amount.max(0.0)).min(obj.count);
                }
            }
            if def.objectives.iter().zip(&o.progress).all(|(obj, p)| *p >= obj.count) {
                finished.push(o.index);
            }
        }
        self.open.retain(|o| !finished.contains(&o.index));
        self.done += finished.len() as u32;
        self.ready.extend(finished.iter().copied());
        finished
    }
}

pub fn contracts_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/contracts.json")
}

pub fn plugin(app: &mut App) {
    let book = ContractBook::load(&contracts_path()).unwrap_or_else(|e| {
        error!("[contracts] {e}");
        ContractBook::default()
    });
    let mut board = Board::default();
    board.fill(&book, &[]);
    let honours = HonourBook::load(&honours_path()).unwrap_or_else(|e| {
        error!("[contracts] {e}");
        HonourBook::default()
    });
    app.insert_resource(book)
        .insert_resource(board)
        .insert_resource(honours)
        .add_message::<TurnIn>()
        .add_message::<Paid>()
        .add_message::<RankUp>()
        .add_systems(Update, (progress, settle, honour, yields, read_board).chain());
}

/// Turn in every ready contract (the guild desk).
#[derive(Message, Clone, Copy, Debug)]
pub struct TurnIn {
    pub who: Entity,
}

/// What a turn-in paid (the guild adds the gold to the purse).
#[derive(Message, Clone, Copy, Debug)]
pub struct Paid {
    pub who: Entity,
    pub gold: u32,
    pub xp: u32,
    pub count: usize,
}

/// The exam is passed: the rank card is issued (index into `RANKS`).
#[derive(Message, Clone, Copy, Debug)]
pub struct RankUp {
    pub to: usize,
}

/// Turn-ins and rank cards.
fn settle(
    book: Res<ContractBook>,
    mut board: ResMut<Board>,
    mut turn_ins: MessageReader<TurnIn>,
    mut ranks: MessageReader<RankUp>,
    mut paid: MessageWriter<Paid>,
    mut grants: MessageWriter<GrantXp>,
    mut log: MessageWriter<HudLog>,
) {
    for t in turn_ins.read() {
        let (gold, xp, count) = board.turn_in(&book);
        if count == 0 {
            continue;
        }
        info!("[contracts] payout {count} contracts: {gold} gold {xp} xp");
        if xp > 0 {
            grants.write(GrantXp { who: t.who, xp });
        }
        paid.write(Paid { who: t.who, gold, xp, count });
    }
    for r in ranks.read() {
        if r.to > board.held && r.to < RANKS.len() {
            board.held = r.to;
            info!("[contracts] rank card {}", RANKS[r.to].0);
            log.write(HudLog::new(LogKind::Item, format!("Guild rank {}: new card, new postings  [K]", RANKS[r.to].0)));
        }
    }
}

fn progress(
    book: Res<ContractBook>,
    mut board: ResMut<Board>,
    mut deeds: MessageReader<Deed>,
    players: Query<Entity, With<Player>>,
    issuers: Query<(&Name, &Standing)>,
    mut log: MessageWriter<HudLog>,
) {
    let Some(player) = players.iter().next() else {
        deeds.clear();
        return;
    };
    for d in deeds.read().filter(|d| d.who == player) {
        let before = board.eligible();
        for i in board.record(&book, d) {
            let def = &book.0[i];
            info!("[contracts] done {}", def.id);
            log.write(HudLog::new(
                LogKind::Item,
                format!("Contract complete: {}  //  report to the guild desk (+{} gold, +{} XP)", def.name, def.reward.gold, def.reward.xp),
            ));
        }
        if board.eligible() > before && board.eligible() > board.held {
            let r = RANKS[board.eligible()].0;
            info!("[contracts] eligible {r}");
            log.write(HudLog::new(LogKind::Item, format!("Eligible for rank {r}: apply at the guild desk")));
        }
        if board.open.len() < OPEN {
            let present: Vec<String> = issuers.iter().filter(|(_, s)| !s.holdings.is_empty()).map(|(n, _)| n.to_string()).collect();
            board.fill(&book, &present);
        }
    }
}

/// The player's honours follow rank, titles and earlier honours; holdings and a log line come
/// with each.
#[allow(clippy::type_complexity)]
fn honour(
    book: Res<HonourBook>,
    board: Res<Board>,
    mut commands: Commands,
    mut players: Query<(Entity, &Sheet, Option<&mut Honours>, Option<&mut Standing>), With<Player>>,
    mut log: MessageWriter<HudLog>,
) {
    let Ok((e, sheet, honours, standing)) = players.single_mut() else { return };
    let mut held = honours.as_deref().map_or(Vec::new(), |h| h.0.clone());
    let mut holdings = standing.as_deref().map_or(Vec::new(), |s| s.holdings.clone());
    let mut new = false;
    // Repeat so an honour that needs another granted this frame follows at once.
    loop {
        let Some(h) = book.0.iter().find(|h| !held.contains(&h.id) && qualifies(h, board.rank(), &sheet.titles, &held)) else {
            break;
        };
        held.push(h.id.clone());
        holdings.extend(h.grants.holdings.iter().cloned());
        new = true;
        info!("[contracts] honour {}", h.id);
        let what = if h.grants.holdings.is_empty() {
            String::new()
        } else {
            format!("  //  you hold {}", h.grants.holdings.join(", "))
        };
        log.write(HudLog::new(LogKind::Item, format!("Honour: {}{what}  [K]", h.name)));
    }
    if new {
        match honours {
            Some(mut h) => h.0 = held,
            None => {
                commands.entity(e).try_insert(Honours(held));
            }
        }
        match standing {
            Some(mut s) => s.holdings = holdings,
            None => {
                commands.entity(e).try_insert(Standing { holdings });
            }
        }
    }
}

/// Each dawn the player's holdings yield their honours' `daily_xp`.
fn yields(
    clock: Option<Res<super::sky::Clock>>,
    book: Res<HonourBook>,
    players: Query<(Entity, &Honours), With<Player>>,
    mut grants: MessageWriter<GrantXp>,
    mut log: MessageWriter<HudLog>,
    mut was_night: Local<Option<bool>>,
) {
    let Some(clock) = clock else { return };
    let night = clock.is_night();
    let dawn = *was_night == Some(true) && !night;
    *was_night = Some(night);
    if !dawn {
        return;
    }
    for (e, h) in &players {
        let xp: u32 = h.0.iter().filter_map(|id| book.get(id)).map(|d| d.grants.daily_xp).sum();
        if xp > 0 {
            grants.write(GrantXp { who: e, xp });
            log.write(HudLog::new(LogKind::Item, format!("Dawn: your holdings yield +{xp} XP")));
        }
    }
}

/// Reading the noticeboard opens the WORK tab.
fn read_board(
    mut effects: MessageReader<UseEffect>,
    boards: Query<&Interactable>,
    players: Query<(), With<Player>>,
    mut toggles: MessageWriter<ToggleStats>,
    open: Res<StatsOpen>,
) {
    for e in effects.read() {
        if players.contains(e.user)
            && boards.get(e.target).is_ok_and(|it| it.kind == "board")
            && open.0 != Some(StatsTab::Contracts)
        {
            toggles.write(ToggleStats(Some(StatsTab::Contracts)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> ContractBook {
        ContractBook::load(&contracts_path()).expect("contracts.json")
    }

    #[test]
    fn board_opens_three_distinct() {
        let b = book();
        let mut board = Board::default();
        board.fill(&b, &[]);
        assert_eq!(board.open.len(), OPEN);
        let mut ids: Vec<usize> = board.open.iter().map(|o| o.index).collect();
        ids.dedup();
        assert_eq!(ids.len(), OPEN);
    }

    #[test]
    fn deeds_finish_contracts_and_new_ones_open() {
        let b = book();
        let e = Entity::PLACEHOLDER;
        let mut board = Board::default();
        board.fill(&b, &[]);
        let first = board.open[0].index;
        let def = b.0[first].clone();
        let mut finished = Vec::new();
        for obj in &def.objectives {
            let tag = obj.tags.first().cloned().unwrap_or_default();
            for _ in 0..(obj.count.ceil() as usize) {
                finished.extend(board.record(&b, &Deed::new(e, Box::leak(obj.kind.clone().into_boxed_str()), tag.clone(), 1.0)));
            }
        }
        assert!(finished.contains(&first));
        assert_eq!(board.done as usize, finished.len());
        board.fill(&b, &[]);
        assert_eq!(board.open.len(), OPEN);
        assert!(board.open.iter().all(|o| o.index != first) || b.0.len() <= OPEN);
    }

    #[test]
    fn rank_gates_postings() {
        let b = book();
        assert_eq!(rank_for(0), 0);
        assert_eq!(RANKS[rank_for(3)].0, "E");
        assert_eq!(RANKS[rank_for(100)].0, "S");
        let mut board = Board::default();
        board.fill(&b, &[]);
        assert!(board.open.iter().all(|o| b.0[o.index].rank == "F"), "rank F posts only F work");
        // Cycle through many completions: higher-rank work appears once the rank allows it.
        let mut seen_d = false;
        for _ in 0..12 {
            board.open.remove(0);
            board.done += 1;
            board.held = board.eligible();
            board.fill(&b, &[]);
            seen_d |= board.open.iter().any(|o| b.0[o.index].rank == "D");
            assert!(board.open.iter().all(|o| rank_index(&b.0[o.index].rank).unwrap() <= board.rank()));
        }
        assert!(seen_d, "D work posted by rank D");
    }

    #[test]
    fn honours_load_and_follow_rank_titles_and_each_other() {
        let h = HonourBook::load(&honours_path()).expect("honours.json");
        let knight = h.get("knight").expect("knight");
        let lord = h.get("lord").expect("lord");
        let two = vec!["blade_adept".to_string(), "duelist".to_string()];
        let c = rank_index("C").unwrap();
        assert!(!qualifies(knight, c - 1, &two, &[]), "rank D is too low");
        assert!(!qualifies(knight, c, &two[..1], &[]), "one title is too few");
        assert!(qualifies(knight, c, &two, &[]));
        let a = rank_index("A").unwrap();
        assert!(!qualifies(lord, a, &two, &[]), "no knighthood");
        assert!(qualifies(lord, a, &two, &["knight".to_string()]));
        assert!(knight.grants.daily_xp > 0 && lord.grants.daily_xp > knight.grants.daily_xp, "holdings pay");
    }

    #[test]
    fn issuer_work_needs_its_issuer() {
        let b = book();
        let smith: Vec<usize> = (0..b.0.len()).filter(|i| b.0[*i].issuer.as_deref() == Some("Smith")).collect();
        assert!(!smith.is_empty());
        let cycle = |issuers: &[String]| {
            let mut board = Board::default();
            let mut seen = Vec::new();
            for _ in 0..40 {
                board.fill(&b, issuers);
                seen.extend(board.open.iter().map(|o| o.index));
                board.open.clear();
            }
            seen
        };
        assert!(cycle(&[]).iter().all(|i| !smith.contains(i)), "no Smith, no Smith work");
        assert!(cycle(&["Smith".to_string()]).iter().any(|i| smith.contains(i)));
    }

    #[test]
    fn finished_work_waits_for_the_desk_and_rank_waits_for_the_exam() {
        let b = book();
        let e = Entity::PLACEHOLDER;
        let rival = b.0.iter().position(|c| c.id == "rival").expect("rival");
        let mut board = Board {
            open: vec![Open { index: rival, progress: vec![0.0] }],
            ..default()
        };
        board.record(&b, &Deed::new(e, "kill", "Rival", 1.0));
        assert_eq!(board.ready, [rival]);
        let (gold, xp, n) = board.turn_in(&b);
        assert_eq!((gold, xp, n), (b.0[rival].reward.gold, b.0[rival].reward.xp, 1));
        assert!(gold > 0, "contracts pay gold");
        assert!(board.ready.is_empty() && board.turn_in(&b).2 == 0, "paid once");
        board.done = 3;
        assert_eq!((board.eligible(), board.rank()), (1, 0), "eligible for E, still F until the exam");
    }

    #[test]
    fn kill_contracts_need_the_named_victim() {
        let b = book();
        let rival = b.0.iter().position(|c| c.id == "rival").expect("rival contract");
        let mut board = Board {
            open: vec![Open { index: rival, progress: vec![0.0] }],
            ..default()
        };
        let e = Entity::PLACEHOLDER;
        assert!(board.record(&b, &Deed::new(e, "kill", "Opponent", 1.0)).is_empty());
        assert_eq!(board.record(&b, &Deed::new(e, "kill", "Rival", 1.0)), [rival]);
    }
}

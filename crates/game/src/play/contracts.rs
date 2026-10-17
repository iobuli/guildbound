//! Contracts: posted work for the player (`assets/data/contracts.json`, `ContractBook`). Three are
//! open at a time (`Board`, written here only); each objective is a deed pattern × count, so
//! progress comes from the player's `deeds::Deed`s like everything else. A finished contract pays
//! its experience (`stats::GrantXp`), is logged, and the next template in turn takes its place
//! (never one already open). **Guild rank** (F, E, D, C, B, A, S) rises with contracts completed
//! (`RANKS`); a template is only posted once the rank reaches its `rank`. **Issuers**: a template
//! with an `issuer` is posted only while a character of that name with `Standing` (holdings: the
//! Smith's forge, the Farmer's fields) is about; the WORK tab names who posted it. Shown on the stats page's WORK tab, which the courtyard noticeboard
//! (interactable kind "board", F) opens.
use super::deeds::Deed;
use super::hud::log::{HudLog, LogKind};
use super::stats::GrantXp;
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

/// Holdings that let a character post contracts (written by `world` at spawn).
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
    pub done: u32,
}

impl Board {
    pub fn rank(&self) -> usize {
        rank_for(self.done)
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
    app.insert_resource(book).insert_resource(board).add_systems(Update, (progress, read_board));
}

fn progress(
    book: Res<ContractBook>,
    mut board: ResMut<Board>,
    mut deeds: MessageReader<Deed>,
    players: Query<Entity, With<Player>>,
    issuers: Query<(&Name, &Standing)>,
    mut grants: MessageWriter<GrantXp>,
    mut log: MessageWriter<HudLog>,
) {
    let Some(player) = players.iter().next() else {
        deeds.clear();
        return;
    };
    for d in deeds.read().filter(|d| d.who == player) {
        let before = board.rank();
        for i in board.record(&book, d) {
            let def = &book.0[i];
            info!("[contracts] done {}", def.id);
            log.write(HudLog::new(LogKind::Item, format!("Contract complete: {}  +{} XP  [K]", def.name, def.reward.xp)));
            if def.reward.xp > 0 {
                grants.write(GrantXp { who: player, xp: def.reward.xp });
            }
        }
        if board.rank() > before {
            let r = RANKS[board.rank()].0;
            info!("[contracts] rank {r}");
            log.write(HudLog::new(LogKind::Item, format!("Guild rank {r}: new postings on the board  [K]")));
        }
        if board.open.len() < OPEN {
            let present: Vec<String> = issuers.iter().filter(|(_, s)| !s.holdings.is_empty()).map(|(n, _)| n.to_string()).collect();
            board.fill(&book, &present);
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
            board.fill(&b, &[]);
            seen_d |= board.open.iter().any(|o| b.0[o.index].rank == "D");
            assert!(board.open.iter().all(|o| rank_index(&b.0[o.index].rank).unwrap() <= board.rank()));
        }
        assert!(seen_d, "D work posted by rank D");
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

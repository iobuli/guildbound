//! Contracts: posted work for the player (`assets/data/contracts.json`, `ContractBook`). Three are
//! open at a time (`Board`, written here only); each objective is a deed pattern × count, so
//! progress comes from the player's `deeds::Deed`s like everything else. A finished contract pays
//! its experience (`stats::GrantXp`), is logged, and the next template in turn takes its place
//! (never one already open). Shown on the stats page's CONTRACTS tab.
use super::deeds::Deed;
use super::hud::log::{HudLog, LogKind};
use super::stats::GrantXp;
use super::Player;
use bevy::prelude::*;

/// Contracts open at once.
pub const OPEN: usize = 3;

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
    /// Fill up to `OPEN` with the next templates not already open.
    pub fn fill(&mut self, book: &ContractBook) {
        let n = book.0.len();
        let mut tries = 0;
        while self.open.len() < OPEN.min(n) && tries < n {
            let i = self.next % n;
            self.next += 1;
            tries += 1;
            if !self.open.iter().any(|o| o.index == i) {
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
    board.fill(&book);
    app.insert_resource(book).insert_resource(board).add_systems(Update, progress);
}

fn progress(
    book: Res<ContractBook>,
    mut board: ResMut<Board>,
    mut deeds: MessageReader<Deed>,
    players: Query<Entity, With<Player>>,
    mut grants: MessageWriter<GrantXp>,
    mut log: MessageWriter<HudLog>,
) {
    let Some(player) = players.iter().next() else {
        deeds.clear();
        return;
    };
    for d in deeds.read().filter(|d| d.who == player) {
        for i in board.record(&book, d) {
            let def = &book.0[i];
            info!("[contracts] done {}", def.id);
            log.write(HudLog::new(LogKind::Item, format!("Contract complete: {}  +{} XP  [K]", def.name, def.reward.xp)));
            if def.reward.xp > 0 {
                grants.write(GrantXp { who: player, xp: def.reward.xp });
            }
        }
        if board.open.len() < OPEN {
            board.fill(&book);
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
        board.fill(&b);
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
        board.fill(&b);
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
        board.fill(&b);
        assert_eq!(board.open.len(), OPEN);
        assert!(board.open.iter().all(|o| o.index != first) || b.0.len() <= OPEN);
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

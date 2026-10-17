//! Traits: the creator madlib, "I'm a [temper] fighter who walks [walk] and fights [fight]."
//! (`assets/data/traits.json`, `TraitBook`). Every NPC rolls one word per slot when it first
//! appears (seeded by its entity, so a run replays the same) and the words shape it:
//! - walk → `motion::compose::StyleScale` (pose exaggeration, holds, step bounce): a heavy walker
//!   bounces and lingers, a light one barely touches the ground;
//! - fight and temper → its `ai::Difficulty` (aggression, guard, parry, dodge, reaction,
//!   casting), added to whatever the spawner set and clamped.
//! The log introduces each one ("Opponent: a grim fighter who walks proudly and fights coldly.").
//! The player has no traits yet (a creator screen would pick them).
use super::ai::Difficulty;
use super::hud::log::{HudLog, LogKind};
use super::{Npc, PlaySet};
use crate::motion::compose::StyleScale;
use bevy::prelude::*;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct TemperWord {
    pub word: String,
    #[serde(default)]
    pub reaction: f32,
    #[serde(default)]
    pub casting: f32,
    #[serde(default)]
    pub aggression: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct WalkWord {
    pub word: String,
    #[serde(default = "one")]
    pub exaggeration: f32,
    #[serde(default = "one")]
    pub hold: f32,
    #[serde(default = "one")]
    pub bounce: f32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct FightWord {
    pub word: String,
    #[serde(default)]
    pub aggression: f32,
    #[serde(default)]
    pub guard: f32,
    #[serde(default)]
    pub parry: f32,
    #[serde(default)]
    pub dodge: f32,
}

/// The madlib's word lists.
#[derive(Resource, Clone, Debug, Default, serde::Deserialize)]
pub struct TraitBook {
    pub temper: Vec<TemperWord>,
    pub walk: Vec<WalkWord>,
    pub fight: Vec<FightWord>,
}

impl TraitBook {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let book: Self = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if book.temper.is_empty() || book.walk.is_empty() || book.fight.is_empty() {
            return Err(format!("{}: every slot needs a word", path.display()));
        }
        Ok(book)
    }
    /// Pick one word per slot from `seed`.
    pub fn roll(&self, seed: u64) -> Traits {
        let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut next = |n: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % n.max(1) as u64) as usize
        };
        Traits {
            temper: next(self.temper.len()),
            walk: next(self.walk.len()),
            fight: next(self.fight.len()),
        }
    }
    /// "a fiery fighter who walks lightly and fights recklessly"
    pub fn sentence(&self, t: &Traits) -> String {
        let w = |v: Option<&str>| v.unwrap_or("?").to_string();
        format!(
            "a {} fighter who walks {} and fights {}",
            w(self.temper.get(t.temper).map(|x| x.word.as_str())),
            w(self.walk.get(t.walk).map(|x| x.word.as_str())),
            w(self.fight.get(t.fight).map(|x| x.word.as_str()))
        )
    }
    pub fn style(&self, t: &Traits) -> StyleScale {
        self.walk.get(t.walk).map_or_else(StyleScale::default, |w| StyleScale {
            exaggeration: w.exaggeration,
            hold: w.hold,
            bounce: w.bounce,
        })
    }
    /// `base` shifted by the temper and fight words, each value kept in its range.
    pub fn difficulty(&self, t: &Traits, base: Difficulty) -> Difficulty {
        let mut d = base;
        if let Some(w) = self.temper.get(t.temper) {
            d.reaction += w.reaction;
            d.casting += w.casting;
            d.aggression += w.aggression;
        }
        if let Some(w) = self.fight.get(t.fight) {
            d.aggression += w.aggression;
            d.guard += w.guard;
            d.parry += w.parry;
            d.dodge += w.dodge;
        }
        let unit = |x: f32| if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
        Difficulty {
            reaction: if d.reaction.is_finite() { d.reaction.clamp(0.08, 0.8) } else { base.reaction },
            parry: unit(d.parry),
            dodge: unit(d.dodge),
            guard: unit(d.guard),
            aggression: unit(d.aggression),
            casting: unit(d.casting),
        }
    }
}

/// One word index per slot (written by `traits` only).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Traits {
    pub temper: usize,
    pub walk: usize,
    pub fight: usize,
}

pub fn traits_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/traits.json")
}

pub fn plugin(app: &mut App) {
    let book = TraitBook::load(&traits_path()).unwrap_or_else(|e| {
        error!("[traits] {e}");
        TraitBook::default()
    });
    app.insert_resource(book).add_systems(Update, assign.before(PlaySet::Think));
}

/// New NPCs roll their traits; style and behaviour follow.
fn assign(
    book: Res<TraitBook>,
    mut commands: Commands,
    npcs: Query<(Entity, Option<&Name>, Option<&Difficulty>), (With<Npc>, Without<Traits>)>,
    mut log: MessageWriter<HudLog>,
) {
    if book.walk.is_empty() {
        return;
    }
    for (e, name, difficulty) in &npcs {
        let t = book.roll(e.to_bits());
        let d = book.difficulty(&t, difficulty.copied().unwrap_or_default());
        let line = book.sentence(&t);
        let who = name.map_or("Someone", |n| n.as_str());
        info!("[traits] {who}: {line} ({d:?})");
        log.write(HudLog::new(LogKind::System, format!("{who}: {line}.")));
        commands.entity(e).try_insert((t, book.style(&t), d));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> TraitBook {
        TraitBook::load(&traits_path()).expect("traits.json")
    }

    #[test]
    fn rolls_are_seeded_and_cover_the_words() {
        let b = book();
        assert_eq!(b.roll(42), b.roll(42));
        let mut walks = std::collections::HashSet::new();
        for seed in 0..200u64 {
            let t = b.roll(seed * 7919);
            assert!(t.temper < b.temper.len() && t.walk < b.walk.len() && t.fight < b.fight.len());
            walks.insert(t.walk);
        }
        assert_eq!(walks.len(), b.walk.len(), "every walk word comes up");
    }

    #[test]
    fn words_shape_behaviour_within_range() {
        let b = book();
        let base = Difficulty::default();
        let find = |slot: &str, word: &str| match slot {
            "fight" => b.fight.iter().position(|w| w.word == word).unwrap(),
            _ => b.temper.iter().position(|w| w.word == word).unwrap(),
        };
        let reckless = Traits { temper: find("temper", "fiery"), walk: 0, fight: find("fight", "recklessly") };
        let careful = Traits { temper: find("temper", "wary"), walk: 0, fight: find("fight", "carefully") };
        let (r, c) = (b.difficulty(&reckless, base), b.difficulty(&careful, base));
        assert!(r.aggression > c.aggression && c.guard > r.guard);
        for d in [r, c] {
            for v in [d.parry, d.dodge, d.guard, d.aggression, d.casting] {
                assert!((0.0..=1.0).contains(&v));
            }
        }
        assert_eq!(b.sentence(&reckless), "a fiery fighter who walks heavily and fights recklessly");
    }
}

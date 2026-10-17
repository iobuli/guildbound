//! Occupations: civilians at work (`assets/data/occupations.json`, `OccupationBook`). A `Civilian`
//! is a character no brain fights with (`ai` skips it as a foe) and that fights no one; its
//! `Occupation` runs a routine of interactable kinds in turn: walk to the nearest free one of the
//! kind, press interact (the same `Intent.interact_pressed` as the player: `interact` walks it into
//! place and plays the clip), stay `stay` s on a looping use (anvil, chair, torch) then press again
//! to stop, and move on. `work` is the only writer of a civilian's `Intent`.
//! The courtyard has a Smith (anvil, well, chair, torch) and a Farmer (seed, water, harvest, well,
//! the noticeboard).
use super::interact::{Using, stand_and_face};
use super::{Fighter, HALF_HEIGHT, Intent, Interactable, PlaySet};
use bevy::prelude::*;

/// Walk until this close to the stand point (m); interact again after this long (s) if nothing
/// started; give a step up after this long (s).
const ARRIVE: f32 = 0.45;
const PRESS_EVERY: f32 = 1.0;
const GIVE_UP: f32 = 30.0;
/// Walking stick share.
const STROLL: f32 = 0.55;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Step {
    pub kind: String,
    #[serde(default)]
    pub stay: f32,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct OccupationDef {
    pub id: String,
    pub name: String,
    pub routine: Vec<Step>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct OccupationsFile {
    occupations: Vec<OccupationDef>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct OccupationBook(pub Vec<OccupationDef>);

impl OccupationBook {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: OccupationsFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(o) = file.occupations.iter().find(|o| o.routine.is_empty()) {
            return Err(format!("occupation '{}' has no routine", o.id));
        }
        Ok(Self(file.occupations))
    }
    pub fn index(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|o| o.id == id)
    }
}

pub fn occupations_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("data/occupations.json")
}

/// A non-combatant: `ai` never picks it as a foe.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Civilian;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Going,
    Working,
}

/// Where a civilian is in its routine (written by `occupations` only).
#[derive(Component, Clone, Debug)]
pub struct Occupation {
    /// Row id, resolved to `def` on first use.
    pub id: &'static str,
    def: Option<usize>,
    pub step: usize,
    phase: Phase,
    /// Seconds left on a looping use, until the next press, and on this step.
    stay: f32,
    press_in: f32,
    elapsed: f32,
}

impl Occupation {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            def: None,
            step: 0,
            phase: Phase::Going,
            stay: 0.0,
            press_in: 0.0,
            elapsed: 0.0,
        }
    }
    fn advance(&mut self, len: usize) {
        self.step = (self.step + 1) % len.max(1);
        self.phase = Phase::Going;
        self.elapsed = 0.0;
        self.press_in = 0.0;
    }
}

pub fn plugin(app: &mut App) {
    let book = OccupationBook::load(&occupations_path()).unwrap_or_else(|e| {
        error!("[occupations] {e}");
        OccupationBook::default()
    });
    app.insert_resource(book).add_systems(Update, work.in_set(PlaySet::Think));
}

#[allow(clippy::type_complexity)]
fn work(
    time: Res<Time>,
    book: Res<OccupationBook>,
    mut workers: Query<(Entity, &Transform, &Fighter, &mut Intent, &mut Occupation, Option<&Using>), With<Civilian>>,
    users: Query<&Using>,
    things: Query<(Entity, &Transform, &Interactable)>,
) {
    let dt = time.delta_secs();
    let busy: Vec<Entity> = users.iter().map(|u| u.target).collect();
    for (me, tf, fighter, mut intent, mut job, using) in &mut workers {
        let look = intent.look;
        *intent = Intent { look, ..default() };
        if job.def.is_none() {
            job.def = book.index(job.id);
        }
        let Some(def) = job.def.and_then(|i| book.0.get(i)) else { continue };
        if fighter.dead {
            continue;
        }
        let len = def.routine.len();
        let step = &def.routine[job.step % len];
        job.elapsed += dt;
        if let Some(u) = using {
            if job.phase == Phase::Going {
                job.phase = Phase::Working;
                job.stay = step.stay;
            }
            let looping = things.get(u.target).is_ok_and(|(_, _, it)| it.looping);
            if looping {
                job.stay -= dt;
                if job.stay <= 0.0 {
                    intent.interact_pressed = true;
                }
            }
            continue;
        }
        if job.phase == Phase::Working || job.elapsed > GIVE_UP {
            job.advance(len);
            continue;
        }
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        let target = things
            .iter()
            .filter(|(e, _, it)| it.kind == step.kind && !busy.iter().any(|b| b == e && *b != me))
            .map(|(e, t, it)| {
                let (stand, _) = stand_and_face(t, it, feet);
                (e, t.translation, stand)
            })
            .min_by(|a, b| a.2.distance(feet).total_cmp(&b.2.distance(feet)));
        let Some((_, center, stand)) = target else {
            job.advance(len);
            continue;
        };
        intent.look = center;
        let to = (stand - feet).with_y(0.0);
        if to.length() > ARRIVE {
            intent.move_dir = to.normalize() * STROLL;
            intent.walk = true;
        } else {
            job.press_in -= dt;
            if job.press_in <= 0.0 {
                job.press_in = PRESS_EVERY;
                intent.interact_pressed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupations_load_and_name_real_kinds() {
        let b = OccupationBook::load(&occupations_path()).expect("occupations.json");
        assert!(b.index("smith").is_some() && b.index("farmer").is_some());
        let kinds = ["chest", "table", "chair", "anvil", "torch", "lever", "board", "well", "crop", "watering", "seed"];
        for o in &b.0 {
            for s in &o.routine {
                assert!(kinds.contains(&s.kind.as_str()), "{}: unknown kind {}", o.id, s.kind);
            }
        }
    }

    #[test]
    fn routines_wrap() {
        let mut o = Occupation::new("smith");
        for _ in 0..5 {
            o.advance(3);
        }
        assert_eq!(o.step, 2);
        assert_eq!(o.phase, Phase::Going);
    }
}

//! Occupations: civilians at work (`assets/data/occupations.json`, `OccupationBook`). A `Civilian`
//! is a character no brain fights with (`ai` skips it as a foe) and that fights no one; its
//! `Occupation` runs a routine of interactable kinds in turn: walk to the nearest free one of the
//! kind, press interact (the same `Intent.interact_pressed` as the player: `interact` walks it into
//! place and plays the clip), stay `stay` s on a looping use (anvil, chair, torch) then press again
//! to stop, and move on. `work` is the only writer of a civilian's `Intent`.
//! After dark (`sky::Clock`) a civilian follows its `night` routine instead, if it has one; by
//! day in heavy rain (`weather::Weather.rain` above `SHELTER_RAIN`) its `rain` routine, if any
//! (sheltering by a fire, staying in).
//! While the player talks to a civilian (`talk::Talking`) it stops and looks at them.
//! Walking: through the hall's doorways when one end is inside (`hall::route`); no progress for
//! `STALL` s sidesteps for `DODGE` s (alternating sides); a step is skipped after `GIVE_UP` s
//! (`POST_GIVE_UP` for a post) without getting closer.
//! At start-up `check_posts` stands a body-sized capsule on every post and its run-up and warns
//! about any that sit in a wall or prop, or up on the guildhouse podium outside its walls (only
//! reachable through the hall), so a moved building shows up in the log, not as NPCs giving up.
//! The courtyard has a Smith (anvil, well, chair, torch) and a Farmer (seed, water, harvest, well,
//! the noticeboard).
use super::interact::{Using, stand_and_face};
use super::{Fighter, HALF_HEIGHT, Intent, Interactable, PlaySet};
use bevy::prelude::*;

/// Walk until this close to the stand point (m), or until `NEAR` when something solid (the chair
/// itself, a bed frame) stops the walk short of it (stalled `NEAR_STALL` s): `interact` walks the
/// last bit into place. Interact again after `PRESS_EVERY` s if nothing started; give a step up
/// after `GIVE_UP` s without getting `PROGRESS` m closer (far targets across the courtyard take
/// as long as they take).
const ARRIVE: f32 = 0.45;
const NEAR: f32 = 0.8;
const NEAR_STALL: f32 = 0.4;
const PRESS_EVERY: f32 = 1.0;
const GIVE_UP: f32 = 30.0;
const PROGRESS: f32 = 0.5;
/// A neighbour this close (m) while using something gets looked at (company at a table).
const SITTER_NEAR: f32 = 3.5;
/// Stuck: under `STALL_MOVE` m of progress for `STALL` s sidesteps for `DODGE` s; a post not
/// reached in `POST_GIVE_UP` s is skipped.
const STALL_MOVE: f32 = 0.25;
const STALL: f32 = 1.2;
const DODGE: f32 = 0.9;
const POST_GIVE_UP: f32 = 45.0;
/// Walking stick share.
const STROLL: f32 = 0.55;
/// Run-up behind a post (m), close enough to it (m), seconds between drill swings.
const POST_RUNUP: f32 = 0.7;
const POST_ARRIVE: f32 = 0.18;
const DRILL_EVERY: f32 = 1.4;

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Step {
    /// An interactable kind, or "post": stand at `at` facing its yaw.
    pub kind: String,
    #[serde(default)]
    pub stay: f32,
    /// For "post": x, z and the yaw faced (deg).
    #[serde(default)]
    pub at: Option<[f32; 3]>,
    /// At a post: swing at whatever is ahead now and then (a captain's drill).
    #[serde(default)]
    pub drill: bool,
}

/// Where a post step walks in from (behind the post along its facing) so the body arrives
/// facing the right way, and the post itself.
pub fn post_points(at: [f32; 3]) -> (Vec3, Vec3) {
    let yaw = at[2].to_radians();
    let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let post = Vec3::new(at[0], 0.0, at[1]);
    (post - forward * POST_RUNUP, post)
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct OccupationDef {
    pub id: String,
    pub name: String,
    pub routine: Vec<Step>,
    /// After dark (`sky::Clock::is_night`), if any.
    #[serde(default)]
    pub night: Vec<Step>,
    /// By day in heavy rain, if any.
    #[serde(default)]
    pub rain: Vec<Step>,
}

/// Rain amount (0..1) above which civilians with a `rain` routine take shelter.
pub const SHELTER_RAIN: f32 = 0.6;

/// Which of a civilian's routines runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Shift {
    #[default]
    Day,
    Night,
    Rain,
}

/// The routine for the hour and the weather: night first, then heavy rain, else the day's (a
/// routine the civilian lacks falls back to the day's).
pub fn shift(def: &OccupationDef, night: bool, rain: f32) -> Shift {
    if night && !def.night.is_empty() {
        Shift::Night
    } else if !night && rain > SHELTER_RAIN && !def.rain.is_empty() {
        Shift::Rain
    } else {
        Shift::Day
    }
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
    /// Post steps: past the run-up point yet.
    run_up: bool,
    /// Which routine the step indexes.
    shift: Shift,
    /// Stuck detection while walking: where the walker last made progress, for how long it has
    /// not, and a sidestep in progress (seconds left, side).
    anchor: Vec3,
    stalled: f32,
    dodge: f32,
    side: f32,
    /// Closest the walker has come to this step's goal (m).
    best: f32,
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
            best: f32::INFINITY,
            run_up: false,
            shift: Shift::Day,
            anchor: Vec3::ZERO,
            stalled: 0.0,
            dodge: 0.0,
            side: 1.0,
        }
    }

    /// The stick toward `to` (horizontal) from `feet`, scaled by `speed`, stepping around whatever
    /// blocks the way: no progress for `STALL` s starts a sidestep of `DODGE` s, alternating sides.
    fn walk(&mut self, feet: Vec3, to: Vec3, speed: f32, dt: f32, balancing: bool) -> Vec3 {
        // A beam is walked slowly on purpose: that is not being stuck.
        if balancing || feet.distance(self.anchor) > STALL_MOVE {
            self.anchor = feet;
            self.stalled = 0.0;
        } else {
            self.stalled += dt;
        }
        if self.stalled > STALL {
            self.stalled = 0.0;
            self.dodge = DODGE;
            self.side = -self.side;
        }
        let dir = to.with_y(0.0).normalize_or_zero();
        if self.dodge > 0.0 {
            self.dodge -= dt;
            let around = Vec3::new(-dir.z, 0.0, dir.x) * self.side;
            return (dir * 0.4 + around).normalize_or_zero() * speed;
        }
        dir * speed
    }
    fn advance(&mut self, len: usize) {
        self.step = (self.step + 1) % len.max(1);
        self.run_up = false;
        self.phase = Phase::Going;
        self.elapsed = 0.0;
        self.press_in = 0.0;
        self.best = f32::INFINITY;
    }
    /// The give-up clock counts time without progress: getting `PROGRESS` m closer restarts it.
    fn progress(&mut self, dist: f32) {
        if dist < self.best - PROGRESS {
            self.best = dist;
            self.elapsed = 0.0;
        }
    }
}

pub fn plugin(app: &mut App) {
    let book = OccupationBook::load(&occupations_path()).unwrap_or_else(|e| {
        error!("[occupations] {e}");
        OccupationBook::default()
    });
    app.insert_resource(book).add_systems(Update, (work.in_set(PlaySet::Think), check_posts));
}

/// Seconds after start-up before `check_posts` looks (the world's colliders are in by then).
const CHECK_AFTER: f32 = 1.5;

/// Warn about posts a civilian can't stand on (see the module docs). Runs once.
fn check_posts(time: Res<Time>, book: Res<OccupationBook>, spatial: avian3d::prelude::SpatialQuery, names: Query<&Name>, mut done: Local<bool>) {
    use avian3d::prelude::{Collider, SpatialQueryFilter};
    if *done || time.elapsed_secs() < CHECK_AFTER {
        return;
    }
    *done = true;
    let world = SpatialQueryFilter::from_mask(super::Phys::World);
    let body = Collider::capsule(super::CAPSULE_RADIUS * 0.9, super::CAPSULE_LENGTH);
    let (mut posts, mut bad) = (0, 0);
    for def in &book.0 {
        for (when, steps) in [("day", &def.routine), ("night", &def.night), ("rain", &def.rain)] {
            for (k, step) in steps.iter().enumerate() {
                let (Some(at), true) = (step.at, step.kind == "post") else { continue };
                posts += 1;
                let (run_up, post) = post_points(at);
                for (what, p) in [("run-up", run_up), ("post", post)] {
                    let top = p.with_y(12.0);
                    let floor = spatial.cast_ray(top, Dir3::NEG_Y, 20.0, true, &world).map_or(0.0, |h| top.y - h.distance);
                    let centre = p.with_y(floor + HALF_HEIGHT + 0.05);
                    let hits: Vec<String> = spatial
                        .shape_intersections(&body, centre, Quat::IDENTITY, &world)
                        .into_iter()
                        .map(|e| names.get(e).map_or_else(|_| format!("{e}"), |n| n.as_str().to_string()))
                        .collect();
                    let stranded = floor > 0.5 && !super::hall::inside(p);
                    if !hits.is_empty() || stranded {
                        bad += 1;
                        let why = if hits.is_empty() { "up on the podium outside the hall".to_string() } else { format!("inside {}", hits.join(", ")) };
                        warn!("[occ] {} {when} step {k}: {what} at ({:.1}, {:.1}) is {why}", def.id, p.x, p.z);
                    }
                }
            }
        }
    }
    info!("[occ] checked {posts} posts: {bad} spots blocked");
}

#[allow(clippy::type_complexity)]
fn work(
    time: Res<Time>,
    book: Res<OccupationBook>,
    clock: Option<Res<super::sky::Clock>>,
    weather: Option<Res<super::weather::Weather>>,
    talking: Option<Res<super::talk::Talking>>,
    players: Query<Entity, With<super::Player>>,
    mut workers: Query<
        (Entity, &Transform, &Fighter, &mut Intent, &mut Occupation, Option<&Using>, Has<super::beam::Balancing>),
        With<Civilian>,
    >,
    users: Query<&Using>,
    sitters: Query<(Entity, &Transform), (With<Civilian>, With<Using>)>,
    things: Query<(Entity, &Transform, &Interactable)>,
) {
    let dt = time.delta_secs();
    let busy: Vec<Entity> = users.iter().map(|u| u.target).collect();
    for (me, tf, fighter, mut intent, mut job, using, balancing) in &mut workers {
        let look = intent.look;
        *intent = Intent { look, ..default() };
        if job.def.is_none() {
            job.def = book.index(job.id);
        }
        let Some(def) = job.def.and_then(|i| book.0.get(i)) else { continue };
        if fighter.dead {
            continue;
        }
        // Spoken to: stop and look at the speaker.
        if talking.as_deref().is_some_and(|t| t.npc == Some(me)) {
            intent.target = players.iter().next();
            continue;
        }
        let night = clock.as_deref().is_some_and(|c| c.is_night());
        let now = shift(def, night, weather.as_deref().map_or(0.0, |w| w.rain));
        if now != job.shift {
            // Dusk, dawn, a downpour or its end: start the other routine (a looping use winds
            // down first).
            job.shift = now;
            job.step = 0;
            job.phase = Phase::Going;
            job.stay = 0.0;
            job.elapsed = 0.0;
        }
        let routine = match now {
            Shift::Night => &def.night,
            Shift::Rain => &def.rain,
            Shift::Day => &def.routine,
        };
        let len = routine.len();
        let step = &routine[job.step % len];
        job.elapsed += dt;
        if let Some(u) = using {
            // Sitting with company: look at the nearest other sitter (talking at the table).
            intent.target = sitters
                .iter()
                .filter(|(e, t)| *e != me && t.translation.distance(tf.translation) < SITTER_NEAR)
                .min_by(|a, b| a.1.translation.distance(tf.translation).total_cmp(&b.1.translation.distance(tf.translation)))
                .map(|(e, _)| e);
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
        let feet = tf.translation - Vec3::Y * HALF_HEIGHT;
        // A post: walk in from behind it, then stand (drilling, if asked) for `stay` s.
        if let (Some(at), true) = (step.at, step.kind == "post") {
            let (run_up, post) = post_points(at);
            let yaw = at[2].to_radians();
            let ahead = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            if job.phase == Phase::Working {
                job.stay -= dt;
                intent.look = feet + ahead * 3.0 + Vec3::Y * 1.4;
                if step.drill {
                    job.press_in -= dt;
                    if job.press_in <= 0.0 {
                        job.press_in = DRILL_EVERY;
                        intent.primary_pressed = true;
                    }
                }
                if job.stay <= 0.0 {
                    job.advance(len);
                }
                continue;
            }
            let goal = if job.run_up { post } else { run_up };
            let to = (goal - feet).with_y(0.0);
            let way = (super::hall::route(feet, goal) - feet).with_y(0.0);
            job.progress(to.length());
            if !job.run_up && to.length() < ARRIVE {
                job.run_up = true;
            } else if job.run_up && to.length() < POST_ARRIVE {
                job.phase = Phase::Working;
                job.stay = step.stay;
                job.press_in = DRILL_EVERY;
            } else if job.elapsed > POST_GIVE_UP {
                info!("[occ] {} gave up on post step {} at ({:.1}, {:.1}), heading for ({:.1}, {:.1})", def.id, job.step, feet.x, feet.z, goal.x, goal.z);
                job.advance(len);
            } else {
                let speed = if job.run_up { 0.3 } else { STROLL };
                intent.move_dir = job.walk(feet, way, speed, dt, balancing);
                intent.walk = true;
            }
            continue;
        }
        if job.phase == Phase::Working || job.elapsed > GIVE_UP {
            if job.phase != Phase::Working {
                info!("[occ] {} gave up on {} step {} at ({:.1}, {:.1})", def.id, step.kind, job.step, feet.x, feet.z);
            }
            job.advance(len);
            continue;
        }
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
        job.progress(to.length());
        let blocked = to.length() < NEAR && job.stalled > NEAR_STALL;
        if to.length() > ARRIVE && !blocked {
            let way = (super::hall::route(feet, stand) - feet).with_y(0.0);
            intent.move_dir = job.walk(feet, way, STROLL, dt, balancing);
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
    fn heavy_rain_by_day_sends_civilians_to_shelter() {
        let book = OccupationBook::load(&occupations_path()).expect("occupations.json");
        let smith = book.0.iter().find(|d| d.id == "smith").unwrap();
        assert!(!smith.rain.is_empty(), "the smith shelters in the rain");
        assert_eq!(shift(smith, false, 0.0), Shift::Day);
        assert_eq!(shift(smith, false, 0.9), Shift::Rain);
        assert_eq!(shift(smith, false, SHELTER_RAIN), Shift::Day, "a drizzle doesn't count");
        assert_eq!(shift(smith, true, 0.9), Shift::Night, "night wins");
        // Without a rain routine, rain changes nothing.
        let receptionist = book.0.iter().find(|d| d.id == "receptionist").unwrap();
        assert!(receptionist.rain.is_empty());
        assert_eq!(shift(receptionist, false, 1.0), Shift::Day);
    }

    #[test]
    fn occupations_load_and_name_real_kinds() {
        let b = OccupationBook::load(&occupations_path()).expect("occupations.json");
        assert!(b.index("smith").is_some() && b.index("farmer").is_some());
        let kinds = ["chest", "table", "chair", "anvil", "torch", "lever", "board", "well", "crop", "watering", "seed", "bed", "post"];
        for o in &b.0 {
            for s in &o.routine {
                assert!(kinds.contains(&s.kind.as_str()), "{}: unknown kind {}", o.id, s.kind);
                assert_eq!(s.kind == "post", s.at.is_some(), "{}: a post needs `at`", o.id);
            }
        }
    }

    #[test]
    fn posts_are_walked_into_facing_their_yaw() {
        let (run_up, post) = post_points([10.0, 5.0, 90.0]);
        // Facing +X: the run-up sits behind, at −X, so the last steps face +X.
        assert!((post - Vec3::new(10.0, 0.0, 5.0)).length() < 1e-5);
        assert!(run_up.x < post.x && (run_up.z - post.z).abs() < 1e-4);
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

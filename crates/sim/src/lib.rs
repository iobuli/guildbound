//! Deterministic combat core, motor slice. Ported from tag `unity-final`, `Assets/Game/Combat`
//! (CombatWorld.Move / Separate / TurnToward, BoxArena.Constrain, Tuning).
//! Positions are on the ground plane as `V2 { x, z }`. The motor is the only mover (MASTER L1).
//! Nothing here reads a clock, a random source or a hash-map order: same inputs, same state.

mod math;
pub use math::V2;

/// Fixed simulation step in seconds.
pub const STEP: f32 = 1.0 / 120.0;

/// Starting hypotheses from GDD v5.1 section 4 until measured (same values as the Unity `Tuning`).
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    pub body_radius: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub arena_half_size: f32,
    /// Degrees per second a free fighter turns toward its movement.
    pub turn_rate: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self { body_radius: 0.30, walk_speed: 2.1, run_speed: 4.0, arena_half_size: 6.0, turn_rate: 720.0 }
    }
}

/// One tick of player or AI intent. `move_dir` is world XZ with magnitude <= 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    pub move_dir: V2,
    pub run: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fighter {
    pub position: V2,
    pub velocity: V2,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Index of the fighter this one faces while free (strafe), if any.
    pub lock_target: Option<usize>,
    move_from: V2,
}

impl Fighter {
    pub fn new(position: V2, facing: V2, hp: f32) -> Self {
        Self { position, velocity: V2::ZERO, facing: facing.normalized(), hp, max_hp: hp, lock_target: None, move_from: position }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub tuning: Tuning,
    pub fighters: Vec<Fighter>,
    pub tick: u64,
}

impl World {
    /// The sparring floor: two fighters 3 m apart, facing each other.
    pub fn sparring(tuning: Tuning) -> Self {
        let fighters = vec![
            Fighter::new(V2::new(0.0, -1.5), V2::new(0.0, 1.0), 100.0),
            Fighter::new(V2::new(0.0, 1.5), V2::new(0.0, -1.0), 100.0),
        ];
        Self { tuning, fighters, tick: 0 }
    }

    /// Advance one fixed step. Missing intents count as idle.
    pub fn step(&mut self, intents: &[Intent]) {
        self.tick += 1;
        for f in &mut self.fighters {
            f.move_from = f.position;
        }
        for i in 0..self.fighters.len() {
            let intent = intents.get(i).copied().unwrap_or_default();
            self.move_fighter(i, intent);
        }
        self.separate();
    }

    fn move_fighter(&mut self, i: usize, intent: Intent) {
        let t = &self.tuning;
        let m = if intent.move_dir.length_squared() > 1.0 { intent.move_dir.normalized() } else { intent.move_dir };
        let speed = if intent.run { t.run_speed } else { t.walk_speed };
        let v = m * speed;
        let lock = self.fighters[i].lock_target.filter(|&j| j != i && j < self.fighters.len()).map(|j| self.fighters[j].position);
        let max_turn = t.turn_rate * STEP;
        let f = &mut self.fighters[i];
        f.velocity = v;
        f.position = f.position + v * STEP;
        if let Some(target) = lock {
            let to = target - f.position;
            if to.length_squared() > 1e-6 {
                f.facing = to.normalized();
            }
        } else if v.length_squared() > 1e-4 {
            f.facing = turn_toward(f.facing, v.normalized(), max_turn);
        }
    }

    fn separate(&mut self) {
        let min = 2.0 * self.tuning.body_radius;
        let n = self.fighters.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.fighters[j].position - self.fighters[i].position;
                let len = d.length();
                if len >= min {
                    continue;
                }
                let dir = if len > 1e-5 { d / len } else { V2::new(1.0, 0.0) };
                let push = dir * ((min - len) * 0.5);
                self.fighters[i].position = self.fighters[i].position - push;
                self.fighters[j].position = self.fighters[j].position + push;
            }
        }
        // one world constraint per tick, from the pre-motion position, so separation never pushes through a wall
        let half = self.tuning.arena_half_size - self.tuning.body_radius;
        for f in &mut self.fighters {
            f.position = V2::new(f.position.x.clamp(-half, half), f.position.z.clamp(-half, half));
        }
    }

    /// FNV-1a over every state field in index order. Equal hashes for equal input scripts prove determinism.
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bits: u64| {
            for b in bits.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        eat(self.tick);
        for f in &self.fighters {
            for v in [f.position.x, f.position.z, f.velocity.x, f.velocity.z, f.facing.x, f.facing.z, f.hp, f.max_hp] {
                eat(v.to_bits() as u64);
            }
            eat(f.lock_target.map_or(u64::MAX, |j| j as u64));
        }
        h
    }
}

/// Planar turn toward `to`, at most `max_degrees`. An exact reversal still turns, never flips.
pub fn turn_toward(from: V2, to: V2, max_degrees: f32) -> V2 {
    let angle = from.signed_angle_to(to);
    let max = max_degrees.to_radians();
    if angle.abs() <= max { to } else { from.rotated(max.copysign(angle)) }
}

#[cfg(test)]
mod tests;

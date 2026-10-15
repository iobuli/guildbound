//! Mob awareness: a vision cone and a notice gauge. Reimagined from the notice states of Snaiel's
//! Godot4ThirdPersonCombatPrototype (MIT), improved: running is heard outside the cone, a lost target is
//! searched for at its last seen spot before the mob gives up, and the gauge is one number (0..2) instead of
//! five state nodes. Unaware 0..1, Suspicious 1..2 (investigates), Alert at 2 (latched until lost).
//! Only fighters without a `notice` (players) are watched.

use crate::{STEP, V2, World};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoticeState {
    #[default]
    Unaware,
    Suspicious,
    Alert,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Notice {
    pub level: f32,
    pub state: NoticeState,
    pub last_seen: V2,
    /// Who it is aware of.
    pub target: Option<usize>,
    /// Seconds since an alert mob last saw its target.
    pub lost: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoticeTuning {
    /// Cone half-angles (deg) and ranges (m): inside the inner cone is an instant alert.
    pub outer_angle: f32,
    pub inner_angle: f32,
    pub outer_range: f32,
    pub inner_range: f32,
    /// Gauge gain per second at the cone edge and at its heart.
    pub min_gain: f32,
    pub max_gain: f32,
    pub run_mult: f32,
    /// A running target is heard this close, cone or not.
    pub hearing: f32,
    pub decay: f32,
    /// Seconds an alert mob searches before dropping to suspicious.
    pub lose_time: f32,
}

impl Default for NoticeTuning {
    fn default() -> Self {
        Self {
            outer_angle: 70.0,
            inner_angle: 40.0,
            outer_range: 10.0,
            inner_range: 3.5,
            min_gain: 0.3,
            max_gain: 2.5,
            run_mult: 1.6,
            hearing: 3.0,
            decay: 0.25,
            lose_time: 8.0,
        }
    }
}

impl World {
    /// Gauge gain per second for mob `i` watching fighter `j`; `None` when `j` is neither seen nor heard,
    /// infinity when `j` is inside the inner cone.
    fn notice_gain(&self, i: usize, j: usize) -> Option<f32> {
        let n = self.notice;
        let (me, t) = (&self.fighters[i], &self.fighters[j]);
        let to = t.position - me.position;
        let dist = to.length();
        let angle = if dist > 1e-4 {
            me.facing
                .signed_angle_to(to * (1.0 / dist))
                .abs()
                .to_degrees()
        } else {
            0.0
        };
        let running = t.velocity.length() > (self.tuning.walk_speed + self.tuning.run_speed) * 0.5;
        if angle <= n.inner_angle && dist <= n.inner_range {
            return Some(f32::INFINITY);
        }
        let seen = angle <= n.outer_angle && dist <= n.outer_range;
        if !seen && !(running && dist <= n.hearing) {
            return None;
        }
        let near = (1.0 - (dist - n.inner_range) / (n.outer_range - n.inner_range).max(1e-3))
            .clamp(0.0, 1.0);
        let ahead = (1.0 - angle / n.outer_angle).clamp(0.0, 1.0);
        let close = (near + ahead) * 0.5;
        let gain = n.min_gain + (n.max_gain - n.min_gain) * close * close;
        Some(if running { gain * n.run_mult } else { gain })
    }

    pub(crate) fn update_notice(&mut self, i: usize) {
        let Some(mut n) = self.fighters[i].notice else {
            return;
        };
        if self.fighters[i].hp <= 0.0 {
            return;
        }
        let tn = self.notice;
        let best = (0..self.fighters.len())
            .filter(|&j| j != i && self.fighters[j].notice.is_none() && self.fighters[j].hp > 0.0)
            .filter_map(|j| self.notice_gain(i, j).map(|g| (j, g)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((j, g)) => {
                n.target = Some(j);
                n.last_seen = self.fighters[j].position;
                n.lost = 0.0;
                n.level = (n.level + g * STEP).min(2.0);
            }
            None if n.state == NoticeState::Alert => {
                n.lost += STEP;
                if n.lost >= tn.lose_time {
                    n.level = 1.5;
                }
            }
            None => n.level = (n.level - tn.decay * STEP).max(0.0),
        }
        n.state = match n.state {
            NoticeState::Alert if n.lost < tn.lose_time => NoticeState::Alert,
            _ if n.level >= 2.0 => NoticeState::Alert,
            _ if n.level >= 1.0 => NoticeState::Suspicious,
            _ => NoticeState::Unaware,
        };
        if n.state == NoticeState::Unaware && n.level <= 0.0 {
            n.target = None;
        }
        self.fighters[i].notice = Some(n);
    }

    /// Being struck always alerts a mob to the striker.
    pub(crate) fn alarm(&mut self, d: usize, by: usize) {
        let pos = self.fighters[by].position;
        if let Some(n) = &mut self.fighters[d].notice {
            n.level = 2.0;
            n.state = NoticeState::Alert;
            n.target = Some(by);
            n.last_seen = pos;
            n.lost = 0.0;
        }
    }
}

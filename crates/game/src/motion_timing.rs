//! Warp authored strike phases onto combat phases without changing rules.
//! Contact landmarks are measured offline from limb extension relative to hips.

#[derive(Clone, Copy, Debug)]
pub struct ContactTiming {
    pub start: f32,
    pub peak: f32,
    pub end: f32,
}

impl ContactTiming {
    pub fn valid(self) -> bool {
        0.0 < self.start && self.start < self.peak && self.peak < self.end && self.end < 1.0
    }

    /// Continuous, monotonic time warp: anticipation, extension, withdrawal,
    /// settle. Both held preparation and released strikes use this same map.
    pub fn sample(self, elapsed: f32, startup: f32, active: f32, recovery: f32, duration: f32) -> f32 {
        let rule = [0.0, startup, startup + active * 0.5, startup + active, startup + active + recovery];
        let clip = [0.0, self.start, self.peak, self.end, 1.0];
        let time = elapsed.clamp(0.0, rule[4]);
        let i = (0..4).find(|&i| time <= rule[i + 1]).unwrap_or(3);
        let t = ((time - rule[i]) / (rule[i + 1] - rule[i]).max(1e-5)).clamp(0.0, 1.0);
        (clip[i] + (clip[i + 1] - clip[i]) * t) * duration
    }

    /// A projectile has one launch instant, not a melee contact interval.
    pub fn release_sample(self, elapsed: f32, prepare: f32, recovery: f32, duration: f32) -> f32 {
        let phase = if elapsed <= prepare {
            self.peak * (elapsed / prepare.max(1e-5)).clamp(0.0, 1.0)
        } else {
            self.peak + (1.0 - self.peak) * ((elapsed - prepare) / recovery.max(1e-5)).clamp(0.0, 1.0)
        };
        phase * duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_markers_match_rules_without_time_reversal_or_release_jump() {
        let timing = ContactTiming { start: 0.3, peak: 0.45, end: 0.6 };
        assert!(timing.valid());
        let sample = |time| timing.sample(time, 0.2, 0.1, 0.4, 2.0);
        for (time, expected) in [(0.0, 0.0), (0.2, 0.6), (0.25, 0.9), (0.3, 1.2), (0.7, 2.0)] {
            assert!((sample(time) - expected).abs() < 1e-5);
        }
        let mut previous = 0.0;
        for i in 0..=1000 {
            let next = sample(i as f32 / 1000.0);
            assert!(next >= previous);
            previous = next;
        }
        let held_clock = 0.2 * 0.75 * 0.8;
        assert!((sample(held_clock + 1e-6) - sample(held_clock)).abs() < 1e-4);
        assert!(!ContactTiming { start: f32::NAN, ..timing }.valid());
        assert!(!ContactTiming { peak: 0.2, ..timing }.valid());
        assert!((timing.release_sample(0.2, 0.2, 0.3, 2.0) - 0.9).abs() < 1e-5);
        assert_eq!(timing.release_sample(0.5, 0.2, 0.3, 2.0), 2.0);
    }
}

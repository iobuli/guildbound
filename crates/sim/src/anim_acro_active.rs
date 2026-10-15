//! acro_active weight in [0,1] for fading the COM pull [team].
//! Smoothstep ease-in over the first `ease` ticks of an acro clip and ease-out over the last `ease`.
//! Integer ticks, pure, no Bevy. Returns 0 outside [start, start+len).

pub const ACRO_EASE_TICKS: u64 = 12; // 0.1 s at 120 Hz

pub fn acro_active(tick: u64, start: u64, len: u64, ease: u64) -> f32 {
    if len == 0 || tick < start || tick >= start + len {
        return 0.0;
    }
    let i = tick - start;
    let e = ease.max(1).min(len / 2).max(1);
    let a = (i + 1).min(len - i).min(e) as f32 / e as f32;
    a * a * (3.0 - 2.0 * a)
}

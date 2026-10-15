//! Ground height (m) at any point: a pure function of position, shared by the sim and the view.
//! Level inside the courtyard and the market clearing, rolling hills over the open world, and ridges rising
//! past the play area's edge so the world has a horizon. Smooth value noise from an integer hash: no RNG,
//! no state, the same answer on every machine.

/// Radius of the level courtyard and its yard.
pub const FLAT_RADIUS: f32 = 20.0;
/// Hills fade in over this distance past the flat radius.
const BLEND: f32 = 14.0;
/// Where the ridges start rising (the play area's half size) and how far they take to reach full height.
const RIDGE_START: f32 = 58.0;
const RIDGE_SPAN: f32 = 90.0;
const RIDGE_HEIGHT: f32 = 30.0;
const MARKET: (f32, f32, f32) = (30.0, 30.0, 10.0);

fn hash(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (z as u32).wrapping_mul(0xD816_3841) ^ 0x9E37_79B9;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise in -1..1, smooth between integer lattice points.
fn noise(x: f32, z: f32) -> f32 {
    let (ix, iz) = (x.floor(), z.floor());
    let (fx, fz) = (smooth(x - ix), smooth(z - iz));
    let (ix, iz) = (ix as i32, iz as i32);
    let a = hash(ix, iz) + (hash(ix + 1, iz) - hash(ix, iz)) * fx;
    let b = hash(ix, iz + 1) + (hash(ix + 1, iz + 1) - hash(ix, iz + 1)) * fx;
    (a + (b - a) * fz) * 2.0 - 1.0
}

fn step(edge0: f32, edge1: f32, x: f32) -> f32 {
    smooth(((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0))
}

pub fn ground(x: f32, z: f32) -> f32 {
    let r = (x * x + z * z).sqrt();
    let hills = 2.2 * noise(x / 36.0, z / 36.0) + 0.9 * noise(x / 14.0 + 31.7, z / 14.0 - 12.3) + 0.25 * noise(x / 5.0 - 7.1, z / 5.0 + 3.9);
    let market = ((x - MARKET.0).powi(2) + (z - MARKET.1).powi(2)).sqrt();
    let level = step(FLAT_RADIUS, FLAT_RADIUS + BLEND, r) * step(MARKET.2, MARKET.2 + 6.0, market);
    let ridge = step(RIDGE_START, RIDGE_START + RIDGE_SPAN, r).powi(2) * RIDGE_HEIGHT * (0.65 + 0.35 * noise(x / 45.0 + 5.3, z / 45.0 - 2.2));
    hills * level + ridge
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courtyard_and_market_are_level_and_the_ground_is_continuous() {
        for (x, z) in [(0.0, 0.0), (15.0, -10.0), (-19.0, 0.0), (30.0, 30.0), (35.0, 27.0)] {
            assert_eq!(ground(x, z), 0.0, "not level at {x},{z}");
        }
        // no step bigger than a steep slope between points 10 cm apart anywhere in the world
        let mut worst = 0.0f32;
        for i in 0..400 {
            for j in 0..400 {
                let (x, z) = (i as f32 * 0.5 - 100.0, j as f32 * 0.5 - 100.0);
                worst = worst.max((ground(x + 0.1, z) - ground(x, z)).abs()).max((ground(x, z + 0.1) - ground(x, z)).abs());
            }
        }
        assert!(worst < 0.15, "ground jumps {worst} m over 10 cm");
        assert!(ground(160.0, 0.0) > 15.0, "no ridge at the horizon");
    }
}

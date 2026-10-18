//! The ruins on the north hill (`world::RUINS_CENTER`, a raised plateau `world::ground_height` blends
//! in): the place to build towards — a broken shrine whose cracked dais will hold the first boss.
//! Reached by the footpath north out of the courtyard, under a red torii.
//! - A ring of pillars (some whole with capitals, most snapped at odd heights, one fallen), two
//!   arches (one with a cracked, sagging lintel) and a broken back wall.
//! - A floor of flagstones with gaps, a low ring of arena stones and the dais at the centre.
//! - Stone lanterns (lit), two banners and scattered rubble.
//! Stone and wood here weather like the rest (`ground::Weathering`: moss, grime, wear).
use super::world::{Kit, RUINS_CENTER, ground_height, ruins_level};
use avian3d::prelude::Collider;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, TAU};

/// The arena's stone ring radius (m) and the pillar ring's.
pub const ARENA_RADIUS: f32 = 6.0;
const PILLAR_RING: f32 = 8.0;
const PILLARS: usize = 9;

fn hash(i: u32, j: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B9) ^ j.wrapping_mul(0x85EB_CA6B) ^ 0x27D4_EB2F;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

pub fn spawn_ruins(kit: &mut Kit) {
    let level = ruins_level();
    let c = Vec3::new(RUINS_CENTER.x, level, RUINS_CENTER.y);
    let (stone, rock, wall, dark, cloth, flame) = (
        kit.pal.stone.clone(),
        kit.pal.rock.clone(),
        kit.pal.wall.clone(),
        kit.pal.dark_wood.clone(),
        kit.pal.cloth.clone(),
        kit.pal.flame.clone(),
    );

    // Flagstones with gaps, each a little sunk or proud and turned.
    let mut k = 0u32;
    let step = 1.25;
    let mut x = -7.5;
    while x <= 7.5 {
        let mut z = -7.5;
        while z <= 7.5 {
            k += 1;
            let at = Vec2::new(x, z);
            if at.length() < 7.6 && hash(k, 1) > 0.22 {
                let size = Vec3::new(1.1 - hash(k, 2) * 0.2, 0.12, 1.1 - hash(k, 3) * 0.2);
                let tf = Transform::from_translation(c + Vec3::new(x + (hash(k, 4) - 0.5) * 0.12, 0.02 + (hash(k, 5) - 0.5) * 0.05, z + (hash(k, 6) - 0.5) * 0.12))
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, (hash(k, 7) - 0.5) * 0.12, (hash(k, 8) - 0.5) * 0.05, (hash(k, 9) - 0.5) * 0.05));
                kit.solid("Flagstone", Cuboid::from_size(size), &stone, tf, None);
            }
            z += step;
        }
        x += step;
    }

    // The dais: a cracked round platform at the centre (the boss's place).
    kit.solid("Dais", Cylinder::new(2.2, 0.3), &wall, Transform::from_translation(c + Vec3::Y * 0.15), Some(Collider::cylinder(2.2, 0.3)));
    kit.solid("Dais step", Cylinder::new(2.7, 0.14), &stone, Transform::from_translation(c + Vec3::Y * 0.07), Some(Collider::cylinder(2.7, 0.14)));
    for i in 0..3 {
        let a = i as f32 * 2.1 + 0.4;
        let crack = Cuboid::new(1.8, 0.02, 0.05);
        kit.solid("Dais crack", crack, &dark, Transform::from_translation(c + Vec3::new(a.cos() * 0.9, 0.305, a.sin() * 0.9)).with_rotation(Quat::from_rotation_y(-a + 0.3)), None);
    }

    // Arena ring of low stones.
    for i in 0..16 {
        let a = i as f32 / 16.0 * TAU;
        let p = c + Vec3::new(a.cos(), 0.0, a.sin()) * ARENA_RADIUS;
        let h = 0.25 + hash(i, 11) * 0.2;
        kit.solid("Arena stone", Cuboid::new(0.5, h, 0.4), &rock, Transform::from_translation(p + Vec3::Y * h * 0.5).with_rotation(Quat::from_rotation_y(-a)), None);
    }

    // Pillars: whole ones with capitals, snapped ones with a tilted broken top; one has fallen.
    for i in 0..PILLARS as u32 {
        let a = i as f32 / PILLARS as f32 * TAU + 0.2;
        // Leave the south open for the entrance.
        if (a - 1.5 * std::f32::consts::PI).abs() < 0.45 {
            continue;
        }
        let base = c + Vec3::new(a.cos(), 0.0, a.sin()) * PILLAR_RING;
        let base_y = ground_height(base.x, base.z).min(level);
        let foot = Vec3::new(base.x, base_y, base.z);
        kit.solid("Pillar base", Cuboid::new(1.15, 0.4, 1.15), &wall, Transform::from_translation(foot + Vec3::Y * 0.2), Some(Collider::cuboid(1.15, 0.4, 1.15)));
        if i == 4 {
            // Fallen: lying across the ring, broken in two.
            let along = Quat::from_rotation_y(-a + 0.6) * Quat::from_rotation_z(FRAC_PI_2);
            let dir = Quat::from_rotation_y(-a + 0.6) * Vec3::X;
            for (j, len) in [(0.0f32, 2.4f32), (1.0, 1.6)] {
                let at = foot + Vec3::Y * 0.42 + dir * (1.4 + j * 2.2);
                kit.solid("Fallen pillar", Cylinder::new(0.42, len), &stone, Transform::from_translation(at).with_rotation(along * Quat::from_rotation_x(j * 0.15)), Some(Collider::cylinder(0.42, len)));
            }
            continue;
        }
        let whole = hash(i, 21) > 0.68;
        let h = if whole { 4.2 } else { 1.2 + hash(i, 22) * 2.2 };
        kit.solid("Pillar", Cylinder::new(0.42, h), &stone, Transform::from_translation(foot + Vec3::Y * (0.4 + h * 0.5)), Some(Collider::cylinder(0.42, h)));
        let top = foot + Vec3::Y * (0.4 + h);
        if whole {
            kit.solid("Capital", Cuboid::new(1.05, 0.32, 1.05), &wall, Transform::from_translation(top + Vec3::Y * 0.16), None);
        } else {
            // A jagged break: a short tilted drum.
            let tilt = Quat::from_euler(EulerRot::XYZ, (hash(i, 23) - 0.5) * 0.7, 0.0, (hash(i, 24) - 0.5) * 0.7);
            kit.solid("Broken drum", Cylinder::new(0.4, 0.3), &stone, Transform::from_translation(top + Vec3::Y * 0.1).with_rotation(tilt), None);
            // A chunk fallen at its foot.
            let off = Vec3::new(hash(i, 25) - 0.5, 0.0, hash(i, 26) - 0.5).normalize_or(Vec3::X) * 1.1;
            kit.solid("Pillar chunk", Cylinder::new(0.38, 0.5), &stone, Transform::from_translation(foot + off + Vec3::Y * 0.3).with_rotation(Quat::from_rotation_x(FRAC_PI_2 * 0.9)), None);
        }
    }

    // Two arches on the north side; the eastern one's lintel has cracked and sags.
    for (n, dx) in [(0u32, -3.4f32), (1, 3.4)] {
        let mid = c + Vec3::new(dx, 0.0, 6.6);
        for side in [-1.0f32, 1.0] {
            let post = mid + Vec3::X * side * 1.5;
            kit.solid("Arch post", Cuboid::new(0.7, 3.6, 0.7), &wall, Transform::from_translation(post + Vec3::Y * 1.8), Some(Collider::cuboid(0.7, 3.6, 0.7)));
        }
        if n == 0 {
            kit.solid("Arch lintel", Cuboid::new(3.8, 0.6, 0.8), &wall, Transform::from_translation(mid + Vec3::Y * 3.9), Some(Collider::cuboid(3.8, 0.6, 0.8)));
        } else {
            for side in [-1.0f32, 1.0] {
                let piece = mid + Vec3::new(side * 0.95, 3.8, 0.0);
                kit.solid("Cracked lintel", Cuboid::new(1.9, 0.6, 0.8), &wall, Transform::from_translation(piece).with_rotation(Quat::from_rotation_z(side * 0.12)), None);
            }
        }
    }
    // Broken back wall: courses of stone at falling heights.
    for i in 0..7u32 {
        let x = -6.0 + i as f32 * 2.0;
        let h = 0.6 + hash(i, 31) * 1.8 * (1.0 - (x.abs() / 8.0));
        let p = c + Vec3::new(x, 0.0, 8.9 + (hash(i, 32) - 0.5) * 0.3);
        kit.solid("Ruin wall", Cuboid::new(1.95, h, 0.6), &wall, Transform::from_translation(p + Vec3::Y * h * 0.5), Some(Collider::cuboid(1.95, h, 0.6)));
    }

    // The torii at the head of the path.
    let gate = Vec3::new(c.x, 0.0, c.z - 9.6);
    let gate = gate.with_y(ground_height(gate.x, gate.z));
    for side in [-1.0f32, 1.0] {
        let post = gate + Vec3::X * side * 2.0;
        kit.solid("Torii post", Cylinder::new(0.2, 4.3), &cloth, Transform::from_translation(post + Vec3::Y * 2.15), Some(Collider::cylinder(0.2, 4.3)));
        kit.solid("Torii foot", Cylinder::new(0.28, 0.35), &dark, Transform::from_translation(post + Vec3::Y * 0.17), None);
    }
    kit.solid("Torii nuki", Cuboid::new(4.7, 0.24, 0.24), &cloth, Transform::from_translation(gate + Vec3::Y * 3.3), None);
    kit.solid("Torii kasagi", Cuboid::new(5.6, 0.32, 0.5), &dark, Transform::from_translation(gate + Vec3::Y * 4.45), None);
    for side in [-1.0f32, 1.0] {
        // Upturned ends of the top beam.
        let end = gate + Vec3::new(side * 3.0, 4.55, 0.0);
        kit.solid("Torii end", Cuboid::new(0.7, 0.28, 0.5), &dark, Transform::from_translation(end).with_rotation(Quat::from_rotation_z(side * 0.22)), None);
    }

    // Stone lanterns: two by the gate, two by the dais.
    let lantern = |kit: &mut Kit, at: Vec3| {
        kit.solid("Lantern base", Cylinder::new(0.3, 0.25), &rock, Transform::from_translation(at + Vec3::Y * 0.12), Some(Collider::cylinder(0.3, 0.25)));
        kit.solid("Lantern post", Cylinder::new(0.12, 0.8), &rock, Transform::from_translation(at + Vec3::Y * 0.65), None);
        kit.solid("Lantern box", Cuboid::new(0.42, 0.36, 0.42), &rock, Transform::from_translation(at + Vec3::Y * 1.23), None);
        kit.solid("Lantern cap", Cone { radius: 0.38, height: 0.3 }, &rock, Transform::from_translation(at + Vec3::Y * 1.55), None);
        let f = kit.solid("Lantern flame", Sphere::new(0.08), &flame, Transform::from_translation(at + Vec3::Y * 1.23), None);
        kit.commands.entity(f).insert((
            NotShadowCaster,
            PointLight { color: Color::srgb(1.0, 0.62, 0.3), intensity: 40_000.0, range: 9.0, shadow_maps_enabled: false, ..default() },
        ));
    };
    for side in [-1.0f32, 1.0] {
        let by_gate = gate + Vec3::new(side * 3.2, 0.0, 1.2);
        lantern(kit, by_gate.with_y(ground_height(by_gate.x, by_gate.z)));
        lantern(kit, c + Vec3::new(side * 3.6, 0.0, -2.6));
    }

    // Banners on poles beside the arches.
    for side in [-1.0f32, 1.0] {
        let pole = c + Vec3::new(side * 6.3, 0.0, 5.2);
        kit.solid("Banner pole", Cylinder::new(0.06, 4.6), &dark, Transform::from_translation(pole + Vec3::Y * 2.3), None);
        kit.solid("Banner bar", Cylinder::new(0.04, 1.0), &dark, Transform::from_translation(pole + Vec3::new(0.0, 4.4, -0.45)).with_rotation(Quat::from_rotation_x(FRAC_PI_2)), None);
        kit.solid("Banner", Cuboid::new(0.03, 2.4, 0.85), &cloth, Transform::from_translation(pole + Vec3::new(0.0, 3.15, -0.45)), None);
    }

    // Rubble.
    for i in 0..26u32 {
        let a = hash(i, 41) * TAU;
        let r = 2.5 + hash(i, 42) * 6.5;
        let p = c + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        let s = 0.12 + hash(i, 43) * 0.28;
        let y = ground_height(p.x, p.z).min(level);
        let tf = Transform::from_translation(p.with_y(y + s * 0.35))
            .with_rotation(Quat::from_euler(EulerRot::XYZ, hash(i, 44) * 3.0, hash(i, 45) * 3.0, hash(i, 46) * 3.0))
            .with_scale(Vec3::new(1.0, 0.6 + hash(i, 47) * 0.4, 0.8 + hash(i, 48) * 0.4));
        kit.solid("Rubble", Cuboid::new(s * 2.0, s * 1.6, s * 1.8), &rock, tf, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::world::{ruins_distance, ground_height, ruins_level};

    #[test]
    fn the_ruins_stand_on_a_flat_plateau() {
        let level = ruins_level();
        for (x, z) in [(0.0, 33.0), (5.0, 35.0), (-6.0, 30.0), (2.0, 40.0)] {
            assert!(ruins_distance(x, z) <= 0.0);
            assert!((ground_height(x, z) - level).abs() < 1e-3, "({x}, {z}) {} vs {level}", ground_height(x, z));
        }
        // The plateau blends into the hills (no cliff at its edge).
        let (a, b) = (ground_height(0.0, 21.0), ground_height(0.0, 21.5));
        assert!((a - b).abs() < 0.6);
    }

    #[test]
    fn hashes_are_in_unit_range() {
        for i in 0..200 {
            let h = hash(i, 7);
            assert!((0.0..=1.0).contains(&h));
        }
    }
}

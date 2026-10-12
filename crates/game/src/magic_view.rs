//! Magic presentation: glowing projectile orbs with lights, charge orbs, ki aura, burst shockwaves.
//! Reads the sim only.

use bevy::prelude::*;
use guildbound_sim as gs;
use gs::magic::{Element, SPELLS};

use crate::sim_bridge::Sim;

#[derive(Component)]
pub struct Orb(usize);

#[derive(Resource)]
pub struct OrbAssets {
    mesh: Handle<Mesh>,
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(OrbAssets { mesh: meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap()) });
}

/// Element colour; a charged fireball runs orange -> blue -> white (GDD D18).
pub fn element_color(e: Element, charge: f32) -> LinearRgba {
    match e {
        Element::Fire => {
            let (orange, blue, white) = (LinearRgba::rgb(4.0, 1.2, 0.2), LinearRgba::rgb(0.6, 1.6, 6.0), LinearRgba::rgb(6.0, 6.0, 6.0));
            if charge < 0.5 { orange.mix(&blue, charge * 2.0) } else { blue.mix(&white, (charge - 0.5) * 2.0) }
        }
        Element::Lightning => LinearRgba::rgb(1.2, 3.5, 7.0),
        Element::Frost => LinearRgba::rgb(1.6, 4.0, 6.0),
        Element::Arcane => LinearRgba::rgb(3.5, 1.2, 5.0),
        Element::Holy => LinearRgba::rgb(5.0, 4.6, 2.0),
    }
}

/// One orb entity per live projectile, plus one charge orb per charging fighter (pooled by index).
pub fn sync_orbs(
    mut commands: Commands,
    sim: Res<Sim>,
    assets: Res<OrbAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rigs: Query<&crate::anim::FighterRig>,
    bones: Query<(&Name, &GlobalTransform), Without<Orb>>,
    mut orbs: Query<(Entity, &Orb, &mut Transform, &mut GlobalTransform, &MeshMaterial3d<StandardMaterial>, &mut PointLight)>,
) {
    let gain = sim.world.magic.charge_gain;
    // what should be lit this frame: (position, radius, colour)
    let mut wanted: Vec<(Vec3, f32, LinearRgba)> = sim
        .world
        .projectiles
        .iter()
        .map(|p| {
            let d = SPELLS[p.spell];
            let r = d.radius * p.scale(gain).sqrt();
            (Vec3::new(p.position.x, 1.25, p.position.z), r, element_color(d.element, p.charge))
        })
        .collect();
    for (i, f) in sim.world.fighters.iter().enumerate() {
        let fallback = Vec3::new(f.position.x, 1.25 + f.height, f.position.z) + Vec3::new(f.facing.x, 0.0, f.facing.z) * 0.55;
        let mut points = Vec::new();
        if let Some(rig) = rigs.iter().find(|r| r.fighter == i) {
            let lightning = matches!(f.action.kind, gs::ActionKind::Cast(gs::magic::CHAIN_LIGHTNING));
            let left = rig.one_shot.as_ref().is_some_and(|c| c.ends_with("_left"));
            for &bone in &rig.bones {
                if let Ok((name, transform)) = bones.get(bone) {
                    let use_bone = if lightning {
                        name.as_str() == if left { "B_L_Forearm.001" } else { "B_R_Hand" }
                    } else { matches!(name.as_str(), "B_L_Forearm.001" | "B_R_Hand") };
                    if use_bone { points.push(transform.translation()); }
                }
            }
        }
        let hand = if points.is_empty() { fallback } else { points.iter().copied().sum::<Vec3>() / points.len() as f32 };
        match f.action.kind {
            gs::ActionKind::Charge(s) => wanted.push((hand, 0.12 + 0.25 * f.charge, element_color(SPELLS[s].element, f.charge))),
            gs::ActionKind::Cast(s) if f.action.elapsed < SPELLS[s].prepare => wanted.push((hand, 0.12, element_color(SPELLS[s].element, f.charge))),
            _ => {}
        }
    }
    let mut have: Vec<_> = orbs.iter_mut().collect();
    have.sort_by_key(|o| o.1.0);
    for (k, (pos, r, color)) in wanted.iter().enumerate() {
        if let Some((_, _, t, global, mat, light)) = have.get_mut(k) {
            t.translation = *pos;
            t.scale = Vec3::splat(*r);
            **global = GlobalTransform::from(**t);
            if let Some(mut m) = materials.get_mut(&mat.0) {
                m.emissive = *color;
            }
            light.color = Color::LinearRgba(*color);
            light.intensity = 60_000.0 * (0.3 + r);
        } else {
            commands.spawn((
                Orb(k),
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::BLACK, emissive: *color, unlit: false, ..default() })),
                Transform::from_translation(*pos).with_scale(Vec3::splat(*r)),
                GlobalTransform::from(Transform::from_translation(*pos).with_scale(Vec3::splat(*r))),
                PointLight { color: Color::LinearRgba(*color), intensity: 60_000.0, range: 6.0, shadow_maps_enabled: false, ..default() },
            ));
        }
    }
    for (e, o, ..) in &orbs {
        if o.0 >= wanted.len() {
            commands.entity(e).despawn();
        }
    }
}

/// Skill effects drawn from sim state: healing light rising while it channels, the frost nova ring, chain
/// lightning bolts (caster -> first victim -> each jump), the blink's departure ring.
pub fn draw_magic(sim: Res<Sim>, mut gizmos: Gizmos) {
    let now = sim.world.tick as f32 * gs::STEP;
    let flat = |p: gs::V2, y: f32| Vec3::new(p.x, y, p.z);
    let ring = |c: Vec3| Isometry3d::new(c, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    for f in &sim.world.fighters {
        if f.action.kind == gs::ActionKind::Cast(gs::magic::HEALING_LIGHT) {
            for k in 0..3 {
                let phase = (now * 1.4 + k as f32 / 3.0).fract();
                gizmos.circle(ring(flat(f.position, 0.05 + phase * 2.0)), 0.6 - phase * 0.25, Color::srgba(1.0, 0.95, 0.5, 1.0 - phase));
            }
        }
    }
    let mut chain_from: Vec<(u64, Vec3)> = Vec::new();
    for (c, at) in &sim.recent {
        let age = now - at;
        if c.source == gs::Source::Spell(gs::magic::FROST_NOVA) && age < 0.4 {
            let src = &sim.world.fighters[c.attacker];
            gizmos.circle(ring(flat(src.position, 0.1)), 0.4 + age / 0.4 * SPELLS[gs::magic::FROST_NOVA].area, Color::srgba(0.6, 0.9, 1.0, 1.0 - age / 0.4));
        }
        if c.source == gs::Source::Spell(gs::magic::CHAIN_LIGHTNING) && age < 0.25 {
            // contacts of one cast share a tick and arrive in jump order
            let start = chain_from.iter().find(|(t, _)| *t == c.tick).map(|x| x.1).unwrap_or_else(|| flat(sim.world.fighters[c.attacker].position, 1.3));
            let end = flat(c.point, 1.2);
            let mut prev = start;
            for k in 1..=6 {
                let t = k as f32 / 6.0;
                let jitter = if k < 6 { Vec3::new(((c.tick + k) as f32 * 12.9898).sin(), ((c.tick + k) as f32 * 78.233).sin() * 0.6, ((c.tick + k) as f32 * 37.719).cos()) * 0.18 } else { Vec3::ZERO };
                let p = start.lerp(end, t) + jitter;
                gizmos.line(prev, p, Color::srgba(0.7, 0.85, 1.0, 1.0 - age / 0.25));
                prev = p;
            }
            if let Some(e) = chain_from.iter_mut().find(|(t, _)| *t == c.tick) { e.1 = end; } else { chain_from.push((c.tick, end)); }
        }
    }
    for p in &sim.world.projectiles {
        let d = SPELLS[p.spell];
        let head = Vec3::new(p.position.x, 1.25, p.position.z);
        let tail = head - Vec3::new(p.direction.x, 0.0, p.direction.z) * (0.4 + d.speed * 0.03);
        gizmos.line(head, tail, Color::LinearRgba(element_color(d.element, p.charge)));
    }
}

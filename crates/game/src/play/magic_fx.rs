//! Compact, spell-specific accents using the existing animated VFX sheets.
//! Particle emission stays in `vfx`; this layer adds one readable silhouette per spell.
use super::flipbook::{Flip, Sheet};
use super::magic::{Ability, Shape};
use super::vfx::Element;
use bevy::prelude::*;

fn sheet(a: &Ability) -> Sheet {
    match a.id {
        "ember_burst" => Sheet::BrightFire,
        "glacier_lance" => Sheet::Freezing,
        "thunder_ring" => Sheet::Magic8,
        "tide_orbs" => Sheet::Bubbles,
        "arcane_step" => Sheet::Phantom,
        "sunflare" => Sheet::Sunburn,
        "shadow_veil" => Sheet::Midnight,
        "stone_ward" => Sheet::Ward,
        "cometfall" => Sheet::FireSpin,
        "gale_crescent" | "crescent_edge" => Sheet::WeaponHit,
        _ => match a.element {
            Element::Fire => Sheet::Fire,
            Element::Frost => Sheet::Freezing,
            Element::Lightning => Sheet::Magic8,
            Element::Arcane => Sheet::Nebula,
            Element::Aura => Sheet::MagicHit,
        },
    }
}

fn tint(a: &Ability) -> Color {
    let c = a.element.core();
    let peak = c.red.max(c.green).max(c.blue).max(1.0);
    Color::LinearRgba(LinearRgba::new(c.red / peak, c.green / peak, c.blue / peak, 0.8))
}

pub fn release(commands: &mut Commands, a: &Ability, hand: Vec3, feet: Vec3, charge: f32) {
    let flat = matches!(a.shape, Shape::Ward { .. } | Shape::Veil { .. } | Shape::Quake { .. });
    let effect = Flip::new(sheet(a), if flat { feet + Vec3::Y * 0.05 } else { hand },
        if flat { 1.8 } else { 0.35 + charge * 0.12 }, 0.26)
        .tinted(tint(a), 0.85).glow(0.7);
    commands.write_message(if flat { effect.flat().spin(0.8) } else { effect });
}

pub fn impact(commands: &mut Commands, a: &Ability, at: Vec3, flair: f32) {
    let size = match a.shape {
        Shape::Bolt { bloom, .. } if bloom > 0.0 => (bloom * 1.8).clamp(1.0, 3.8),
        Shape::Quake { radius } => (radius * 1.8).min(8.0),
        Shape::Crescent { .. } => 1.4,
        _ => 1.15,
    } * flair.min(1.4);
    let effect = Flip::new(sheet(a), at, size, if a.id == "cometfall" { 0.55 } else { 0.38 })
        .tinted(tint(a), 0.85).glow(0.95);
    commands.write_message(if matches!(a.shape, Shape::Quake { .. }) { effect.flat() } else { effect });
}

/// An elongated comet, round ember, compact orb, and thin gale each retain a distinct shape.
pub fn core_scale(a: &Ability, flair: f32) -> Vec3 {
    let size = match a.id {
        "cometfall" => Vec3::new(0.16, 0.16, 0.38),
        "sunflare" => Vec3::splat(0.19),
        "cinder_bolt" => Vec3::splat(0.18),
        "ember_burst" => Vec3::splat(0.22),
        "tide_orbs" => Vec3::new(0.09, 0.12, 0.09),
        _ => Vec3::splat(0.11),
    };
    size * flair
}

pub fn charge_color(a: &Ability, charge: f32) -> Color {
    let c = a.element.charged_core(charge);
    let peak = c.red.max(c.green).max(c.blue).max(1.0);
    Color::LinearRgba(LinearRgba::new(c.red / peak, c.green / peak, c.blue / peak, 0.3 + charge * 0.35))
}

pub fn two_handed(a: &Ability) -> bool {
    !a.is_art() && !matches!(a.shape, Shape::Lance { .. })
}

/// A moving preparation from the spell's own animation family. Upper-body ownership lets
/// the controller keep natural footwork while holding; no full-body static ki-charge stance.
pub fn channel(a: &Ability, elapsed: f32) -> super::ActionLayer {
    let beam = matches!(a.shape, Shape::Lance { .. });
    let ki = beam || (a.is_art() && matches!(a.shape, Shape::Bolt { .. }));
    let (track, start, ready) = if ki { ("sparking/DBZ_Ki_Blast", 0.0, 0.18) }
        else if matches!(a.shape, Shape::Crescent { .. }) { (a.track, a.start, a.start + (a.end - a.start) * 0.32) }
        else { ("sekiro/Magic Action Big Projectile", 0.5, 0.93) };
    let time = if beam { (elapsed * 0.5).min(0.08) + (elapsed * 4.0).sin() * 0.008 }
        else if ki { (elapsed * 0.65) % ready }
        else if elapsed < 0.45 { start + (ready - start) * (elapsed / 0.45) }
        else { ready - 0.035 + 0.035 * ((elapsed - 0.45) * 5.0).cos() };
    let mut layer = super::ActionLayer::new(track, time, crate::motion::Mask::Upper);
    layer.style = Some(crate::motion::timing::Style { exaggeration: 1.0, hold: 0.0, bias: 0.5, ..default() });
    layer
}

pub fn orbit(commands: &mut Commands, a: &Ability, entity: Entity, at: Vec3) {
    if a.id == "tide_orbs" {
        commands.write_message(Flip::new(Sheet::Bubbles, at, 0.38, 3.6)
            .animated(0.75).follow(entity, Vec3::ZERO).tinted(tint(a), 0.9).glow(0.45));
    }
}

pub fn beam(commands: &mut Commands, a: &Ability, from: Vec3, to: Vec3, charge: f32, hand: Option<Entity>) {
    let preset = match a.element {
        Element::Frost => 5, Element::Lightning => 7, Element::Fire => 1,
        Element::Arcane => 2, Element::Aura => 0,
    };
    commands.write_message(super::energy::BeamRequest {
        preset, from, to, life: 0.65 + charge * 0.3, strength: 1.0 + charge * 0.45, hand,
    });
}

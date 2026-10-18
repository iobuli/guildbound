//! Stats: the universal base of every fighter (player and NPCs alike). `Stats` (required by
//! `Fighter`) holds a level, six attributes, unspent points and experience; `derive` turns them
//! into `Derived`, which the rest of the game reads:
//! - `combat` sets `Fighter` max hp / stamina / poise from it (keeping the current shares) and
//!   regenerates stamina at `regen`; melee blows scale by `might` (`combat::DamageScale`, written
//!   here only);
//! - `magic` sizes the magicule pool (`max_mana`, `mana_regen`) for spells and the Battlewill
//!   pool (`max_battlewill`) for arts, and scales both by `spell` / `might`.
//!
//! Attributes (10 = an ordinary person, `derive` is linear in each):
//! Vigor → hp; Endurance → stamina, poise, stamina regen, Battlewill; Strength → might, poise,
//! Battlewill;
//! Dexterity → might (less), cast speed; Intelligence → mana, spell power; Faith → mana regen,
//! mana, spell power (less).
//!
//! Levelling: a kill grants the killer (the last to strike the victim, player or NPC)
//! `XP_PER_KILL` × the victim's level; each level
//! needs `xp_to_next` and gives `POINTS_PER_LEVEL` points. `SpendPoint` (the stats panel's +
//! buttons) raises an attribute while points remain. NPC strength comes from their `Stats` (e.g.
//! `Stats::preset`), not from a separate difficulty knob.
use super::combat::DamageScale;
use super::{Fighter, Hit};
use std::collections::HashMap;
use bevy::prelude::*;

/// Attribute value of an ordinary person; every derived value is "normal" at all-10s.
pub const BASE: u32 = 10;
pub const MAX_ATTR: u32 = 60;
/// Experience per kill (× the victim's level), points per level.
pub const XP_PER_KILL: u32 = 60;
pub const POINTS_PER_LEVEL: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attr {
    Vigor,
    Endurance,
    Strength,
    Dexterity,
    Intelligence,
    Faith,
}

impl Attr {
    pub const ALL: [Attr; 6] = [
        Attr::Vigor,
        Attr::Endurance,
        Attr::Strength,
        Attr::Dexterity,
        Attr::Intelligence,
        Attr::Faith,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Attr::Vigor => "Vigor",
            Attr::Endurance => "Endurance",
            Attr::Strength => "Strength",
            Attr::Dexterity => "Dexterity",
            Attr::Intelligence => "Intelligence",
            Attr::Faith => "Faith",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Attr::Vigor => "VIG",
            Attr::Endurance => "END",
            Attr::Strength => "STR",
            Attr::Dexterity => "DEX",
            Attr::Intelligence => "INT",
            Attr::Faith => "FTH",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Attr::Vigor => "Health.",
            Attr::Endurance => "Stamina, poise and stamina recovery.",
            Attr::Strength => "Weapon damage (most) and poise.",
            Attr::Dexterity => "Weapon damage and casting speed.",
            Attr::Intelligence => "Mana and spell power.",
            Attr::Faith => "Mana recovery, mana and some spell power.",
        }
    }
    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct Stats {
    pub level: u32,
    attrs: [u32; 6],
    /// Unspent attribute points.
    pub points: u32,
    /// Experience toward the next level.
    pub xp: u32,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            level: 1,
            attrs: [BASE; 6],
            points: 0,
            xp: 0,
        }
    }
}

impl Stats {
    /// A fighter of `level` whose extra points (`POINTS_PER_LEVEL` per level above 1) went into
    /// `build` in turn (repeat an attribute to weight it).
    pub fn preset(level: u32, build: &[Attr]) -> Self {
        let mut s = Self {
            level: level.max(1),
            ..default()
        };
        let extra = (s.level - 1) * POINTS_PER_LEVEL;
        for i in 0..extra as usize {
            if let Some(a) = build.get(i % build.len().max(1)) {
                let v = &mut s.attrs[a.index()];
                *v = (*v + 1).min(MAX_ATTR);
            }
        }
        s
    }
    pub fn get(&self, a: Attr) -> u32 {
        self.attrs[a.index()]
    }
    /// Spend one point on `a`; false when there are none or it is maxed.
    pub fn spend(&mut self, a: Attr) -> bool {
        let v = &mut self.attrs[a.index()];
        if self.points == 0 || *v >= MAX_ATTR {
            return false;
        }
        *v += 1;
        self.points -= 1;
        true
    }
    /// Experience needed from this level to the next.
    pub fn xp_to_next(&self) -> u32 {
        100 + 50 * self.level
    }
    /// Add experience; returns the levels gained.
    pub fn gain(&mut self, xp: u32) -> u32 {
        self.xp = self.xp.saturating_add(xp);
        let mut gained = 0;
        while self.xp >= self.xp_to_next() {
            self.xp -= self.xp_to_next();
            self.level += 1;
            self.points += POINTS_PER_LEVEL;
            gained += 1;
        }
        gained
    }
}

/// Values derived from `Stats` (written by `stats` only).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Derived {
    pub max_hp: f32,
    pub max_stamina: f32,
    pub max_poise: f32,
    pub max_mana: f32,
    /// Battlewill (aura) pool for arts.
    pub max_battlewill: f32,
    /// Melee damage and poise multiplier.
    pub might: f32,
    /// Spell damage multiplier.
    pub spell: f32,
    /// Stamina and mana regeneration multipliers.
    pub regen: f32,
    pub mana_regen: f32,
    /// Cast animation speed multiplier.
    pub cast_speed: f32,
    /// Melee attack speed multiplier (`combat::phase_speed` turns it into a shorter wind-up,
    /// not a faster clip).
    pub attack_speed: f32,
}

/// Everything at 10 gives 100 hp / stamina / mana, 50 poise and ×1 multipliers.
pub fn derive(s: &Stats) -> Derived {
    let a = |x: Attr| s.get(x) as f32 - BASE as f32;
    Derived {
        max_hp: 100.0 + 8.0 * a(Attr::Vigor),
        max_stamina: 100.0 + 5.0 * a(Attr::Endurance),
        max_poise: 50.0 + 2.0 * a(Attr::Endurance) + 1.5 * a(Attr::Strength),
        max_mana: 100.0 + 7.0 * a(Attr::Intelligence) + 3.0 * a(Attr::Faith),
        max_battlewill: 100.0 + 4.0 * a(Attr::Endurance) + 4.0 * a(Attr::Strength),
        might: (1.0 + 0.03 * a(Attr::Strength) + 0.015 * a(Attr::Dexterity)).max(0.3),
        spell: (1.0 + 0.035 * a(Attr::Intelligence) + 0.012 * a(Attr::Faith)).max(0.3),
        regen: (1.0 + 0.02 * a(Attr::Endurance)).max(0.3),
        mana_regen: (1.0 + 0.03 * a(Attr::Faith) + 0.01 * a(Attr::Intelligence)).max(0.3),
        cast_speed: (1.0 + 0.01 * a(Attr::Dexterity)).clamp(0.5, 1.6),
        attack_speed: (1.0 + 0.02 * a(Attr::Dexterity)).clamp(0.7, 1.5),
    }
}

/// Flat additions to `Derived` from skill levels (written by `skills` only; each skill row's
/// `bonus` × its level).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkillBonus {
    pub max_hp: f32,
    pub max_stamina: f32,
    pub max_mana: f32,
    pub max_battlewill: f32,
    pub might: f32,
    pub spell: f32,
    pub cast_speed: f32,
}

impl SkillBonus {
    /// `self += other × k`.
    pub fn add(&mut self, other: &SkillBonus, k: f32) {
        self.max_hp += other.max_hp * k;
        self.max_stamina += other.max_stamina * k;
        self.max_mana += other.max_mana * k;
        self.max_battlewill += other.max_battlewill * k;
        self.might += other.might * k;
        self.spell += other.spell * k;
        self.cast_speed += other.cast_speed * k;
    }
}

/// `derive` plus skill bonuses.
pub fn derive_with(s: &Stats, b: &SkillBonus) -> Derived {
    let d = derive(s);
    Derived {
        max_hp: d.max_hp + b.max_hp,
        max_stamina: d.max_stamina + b.max_stamina,
        max_mana: d.max_mana + b.max_mana,
        max_battlewill: d.max_battlewill + b.max_battlewill,
        might: d.might + b.might,
        spell: d.spell + b.spell,
        cast_speed: (d.cast_speed + b.cast_speed).clamp(0.5, 1.8),
        ..d
    }
}

/// Raise an attribute with a point (stats panel).
#[derive(Message, Clone, Copy, Debug)]
pub struct SpendPoint {
    pub who: Entity,
    pub attr: Attr,
}

/// Experience from elsewhere (contract rewards).
#[derive(Message, Clone, Copy, Debug)]
pub struct GrantXp {
    pub who: Entity,
    pub xp: u32,
}

/// A fighter levelled up (the HUD logs the player's).
#[derive(Message, Clone, Copy, Debug)]
pub struct LevelUp {
    pub who: Entity,
    pub level: u32,
}

pub fn plugin(app: &mut App) {
    app.register_required_components::<Fighter, Stats>()
        .add_message::<SpendPoint>()
        .add_message::<LevelUp>()
        .add_message::<GrantXp>()
        .add_systems(Update, (spend, award_xp, grant, publish).chain());
}

fn spend(mut spends: MessageReader<SpendPoint>, mut stats: Query<&mut Stats>) {
    for s in spends.read() {
        if let Ok(mut st) = stats.get_mut(s.who) {
            st.spend(s.attr);
        }
    }
}

/// A fighter that just died gives experience to whoever struck it last (player or NPC).
fn award_xp(
    mut hits: MessageReader<Hit>,
    mut fighters: Query<(Entity, &Fighter, &mut Stats)>,
    mut ups: MessageWriter<LevelUp>,
    mut dead: Local<Vec<Entity>>,
    mut last: Local<HashMap<Entity, Entity>>,
) {
    for h in hits.read() {
        if h.attacker != h.target {
            last.insert(h.target, h.attacker);
        }
    }
    let mut awards = Vec::new();
    for (e, f, s) in &fighters {
        let was = dead.contains(&e);
        if f.dead && !was {
            dead.push(e);
            if let Some(killer) = last.remove(&e) {
                awards.push((killer, XP_PER_KILL * s.level));
            }
        } else if !f.dead && was {
            dead.retain(|d| *d != e);
        }
    }
    for (killer, xp) in awards {
        if let Ok((e, f, mut s)) = fighters.get_mut(killer)
            && !f.dead
            && s.gain(xp) > 0
        {
            ups.write(LevelUp { who: e, level: s.level });
        }
    }
}

fn grant(mut grants: MessageReader<GrantXp>, mut stats: Query<(&Fighter, &mut Stats)>, mut ups: MessageWriter<LevelUp>) {
    for g in grants.read() {
        if let Ok((f, mut s)) = stats.get_mut(g.who)
            && !f.dead
            && s.gain(g.xp) > 0
        {
            ups.write(LevelUp { who: g.who, level: s.level });
        }
    }
}

/// Keep `Derived` and `DamageScale` in step with `Stats` and `SkillBonus`.
#[allow(clippy::type_complexity)]
fn publish(
    mut commands: Commands,
    stats: Query<(Entity, &Stats, Option<&SkillBonus>, Option<&Derived>), Or<(Changed<Stats>, Changed<SkillBonus>)>>,
) {
    for (e, s, b, old) in &stats {
        let d = derive_with(s, &b.copied().unwrap_or_default());
        if old != Some(&d) {
            commands.entity(e).try_insert((d, DamageScale(d.might)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tens_is_the_baseline() {
        let d = derive(&Stats::default());
        assert_eq!((d.max_hp, d.max_stamina, d.max_poise, d.max_mana), (100.0, 100.0, 50.0, 100.0));
        assert_eq!((d.might, d.spell, d.regen, d.mana_regen, d.cast_speed), (1.0, 1.0, 1.0, 1.0, 1.0));
    }

    #[test]
    fn attributes_raise_what_they_say() {
        let base = derive(&Stats::default());
        for a in Attr::ALL {
            let mut s = Stats {
                points: 5,
                ..default()
            };
            for _ in 0..5 {
                assert!(s.spend(a));
            }
            assert!(!s.spend(a), "out of points");
            let d = derive(&s);
            let raised = match a {
                Attr::Vigor => d.max_hp > base.max_hp,
                Attr::Endurance => d.max_stamina > base.max_stamina && d.regen > base.regen,
                Attr::Strength => d.might > base.might && d.max_poise > base.max_poise,
                Attr::Dexterity => d.might > base.might && d.cast_speed > base.cast_speed,
                Attr::Intelligence => d.max_mana > base.max_mana && d.spell > base.spell,
                Attr::Faith => d.mana_regen > base.mana_regen,
            };
            assert!(raised, "{a:?}");
        }
    }

    #[test]
    fn levels_grant_points_and_presets_spend_them() {
        let mut s = Stats::default();
        assert_eq!(s.gain(s.xp_to_next() - 1), 0);
        assert_eq!(s.gain(1), 1);
        assert_eq!((s.level, s.points, s.xp), (2, POINTS_PER_LEVEL, 0));
        let p = Stats::preset(5, &[Attr::Strength, Attr::Dexterity]);
        assert_eq!(p.get(Attr::Strength) + p.get(Attr::Dexterity), 2 * BASE + 4 * POINTS_PER_LEVEL);
        assert_eq!(p.points, 0);
    }

    #[test]
    fn skill_bonus_adds_on_top() {
        let s = Stats::default();
        let b = SkillBonus {
            max_hp: 12.0,
            might: 0.06,
            ..default()
        };
        let d = derive_with(&s, &b);
        assert_eq!(d.max_hp, 112.0);
        assert!((d.might - 1.06).abs() < 1e-6);
        assert_eq!(derive_with(&s, &SkillBonus::default()), derive(&s));
    }
}

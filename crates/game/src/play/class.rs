//! Choosing a path: on a new game a panel offers three classes (keys 1–3). The choice sets the
//! starting weapon (`combat::Equip`), skills (`skills::Sheet::grant`), known arts and spells and
//! which are bound (`magic::Grimoire`), and three attribute points (`stats::Stats`).
//! - Blade: katana; Swordsmanship 5, Aura Arts 3; Flash Step bound to Q; Strength, Dexterity, Vigor.
//! - Arcanist: dagger; Frost 5, Lightning 3; Frost Bloom and Storm Lance, Frost Bloom on E;
//!   Intelligence ×2, Faith.
//! - Hunter: bow; Archery 5, Stealth 3; Veil bound to Q; Dexterity ×2, Endurance.
//! - Monk: fists; Unarmed 5, Defence 3, Aura Arts 2; Aura Bullet bound to Q; Strength, Dexterity,
//!   Endurance. Fists hit harder than any weapon he holds; his guard turns blows aside and a
//!   guard raised inside the blow is an auto counter that stuns (`discipline`).
//! The choice also dresses the player for the path (`appearance::dress_class`).
//! While the panel is open the player's input is held (`input`) and nobody attacks (`ai`).
//! Smoke runs (any `GB_*` set) skip the panel; `GB_CLASS=blade|arcanist|hunter` picks one.
use super::combat::Equip;
use super::hud::log::{HudLog, LogKind};
use super::magic::Grimoire;
use super::skills::{Sheet, xp_for_level};
use super::stats::{Attr, Stats};
use super::Player;
use bevy::prelude::*;
use gbui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Blade,
    Arcanist,
    Hunter,
    Monk,
}

/// What a class starts with.
pub struct Kit {
    pub name: &'static str,
    pub about: &'static str,
    pub weapon: &'static str,
    pub skills: &'static [(&'static str, u32)],
    pub abilities: &'static [&'static str],
    pub art: Option<&'static str>,
    pub spell: Option<&'static str>,
    pub attrs: [Attr; 3],
}

impl Class {
    pub const ALL: [Class; 4] = [Class::Blade, Class::Arcanist, Class::Hunter, Class::Monk];

    pub fn kit(self) -> Kit {
        match self {
            Class::Blade => Kit {
                name: "Blade",
                about: "Katana in hand, aura in the legs: close the gap in a blink and cut.",
                weapon: "katana",
                skills: &[("swordsmanship", 5), ("aura_arts", 3)],
                abilities: &["flash_step"],
                art: Some("flash_step"),
                spell: None,
                attrs: [Attr::Strength, Attr::Dexterity, Attr::Vigor],
            },
            Class::Arcanist => Kit {
                name: "Arcanist",
                about: "A dagger for show; frost and lightning for the work.",
                weapon: "dagger",
                skills: &[("frost", 5), ("lightning", 3)],
                abilities: &["frost_bloom", "storm_lance"],
                art: None,
                spell: Some("frost_bloom"),
                attrs: [Attr::Intelligence, Attr::Intelligence, Attr::Faith],
            },
            Class::Hunter => Kit {
                name: "Hunter",
                about: "Bow and patience: unseen under the Veil, loosed from afar.",
                weapon: "bow",
                skills: &[("archery", 5), ("stealth", 3)],
                abilities: &["veil"],
                art: Some("veil"),
                spell: None,
                attrs: [Attr::Dexterity, Attr::Dexterity, Attr::Endurance],
            },
            Class::Monk => Kit {
                name: "Monk",
                about: "Empty hands, iron guard: turn the blow aside and answer it before it ends.",
                weapon: "unarmed",
                skills: &[("unarmed", 5), ("defence", 3), ("aura_arts", 2)],
                abilities: &["aura_bullet"],
                art: Some("aura_bullet"),
                spell: None,
                attrs: [Attr::Strength, Attr::Dexterity, Attr::Endurance],
            },
        }
    }

    /// The fighting path this class walks (`discipline`).
    pub fn path(self) -> super::discipline::Path {
        use super::discipline::Path;
        match self {
            Class::Blade => Path::Blade,
            Class::Arcanist => Path::Arcanist,
            Class::Hunter => Path::Hunter,
            Class::Monk => Path::Monk,
        }
    }

    pub fn from_name(s: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.kit().name.eq_ignore_ascii_case(s.trim()))
    }
}

/// The choice: whether the panel is up, and what was picked (applied once).
#[derive(Resource, Default)]
pub struct ClassChoice {
    pub open: bool,
    pub picked: Option<Class>,
    pub(crate) applied: bool,
}

pub fn plugin(app: &mut App) {
    // `GB_CLASS_MENU=1` shows the panel even in a smoke run.
    let smoke = super::world::duel_mode() || (std::env::vars().any(|(k, _)| k.starts_with("GB_")) && std::env::var_os("GB_CLASS_MENU").is_none());
    let picked = std::env::var("GB_CLASS").ok().and_then(|s| Class::from_name(&s));
    app.insert_resource(ClassChoice { open: !smoke, picked, applied: false })
        .add_systems(Startup, spawn_panel)
        .add_systems(Update, (choose, apply, show).chain());
}

#[derive(Component)]
struct Panel;

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            super::talk::KeepInTalk,
            panel(
                "stats",
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(30.0),
                    top: percent(26.0),
                    width: percent(40.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(18.0)),
                    row_gap: px(10.0),
                    display: Display::None,
                    ..default()
                },
            ),
            GlobalZIndex(70),
        ))
        .with_children(|p| {
            p.spawn(label("▌CHOOSE YOUR PATH", "title"));
            for (i, c) in Class::ALL.into_iter().enumerate() {
                let k = c.kit();
                p.spawn(label(format!("{}.  {}", i + 1, k.name), "row.text"));
                p.spawn(label(format!("     {}", k.about), "text.dim"));
            }
            p.spawn(label("Press 1, 2, 3 or 4", "text.dim"));
        });
}

fn choose(keys: Res<ButtonInput<KeyCode>>, mut choice: ResMut<ClassChoice>) {
    if !choice.open {
        return;
    }
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].into_iter().enumerate() {
        if keys.just_pressed(key) {
            choice.picked = Some(Class::ALL[i]);
            choice.open = false;
        }
    }
}

/// Give the player the chosen kit (once).
#[allow(clippy::type_complexity)]
pub(crate) fn apply(
    mut choice: ResMut<ClassChoice>,
    mut players: Query<(Entity, &mut Sheet, &mut Grimoire, &mut Stats, Option<&super::appearance::Appearance>), With<Player>>,
    mut equip: MessageWriter<Equip>,
    mut commands: Commands,
    mut log: MessageWriter<HudLog>,
) {
    if choice.applied {
        return;
    }
    let Some(class) = choice.picked else { return };
    let Ok((e, mut sheet, mut grimoire, mut stats, worn)) = players.single_mut() else { return };
    let kit = class.kit();
    // Dressed for the path (`appearance.json` `classes`).
    let class_name = format!("{class:?}").to_lowercase();
    super::appearance::dress_class(&mut commands, e, worn, &class_name);
    equip.write(Equip { who: e, set: kit.weapon.to_string() });
    for (skill, level) in kit.skills {
        sheet.grant(skill, xp_for_level(*level));
    }
    for a in kit.abilities {
        if !sheet.has_ability(a) {
            sheet.abilities.push(a);
        }
    }
    if let Some(a) = kit.art {
        grimoire.art = a;
    }
    if let Some(s) = kit.spell {
        grimoire.spell = s;
    }
    stats.points += kit.attrs.len() as u32;
    for a in kit.attrs {
        stats.spend(a);
    }
    choice.applied = true;
    log.write(HudLog::new(LogKind::System, format!("You walk the path of the {}.", kit.name)));
    info!("[class] {}", kit.name);
}

fn show(choice: Res<ClassChoice>, mut panels: Query<&mut Node, With<Panel>>) {
    if !choice.is_changed() {
        return;
    }
    for mut n in &mut panels {
        n.display = if choice.open { Display::Flex } else { Display::None };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_a_real_kit() {
        for c in Class::ALL {
            let k = c.kit();
            assert!(super::super::magic::ability(k.abilities[0]).is_some(), "{}", k.name);
            assert_eq!(Class::from_name(&k.name.to_lowercase()), Some(c));
            assert!(k.art.is_some() || k.spell.is_some());
        }
        let book = super::super::skills::SkillBook::load(&super::super::skills::skills_path()).expect("skills");
        for c in Class::ALL {
            for (s, _) in c.kit().skills {
                assert!(book.get(s).is_some(), "{s}");
            }
        }
    }
}

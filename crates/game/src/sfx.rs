//! Sound: the audio layer of every feedback bundle. Reads sim events (contacts, action starts, landings,
//! drinks, pickups, distance walked) and plays Kenney CC0 cues from assets/audio (credits in
//! assets/audio/CREDITS.txt). Cues are tiered by impact like the camera shake: a jab ticks, a deathblow
//! booms. Pitch and variant are picked from the tick so repeats never sound identical. Presentation only.
use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;
use guildbound_sim::{ActionKind, Outcome, Source};

use crate::sim_bridge::Sim;

#[derive(Resource, Default)]
pub struct Sfx {
    cues: HashMap<String, Vec<Handle<AudioSource>>>,
    last_contact_tick: u64,
    last_tick: u64,
    seq: Vec<u32>,
    airborne: Vec<bool>,
    walked: Vec<f32>,
    last_pos: Vec<guildbound_sim::V2>,
    bag: Vec<usize>,
    hp: Vec<f32>,
    next_bird: f32,
}

#[derive(Resource)]
pub struct SfxSettings {
    pub volume: f32,
}

impl Default for SfxSettings {
    fn default() -> Self {
        Self { volume: 0.7 }
    }
}

/// Every cue name with its variants: synth `name_N.wav`, kenney `nameNNN.ogg` / `name_NNN.ogg`, combat `name.wav`.
pub fn load(mut commands: Commands, assets: Res<AssetServer>) {
    let mut cues: HashMap<String, Vec<Handle<AudioSource>>> = HashMap::new();
    for dir in ["synth", "kenney", "combat"] {
        let Ok(files) = std::fs::read_dir(format!("{}/assets/audio/{dir}", crate::ROOT)) else { continue };
        let mut names: Vec<String> = files.flatten().map(|f| f.file_name().to_string_lossy().to_string()).filter(|f| f.ends_with(".wav") || f.ends_with(".ogg")).collect();
        names.sort();
        for file in names {
            let stem = file.rsplit_once('.').map_or(file.as_str(), |s| s.0);
            let base = stem.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('_');
            let key = format!("{dir}/{base}");
            cues.entry(key).or_default().push(assets.load(format!("audio/{dir}/{file}")));
        }
    }
    commands.insert_resource(Sfx { cues, next_bird: 4.0, ..default() });
}

fn hash(a: u64, b: u64) -> u64 {
    let mut h = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h.wrapping_mul(0x1656_67B1_9E37_79F9)
}

impl Sfx {
    /// Play one variant of a cue; `salt` picks the variant and a +-7% pitch.
    fn play(&self, commands: &mut Commands, cue: &str, volume: f32, salt: u64) {
        let Some(list) = self.cues.get(cue).filter(|l| !l.is_empty()) else { return };
        let h = hash(salt, cue.len() as u64);
        let clip = list[(h % list.len() as u64) as usize].clone();
        let pitch = 0.93 + ((h >> 16) % 1000) as f32 / 1000.0 * 0.14;
        commands.spawn((AudioPlayer::new(clip), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(pitch)));
    }
}

fn kick(cluster: &str) -> bool {
    ["kick", "roundhouse", "sweep"].iter().any(|k| cluster.contains(k))
}

pub fn play(mut commands: Commands, sim: Res<Sim>, settings: Res<SfxSettings>, time: Res<Time>, state: Option<ResMut<Sfx>>) {
    let Some(mut sfx) = state else { return };
    let world = &sim.world;
    let n = world.fighters.len();
    if world.tick < sfx.last_tick || sfx.seq.len() != n {
        // reset or a new fighter: re-baseline without playing anything
        sfx.seq = world.fighters.iter().map(|f| f.action.seq).collect();
        sfx.airborne = world.fighters.iter().map(|f| f.height > 0.0).collect();
        sfx.walked = vec![0.0; n];
        sfx.last_pos = world.fighters.iter().map(|f| f.position).collect();
        sfx.bag = world.fighters.iter().map(|f| f.kit.bag.len()).collect();
        sfx.hp = world.fighters.iter().map(|f| f.hp).collect();
        sfx.last_contact_tick = world.tick;
        sfx.last_tick = world.tick;
        return;
    }
    let v = settings.volume;
    let player = world.fighters[0].position;
    // nearer the player = louder; the player's own sounds are full volume
    let near = |p: guildbound_sim::V2| 1.0 / (1.0 + (p - player).length() / 8.0);

    // ambience: a low wind bed and the odd bird
    if !sfx.cues.is_empty() && sfx.last_tick == 0 && world.tick > 0 {
        if let Some(wind) = sfx.cues.get("synth/ambience_wind").and_then(|l| l.first()) {
            commands.spawn((AudioPlayer::new(wind.clone()), PlaybackSettings::LOOP.with_volume(Volume::Linear(0.18 * v))));
        }
    }
    sfx.next_bird -= time.delta_secs();
    if sfx.next_bird <= 0.0 {
        sfx.play(&mut commands, "synth/bird", 0.12 * v, world.tick);
        sfx.next_bird = 6.0 + (hash(world.tick, 77) % 9000) as f32 / 1000.0;
    }

    // impacts, tiered
    let fresh: Vec<_> = sim.recent.iter().map(|(c, _)| *c).filter(|c| c.tick > sfx.last_contact_tick).collect();
    for c in &fresh {
        let at = world.fighters.get(c.defender).map_or(player, |f| f.position);
        let g = near(at) * v;
        let salt = c.tick ^ ((c.attacker as u64) << 20) ^ ((c.defender as u64) << 28);
        let heavy = matches!(c.source, Source::Move(m) if guildbound_sim::MOVES[m].heavy);
        match (c.outcome, c.source) {
            (Outcome::Deathblow, _) => {
                sfx.play(&mut commands, "kenney/impactPunch_heavy", 1.0 * g, salt);
                sfx.play(&mut commands, "synth/shadow_hit", 0.9 * g, salt + 1);
                sfx.play(&mut commands, "synth/clang", 0.5 * g, salt + 2);
            }
            (Outcome::Parried, _) => {
                sfx.play(&mut commands, "synth/parry", 0.9 * g, salt);
                sfx.play(&mut commands, "combat/parry", 0.5 * g, salt + 1);
            }
            (Outcome::GuardBroken, _) => {
                sfx.play(&mut commands, "synth/clang", 0.9 * g, salt);
                sfx.play(&mut commands, "kenney/impactPunch_heavy", 0.7 * g, salt + 1);
            }
            (Outcome::Blocked, _) => sfx.play(&mut commands, "synth/block", 0.65 * g, salt),
            (_, Source::Spell(s)) => {
                let cue = match guildbound_sim::SPELLS[s].element {
                    guildbound_sim::magic::Element::Lightning => "synth/lightning_hit",
                    guildbound_sim::magic::Element::Fire => "synth/fire_hit",
                    guildbound_sim::magic::Element::Frost => "synth/ice_hit",
                    _ => "synth/arcane_hit",
                };
                sfx.play(&mut commands, cue, 0.8 * g, salt);
            }
            (Outcome::CounterHit, _) | (Outcome::Hit, _) => {
                let big = heavy || c.outcome == Outcome::CounterHit || c.crit;
                sfx.play(&mut commands, if big { "kenney/impactPunch_heavy" } else { "kenney/impactPunch_medium" }, (if big { 0.95 } else { 0.75 }) * g, salt);
                sfx.play(&mut commands, "kenney/impactSoft_medium", 0.35 * g, salt + 3);
                if c.crit {
                    sfx.play(&mut commands, "combat/punch", 0.7 * g, salt + 4);
                }
            }
        }
    }
    if let Some(t) = fresh.iter().map(|c| c.tick).max() {
        sfx.last_contact_tick = t;
    }

    for (i, f) in world.fighters.iter().enumerate() {
        let g = near(f.position) * v;
        let salt = world.tick ^ ((i as u64) << 24);
        // action starts: swings, casts, dodges, posture break, drinking
        if f.action.seq != sfx.seq[i] {
            sfx.seq[i] = f.action.seq;
            match f.action.kind {
                ActionKind::Strike(m) => {
                    let d = &guildbound_sim::MOVES[m];
                    let cue = if kick(d.cluster) { "synth/swish_kick" } else if d.heavy { "synth/swish_heavy" } else { "synth/swish_light" };
                    sfx.play(&mut commands, cue, (if d.heavy { 0.6 } else { 0.4 }) * g, salt);
                }
                ActionKind::Cast(s) => {
                    let cue = match guildbound_sim::SPELLS[s].element {
                        guildbound_sim::magic::Element::Lightning => "synth/lightning_cast",
                        guildbound_sim::magic::Element::Fire => "synth/fire_cast",
                        guildbound_sim::magic::Element::Frost => "synth/ice_cast",
                        guildbound_sim::magic::Element::Holy => "synth/rank_up",
                        guildbound_sim::magic::Element::Arcane => "synth/arcane_cast",
                    };
                    sfx.play(&mut commands, cue, 0.6 * g, salt);
                }
                ActionKind::Charge(_) => sfx.play(&mut commands, "combat/charge", 0.35 * g, salt),
                ActionKind::Step => sfx.play(&mut commands, "combat/dash", 0.5 * g, salt),
                ActionKind::Roll => sfx.play(&mut commands, "kenney/cloth", 0.6 * g, salt),
                ActionKind::Dizzy => sfx.play(&mut commands, "synth/awaken", 0.7 * g, salt),
                ActionKind::Drink(_) => sfx.play(&mut commands, "kenney/clothBelt", 0.5 * g, salt),
                ActionKind::GetUp => sfx.play(&mut commands, "kenney/cloth", 0.35 * g, salt),
                _ => {}
            }
        }
        // jump take-off and landing
        let air = f.height > 0.0;
        if air != sfx.airborne[i] {
            sfx.airborne[i] = air;
            sfx.play(&mut commands, if air { "combat/whoosh" } else { "combat/land" }, 0.45 * g, salt + 9);
        }
        // a drink taking effect
        if matches!(f.action.kind, ActionKind::Drink(_)) && f.hp > sfx.hp[i] + 0.5 {
            sfx.play(&mut commands, "synth/rank_up", 0.45 * g, salt + 5);
        }
        sfx.hp[i] = f.hp;
        // pickups
        if f.kit.bag.len() > sfx.bag[i] {
            sfx.play(&mut commands, "kenney/handleCoins", 0.6 * g, salt + 6);
        }
        sfx.bag[i] = f.kit.bag.len();
        // footsteps: one every half stride of ground travel
        let moved = (f.position - sfx.last_pos[i]).length();
        sfx.last_pos[i] = f.position;
        if !air && f.hp > 0.0 && moved < 0.5 {
            sfx.walked[i] += moved;
            let half = if f.velocity.length() > 3.0 { 1.45 } else { 0.75 };
            if sfx.walked[i] >= half {
                sfx.walked[i] -= half;
                sfx.play(&mut commands, "synth/step_stone", 0.22 * g, salt + 7);
            }
        }
    }
    sfx.last_tick = world.tick;
}

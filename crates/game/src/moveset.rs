use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::{Value, json};

use crate::{anim::AnimLibrary, studio::Studio, ui_theme as theme};

const FILE: &str = "assets/moveset.json";
const DIRECTIONS: [&str; 8] = ["Forward", "Forward right", "Right", "Back right", "Back", "Back left", "Left", "Forward left"];

#[derive(Resource)]
pub struct Moveset {
    pub data: Value,
    dirty: bool,
    graph: Option<AssetId<AnimationGraph>>,
    category: usize,
    filter: String,
    clip_filter: String,
}

fn defaults() -> Value {
    let base: Value = serde_json::from_str(include_str!("../../../Tools/blender/hero_mapping.json")).expect("valid hero mapping");
    let mut roles = base["roles"].clone();
    roles["idle"] = json!("nc_idle");
    roles["landing"] = json!("ual_jump_land");
    for (key, name) in [("turn_left_90", "mx_turn_left_90"), ("turn_right_90", "mx_turn_right_90"), ("turn_left_180", "mx_turn_left_180"), ("turn_right_180", "mx_turn_right_180")] { roles[key] = json!(name); }
    for (key, name) in [("step_forward", "nc_run_forward"), ("step_back", "nc_run_backward"), ("step_left", "nc_run_left"), ("step_right", "nc_run_right")] {
        roles[key] = json!(name);
    }
    let mut locomotion = base["locomotion_relaxed"].clone();
    let run = locomotion["run"].clone();
    let sprint = locomotion["sprint"].clone();
    locomotion["sprint"] = json!([sprint[0], sprint[1], run[2], run[3], run[4], run[5], run[6], sprint[2]]);
    json!({"moves": base["combat"], "roles": roles, "locomotion": base["locomotion"], "locomotion_relaxed": locomotion, "spells": base["spells"],
        "weapons": {"light": ["u1_sword_attack"], "heavy": ["u2_sword_regular_c"], "idle": ["u1_sword_idle"], "guard": ["u2_sword_block"], "step": ["u2_sword_dash"]},
        "randomize_enemies": true, "enemy_seed": 1})
}

fn merge(base: &mut Value, saved: &Value) {
    if let (Some(dst), Some(src)) = (base.as_object_mut(), saved.as_object()) {
        for (key, value) in src {
            if let Some(at) = dst.get_mut(key) { merge(at, value); }
            else { dst.insert(key.clone(), value.clone()); }
        }
    } else if !saved.is_null() {
        *base = saved.clone();
    }
}

impl Default for Moveset {
    fn default() -> Self {
        let mut data = defaults();
        merge(&mut data, &crate::read_json(FILE));
        Self { data, dirty: true, graph: None, category: 0, filter: String::new(), clip_filter: String::new() }
    }
}

impl Moveset {
    pub fn role<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.data["roles"][key].as_str().filter(|s| !s.is_empty()).unwrap_or(fallback)
    }

    pub fn pick(&self, lib: &AnimLibrary, group: &str, key: &str, fighter: usize, seq: u32) -> Option<String> {
        let names: Vec<&str> = self.data[group][key].as_array()?.iter().filter_map(Value::as_str).filter(|n| lib.node(n).is_some()).collect();
        if names.is_empty() { return None; }
        let mut index = seq as usize % names.len();
        if fighter > 0 && self.data["randomize_enemies"].as_bool().unwrap_or(true) {
            let mut seed = self.data["enemy_seed"].as_u64().unwrap_or(1) ^ (fighter as u64).wrapping_mul(0x9e3779b97f4a7c15);
            for byte in key.bytes() { seed = (seed ^ byte as u64).wrapping_mul(0x100000001b3); }
            index = (seed as usize % names.len() + seq as usize % names.len().min(2)) % names.len();
        }
        Some(names[index].to_owned())
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        std::fs::write(format!("{}/{FILE}", crate::ROOT), serde_json::to_string_pretty(&self.data)?)?;
        self.dirty = true;
        Ok(())
    }
}

pub fn apply(mut config: ResMut<Moveset>, lib: Option<ResMut<AnimLibrary>>, mut loco: ResMut<crate::controller::LocomotionMap>) {
    let Some(mut lib) = lib else { return };
    let Some(graph) = lib.graph.as_ref().map(Handle::id) else { return };
    if !config.dirty && config.graph == Some(graph) { return; }
    macro_rules! role {
        ($($field:ident),*) => {$({
            if let Some(name) = config.data["roles"][stringify!($field)].as_str().filter(|n| lib.node(n).is_some()) {
                lib.roles.$field = Some(name.to_owned());
            }
        })*};
    }
    role!(idle, walk, run, back, hit, knockdown, getup, block, block_body, block_low, guard, spell_charge, hit_body, hit_low, jump, fall, roll, dizzy, drink, hit_back, death, death_backstab, guard_break, staggered, pickup);
    let clips: std::collections::HashSet<_> = lib.names().into_iter().collect();
    for (name, _, names) in &mut lib.clusters {
        if let Some(list) = config.data["moves"][*name].as_array() {
            let valid: Vec<_> = list.iter().filter_map(Value::as_str).filter(|n| clips.contains(*n)).map(str::to_owned).collect();
            if !valid.is_empty() { *names = valid; }
        }
    }
    loco.apply_mapping(&config.data);
    config.graph = Some(graph);
    config.dirty = false;
}

fn friendly(name: &str) -> String {
    name.replace('_', " ").replace('-', " ")
}

fn picker(ui: &mut egui::Ui, id: &str, clip: &mut String, names: &[String], search: &mut String, studio: &mut Studio) -> bool {
    let before = clip.clone();
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(id).selected_text(friendly(clip)).width(180.0).show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(search).hint_text("Find animation..."));
            let query = search.to_lowercase();
            for name in names.iter().filter(|n| n.to_lowercase().contains(&query)) {
                ui.selectable_value(clip, name.clone(), friendly(name));
            }
        });
        if ui.small_button("Preview").clicked() {
            studio.preview_clip(clip.clone());
        }
    });
    *clip != before
}

pub fn ui(ui: &mut egui::Ui, config: &mut Moveset, studio: &mut Studio, lib: Option<&AnimLibrary>) {
    theme::section(ui, "Player animation mapping");
    theme::hint(ui, "Choose animations for each action. Preview plays on the stage. Changes apply while you work; Save keeps them for next time.");
    let Some(lib) = lib else { ui.label("Loading animation library..."); return };
    let names = lib.names();
    theme::tabs(ui, &["Move", "Fight", "React", "Magic", "Weapons"], &mut config.category);
    if config.category == 1 {
        egui::CollapsingHeader::new("All combos & their controls").show(ui, crate::inventory::combo_guide);
    }
    ui.add(egui::TextEdit::singleline(&mut config.filter).hint_text("Find an action..."));
    ui.horizontal(|ui| {
        let mut random = config.data["randomize_enemies"].as_bool().unwrap_or(true);
        if ui.checkbox(&mut random, "Different movesets for enemies").changed() { config.data["randomize_enemies"] = json!(random); }
        if ui.small_button("Reshuffle").clicked() {
            config.data["enemy_seed"] = json!(config.data["enemy_seed"].as_u64().unwrap_or(1).wrapping_add(1));
        }
    });
    theme::hint(ui, "Enemies get their own selection from the fight animations you allow.");
    let filter = config.filter.to_lowercase();
    let role_keys: &[(&str, &str)] = match config.category {
        0 => &[("idle", "Standing"), ("jump", "Jump takeoff - Space"), ("fall", "Falling"), ("landing", "Landing"), ("roll", "Roll - C / fast landing"), ("step_forward", "Step forward - Ctrl"), ("step_back", "Step back - Ctrl + S"), ("step_left", "Step left - Ctrl + A"), ("step_right", "Step right - Ctrl + D"), ("turn_left_90", "Turn left"), ("turn_right_90", "Turn right"), ("turn_left_180", "Turn around left"), ("turn_right_180", "Turn around right"), ("pickup", "Pick up - E"), ("interact", "Interact / open chest - E"), ("drink", "Drink item - R / T")],
        2 => &[("guard", "Guard - RMB"), ("block", "Block head"), ("block_body", "Block body"), ("block_low", "Block legs"), ("hit", "Hit head"), ("hit_body", "Hit body"), ("hit_low", "Hit legs"), ("hit_back", "Hit from behind"), ("guard_break", "Guard broken"), ("staggered", "Parried"), ("dizzy", "Posture broken"), ("knockdown", "Knocked down"), ("getup", "Get up"), ("death", "Death"), ("death_backstab", "Backstab death")],
        3 => &[("spell_charge", "Hold to charge - 1")],
        _ => &[],
    };
    for &(key, label) in role_keys.iter().filter(|(_, label)| label.to_lowercase().contains(&filter)) {
        theme::section(ui, label);
        let mut clip = config.data["roles"][key].as_str().unwrap_or("").to_owned();
        if picker(ui, key, &mut clip, &names, &mut config.clip_filter, studio) { config.data["roles"][key] = json!(clip); config.dirty = true; }
    }
    if config.category == 0 {
        for (group, title) in [("locomotion_relaxed", "Exploring"), ("locomotion", "Combat movement")] {
            for gait in ["walk", "run", "sprint"] {
                if !config.data[group][gait].is_array() { continue; }
                egui::CollapsingHeader::new(format!("{title}: {gait}")).show(ui, |ui| {
                    for (i, direction) in DIRECTIONS.iter().enumerate() {
                        let label = format!("{gait} {direction}");
                        if !label.to_lowercase().contains(&filter) { continue; }
                        ui.label(direction.to_string());
                        let mut clip = config.data[group][gait][i].as_str().unwrap_or("").to_owned();
                        if picker(ui, &format!("{group}{gait}{i}"), &mut clip, &names, &mut config.clip_filter, studio) {
                            config.data[group][gait][i] = json!(clip); config.dirty = true;
                        }
                    }
                });
            }
        }
    }
    let group = match config.category { 1 => Some("moves"), 3 => Some("spells"), 4 => Some("weapons"), _ => None };
    if let Some(group) = group {
        let keys: Vec<String> = config.data[group].as_object().into_iter().flatten().map(|(k, _)| k.clone()).collect();
        for key in keys.into_iter().filter(|k| k.to_lowercase().contains(&filter)) {
            let control = match key.as_str() { "jab" => "LMB - string 1", "cross" => "LMB - string 2", "hook" => "LMB - string 3", "uppercut" => "LMB - string 4 / forward + F", "front kick" => "F / LMB > F", "low kick" => "LMB > LMB > F / backward + F", "side kick" => "sideways + F", "roundhouse" => "3 or 4 rush hits > F", "counter" => "LMB after parry", "run punch" => "LMB while running", _ => "" };
            egui::CollapsingHeader::new(format!("{}  {control}", friendly(&key))).id_salt(format!("{group}{key}")).default_open(group != "moves").show(ui, |ui| {
                let mut chosen: Vec<String> = config.data[group][&key].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
                let mut remove = None;
                for (i, name) in chosen.iter_mut().enumerate() {
                    if picker(ui, &format!("{group}{key}{i}"), name, &names, &mut config.clip_filter, studio) { config.dirty = true; }
                    if ui.small_button("Remove variant").clicked() { remove = Some(i); config.dirty = true; }
                }
                if let Some(i) = remove { if chosen.len() > 1 { chosen.remove(i); } }
                if ui.small_button("Add animation variant").clicked() {
                    chosen.push(names.first().cloned().unwrap_or_default()); config.dirty = true;
                }
                config.data[group][&key] = json!(chosen);
            });
        }
    }
    if config.category == 4 { theme::hint(ui, "Sword and axe use these one hand weapon animations. Equip a weapon in Inventory (I)."); }
    ui.separator();
    ui.horizontal(|ui| {
        if theme::button(ui, "Save mapping", false).clicked() {
            studio.status = match config.save() { Ok(()) => "Animation mapping saved and applied".into(), Err(e) => format!("Could not save mapping: {e}") };
        }
        if theme::button(ui, "Restore defaults", false).clicked() {
            config.data = defaults(); config.dirty = true;
            studio.status = "Default animations applied. Save to keep them.".into();
        }
    });
}

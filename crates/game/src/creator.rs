//! Character creator (Studio, Creator tab): build a character from the Quaternius modular parts by code.
//!
//! The Modular Character Outfits pack ships every outfit cut into slots (Head, Body, Arms, Legs, Feet,
//! accessories), all skinned to the same UAL skeleton as the Universal Base Characters, so any mix of parts
//! of one body type fits. The tab previews the pick live on the stage (the base body with each part on top),
//! Export runs Tools/blender/build_master.py headless (base head and the skin left uncovered, the chosen parts,
//! hair; bones renamed to the game's names; the skin under each part cut away) to assets/packs/custom/NAME.glb,
//! and the model is immediately on the stage, in the model list and wearable by the player (the hero clip
//! library plays on it: same skeleton, same bone names). Check runs Tools/blender/model_check.py: a data-only
//! report (bones, skin weights, body coverage, uncovered cut seams, size) that Send posts to Claude.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use bevy_egui::egui;
use serde_json::{Value, json};

use crate::studio::{STAGE, Studio};
use crate::ui_theme as theme;

const BLENDER: &str = "D:/blender.exe";
const STAGED: &str = "packs/_sources/chars_staged";
const SLOTS: [&str; 5] = ["Head", "Body", "Arms", "Legs", "Feet"];
const WORN_FILE: &str = "assets/player_model.json";

#[derive(Clone)]
struct Part {
    name: String,
    female: bool,
    outfit: String,
    slot: String,
}

#[derive(Default)]
struct Job {
    running: bool,
    log: Vec<String>,
    /// (succeeded, model path under assets/, was an export)
    done: Option<(bool, String, bool)>,
}

#[derive(Resource)]
pub struct Creator {
    parts: Vec<Part>,
    hair: Vec<String>,
    female: bool,
    /// slot -> part name; empty = the base body's own skin
    pick: BTreeMap<String, String>,
    acc: BTreeSet<String>,
    hair_pick: String,
    name: String,
    show_base: bool,
    preview: Vec<Entity>,
    dirty: bool,
    /// The creator's preview is on the stage (instead of a Studio model).
    active: bool,
    job: Arc<Mutex<Job>>,
    report: Value,
    report_for: String,
    seed: u64,
    /// Colour multiplied over each slot's texture (sRGB): Head, Body, Arms, Legs, Feet, Acc, Hair, Skin.
    tints: BTreeMap<String, [u8; 3]>,
}

impl Default for Creator {
    fn default() -> Self {
        Self {
            parts: Vec::new(), hair: Vec::new(), female: false, pick: BTreeMap::new(), acc: BTreeSet::new(),
            hair_pick: "Hair_SimpleParted".into(), name: "my_character".into(), show_base: true, preview: Vec::new(), dirty: false,
            active: false, job: Arc::default(), report: Value::Null, report_for: String::new(), seed: 0x9e37_79b9_7f4a_7c15,
            tints: BTreeMap::new(),
        }
    }
}

/// The player model choice: a custom model worn by fighter 0 with the hero clip library.
#[derive(Resource, Default)]
pub struct Wear {
    pub model: Option<String>,
}

/// The model a player rig was spawned with (None = the default hero).
#[derive(Component)]
struct Worn(Option<String>);

/// A preview piece and the tint slot it belongs to.
#[derive(Component)]
struct Piece(String);

/// A preview mesh that has its own material copy (so tinting one part leaves the shared atlas alone).
#[derive(Component)]
struct OwnMaterial;

const TINT_SLOTS: [&str; 8] = ["Skin", "Hair", "Head", "Body", "Arms", "Legs", "Feet", "Acc"];

pub fn plugin(app: &mut App) {
    let worn = crate::read_json(WORN_FILE)["model"].as_str().map(str::to_owned);
    app.init_resource::<Creator>()
        .insert_resource(Wear { model: worn.filter(|m| std::path::Path::new(&format!("{}/assets/{m}", crate::ROOT)).exists()) })
        .add_systems(Update, (preview, tint_preview, finish, wear));
}

fn abs(rel: &str) -> String {
    format!("{}/assets/{rel}", crate::ROOT)
}

/// Copy the Modular Parts folder to a bracket-free place once (Blender's glTF importer rejects '[' in paths).
fn stage_parts() {
    let dst = std::path::PathBuf::from(abs(&format!("{STAGED}/parts")));
    if dst.is_dir() {
        return;
    }
    let root = std::path::PathBuf::from(abs("packs/_sources/quaternius/Modular_Character_Outfits"));
    let src = std::fs::read_dir(&root).into_iter().flatten().flatten().filter_map(|pack| {
        std::fs::read_dir(pack.path().join("Exports")).ok()?.flatten().find(|d| d.file_name().to_string_lossy().starts_with("glTF")).map(|d| d.path().join("Modular Parts"))
    }).find(|p| p.is_dir());
    let Some(src) = src else { return };
    let _ = std::fs::create_dir_all(&dst);
    for f in std::fs::read_dir(src).into_iter().flatten().flatten() {
        let _ = std::fs::copy(f.path(), dst.join(f.file_name()));
    }
}

fn scan(c: &mut Creator) {
    stage_parts();
    let stems = |dir: &str| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(abs(dir)).into_iter().flatten().flatten().filter_map(|f| f.file_name().to_string_lossy().strip_suffix(".gltf").map(str::to_owned)).collect();
        v.sort();
        v
    };
    c.parts = stems(&format!("{STAGED}/parts")).into_iter().map(|name| {
        let w: Vec<&str> = name.split('_').collect();
        Part { female: w[0] == "Female", outfit: w.get(1).copied().unwrap_or("").to_owned(), slot: w.get(2).copied().unwrap_or("").to_owned(), name }
    }).collect();
    c.hair = stems(&format!("{STAGED}/hair")).into_iter().filter(|h| h.starts_with("Hair_")).collect();
    if c.pick.is_empty() {
        outfit(c, "Ranger");
        c.pick.insert("Head".into(), String::new());
    }
}

/// Pick every slot (and accessory) of one outfit for the current body type.
fn outfit(c: &mut Creator, name: &str) {
    c.pick.clear();
    c.acc.clear();
    for p in c.parts.iter().filter(|p| p.female == c.female && p.outfit == name) {
        if p.slot == "Acc" {
            c.acc.insert(p.name.clone());
        } else {
            c.pick.insert(p.slot.clone(), p.name.clone());
        }
    }
    c.dirty = true;
}

fn chosen(c: &Creator) -> Vec<String> {
    SLOTS.iter().filter_map(|s| c.pick.get(*s).filter(|p| !p.is_empty()).cloned()).chain(c.acc.iter().cloned()).collect()
}

fn rand(c: &mut Creator) -> u64 {
    c.seed ^= c.seed << 13;
    c.seed ^= c.seed >> 7;
    c.seed ^= c.seed << 17;
    c.seed
}

fn randomize(c: &mut Creator) {
    for slot in SLOTS {
        let opts: Vec<String> = c.parts.iter().filter(|p| p.female == c.female && p.slot == slot).map(|p| p.name.clone()).collect();
        // the head slot (hoods) stays optional; body slots are always covered
        let n = opts.len() + usize::from(slot == "Head");
        let i = (rand(c) % n.max(1) as u64) as usize;
        c.pick.insert(slot.into(), opts.get(i).cloned().unwrap_or_default());
    }
    c.acc.clear();
    let accs: Vec<String> = c.parts.iter().filter(|p| p.female == c.female && p.slot == "Acc").map(|p| p.name.clone()).collect();
    for a in accs {
        if rand(c) % 2 == 0 {
            c.acc.insert(a);
        }
    }
    let hn = c.hair.len() + 1;
    let h = (rand(c) % hn as u64) as usize;
    c.hair_pick = c.hair.get(h).cloned().unwrap_or_else(|| "none".into());
    c.dirty = true;
}

fn spawn_job(job: &Arc<Mutex<Job>>, steps: Vec<(String, Vec<String>)>, model: String, export: bool) {
    {
        let mut j = job.lock().unwrap();
        j.running = true;
        j.log.clear();
        j.done = None;
    }
    let job = job.clone();
    std::thread::spawn(move || {
        let mut ok = true;
        for (label, args) in steps {
            job.lock().unwrap().log.push(format!("{label} ..."));
            let mut cmd = std::process::Command::new(BLENDER);
            cmd.args(&args).current_dir(crate::ROOT);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // no console window
            }
            match cmd.output() {
                Ok(o) => {
                    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
                    let mut j = job.lock().unwrap();
                    for line in text.lines().filter(|l| l.contains("HERO_DONE") || l.contains("CHECK_DONE") || l.contains("PART_MISSING") || l.contains("Error") || l.contains("Traceback")) {
                        j.log.push(line.chars().take(160).collect());
                    }
                    if !o.status.success() || text.contains("Traceback") {
                        ok = false;
                        j.log.push(format!("{label} failed (exit {:?})", o.status.code()));
                        break;
                    }
                }
                Err(e) => {
                    job.lock().unwrap().log.push(format!("could not start Blender at {BLENDER}: {e}"));
                    ok = false;
                    break;
                }
            }
        }
        let mut j = job.lock().unwrap();
        j.running = false;
        j.done = Some((ok, model, export));
    });
}

fn blender_args(script: &str, extra: Vec<String>) -> Vec<String> {
    let mut v = vec!["-b".into(), "--factory-startup".into(), "--python-exit-code".into(), "1".into(), "-P".into(), format!("Tools/blender/{script}"), "--".into()];
    v.extend(extra);
    v
}

fn check_args(model: &str) -> (String, Vec<String>) {
    ("Data check".into(), blender_args("model_check.py", vec!["--glb".into(), format!("assets/{model}")]))
}

fn export(c: &mut Creator) {
    let name: String = c.name.trim().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' { ch.to_ascii_lowercase() } else { '_' }).collect();
    let name = if name.is_empty() { "custom".to_owned() } else { name };
    let model = format!("packs/custom/{name}.glb");
    let build = blender_args("build_master.py", vec![
        "--base".into(), if c.female { "female" } else { "male" }.into(),
        "--parts".into(), chosen(c).join(","),
        "--hair".into(), c.hair_pick.clone(),
        "--tints".into(), c.tints.iter().filter(|(_, v)| **v != [255, 255, 255]).map(|(k, v)| format!("{k}={:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect::<Vec<_>>().join(","),
        "--set".into(), "none".into(),
        "--out".into(), format!("assets/{model}"),
    ]);
    spawn_job(&c.job, vec![("Export (Blender, headless)".into(), build), check_args(&model)], model, true);
}

fn read_report(model: &str) -> Value {
    crate::read_json(&format!("assets/{model}.check.json"))
}

/// Load a custom model's recipe back into the picker (from its build report).
fn load_recipe(c: &mut Creator, model: &str) {
    let r = crate::read_json(&format!("assets/{model}.report.json"));
    if r.is_null() {
        return;
    }
    c.female = r["base"] == "female";
    c.pick.clear();
    c.acc.clear();
    for p in r["parts"].as_array().into_iter().flatten().filter_map(|p| p.as_str()) {
        if let Some(part) = c.parts.iter().find(|x| x.name == p) {
            if part.slot == "Acc" {
                c.acc.insert(p.to_owned());
            } else {
                c.pick.insert(part.slot.clone(), p.to_owned());
            }
        }
    }
    c.hair_pick = r["hair"].as_str().unwrap_or("none").to_owned();
    c.tints.clear();
    for t in r["tints"].as_str().unwrap_or("").split(',') {
        if let Some((k, h)) = t.split_once('=')
            && let Ok(v) = u32::from_str_radix(h, 16)
        {
            c.tints.insert(k.to_owned(), [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        }
    }
    c.name = model.trim_start_matches("packs/custom/").trim_end_matches(".glb").to_owned();
    c.dirty = true;
}

/// Rebuild the stage preview when the pick changed; give the stage back when a Studio model is loaded.
fn preview(mut commands: Commands, mut c: ResMut<Creator>, mut studio: ResMut<Studio>, assets: Res<AssetServer>) {
    if !studio.open {
        return;
    }
    if c.parts.is_empty() {
        scan(&mut c);
    }
    if studio.entity.is_some() && !c.preview.is_empty() && !c.dirty {
        for e in c.preview.drain(..) {
            commands.entity(e).despawn();
        }
        c.active = false;
        return;
    }
    if !(c.active && c.dirty) {
        return;
    }
    c.dirty = false;
    for e in c.preview.drain(..) {
        commands.entity(e).despawn();
    }
    if let Some(e) = studio.entity.take() {
        commands.entity(e).despawn();
    }
    studio.model = None;
    studio.status = "Creator preview: the base body shows under the parts; Export cuts it away where a part covers it".into();
    let base = if c.female { "Superhero_Female_FullBody" } else { "Superhero_Male_FullBody" };
    let slot_of = |p: &str| c.parts.iter().find(|x| x.name == p).map_or(String::new(), |x| x.slot.clone());
    let mut files: Vec<(String, String)> = chosen(&c).into_iter().map(|p| (format!("{STAGED}/parts/{p}.gltf"), slot_of(&p))).collect();
    if c.show_base {
        files.push((format!("{STAGED}/base/{base}.gltf"), "Skin".into()));
    }
    if c.hair_pick != "none" {
        files.push((format!("{STAGED}/hair/{}.gltf", c.hair_pick), "Hair".into()));
    }
    let at = Transform::from_translation(STAGE).with_rotation(Quat::from_rotation_y(studio.yaw.to_radians()));
    for (f, slot) in files {
        let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(f));
        c.preview.push(commands.spawn((Name::new("creator preview"), WorldAssetRoot(scene), at, Piece(slot))).id());
    }
}

/// Live tint on the preview: each mesh gets its own material copy once, then its base colour follows the pick.
/// On the base body only the skin mesh takes the skin tint (eyes and brows keep theirs).
#[allow(clippy::type_complexity)]
fn tint_preview(
    mut commands: Commands,
    c: Res<Creator>,
    pieces: Query<(Entity, &Piece)>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut mats: Query<(&mut MeshMaterial3d<StandardMaterial>, Has<OwnMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (root, piece) in &pieces {
        let rgb = c.tints.get(&piece.0).copied().unwrap_or([255, 255, 255]);
        let color = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
        for e in children.iter_descendants(root) {
            let Ok((mut m, own)) = mats.get_mut(e) else { continue };
            if piece.0 == "Skin" {
                // the mesh node is the primitive's parent and carries the mesh name
                let named = |x: Entity| names.get(x).is_ok_and(|n| n.as_str().starts_with("SuperHero"));
                if !(named(e) || parents.get(e).is_ok_and(|p| named(p.parent()))) {
                    continue;
                }
            }
            if !own {
                let Some(copy) = materials.get(&m.0).cloned() else { continue };
                m.0 = materials.add(copy);
                commands.entity(e).insert(OwnMaterial);
            }
            if materials.get(&m.0).is_some_and(|x| x.base_color != color)
                && let Some(mut x) = materials.get_mut(&m.0)
            {
                x.base_color = color;
            }
        }
    }
}

/// A finished export or check: show the report; an export goes on the stage and into the model list.
fn finish(mut c: ResMut<Creator>, mut studio: ResMut<Studio>) {
    let done = c.job.lock().unwrap().done.take();
    let Some((ok, model, export)) = done else { return };
    c.report = read_report(&model);
    c.report_for = model.clone();
    if ok && export {
        studio.add_asset(model.clone());
        c.active = false;
        studio.request_spawn(model);
    }
}

/// Fighter 0 wears the chosen custom model (re-applied whenever its rig is respawned).
fn wear(mut commands: Commands, w: Res<Wear>, assets: Res<AssetServer>, rigs: Query<(Entity, &crate::anim::FighterRig, Option<&Worn>)>, views: Query<(Entity, &crate::sim_bridge::FighterView)>) {
    let Some((rig, _, worn)) = rigs.iter().find(|(_, r, _)| r.fighter == 0) else { return };
    // a rig without Worn was spawned by the game with the default hero
    if worn.map_or(w.model.is_none(), |x| x.0 == w.model) {
        return;
    }
    let Some((view, _)) = views.iter().find(|(_, v)| v.0 == 0) else { return };
    commands.entity(rig).despawn();
    let path = w.model.clone().unwrap_or_else(|| "packs/hero/fighter.glb".into());
    let e = crate::anim::spawn_rig(&mut commands, view, 0, assets.load(GltfAssetLabel::Scene(0).from_asset(path)));
    commands.entity(e).insert(Worn(w.model.clone()));
}

fn set_worn(w: &mut Wear, model: Option<String>) {
    let _ = std::fs::write(format!("{}/{WORN_FILE}", crate::ROOT), serde_json::to_string_pretty(&json!({"model": model})).unwrap_or_default());
    w.model = model;
}

fn verdict_color(report: &Value) -> (egui::Color32, String) {
    let flags: Vec<&str> = report["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).collect();
    if flags.iter().any(|f| f.starts_with("missing") || f.starts_with("no geometry") || f.contains("no skin weight")) {
        (theme::BAD, format!("{} flags, structural", flags.len()))
    } else if flags.is_empty() {
        (theme::GOOD, "clean".into())
    } else {
        (theme::WARN, format!("{} flags", flags.len()))
    }
}

pub fn tab(ui: &mut egui::Ui, c: &mut Creator, studio: &mut Studio, wear: &mut Wear, feed: &mut crate::feed::DevFeed) {
    let running = c.job.lock().unwrap().running;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Character creator").size(16.0).strong().color(theme::PAPER));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::button(ui, if c.active { "Previewing" } else { "Preview on stage" }, c.active).clicked() {
                c.active = true;
                c.dirty = true;
            }
        });
    });
    ui.label(egui::RichText::new("Quaternius modular parts on one skeleton: pick a body type, then a part per slot. Parts of one body type always fit together.").size(11.5).color(theme::MUTED));

    theme::section(ui, "Body");
    let before = (c.female, c.pick.clone(), c.acc.clone(), c.hair_pick.clone(), c.show_base);
    ui.horizontal(|ui| {
        if theme::button(ui, "Male", !c.female).clicked() && c.female {
            c.female = false;
            outfit(c, "Ranger");
        }
        if theme::button(ui, "Female", c.female).clicked() && !c.female {
            c.female = true;
            outfit(c, "Ranger");
        }
        ui.checkbox(&mut c.show_base, "base body in preview");
    });
    let outfits: BTreeSet<String> = c.parts.iter().filter(|p| p.female == c.female).map(|p| p.outfit.clone()).collect();
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Full outfit:").size(12.0).color(theme::MUTED));
        for o in &outfits {
            if theme::button(ui, o, false).clicked() {
                outfit(c, o);
            }
        }
        if theme::button(ui, "Randomize", false).clicked() {
            randomize(c);
        }
    });

    theme::section(ui, "Parts by slot");
    for slot in SLOTS {
        let opts: Vec<Part> = c.parts.iter().filter(|p| p.female == c.female && p.slot == slot).cloned().collect();
        let cur = c.pick.get(slot).cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.add_sized([56.0, 22.0], egui::Label::new(egui::RichText::new(slot).size(12.5).color(theme::PAPER)));
            let none = if slot == "Head" { "none (hair shows)" } else { "base skin" };
            let mut pick = cur.clone();
            if ui.selectable_label(pick.is_empty(), none).clicked() {
                pick.clear();
            }
            for p in &opts {
                let label = p.name.split('_').skip(1).filter(|w| *w != slot).collect::<Vec<_>>().join(" ");
                if ui.selectable_label(pick == p.name, label).clicked() {
                    pick = p.name.clone();
                }
            }
            if pick != cur {
                c.pick.insert(slot.into(), pick);
            }
        });
    }
    let accs: Vec<Part> = c.parts.iter().filter(|p| p.female == c.female && p.slot == "Acc").cloned().collect();
    if !accs.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_sized([56.0, 22.0], egui::Label::new(egui::RichText::new("Extras").size(12.5).color(theme::PAPER)));
            for p in &accs {
                let mut on = c.acc.contains(&p.name);
                if ui.checkbox(&mut on, p.name.split('_').skip(3).collect::<Vec<_>>().join(" ")).changed() {
                    if on {
                        c.acc.insert(p.name.clone());
                    } else {
                        c.acc.remove(&p.name);
                    }
                }
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.add_sized([56.0, 22.0], egui::Label::new(egui::RichText::new("Hair").size(12.5).color(theme::PAPER)));
        if ui.selectable_label(c.hair_pick == "none", "none").clicked() {
            c.hair_pick = "none".into();
        }
        for h in c.hair.clone() {
            if ui.selectable_label(c.hair_pick == h, h.trim_start_matches("Hair_")).clicked() {
                c.hair_pick = h;
            }
        }
    });
    if before != (c.female, c.pick.clone(), c.acc.clone(), c.hair_pick.clone(), c.show_base) {
        c.dirty = true;
        c.active = true;
    }
    theme::section(ui, "Colours");
    ui.label(egui::RichText::new("Multiplied over each slot's texture; white keeps the original.").size(11.0).color(theme::MUTED));
    ui.horizontal_wrapped(|ui| {
        for slot in TINT_SLOTS {
            let mut rgb = c.tints.get(slot).copied().unwrap_or([255, 255, 255]);
            ui.label(egui::RichText::new(slot).size(12.0).color(theme::PAPER));
            if egui::widgets::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
                c.tints.insert(slot.into(), rgb);
            }
            ui.add_space(6.0);
        }
    });
    if theme::button(ui, "Reset colours", false).clicked() {
        c.tints.clear();
    }

    let uncovered: Vec<&str> = SLOTS[1..].iter().copied().filter(|s| c.pick.get(*s).is_none_or(|p| p.is_empty())).collect();
    if !uncovered.is_empty() {
        ui.label(egui::RichText::new(format!("Base skin shows on: {}", uncovered.join(", "))).size(11.5).color(theme::MUTED));
    }

    theme::section(ui, "Export");
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(170.0));
        ui.add_enabled_ui(!running, |ui| {
            if theme::button(ui, "Export model", true).clicked() {
                export(c);
            }
        });
    });
    ui.label(egui::RichText::new(format!("Saves assets/packs/custom/{}.glb, then runs the data check. About a minute.", c.name.trim())).size(11.0).color(theme::MUTED));
    {
        let j = c.job.lock().unwrap();
        if j.running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new(j.log.last().cloned().unwrap_or_default()).size(12.0).color(theme::TEAL));
            });
        }
        for l in j.log.iter().rev().take(6).rev() {
            ui.label(egui::RichText::new(l).size(11.0).monospace().color(theme::MUTED));
        }
    }

    theme::section(ui, "Custom models");
    let customs: Vec<String> = studio.assets_in("packs/custom/");
    if customs.is_empty() {
        ui.label(egui::RichText::new("None yet.").color(theme::MUTED));
    }
    for m in customs {
        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                let short = m.trim_start_matches("packs/custom/").to_owned();
                ui.label(egui::RichText::new(&short).strong().color(theme::PAPER));
                if wear.model.as_deref() == Some(m.as_str()) {
                    theme::badge(ui, "worn by player", theme::TEAL);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if theme::button(ui, "Stage", studio.model.as_deref() == Some(m.as_str())).clicked() {
                    c.active = false;
                    studio.request_spawn(m.clone());
                }
                if theme::button(ui, "Edit recipe", false).clicked() {
                    load_recipe(c, &m);
                    c.active = true;
                }
                let worn = wear.model.as_deref() == Some(m.as_str());
                if theme::button(ui, if worn { "Take off" } else { "Wear (player)" }, false).clicked() {
                    set_worn(wear, if worn { None } else { Some(m.clone()) });
                }
                ui.add_enabled_ui(!running, |ui| {
                    if theme::button(ui, "Check", false).clicked() {
                        spawn_job(&c.job, vec![check_args(&m)], m.clone(), false);
                    }
                });
                if theme::button(ui, "Show report", c.report_for == m).clicked() {
                    c.report = read_report(&m);
                    c.report_for = m.clone();
                }
            });
        });
    }

    if !c.report_for.is_empty() {
        theme::section(ui, &format!("Data check: {}", c.report_for.trim_start_matches("packs/custom/")));
        let r = c.report.clone();
        if r.is_null() {
            ui.label(egui::RichText::new("No report yet: press Check.").color(theme::MUTED));
            return;
        }
        let (color, text) = verdict_color(&r);
        ui.horizontal(|ui| {
            theme::badge(ui, &text, color);
            if theme::button(ui, "Send to Claude", false).clicked() {
                let flags: Vec<String> = r["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned)).collect();
                let summary = format!("MODEL CHECK {} for a good/okay/bad call: height {} m, {} tris, {} MB, coverage {}, flags: {}. Full report assets/{}.check.json",
                    c.report_for, r["height_m"], r["tris"], r["mb"], r["coverage"], if flags.is_empty() { "none".into() } else { flags.join(" | ") }, c.report_for);
                feed.post("boss", &summary, 0);
                studio.status = "Check sent to Claude (collab/devfeed.log)".into();
            }
        });
        theme::stat(ui, "Height", format!("{} m", r["height_m"]), theme::PAPER);
        theme::stat(ui, "Feet off floor", format!("{} cm", r["feet_off_floor_cm"]), theme::PAPER);
        theme::stat(ui, "Triangles", format!("{}", r["tris"]), theme::PAPER);
        theme::stat(ui, "File", format!("{} MB, {} materials, {} images", r["mb"], r["materials"], r["images"]), theme::PAPER);
        theme::stat(ui, "Bones", format!("{} ({} game bones missing)", r["bones"], r["missing_bones"].as_array().map_or(0, |a| a.len())), theme::PAPER);
        let s = &r["skin"];
        theme::stat(ui, "Skin", format!("{} unweighted, {} sums off, {} stray", s["unweighted_verts"], s["weight_sum_off"], s["stray_bone_verts"]), theme::PAPER);
        if let Some(cov) = r["coverage"].as_object() {
            theme::stat(ui, "Coverage (verts)", cov.iter().filter(|(k, _)| *k != "other").map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join("  "), theme::PAPER);
        }
        if let Some(seams) = r["build"]["seams"].as_object() {
            let holes: u64 = seams.get("uncovered").and_then(|u| u.as_object()).map_or(0, |u| u.values().filter_map(|v| v.as_u64()).sum());
            theme::stat(ui, "Cut seams uncovered", format!("{holes} of {}", seams.get("cut_edge_verts").cloned().unwrap_or(Value::Null)), if holes > 10 { theme::WARN } else { theme::GOOD });
        }
        for f in r["flags"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) {
            ui.label(egui::RichText::new(format!("- {f}")).size(12.0).color(theme::WARN));
        }
    }
}

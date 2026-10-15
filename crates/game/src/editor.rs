//! World editor (Tab panel, World tab): fly the camera freely (noclip, in camera.rs) and place props on the
//! ground. A placement follows the mouse over the terrain; left click drops it, Q/E turn it, the wheel resizes
//! it, right click stops placing. Click a placed object to select it; Delete removes it. Placements are saved to
//! assets/world/placed.json and loaded at start, solid ones with a convex collision hull.
use avian3d::prelude::*;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot};
use bevy_egui::egui;
use serde_json::{Value, json};

use crate::ui_theme as theme;

const FILE: &str = "assets/world/placed.json";

#[derive(Clone)]
pub struct Placed {
    pub path: String,
    pub pos: Vec2,
    pub yaw: f32,
    pub scale: f32,
    pub solid: bool,
    entity: Option<Entity>,
}

#[derive(Resource, Default)]
pub struct WorldEdit {
    pub placed: Vec<Placed>,
    /// The prop being placed (follows the mouse), its turn and size.
    placing: Option<String>,
    ghost: Option<Entity>,
    ghost_yaw: f32,
    ghost_scale: f32,
    selected: Option<usize>,
    filter: String,
    /// Unsaved changes.
    dirty: bool,
}

#[derive(Component)]
struct Ghost;

pub fn plugin(app: &mut App) {
    app.insert_resource(load()).add_systems(PostStartup, spawn_all).add_systems(Update, (place, draw));
}

fn load() -> WorldEdit {
    let v = crate::read_json(FILE);
    let placed = v["placed"].as_array().into_iter().flatten().filter_map(|p| Some(Placed {
        path: p["path"].as_str()?.to_owned(),
        pos: Vec2::new(p["x"].as_f64()? as f32, p["z"].as_f64()? as f32),
        yaw: p["yaw"].as_f64().unwrap_or(0.0) as f32,
        scale: p["scale"].as_f64().unwrap_or(1.0) as f32,
        solid: p["solid"].as_bool().unwrap_or(true),
        entity: None,
    })).collect();
    WorldEdit { placed, ghost_scale: 1.0, ..default() }
}

fn save(e: &mut WorldEdit) {
    let list: Vec<Value> = e.placed.iter().map(|p| json!({"path": p.path, "x": p.pos.x, "z": p.pos.y, "yaw": p.yaw, "scale": p.scale, "solid": p.solid})).collect();
    let _ = std::fs::create_dir_all(format!("{}/assets/world", crate::ROOT));
    let _ = std::fs::write(format!("{}/{FILE}", crate::ROOT), serde_json::to_string_pretty(&json!({"placed": list})).unwrap_or_default());
    e.dirty = false;
}

fn transform(pos: Vec2, yaw: f32, scale: f32) -> Transform {
    Transform::from_xyz(pos.x, crate::terrain::ground_at(Vec3::new(pos.x, 0.0, pos.y)), pos.y).with_rotation(Quat::from_rotation_y(yaw.to_radians())).with_scale(Vec3::splat(scale))
}

fn spawn_one(commands: &mut Commands, assets: &AssetServer, p: &Placed) -> Entity {
    let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(p.path.clone()));
    let mut e = commands.spawn((Name::new("placed"), WorldAssetRoot(scene), transform(p.pos, p.yaw, p.scale)));
    if p.solid {
        e.insert((RigidBody::Static, ColliderConstructorHierarchy::new(ColliderConstructor::ConvexHullFromMesh)));
    }
    e.id()
}

fn spawn_all(mut commands: Commands, assets: Res<AssetServer>, mut e: ResMut<WorldEdit>) {
    for p in &mut e.placed {
        p.entity = Some(spawn_one(&mut commands, &assets, p));
    }
}

/// Where the cursor ray meets the terrain (marched in 0.5 m steps, then refined).
fn cursor_ground(window: &Window, cam: (&Camera, &GlobalTransform)) -> Option<Vec3> {
    let ray = cam.0.viewport_to_world(cam.1, window.cursor_position()?).ok()?;
    let under = |t: f32| {
        let p = ray.origin + *ray.direction * t;
        p.y - crate::terrain::ground_at(p)
    };
    let mut t = 0.0;
    while t < 400.0 {
        if under(t + 0.5) < 0.0 {
            let (mut a, mut b) = (t, t + 0.5);
            for _ in 0..12 {
                let m = 0.5 * (a + b);
                if under(m) < 0.0 { b = m } else { a = m }
            }
            return Some(ray.origin + *ray.direction * b);
        }
        t += 0.5;
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn place(
    mut commands: Commands,
    mut e: ResMut<WorldEdit>,
    assets: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    wants: Res<bevy_egui::input::EguiWantsInput>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    cam: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    mut ghosts: Query<&mut Transform, With<Ghost>>,
    panel: Res<crate::panel::PanelState>,
    state: Res<State<crate::menu::AppState>>,
    studio: Res<crate::studio::Studio>,
) {
    let e = &mut *e;
    if !panel.editing_world() || *state.get() != crate::menu::AppState::Playing || studio.open {
        if e.placing.is_some() {
            stop_placing(&mut commands, e);
        }
        return;
    }
    if !window.focused || wants.wants_any_keyboard_input() {
        return;
    }
    let free = !wants.wants_any_pointer_input();
    let hit = cursor_ground(&window, (cam.0, cam.1));
    if let Some(path) = e.placing.clone() {
        if e.ghost.is_none() {
            let scene: Handle<WorldAsset> = assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone()));
            e.ghost = Some(commands.spawn((Name::new("placement"), Ghost, WorldAssetRoot(scene), Transform::default())).id());
        }
        if keys.just_pressed(KeyCode::KeyQ) {
            e.ghost_yaw += 15.0;
        }
        if keys.just_pressed(KeyCode::KeyE) {
            e.ghost_yaw -= 15.0;
        }
        if free && scroll.delta.y != 0.0 {
            e.ghost_scale = (e.ghost_scale * (1.0 + 0.1 * scroll.delta.y.signum())).clamp(0.1, 10.0);
        }
        if let (Some(g), Some(h)) = (e.ghost, hit)
            && let Ok(mut t) = ghosts.get_mut(g)
        {
            *t = transform(Vec2::new(h.x, h.z), e.ghost_yaw, e.ghost_scale);
        }
        if free && mouse.just_pressed(MouseButton::Left) && let Some(h) = hit {
            let mut p = Placed { path, pos: Vec2::new(h.x, h.z), yaw: e.ghost_yaw, scale: e.ghost_scale, solid: true, entity: None };
            p.entity = Some(spawn_one(&mut commands, &assets, &p));
            e.placed.push(p);
            e.selected = Some(e.placed.len() - 1);
            e.dirty = true;
        }
        if mouse.just_pressed(MouseButton::Right) {
            stop_placing(&mut commands, e);
        }
        return;
    }
    // select the placed object nearest the click (within 2 m of where the cursor meets the ground)
    if free && mouse.just_pressed(MouseButton::Left) && let Some(h) = hit {
        let near = e.placed.iter().enumerate().map(|(i, p)| (i, p.pos.distance(Vec2::new(h.x, h.z)))).filter(|x| x.1 < 2.0).min_by(|a, b| a.1.total_cmp(&b.1));
        e.selected = near.map(|x| x.0);
    }
    if let Some(i) = e.selected.filter(|&i| i < e.placed.len()) {
        if keys.just_pressed(KeyCode::Delete) {
            remove(&mut commands, e, i);
        } else if keys.just_pressed(KeyCode::KeyQ) || keys.just_pressed(KeyCode::KeyE) {
            e.placed[i].yaw += if keys.just_pressed(KeyCode::KeyQ) { 15.0 } else { -15.0 };
            respawn(&mut commands, &assets, e, i);
        }
    }
}

fn stop_placing(commands: &mut Commands, e: &mut WorldEdit) {
    e.placing = None;
    if let Some(g) = e.ghost.take() {
        commands.entity(g).despawn();
    }
}

fn remove(commands: &mut Commands, e: &mut WorldEdit, i: usize) {
    if let Some(x) = e.placed.remove(i).entity {
        commands.entity(x).despawn();
    }
    e.selected = None;
    e.dirty = true;
}

fn respawn(commands: &mut Commands, assets: &AssetServer, e: &mut WorldEdit, i: usize) {
    if let Some(x) = e.placed[i].entity.take() {
        commands.entity(x).despawn();
    }
    e.placed[i].entity = Some(spawn_one(commands, assets, &e.placed[i]));
    e.dirty = true;
}

fn draw(e: Res<WorldEdit>, panel: Res<crate::panel::PanelState>, state: Res<State<crate::menu::AppState>>, studio: Res<crate::studio::Studio>, mut gizmos: Gizmos) {
    if !panel.editing_world() || *state.get() != crate::menu::AppState::Playing || studio.open {
        return;
    }
    if let Some(p) = e.selected.and_then(|i| e.placed.get(i)) {
        let c = Vec3::new(p.pos.x, crate::terrain::ground_at(Vec3::new(p.pos.x, 0.0, p.pos.y)) + 0.05, p.pos.y);
        gizmos.circle(Isometry3d::new(c, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 1.2 * p.scale.max(0.5), Color::srgb(1.0, 0.8, 0.3));
    }
}

/// The World tab of the Tab panel.
pub fn ui(ui: &mut egui::Ui, e: &mut WorldEdit, noclip: &mut bool, props: &[String], commands: &mut Commands, assets: &AssetServer) {
    theme::section(ui, "Camera");
    ui.checkbox(noclip, "Fly freely (noclip)").on_hover_text("Leave the character and fly the camera anywhere.");
    theme::hint(ui, "Flying: WASD to move, Space up, Ctrl down, Shift faster. Hold the right mouse button to look around.");

    theme::section(ui, "Place an object");
    if let Some(p) = e.placing.clone() {
        ui.label(egui::RichText::new(format!("Placing {}", p.rsplit('/').next().unwrap_or(&p))).strong().color(theme::TEAL));
        theme::hint(ui, "Point at the ground and left click to place. Q / E turn it, the mouse wheel resizes it, right click stops.");
        if theme::button(ui, "Stop placing", false).clicked() {
            stop_placing(commands, e);
        }
    } else {
        theme::hint(ui, "Pick an object, then click on the ground to place it.");
    }
    ui.add(egui::TextEdit::singleline(&mut e.filter).hint_text("search: tree, barrel, fence ..."));
    let f = e.filter.to_lowercase();
    egui::ScrollArea::vertical().id_salt("props").max_height(220.0).show(ui, |ui| {
        for p in props.iter().filter(|p| f.is_empty() || p.to_lowercase().contains(&f)).take(200) {
            let name = p.rsplit('/').next().unwrap_or(p).trim_end_matches(".gltf").trim_end_matches(".glb");
            if ui.selectable_label(e.placing.as_deref() == Some(p.as_str()), name).on_hover_text(p.as_str()).clicked() {
                stop_placing(commands, e);
                e.placing = Some(p.clone());
            }
        }
    });

    theme::section(ui, &format!("Placed objects ({})", e.placed.len()));
    theme::hint(ui, "Click an object in the world (or here) to select it. Delete removes it, Q / E turn it.");
    if let Some(i) = e.selected.filter(|&i| i < e.placed.len()) {
        theme::card().show(ui, |ui| {
            let p = &mut e.placed[i];
            ui.label(egui::RichText::new(p.path.rsplit('/').next().unwrap_or(&p.path)).strong());
            let before = (p.pos, p.yaw, p.scale, p.solid);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut p.pos.x).speed(0.05).prefix("x "));
                ui.add(egui::DragValue::new(&mut p.pos.y).speed(0.05).prefix("z "));
            });
            ui.add(egui::Slider::new(&mut p.yaw, -180.0..=180.0).text("turn")).on_hover_text("Turn the object (degrees).");
            ui.add(egui::Slider::new(&mut p.scale, 0.1..=5.0).text("size"));
            ui.checkbox(&mut p.solid, "solid").on_hover_text("Characters and physics bump into it.");
            if before != (p.pos, p.yaw, p.scale, p.solid) {
                respawn(commands, assets, e, i);
            }
        });
        if theme::button(ui, "Delete", false).clicked() {
            remove(commands, e, i);
        }
    }
    egui::ScrollArea::vertical().id_salt("placed").max_height(160.0).show(ui, |ui| {
        for i in 0..e.placed.len() {
            let name = e.placed[i].path.rsplit('/').next().unwrap_or("").to_owned();
            if ui.selectable_label(e.selected == Some(i), format!("{name}  ({:.0}, {:.0})", e.placed[i].pos.x, e.placed[i].pos.y)).clicked() {
                e.selected = Some(i);
            }
        }
    });
    ui.horizontal(|ui| {
        if theme::button(ui, if e.dirty { "Save world *" } else { "Save world" }, e.dirty).on_hover_text("Keep the placed objects for next time (assets/world/placed.json).").clicked() {
            save(e);
        }
    });
}

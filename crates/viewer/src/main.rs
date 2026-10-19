//! Guildbound viewer: a read-only window onto the asset library and the game's pure logic.
//!
//! - A fly camera over an endless plane. The ground is `guildbound_sim::terrain::ground`, the same
//!   function the game's terrain uses, in a patch that follows the camera.
//! - The asset list: every GLB under `assets/`. Clicking one places a copy of it in front of the camera.
//!   Loading goes through the asset server; nothing is written back.
//! - The Kev buttons run the Python tools (`tools/kev/*`) and show their output. Their outputs go to
//!   `staging/viewer/` (gitignored), never to `assets/`.
//!
//! Run: `cargo run -p gb_viewer`. Controls: right mouse drag to look, WASD to move, Q/E down/up,
//! Shift to go faster, scroll to change the base speed.

use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfAssetLabel;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use guildbound_sim::terrain;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};

/// Repository root, the assets folder (read only) and where tool output goes.
const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const OUT: &str = "staging/viewer";

/// Terrain patch: side (m), cell (m), and how far the camera must move before the patch is rebuilt.
const PATCH: f32 = 240.0;
const CELL: f32 = 2.0;
const SNAP: f32 = 20.0;
/// Asset list: entries per page, and how many lines of tool output the panel keeps.
const PAGE: usize = 12;
const LOG_LINES: usize = 28;
/// Kev queries offered as buttons: each is a question for the context index.
const QUERIES: [(&str, &str); 3] = [
    ("Context: rain shelter", "villagers go inside when a heavy rain starts"),
    ("Context: monk aura", "monks have a black capsule around them, the aura looks wrong"),
    ("Context: save and load", "save the game and load it back on start"),
];

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.55, 0.68, 0.82)))
        .insert_resource(Catalog::scan())
        .insert_resource(Page(0))
        .insert_resource(Output { text: "Pick a tool on the left.".into(), rx: Mutex::new(None) })
        .add_plugins(DefaultPlugins.set(AssetPlugin { file_path: ASSETS.into(), ..default() }).set(WindowPlugin {
            primary_window: Some(Window { title: "Guildbound viewer".into(), resolution: (1400, 860).into(), ..default() }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (fly, terrain_patch, grid, kev_buttons, model_buttons, page_buttons, refresh_list, poll_output, clear_button))
        .run();
}

/// Relative GLB paths under `assets/`, sorted, so the list is stable between runs.
#[derive(Resource, Default)]
struct Catalog {
    models: Vec<String>,
}

impl Catalog {
    fn scan() -> Self {
        let mut models = Vec::new();
        collect(Path::new(ASSETS), Path::new(ASSETS), &mut models);
        models.sort();
        Self { models }
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "glb") {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

#[derive(Resource)]
struct Page(usize);

/// Tool output shown in the panel. `rx` holds the channel of a running tool, if any.
#[derive(Resource)]
struct Output {
    text: String,
    rx: Mutex<Option<Receiver<String>>>,
}

#[derive(Component)]
struct Viewer {
    yaw: f32,
    pitch: f32,
    speed: f32,
}

#[derive(Component)]
struct Patch;

#[derive(Component)]
struct Placed;

#[derive(Component)]
struct Clear;

#[derive(Component)]
struct OutputText;

#[derive(Component)]
struct ListSlot(usize);

#[derive(Component)]
struct ListLabel(usize);

#[derive(Component)]
struct PageButton(isize);

#[derive(Component, Clone, Copy)]
enum Kev {
    Survey,
    SurveyAnim,
    Categorise,
    Ask(usize),
}

/// Patch of the terrain last built, so it is only rebuilt when the camera has moved far enough.
#[derive(Component)]
struct PatchCentre(Vec2);

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 14.0, 42.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        Viewer { yaw: 0.0, pitch: -0.25, speed: 12.0 },
    ));
    commands.spawn((DirectionalLight { illuminance: 9000.0, ..default() }, Transform::from_xyz(30.0, 60.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 350.0, ..default() });
    commands.spawn((
        Patch,
        PatchCentre(Vec2::splat(f32::MAX)),
        Mesh3d(meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.9, ..default() })),
        Transform::default(),
    ));
    spawn_panel(&mut commands);
}

fn spawn_panel(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                top: px(12),
                bottom: px(12),
                width: px(330),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(10)),
                row_gap: px(6),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.08, 0.10, 0.92)),
        ))
        .with_children(|panel| {
            panel.spawn((Text::new("Kev tools"), TextFont::from_font_size(18.0), TextColor(Color::WHITE)));
            button(panel, "Survey assets (usability)", Kev::Survey);
            button(panel, "Survey animations", Kev::SurveyAnim);
            button(panel, "Categorise assets (Kev)", Kev::Categorise);
            for (i, (label, _)) in QUERIES.iter().enumerate() {
                button(panel, label, Kev::Ask(i));
            }
            panel.spawn((
                Node { width: Val::Percent(100.0), height: px(260), overflow: Overflow::clip(), padding: UiRect::all(px(6)), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            ))
            .with_children(|p| {
                p.spawn((OutputText, Text::new(""), TextFont::from_font_size(12.0), TextColor(Color::srgb(0.85, 0.9, 0.85))));
            });
            panel.spawn((Text::new("Assets (click to place in front of you)"), TextFont::from_font_size(15.0), TextColor(Color::WHITE)));
            for slot in 0..PAGE {
                panel
                    .spawn((
                        Button,
                        ListSlot(slot),
                        Visibility::Inherited,
                        Node { width: Val::Percent(100.0), height: px(22), padding: UiRect::horizontal(px(6)), align_items: AlignItems::Center, ..default() },
                        BackgroundColor(Color::srgba(0.2, 0.22, 0.26, 0.9)),
                    ))
                    .with_child((ListLabel(slot), Text::new(""), TextFont::from_font_size(12.0), TextColor(Color::WHITE)));
            }
            panel.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(6), ..default() }).with_children(|row| {
                row_button(row, "< prev", PageButton(-1));
                row_button(row, "next >", PageButton(1));
                row_button(row, "clear placed", Clear);
            });
        });
}

fn button(parent: &mut ChildSpawnerCommands, label: &str, action: Kev) {
    parent
        .spawn((
            Button,
            action,
            Node { width: Val::Percent(100.0), height: px(28), padding: UiRect::horizontal(px(8)), align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.18, 0.24, 0.34, 0.95)),
        ))
        .with_child((Text::new(label), TextFont::from_font_size(13.0), TextColor(Color::WHITE)));
}

fn row_button<T: Component>(parent: &mut ChildSpawnerCommands, label: &str, marker: T) {
    parent
        .spawn((
            Button,
            marker,
            Node { height: px(24), padding: UiRect::horizontal(px(8)), align_items: AlignItems::Center, ..default() },
            BackgroundColor(Color::srgba(0.2, 0.22, 0.26, 0.9)),
        ))
        .with_child((Text::new(label), TextFont::from_font_size(12.0), TextColor(Color::WHITE)));
}

/// Right mouse drag looks, WASD moves on the ground plane, Q and E go down and up, Shift is faster,
/// scroll sets the base speed.
fn fly(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut q: Query<(&mut Transform, &mut Viewer)>,
) {
    let Ok((mut tf, mut v)) = q.single_mut() else { return };
    if buttons.pressed(MouseButton::Right) {
        v.yaw -= motion.delta.x * 0.003;
        v.pitch = (v.pitch - motion.delta.y * 0.003).clamp(-1.5, 1.5);
    }
    v.speed = (v.speed * (1.0 + scroll.delta.y * 0.1)).clamp(1.0, 200.0);
    tf.rotation = Quat::from_euler(EulerRot::YXZ, v.yaw, v.pitch, 0.0);
    let flat_fwd = Vec3::new(-v.yaw.sin(), 0.0, -v.yaw.cos());
    let right = Vec3::new(v.yaw.cos(), 0.0, -v.yaw.sin());
    let mut dir = Vec3::ZERO;
    for (key, d) in [(KeyCode::KeyW, flat_fwd), (KeyCode::KeyS, -flat_fwd), (KeyCode::KeyD, right), (KeyCode::KeyA, -right)] {
        if keys.pressed(key) {
            dir += d;
        }
    }
    if keys.pressed(KeyCode::KeyE) {
        dir += Vec3::Y;
    }
    if keys.pressed(KeyCode::KeyQ) {
        dir -= Vec3::Y;
    }
    let boost = if keys.pressed(KeyCode::ShiftLeft) { 3.0 } else { 1.0 };
    tf.translation += dir.normalize_or_zero() * v.speed * boost * time.delta_secs();
}

/// The ground patch around the camera, rebuilt when the camera has moved a snap step.
fn terrain_patch(
    camera: Query<&Transform, With<Viewer>>,
    mut patch: Query<(&Mesh3d, &mut PatchCentre), With<Patch>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Ok(cam), Ok((mesh, mut centre))) = (camera.single(), patch.single_mut()) else { return };
    let snapped = Vec2::new((cam.translation.x / SNAP).round() * SNAP, (cam.translation.z / SNAP).round() * SNAP);
    if snapped == centre.0 {
        return;
    }
    centre.0 = snapped;
    if let Some(mut m) = meshes.get_mut(&mesh.0) {
        *m = build_patch(snapped);
    }
}

fn build_patch(centre: Vec2) -> Mesh {
    let n = (PATCH / CELL) as usize;
    let mut pos = Vec::new();
    let mut col = Vec::new();
    for iz in 0..=n {
        for ix in 0..=n {
            let x = centre.x - PATCH * 0.5 + ix as f32 * CELL;
            let z = centre.y - PATCH * 0.5 + iz as f32 * CELL;
            let h = terrain::ground(x, z);
            pos.push([x, h, z]);
            // Lowland green through to brown on the rising ground: the height alone, no textures.
            let t = (h / 12.0).clamp(0.0, 1.0);
            col.push([0.30 + 0.35 * t, 0.45 - 0.12 * t, 0.22 - 0.08 * t, 1.0]);
        }
    }
    let mut idx = Vec::new();
    let row = (n + 1) as u32;
    for iz in 0..n as u32 {
        for ix in 0..n as u32 {
            let a = iz * row + ix;
            idx.extend_from_slice(&[a, a + row, a + 1, a + 1, a + row, a + row + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx));
    mesh.compute_normals();
    mesh
}

/// Grid lines on the ground every 10 m, around the patch, so the plane reads as a plane.
fn grid(mut gizmos: Gizmos, patch: Query<&PatchCentre>) {
    let Ok(centre) = patch.single() else { return };
    let half = PATCH * 0.5;
    let colour = Color::srgba(0.1, 0.1, 0.1, 0.35);
    let mut x = (centre.0.x - half / 10.0).floor() * 10.0;
    while x <= centre.0.x + half {
        gizmos.line(Vec3::new(x, 0.08, centre.0.y - half), Vec3::new(x, 0.08, centre.0.y + half), colour);
        x += 10.0;
    }
    let mut z = (centre.0.y - half / 10.0).floor() * 10.0;
    while z <= centre.0.y + half {
        gizmos.line(Vec3::new(centre.0.x - half, 0.08, z), Vec3::new(centre.0.x + half, 0.08, z), colour);
        z += 10.0;
    }
}

fn kev_buttons(interactions: Query<(&Interaction, &Kev), Changed<Interaction>>, mut output: ResMut<Output>) {
    for (interaction, kev) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (label, args): (String, Vec<String>) = match kev {
            Kev::Survey => ("survey assets".into(), vec!["tools/kev/survey.py".into(), "assets".into(), format!("{OUT}/survey"), "--jobs".into(), "4".into()]),
            Kev::SurveyAnim => ("survey animations".into(), vec!["tools/kev/survey.py".into(), "assets/anim".into(), format!("{OUT}/survey-anim"), "--jobs".into(), "4".into()]),
            Kev::Categorise => ("categorise assets".into(), vec!["tools/kev/kev.py".into(), "assets".into(), format!("{OUT}/kev")]),
            Kev::Ask(i) => (
                format!("context: {}", QUERIES[*i].0),
                vec!["tools/kev/context/build.py".into(), "ask".into(), QUERIES[*i].1.into(), "--k".into(), "5".into()],
            ),
        };
        start_job(&mut output, &label, args);
    }
}

/// Run a Python tool from the repo root on a background thread. The output lands in the panel
/// when it finishes, so the window keeps drawing while a survey runs.
fn start_job(output: &mut Output, label: &str, args: Vec<String>) {
    let (tx, rx) = mpsc::channel();
    *output.rx.get_mut().unwrap() = Some(rx);
    output.text = format!("running {label}...");
    let label = label.to_string();
    std::thread::spawn(move || {
        let text = match Command::new("python3").arg("-I").args(&args).current_dir(REPO).output() {
            Ok(o) => {
                let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
                s.push_str(&String::from_utf8_lossy(&o.stderr));
                let lines: Vec<&str> = s.lines().collect();
                let tail = lines[lines.len().saturating_sub(LOG_LINES)..].join("\n");
                format!("{label}\n{tail}")
            }
            Err(e) => format!("could not start python3: {e}"),
        };
        let _ = tx.send(text);
    });
}

fn poll_output(mut output: ResMut<Output>, mut text: Query<&mut Text, With<OutputText>>) {
    let msg = output.rx.get_mut().unwrap().as_ref().and_then(|rx| rx.try_recv().ok());
    if let Some(t) = msg {
        output.text = t;
        *output.rx.get_mut().unwrap() = None;
    }
    if output.is_changed() {
        if let Ok(mut t) = text.single_mut() {
            **t = output.text.clone();
        }
    }
}

fn page_buttons(interactions: Query<(&Interaction, &PageButton), Changed<Interaction>>, catalog: Res<Catalog>, mut page: ResMut<Page>) {
    for (interaction, step) in &interactions {
        if *interaction == Interaction::Pressed {
            let pages = catalog.models.len().div_ceil(PAGE).max(1) as isize;
            page.0 = (page.0 as isize + step.0).rem_euclid(pages) as usize;
        }
    }
}

/// Fill the list slots from the current page. Runs every frame; it only writes when the page changed.
fn refresh_list(
    page: Res<Page>,
    catalog: Res<Catalog>,
    mut slots: Query<(&ListSlot, &mut Visibility), With<Button>>,
    mut labels: Query<(&ListLabel, &mut Text)>,
    mut seen: Local<Option<usize>>,
) {
    if *seen == Some(page.0) && !catalog.is_changed() {
        return;
    }
    *seen = Some(page.0);
    for (ListLabel(i), mut text) in &mut labels {
        let idx = page.0 * PAGE + i;
        **text = catalog.models.get(idx).map(|m| short_model(m)).unwrap_or_default();
    }
    for (ListSlot(i), mut vis) in &mut slots {
        *vis = if page.0 * PAGE + i < catalog.models.len() { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn short_model(m: &str) -> String {
    if m.len() > 42 { format!("...{}", &m[m.len() - 39..]) } else { m.to_string() }
}

/// A list slot click places that model in front of the camera. Loads go through the asset server.
fn model_buttons(
    interactions: Query<(&Interaction, &ListSlot), Changed<Interaction>>,
    page: Res<Page>,
    catalog: Res<Catalog>,
    camera: Query<&Transform, With<Viewer>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(cam) = camera.single() else { return };
    for (interaction, ListSlot(i)) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(rel) = catalog.models.get(page.0 * PAGE + i) else { continue };
        info!("placing {rel}");
        let fwd = cam.forward().as_vec3() * Vec3::new(1.0, 0.0, 1.0);
        let at = cam.translation + fwd.normalize_or_zero() * 10.0;
        let ground = Vec3::new(at.x, terrain::ground(at.x, at.z), at.z);
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(rel.clone()));
        commands.spawn((Placed, Name::new(rel.clone()), WorldAssetRoot(scene), Transform::from_translation(ground)));
    }
}

fn clear_button(interactions: Query<&Interaction, (Changed<Interaction>, With<Clear>)>, placed: Query<Entity, With<Placed>>, mut commands: Commands) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        for e in &placed {
            commands.entity(e).despawn();
        }
    }
}

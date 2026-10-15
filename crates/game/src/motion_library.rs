//! Whole-library indexing plus light-weight rotation reconstruction.
//!
//! Every fighter clip is classified and retained as a safe, body-group-weighted rotation
//! analogue. The state machine selects the relevant analogue before the Ranger's terrain/contact
//! residuals. Root motion is excluded: the controller owns momentum and collision.

use crate::{
    combat, ranger,
    tuning::{Knob as K, Tuning},
};
use bevy::{prelude::*, ui::RelativeCursorPosition};
use gltf::animation::util::ReadOutputs;
use std::collections::{HashMap, VecDeque};

#[derive(Resource, Default)]
struct Library {
    clips: HashMap<String, Clip>,
    targets: HashMap<String, Target>,
    analogs: HashMap<String, Analog>,
    names: Vec<String>,
    selected: usize,
    trails_enabled: bool,
    current_base: Option<[Vec3; 4]>,
    current_reference: Option<[Vec3; 4]>,
    base_trails: [VecDeque<Vec3>; 4],
    reference_trails: [VecDeque<Vec3>; 4],
    result_trails: [VecDeque<Vec3>; 4],
    dragging: bool,
}

struct Target {
    entity: Entity,
    rest: Quat,
}

struct Clip {
    duration: f32,
    tracks: Vec<Track>,
}

struct Track {
    bone: String,
    rest: Quat,
    times: Vec<f32>,
    rotations: Vec<Quat>,
    step: bool,
}

#[derive(Clone, Copy)]
enum BoneGroup {
    Core = 0,
    Spine = 1,
    Arms = 2,
    Legs = 3,
    Hands = 4,
    Head = 5,
}

#[derive(Clone)]
struct Analog {
    duration: f32,
    category: &'static str,
    channels: usize,
}

#[derive(Component)]
struct PickerText;
#[derive(Component)]
struct PickerButton(i8);
#[derive(Component)]
struct ReferenceSlider;
#[derive(Component)]
struct ReferenceFill;

fn bone_group(name: &str) -> BoneGroup {
    if name.contains("Finger") || name.contains("Hand") || name.contains("Forearm.001") {
        BoneGroup::Hands
    } else if name.contains("UpperArm") || name.contains("Forearm") {
        BoneGroup::Arms
    } else if name.contains("Thigh")
        || name.contains("Calf")
        || name.contains("Foot")
        || name.contains("Toe")
    {
        BoneGroup::Legs
    } else if name.contains("Head") || name.contains("Neck") {
        BoneGroup::Head
    } else if name.contains("Spine") || name.contains("Clavicle") {
        BoneGroup::Spine
    } else {
        BoneGroup::Core
    }
}

fn category(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("punch")
        || lower.contains("straight")
        || lower.contains("hook")
        || lower.contains("attack")
        || lower.starts_with("hp_")
        || lower.starts_with("bp_")
    {
        "combat"
    } else if lower.contains("jump") || lower.contains("fall") || lower.contains("land") {
        "air_land"
    } else if lower.contains("roll") || lower.contains("slide") {
        "evasion"
    } else if lower.contains("run") || lower.contains("walk") || lower.contains("sprint") {
        "locomotion"
    } else {
        "other"
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, (load, spawn_picker))
        .add_systems(Update, (picker_input, picker_hud, draw_trails))
        .add_systems(
            PostUpdate,
            apply
                .after(bevy::app::AnimationSystems)
                .before(ranger::Stage::Pose),
        )
        .add_systems(
            PostUpdate,
            capture_result
                .after(crate::paper_motion::Stage::Solve)
                .before(bevy::transform::TransformSystems::Propagate),
        );
}

fn load(mut commands: Commands) {
    let base = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/packs/hero/fighter.glb"
    );
    let result = (|| -> Result<(Library, serde_json::Value), String> {
        let glb = gltf::Gltf::open(base).map_err(|e| e.to_string())?;
        let blob = glb
            .blob
            .as_deref()
            .ok_or("fighter.glb has no binary buffer")?;
        let node_names: HashMap<usize, String> = glb
            .nodes()
            .filter_map(|node| node.name().map(|name| (node.index(), name.to_string())))
            .collect();
        let node_rest: HashMap<usize, Quat> = glb
            .nodes()
            .map(|node| {
                let (_, rotation, _) = node.transform().decomposed();
                (node.index(), Quat::from_array(rotation).normalize())
            })
            .collect();
        let mut clips = HashMap::new();
        let mut analogs = HashMap::new();
        let mut categories: HashMap<&str, usize> = HashMap::new();
        let mut rows = Vec::new();
        for animation in glb.animations() {
            let Some(name) = animation.name().map(str::to_string) else {
                continue;
            };
            let class = category(&name);
            *categories.entry(class).or_default() += 1;
            let mut duration = 0.0f32;
            let mut tracks = Vec::new();
            let mut squared = [0.0f32; 6];
            let mut peak = [0.0f32; 6];
            let mut samples = [0usize; 6];
            let mut channels = 0usize;
            for channel in animation.channels() {
                let node = channel.target().node().index();
                let Some(bone) = node_names.get(&node).cloned() else {
                    continue;
                };
                let reader = channel.reader(|buffer| {
                    matches!(buffer.source(), gltf::buffer::Source::Bin).then_some(blob)
                });
                let Some(inputs) = reader.read_inputs() else {
                    continue;
                };
                let times: Vec<f32> = inputs.collect();
                if times.is_empty() {
                    continue;
                }
                duration = duration.max(*times.last().unwrap());
                let Some(ReadOutputs::Rotations(values)) = reader.read_outputs() else {
                    continue;
                };
                let rotations: Vec<Quat> = values
                    .into_f32()
                    .map(|q| Quat::from_array(q).normalize())
                    .collect();
                if rotations.len() != times.len() {
                    continue;
                }
                channels += 1;
                let group = bone_group(&bone);
                let gi = group as usize;
                let rest = node_rest[&node];
                for &rotation in &rotations {
                    let angle = (rest.inverse() * rotation)
                        .to_scaled_axis()
                        .length()
                        .min(std::f32::consts::PI);
                    squared[gi] += angle * angle;
                    peak[gi] = peak[gi].max(angle);
                    samples[gi] += 1;
                }
                tracks.push(Track {
                    bone,
                    rest,
                    times,
                    rotations,
                    step: channel.sampler().interpolation() == gltf::animation::Interpolation::Step,
                });
            }
            let rms: [f32; 6] =
                std::array::from_fn(|i| (squared[i] / samples[i].max(1) as f32).sqrt());
            // Each source clip gets its own safe analogue. High-excursion groups are attenuated,
            // and locomotion legs stay descriptive because target IK owns final support.
            let base_weights: [f32; 6] = match class {
                "locomotion" => [0.22, 0.58, 0.72, 0.10, 0.38, 0.30],
                "combat" => [0.34, 0.72, 0.82, 0.08, 0.70, 0.42],
                "air_land" => [0.44, 0.66, 0.62, 0.26, 0.44, 0.36],
                "evasion" => [0.62, 0.74, 0.58, 0.34, 0.38, 0.44],
                _ => [0.28, 0.55, 0.58, 0.16, 0.45, 0.35],
            };
            let weights: [f32; 6] = std::array::from_fn(|i| {
                let excess = (peak[i] - 1.25).max(0.0);
                (base_weights[i] / (1.0 + excess * 1.2)).clamp(0.02, 0.9)
            });
            let analog = Analog {
                duration,
                category: class,
                channels,
            };
            rows.push(serde_json::json!({
                "name": name,
                "category": class,
                "duration_s": duration,
                "rotation_channels": channels,
                "rms_rad": rms,
                "peak_rad": peak,
                "group_weights": weights,
            }));
            if !tracks.is_empty() {
                clips.insert(name.clone(), Clip { duration, tracks });
            }
            analogs.insert(name, analog);
        }
        let report = serde_json::json!({
            "source": base,
            "clips_indexed": analogs.len(),
            "categories": categories,
            "retargetable_clips": clips.keys().collect::<Vec<_>>(),
            "group_order": ["core", "spine", "arms", "legs", "hands", "head"],
            "analogs": rows,
            "method": "every clip receives duration, group energy, peak excursion and safe retarget weights; selected curves are rest-relative; target terrain IK owns planted legs",
        });
        let mut names: Vec<_> = clips.keys().cloned().collect();
        names.sort();
        let selected = names
            .iter()
            .position(|name| name == "nc_run_forward")
            .unwrap_or(0);
        Ok((
            Library {
                clips,
                targets: HashMap::new(),
                analogs,
                names,
                selected,
                trails_enabled: true,
                current_base: None,
                current_reference: None,
                base_trails: std::array::from_fn(|_| VecDeque::new()),
                reference_trails: std::array::from_fn(|_| VecDeque::new()),
                result_trails: std::array::from_fn(|_| VecDeque::new()),
                dragging: false,
            },
            report,
        ))
    })();
    match result {
        Ok((library, report)) => {
            let _ = std::fs::create_dir_all("profiles");
            let _ = std::fs::write(
                "profiles/motion_library.json",
                serde_json::to_string_pretty(&report).unwrap(),
            );
            println!(
                "[motion_library] {} per-clip analogues retained and retargetable",
                library.clips.len()
            );
            commands.insert_resource(library);
        }
        Err(error) => {
            println!("[motion_library] unavailable: {error}");
            commands.insert_resource(Library::default());
        }
    }
}

fn spawn_picker(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(18.0),
                bottom: px(16.0),
                width: px(470.0),
                padding: UiRect::all(px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(5.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.055, 0.075, 0.9)),
        ))
        .with_children(|ui| {
            ui.spawn((
                PickerText,
                Text::new("MOTION REFERENCE: loading..."),
                TextFont {
                    font_size: bevy::text::FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            ui.spawn((
                Button,
                ReferenceSlider,
                RelativeCursorPosition::default(),
                Node {
                    width: percent(100.0),
                    height: px(18.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.42, 0.52)),
            ))
            .with_child((
                ReferenceFill,
                Node {
                    width: percent(50.0),
                    height: percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.82, 0.18, 0.72)),
            ));
            ui.spawn(Node {
                column_gap: px(5.0),
                ..default()
            })
            .with_children(|row| {
                for (code, label) in [(-1, "< Reference"), (2, "Trails (T)"), (1, "Reference >")] {
                    row.spawn((
                        Button,
                        PickerButton(code),
                        Node {
                            padding: UiRect::all(px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.13, 0.22, 0.29)),
                    ))
                    .with_child((
                        Text::new(label),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                }
            });
        });
}

fn picker_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut feel: ResMut<Tuning>,
    library: Option<ResMut<Library>>,
    buttons: Query<(&PickerButton, &Interaction), Changed<Interaction>>,
    slider: Query<(&Interaction, &RelativeCursorPosition), With<ReferenceSlider>>,
) {
    let Some(mut library) = library else { return };
    let mut delta = 0i32;
    if keys.just_pressed(KeyCode::PageUp) {
        delta -= 1;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        delta += 1;
    }
    let mut toggle_trails = keys.just_pressed(KeyCode::KeyT);
    for (button, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button.0 {
            -1 => delta -= 1,
            1 => delta += 1,
            2 => toggle_trails = true,
            _ => {}
        }
    }
    if delta != 0 && !library.names.is_empty() {
        library.selected =
            (library.selected as i32 + delta).rem_euclid(library.names.len() as i32) as usize;
        clear_trails(&mut library);
    }
    if toggle_trails {
        library.trails_enabled = !library.trails_enabled;
    }
    if !mouse.pressed(MouseButton::Left) {
        library.dragging = false;
    }
    for (interaction, cursor) in &slider {
        if mouse.just_pressed(MouseButton::Left) && *interaction == Interaction::Pressed {
            library.dragging = true;
        }
        if library.dragging {
            if let Some(position) = cursor.normalized {
                feel.set_value(K::LibraryBlend, (position.x + 0.5).clamp(0.0, 1.0));
            }
        }
    }
}

fn picker_hud(
    library: Option<Res<Library>>,
    feel: Res<Tuning>,
    mut text: Query<&mut Text, With<PickerText>>,
    mut fill: Query<&mut Node, With<ReferenceFill>>,
) {
    let (Some(library), Ok(mut text)) = (library, text.single_mut()) else {
        return;
    };
    if library.names.is_empty() {
        return;
    }
    let name = &library.names[library.selected];
    let analog = &library.analogs[name];
    let blend = feel.value_fraction(K::LibraryBlend).clamp(0.0, 1.0);
    if let Ok(mut fill) = fill.single_mut() {
        fill.width = percent(blend * 100.0);
    }
    text.0 = format!(
        "MOTION REFERENCE {}/{}: {}\nBASE  {:.0}%  <———— SOURCE BLEND ————>  {:.0}%  REFERENCE\n{} | {:.2}s | {} channels | PageUp/PageDown | trails {}\ncyan: raw base   magenta: raw reference   yellow: fitted result",
        library.selected + 1,
        library.names.len(),
        name,
        (1.0 - blend) * 100.0,
        blend * 100.0,
        analog.category,
        analog.duration,
        analog.channels,
        if library.trails_enabled { "ON" } else { "OFF" },
    );
}

fn clear_trails(library: &mut Library) {
    library.current_base = None;
    library.current_reference = None;
    for trail in &mut library.base_trails {
        trail.clear();
    }
    for trail in &mut library.reference_trails {
        trail.clear();
    }
    for trail in &mut library.result_trails {
        trail.clear();
    }
}

pub(crate) fn calibrate(world: &mut World, names: &HashMap<String, Entity>) {
    let Some(mut library) = world.remove_resource::<Library>() else {
        return;
    };
    clear_trails(&mut library);
    library.targets.clear();
    for (name, &entity) in names {
        if let Some(transform) = world.get::<Transform>(entity) {
            library.targets.insert(
                name.clone(),
                Target {
                    entity,
                    rest: transform.rotation,
                },
            );
        }
    }
    world.insert_resource(library);
}

fn sample(track: &Track, time: f32) -> Quat {
    let lo = track
        .times
        .partition_point(|&t| t <= time)
        .saturating_sub(1);
    let hi = (lo + 1).min(track.times.len() - 1);
    let span = track.times[hi] - track.times[lo];
    let u = if track.step || span < 1e-6 {
        0.0
    } else {
        ((time - track.times[lo]) / span).clamp(0.0, 1.0)
    };
    track.rotations[lo].slerp(track.rotations[hi], u)
}

fn apply(world: &mut World) {
    let Some(context) = ranger::library_context(world) else {
        return;
    };
    let selected = {
        let library = world.resource::<Library>();
        (!library.names.is_empty()).then(|| library.names[library.selected].clone())
    };
    let Some(name) = selected else { return };
    let combat_phase = world.resource::<combat::Combat>().punch_phase();
    let phase = if let Some((_, phase)) = combat_phase {
        phase
    } else if let Some(roll) = context.roll {
        roll
    } else if context.landing > 0.0 {
        1.0 - context.landing
    } else if context.charge > 0.0 || context.push > 0.0 {
        context.charge.max(1.0 - context.push)
    } else {
        context.phase
    };
    let blend = world
        .resource::<Tuning>()
        .get(K::LibraryBlend)
        .clamp(0.0, 1.0);
    let Some(mut library) = world.remove_resource::<Library>() else {
        return;
    };
    library.current_base = landmark_positions(world, &library);
    if let Some(clip) = library.clips.get(&name) {
        let time = phase.rem_euclid(1.0) * clip.duration;
        let mut poses = Vec::with_capacity(clip.tracks.len());
        for track in &clip.tracks {
            let Some(target) = library.targets.get(&track.bone) else {
                continue;
            };
            let sampled = sample(track, time);
            let Some(transform) = world.get::<Transform>(target.entity) else {
                continue;
            };
            // Both ends of the slider are source poses at the same normalized phase. Rest-relative
            // reconstruction carries the reference onto the sampled target bind axes.
            let source_vector = (track.rest.inverse() * sampled)
                .to_scaled_axis()
                .clamp_length_max(2.6);
            let reconstructed = target.rest * Quat::from_scaled_axis(source_vector);
            poses.push((target.entity, transform.rotation, reconstructed));
        }

        // Temporarily pose the complete raw reference to measure its landmark trail, then leave
        // the exact base/reference interpolation for the downstream fitting systems.
        for &(entity, _, reference) in &poses {
            world.get_mut::<Transform>(entity).unwrap().rotation = reference;
        }
        library.current_reference = landmark_positions(world, &library);
        for (entity, base, reference) in poses {
            world.get_mut::<Transform>(entity).unwrap().rotation = base.slerp(reference, blend);
        }
    }
    world.insert_resource(library);
}

fn landmark_positions(world: &World, library: &Library) -> Option<[Vec3; 4]> {
    let entity = |choices: &[&str]| {
        choices
            .iter()
            .find_map(|name| library.targets.get(*name).map(|target| target.entity))
    };
    let entities = [
        entity(&["B_L_Foot"]),
        entity(&["B_R_Foot"]),
        entity(&["B_L_Forearm.001", "B_L_Hand"]),
        entity(&["B_R_Hand", "B_R_Forearm.001"]),
    ];
    Some([
        ranger::world_transform(world, entities[0]?)?.translation(),
        ranger::world_transform(world, entities[1]?)?.translation(),
        ranger::world_transform(world, entities[2]?)?.translation(),
        ranger::world_transform(world, entities[3]?)?.translation(),
    ])
}

fn push_point(trail: &mut VecDeque<Vec3>, point: Vec3) {
    const LIMIT: usize = 56;
    trail.push_back(point);
    while trail.len() > LIMIT {
        trail.pop_front();
    }
}

fn capture_result(world: &mut World) {
    let Some(mut library) = world.remove_resource::<Library>() else {
        return;
    };
    if !library.trails_enabled {
        clear_trails(&mut library);
        world.insert_resource(library);
        return;
    }
    let result = landmark_positions(world, &library);
    if let (Some(base), Some(reference), Some(result)) = (
        library.current_base.take(),
        library.current_reference.take(),
        result,
    ) {
        let teleported = library
            .result_trails
            .iter()
            .enumerate()
            .any(|(index, trail)| {
                trail
                    .back()
                    .is_some_and(|last| last.distance(result[index]) > 2.0)
            });
        if teleported {
            clear_trails(&mut library);
        }
        for index in 0..4 {
            push_point(&mut library.base_trails[index], base[index]);
            push_point(&mut library.reference_trails[index], reference[index]);
            push_point(&mut library.result_trails[index], result[index]);
        }
    }
    world.insert_resource(library);
}

fn draw_trails(library: Option<Res<Library>>, mut gizmos: Gizmos) {
    let Some(library) = library else { return };
    if !library.trails_enabled {
        return;
    }
    for index in 0..4 {
        let brightness = if index < 2 { 0.78 } else { 1.0 };
        if library.base_trails[index].len() > 1 {
            gizmos.linestrip(
                library.base_trails[index].iter().copied(),
                Color::srgb(0.10, 0.78 * brightness, 1.0 * brightness),
            );
        }
        if library.reference_trails[index].len() > 1 {
            gizmos.linestrip(
                library.reference_trails[index].iter().copied(),
                Color::srgb(0.95 * brightness, 0.12, 0.82 * brightness),
            );
        }
        if library.result_trails[index].len() > 1 {
            gizmos.linestrip(
                library.result_trails[index].iter().copied(),
                Color::srgb(1.0 * brightness, 0.88 * brightness, 0.10),
            );
        }
        if let Some(point) = library.result_trails[index].back() {
            gizmos.cross(
                Isometry3d::from_translation(*point),
                if index < 2 { 0.025 } else { 0.035 },
                Color::srgb(1.0, 0.9, 0.1),
            );
        }
    }
}

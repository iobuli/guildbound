//! Athorina Stage 1: a model and motion laboratory with a physical impact wall.
use super::*;
use crate::motion::{bank::PoseBank, Layer, Mask, ModelRequest, MotionLayers, Source};
use avian3d::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::asset::RenderAssetUsages;

const MODELS: [(&str, &str); 4] = [
    ("Mannequin", "anim/ual2.glb"),
    ("Fighter", "anim/ual1.glb"),
    ("Hero", "anim/hero_styled.glb"),
    ("Block", "anim/blocky.glb"),
];
const TINTS: [Color; 5] = [
    Color::srgb(0.78, 0.81, 0.83), Color::srgb(0.97, 0.64, 0.36),
    Color::srgb(0.41, 0.72, 0.75), Color::srgb(0.73, 0.55, 0.87),
    Color::srgb(0.52, 0.69, 0.47),
];
const W: f32 = 5.0;
const H: f32 = 2.6;
const WALL_Z: f32 = 3.0;
const FRONT: f32 = WALL_Z - 0.18;
const NX: usize = 40;
const NY: usize = 24;

#[derive(Resource)]
struct Studio {
    model: usize,
    tint: usize,
    clip: String,
    clip_time: f32,
    preview: bool,
    attack_time: f32,
    attack_clip: String,
    attack_hit: bool,
    combo: usize,
    yaw: f32,
    pitch: f32,
    distance: f32,
    clips: Vec<String>,
    page: usize,
    wall: Handle<Mesh>,
    dents: Vec<(Vec2, f32, f32)>,
}

#[derive(Component)] struct StudioCamera;
#[derive(Component)] struct StatusText;
#[derive(Component)] struct ClipText(usize);
#[derive(Component)] struct ModelText;
#[derive(Component)] struct PageText;
#[derive(Component)] struct Chip { life: f32 }
#[derive(Component, Clone, Copy)] enum UiAction { Model(i8), Tint(i8), Page(i8), Clip(usize), Play }

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (browse, input).chain().in_set(PlaySet::Input))
        .add_systems(Update, (attack, select_motion, update_ui, chips).chain().in_set(PlaySet::Select))
        .add_systems(FixedUpdate, climb.after(PlaySet::Move))
        .add_systems(PostUpdate, camera_follow.before(bevy::transform::TransformSystems::Propagate));
}

fn panel_button(parent: &mut ChildSpawnerCommands, label: &str, action: UiAction, font: Handle<Font>) {
    parent.spawn((Button, action, Node { width: Val::Percent(100.0), height: Val::Px(33.0),
        align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(10.0), Val::Px(2.0)), ..default() },
        BackgroundColor(Color::srgba(0.25, 0.27, 0.29, 0.94))))
        .with_children(|p| { p.spawn((Text::new(label), TextFont { font: font.into(), font_size: bevy::text::FontSize::Px(18.0), ..default() },
            TextColor(Color::srgb(0.94, 0.82, 0.65)))); });
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, assets: Res<AssetServer>) {
    let floor = materials.add(StandardMaterial { base_color: Color::srgb(0.42, 0.44, 0.46), metallic: 0.1,
        perceptual_roughness: 0.85, ..default() });
    let wall_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.57, 0.58, 0.59),
        perceptual_roughness: 0.88, double_sided: true, cull_mode: None, ..default() });
    commands.spawn((Name::new("Plane"), Mesh3d(meshes.add(Cuboid::new(40.0, 0.12, 40.0))),
        MeshMaterial3d(floor), Transform::from_xyz(0.0, -0.06, 0.0),
        RigidBody::Static, Collider::cuboid(40.0, 0.12, 40.0), CollisionLayers::new(Phys::World, [Phys::Character])));
    commands.spawn((Name::new("Solid impact wall"), RigidBody::Static,
        Collider::cuboid(W, H, 0.36), CollisionLayers::new(Phys::World, [Phys::Character]),
        Transform::from_xyz(0.0, H * 0.5, WALL_Z)));
    let wall = meshes.add(wall_mesh(&[]));
    commands.spawn((Name::new("Dentable wall face"), Mesh3d(wall.clone()), MeshMaterial3d(wall_mat.clone()),
        Transform::IDENTITY));
    commands.spawn((Mesh3d(meshes.add(Cuboid::new(W, H, 0.02))), MeshMaterial3d(wall_mat),
        Transform::from_xyz(0.0, H * 0.5, WALL_Z + 0.19)));
    commands.spawn((Name::new("Athorina"), Player, RigidBody::Kinematic,
        Collider::capsule(CAPSULE_RADIUS, CAPSULE_LENGTH),
        CollisionLayers::new(Phys::Character, [Phys::World]),
        Transform::from_xyz(0.0, HALF_HEIGHT, 0.0), Visibility::default(),
        Mover::default(), Intent::default(),
        ModelRequest { path: MODELS[0].1.into(), offset: Vec3::new(0.0, -HALF_HEIGHT, 0.0), tint: Some(TINTS[0]) }));
    commands.spawn((StudioCamera, Camera3d::default(), Transform::from_xyz(0.0, 2.5, -5.5).looking_at(Vec3::Y, Vec3::Y)));
    commands.spawn((DirectionalLight { illuminance: 17000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(-5.0, 9.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((PointLight { intensity: 350000.0, range: 20.0, ..default() }, Transform::from_xyz(5.0, 6.0, 2.0)));
    let font = assets.load("fonts/rajdhani/Rajdhani-Medium.ttf");
    let bold = assets.load("fonts/rajdhani/Rajdhani-Bold.ttf");
    commands.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(18.0), top: Val::Px(18.0),
        width: Val::Px(310.0), padding: UiRect::all(Val::Px(15.0)), row_gap: Val::Px(5.0),
        flex_direction: FlexDirection::Column, ..default() },
        BackgroundColor(Color::srgba(0.09, 0.11, 0.13, 0.9))))
        .with_children(|p| {
            p.spawn((Text::new("ATHORINA  /  STAGE 01"), TextFont { font: bold.clone().into(), font_size: bevy::text::FontSize::Px(25.0), ..default() }, TextColor(Color::srgb(1.0, 0.69, 0.37))));
            p.spawn((Text::new("CHARACTER"), TextFont { font: bold.clone().into(), font_size: bevy::text::FontSize::Px(17.0), ..default() }, TextColor(Color::WHITE)));
            p.spawn((ModelText, Text::new(""), TextFont { font: font.clone().into(), font_size: bevy::text::FontSize::Px(19.0), ..default() }, TextColor(Color::WHITE)));
            panel_button(p, "<  MODEL  >", UiAction::Model(1), font.clone());
            panel_button(p, "CHANGE COLOUR", UiAction::Tint(1), font.clone());
            p.spawn((Text::new("MOTION LIBRARY"), TextFont { font: bold.clone().into(), font_size: bevy::text::FontSize::Px(17.0), ..default() }, TextColor(Color::WHITE)));
            p.spawn((PageText, Text::new(""), TextFont { font: font.clone().into(), font_size: bevy::text::FontSize::Px(16.0), ..default() }, TextColor(Color::WHITE)));
            panel_button(p, "PREVIOUS PAGE", UiAction::Page(-1), font.clone());
            for i in 0..8 {
                p.spawn((Button, UiAction::Clip(i), Node { width: Val::Percent(100.0), height: Val::Px(28.0),
                    align_items: AlignItems::Center, padding: UiRect::left(Val::Px(8.0)), ..default() },
                    BackgroundColor(Color::srgba(0.21, 0.23, 0.25, 0.85))))
                    .with_children(|c| { c.spawn((ClipText(i), Text::new(""), TextFont { font: font.clone().into(), font_size: bevy::text::FontSize::Px(15.0), ..default() }, TextColor(Color::WHITE))); });
            }
            panel_button(p, "NEXT PAGE", UiAction::Page(1), font.clone());
            panel_button(p, "PLAY / STOP", UiAction::Play, font.clone());
            p.spawn((StatusText, Text::new("Loading motion bank..."), TextFont { font: font.clone().into(), font_size: bevy::text::FontSize::Px(16.0), ..default() }, TextColor(Color::srgb(0.94, 0.82, 0.65))));
            p.spawn((Text::new("WASD move  /  Shift sprint  /  Space jump & climb\nLMB punch  /  K kick  /  RMB drag camera  /  wheel zoom\nM model  /  N colour  /  P preview"),
                TextFont { font: font.into(), font_size: bevy::text::FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.72, 0.75, 0.76))));
        });
    commands.insert_resource(Studio { model: 0, tint: 0, clip: String::new(), clip_time: 0.0,
        preview: false, attack_time: 0.0, attack_clip: String::new(), attack_hit: false,
        combo: 0, yaw: 0.0, pitch: 0.2, distance: 5.0, clips: Vec::new(), page: 0, wall, dents: Vec::new() });
}

fn allowed(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    !["cast", "magic", "spell", "ki_", "blast", "beam", "charge", "pistol", "bow", "rifle", "gun", "staff", "fireball"].iter().any(|s| n.contains(s))
}

fn browse(mut studio: ResMut<Studio>, bank: Option<Res<PoseBank>>, keys: Res<ButtonInput<KeyCode>>,
    mut req: Query<&mut ModelRequest, With<Player>>, mut buttons: Query<(&Interaction, &UiAction, &mut BackgroundColor), Changed<Interaction>>) {
    if studio.clips.is_empty() {
        if let Some(bank) = bank {
            studio.clips = bank.tracks.keys().filter(|n| allowed(n)).cloned().collect();
            studio.clips.sort();
            if let Some(first) = studio.clips.first() { studio.clip = first.clone(); }
        }
    }
    let mut action = None;
    for (interaction, choice, mut bg) in &mut buttons {
        bg.0 = if *interaction == Interaction::Hovered { Color::srgb(0.43, 0.33, 0.26) } else { Color::srgb(0.22, 0.24, 0.26) };
        if *interaction == Interaction::Pressed { action = Some(*choice); }
    }
    if keys.just_pressed(KeyCode::KeyM) { action = Some(UiAction::Model(1)); }
    if keys.just_pressed(KeyCode::KeyN) { action = Some(UiAction::Tint(1)); }
    if keys.just_pressed(KeyCode::KeyP) { action = Some(UiAction::Play); }
    if keys.just_pressed(KeyCode::PageDown) { action = Some(UiAction::Page(1)); }
    if keys.just_pressed(KeyCode::PageUp) { action = Some(UiAction::Page(-1)); }
    match action {
        Some(UiAction::Model(d)) => { studio.model = (studio.model as i32 + d as i32).rem_euclid(MODELS.len() as i32) as usize; }
        Some(UiAction::Tint(d)) => { studio.tint = (studio.tint as i32 + d as i32).rem_euclid(TINTS.len() as i32) as usize; }
        Some(UiAction::Page(d)) => { let pages = studio.clips.len().div_ceil(8).max(1); studio.page = (studio.page as i32 + d as i32).rem_euclid(pages as i32) as usize; }
        Some(UiAction::Clip(i)) => { if let Some(n) = studio.clips.get(studio.page * 8 + i).cloned() { studio.clip = n; studio.clip_time = 0.0; studio.preview = true; } }
        Some(UiAction::Play) => { studio.preview = !studio.preview; studio.clip_time = 0.0; }
        None => {}
    }
    if matches!(action, Some(UiAction::Model(_) | UiAction::Tint(_))) {
        if let Ok(mut req) = req.single_mut() { req.path = MODELS[studio.model].1.into(); req.tint = Some(TINTS[studio.tint]); }
    }
}

fn input(keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>, mut studio: ResMut<Studio>, mut player: Query<&mut Intent, With<Player>>) {
    if mouse.pressed(MouseButton::Right) {
        studio.yaw -= motion.delta.x * 0.004;
        studio.pitch = (studio.pitch + motion.delta.y * 0.003).clamp(-0.6, 1.15);
    }
    let wheel = if scroll.unit == MouseScrollUnit::Line { scroll.delta.y } else { scroll.delta.y / 40.0 };
    studio.distance = (studio.distance - wheel * 0.5).clamp(2.1, 8.0);
    let Ok(mut intent) = player.single_mut() else { return; };
    let x = (keys.pressed(KeyCode::KeyD) as i32 - keys.pressed(KeyCode::KeyA) as i32) as f32;
    let z = (keys.pressed(KeyCode::KeyW) as i32 - keys.pressed(KeyCode::KeyS) as i32) as f32;
    intent.move_dir = (Quat::from_rotation_y(studio.yaw) * Vec3::new(x, 0.0, z)).normalize_or_zero();
    intent.sprint = keys.pressed(KeyCode::ShiftLeft);
    intent.walk = keys.pressed(KeyCode::AltLeft);
    intent.crouch = keys.pressed(KeyCode::ControlLeft);
    intent.jump_pressed = keys.just_pressed(KeyCode::Space);
    intent.jump_held = keys.pressed(KeyCode::Space);
    intent.primary_pressed = mouse.just_pressed(MouseButton::Left);
    intent.secondary_pressed = keys.just_pressed(KeyCode::KeyK);
}

fn attack(time: Res<Time>, mut studio: ResMut<Studio>, mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>,
    mut player: Query<(&Transform, &Intent, &mut Mover), With<Player>>) {
    let Ok((transform, intent, mut mover)) = player.single_mut() else { return; };
    if intent.primary_pressed || intent.secondary_pressed {
        studio.attack_clip = if intent.secondary_pressed { "brawler/Roundhouse_Kick".into() }
            else { let clips = ["ual1/Punch_Jab", "ual1/Punch_Cross", "sparking/DBZ_Rush_01"];
                let n = clips[studio.combo % clips.len()]; studio.combo += 1; n.into() };
        studio.attack_time = 0.0;
        studio.attack_hit = false;
        studio.preview = false;
        mover.face = Some(mover.yaw);
    }
    if !studio.attack_clip.is_empty() {
        studio.attack_time += time.delta_secs();
        let threshold = if studio.attack_clip.contains("Kick") { 0.31 } else { 0.22 };
        if studio.attack_time >= threshold && !studio.attack_hit {
            studio.attack_hit = true;
            let from = transform.translation + Vec3::Y * 0.45;
            let forward = mover.forward();
            let reach = if studio.attack_clip.contains("Kick") { 1.65 } else { 1.25 };
            let dz = FRONT - from.z;
            if forward.z > 0.35 && dz > -0.05 && dz / forward.z < reach {
                let x = from.x + forward.x * dz / forward.z;
                let y = from.y + if studio.attack_clip.contains("Kick") { -0.1 } else { 0.25 };
                if x.abs() < W * 0.5 - 0.2 && y > 0.2 && y < H - 0.1 {
                    studio.dents.push((Vec2::new(x, y), 0.12, 0.42));
                    if studio.dents.len() > 60 { studio.dents.remove(0); }
                    if let Some(mut mesh) = meshes.get_mut(&studio.wall) { *mesh = wall_mesh(&studio.dents); }
                    let chip_mesh = meshes.add(Cuboid::new(0.045, 0.055, 0.035));
                    let chip_mat = materials.add(Color::srgb(0.54, 0.55, 0.56));
                    for i in 0..4 {
                        let scatter = Vec3::new((i as f32 - 1.5) * 0.36, 0.5 + i as f32 * 0.19, -1.5 - i as f32 * 0.17);
                        commands.spawn((Name::new("Wall chip"), Chip { life: 2.0 }, Mesh3d(chip_mesh.clone()),
                            MeshMaterial3d(chip_mat.clone()), Transform::from_xyz(x, y, FRONT - 0.06),
                            RigidBody::Dynamic, Collider::cuboid(0.045, 0.055, 0.035),
                            CollisionLayers::new(Phys::World, [Phys::World]), LinearVelocity(scatter)));
                    }
                }
            }
        }
        if studio.attack_time > 0.8 { studio.attack_clip.clear(); mover.face = None; }
    }
}

fn select_motion(time: Res<Time>, bank: Option<Res<PoseBank>>, mut studio: ResMut<Studio>,
    mut player: Query<(&Transform, &Mover, &Intent, &mut MotionLayers), With<Player>>) {
    let Some(bank) = bank else { return; };
    let Ok((transform, mover, intent, mut layers)) = player.single_mut() else { return; };
    studio.clip_time += time.delta_secs();
    let base = if studio.preview { studio.clip.as_str() }
        else if mover.mode == MoveMode::Climb { "ual2/ClimbUp_1m" }
        else if !mover.grounded { if mover.velocity.y > 0.0 { "ual1/Jump_Loop" } else { "ual1/Jump_Land" } }
        else if mover.crouch { "ual1/Crouch_Idle_Loop" }
        else if intent.move_dir.length_squared() > 0.01 { if mover.sprinting { "ual1/Sprint_Loop" } else { "ual1/Jog_Fwd_Loop" } }
        else { "ual1/Idle_Loop" };
    let base = if bank.has(base) { base } else { "ual2/Idle_Loop" };
    layers.layers.clear();
    layers.layers.push(Layer { source: Source::Track { name: base.into(), time: studio.clip_time, looping: true }, weight: 1.0, mask: Mask::Full, style: None });
    if !studio.attack_clip.is_empty() && bank.has(&studio.attack_clip) {
        layers.layers.push(Layer { source: Source::Track { name: studio.attack_clip.clone(), time: studio.attack_time,
            looping: false }, weight: 1.0, mask: Mask::Full, style: None });
    }
    layers.contact.foot_ik = if mover.mode == MoveMode::Climb { 0.0 } else { 1.0 };
    layers.contact.accel = mover.accel;
    layers.contact.root_ease = mover.step_ease;
    layers.contact.wall_clear = mover.mode == MoveMode::Climb;
    layers.contact.hands = [None, None];
    layers.contact.feet = [None, None];
    if mover.mode == MoveMode::Climb {
        let front = Vec3::new(transform.translation.x, H - 0.1, FRONT - 0.03);
        layers.contact.hands = [Some((front + Vec3::X * 0.24, 0.85)), Some((front - Vec3::X * 0.24, 0.85))];
    } else if !studio.attack_clip.is_empty() && studio.attack_time > 0.12 && studio.attack_time < 0.42 {
        let at = mover.forward();
        if at.z > 0.35 && FRONT - transform.translation.z < 1.4 {
            let target = Vec3::new(transform.translation.x, transform.translation.y + 0.28, FRONT - 0.06);
            if studio.attack_clip.contains("Kick") { layers.contact.feet[1] = Some((target - Vec3::Y * 0.55, 0.75)); }
            else { layers.contact.hands[if studio.attack_clip.contains("Jab") { 0 } else { 1 }] = Some((target, 0.7)); }
        }
    }
}

fn climb(time: Res<Time<Fixed>>, mut player: Query<(&mut Transform, &mut Mover, &Intent), With<Player>>) {
    let Ok((mut transform, mut mover, intent)) = player.single_mut() else { return; };
    let dt = time.delta_secs();
    let at_front = transform.translation.z > FRONT - 1.0 && transform.translation.z < FRONT - 0.28
        && transform.translation.x.abs() < W * 0.5 - 0.3;
    if mover.mode != MoveMode::Climb && at_front && intent.jump_held && intent.move_dir.z > 0.3 {
        mover.mode = MoveMode::Climb; mover.grounded = false; mover.velocity = Vec3::ZERO;
    }
    if mover.mode == MoveMode::Climb {
        let feet = transform.translation.y - HALF_HEIGHT;
        if feet < H + 0.08 {
            transform.translation.z = FRONT - CAPSULE_RADIUS - 0.04;
            transform.translation.y = (transform.translation.y + 2.5 * dt).min(H + HALF_HEIGHT + 0.1);
            mover.velocity = Vec3::Y * 2.5;
        } else {
            transform.translation.z += 2.2 * dt;
            mover.velocity = Vec3::Z * 2.2;
            if transform.translation.z > WALL_Z + 0.18 + CAPSULE_RADIUS + 0.1 {
                mover.mode = MoveMode::Air;
            }
        }
    }
}

fn camera_follow(time: Res<Time>, studio: Res<Studio>, spatial: SpatialQuery, player: Query<&Transform, With<Player>>,
    mut camera: Query<&mut Transform, (With<StudioCamera>, Without<Player>)>) {
    let (Ok(target), Ok(mut camera)) = (player.single(), camera.single_mut()) else { return; };
    let focus = target.translation + Vec3::Y * 0.45;
    let rot = Quat::from_rotation_y(studio.yaw) * Quat::from_rotation_x(studio.pitch);
    let mut desired = focus + rot * Vec3::new(0.0, 0.0, -studio.distance);
    let boom = desired - focus;
    if let Ok(dir) = Dir3::new(boom) {
        if let Some(hit) = spatial.cast_ray(focus, dir, boom.length(), true, &SpatialQueryFilter::from_mask([Phys::World])) {
            desired = focus + *dir * (hit.distance - 0.18).max(0.4);
        }
    }
    camera.translation = camera.translation.lerp(desired, 1.0 - (-12.0 * time.delta_secs()).exp());
    camera.look_at(focus, Vec3::Y);
}

fn update_ui(studio: Res<Studio>, mut model: Query<&mut Text, (With<ModelText>, Without<ClipText>, Without<StatusText>, Without<PageText>)>,
    mut clips: Query<(&ClipText, &mut Text)>, mut page: Query<&mut Text, (With<PageText>, Without<ClipText>, Without<ModelText>, Without<StatusText>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<ClipText>, Without<ModelText>, Without<PageText>)>) {
    if let Ok(mut t) = model.single_mut() { **t = format!("{}  /  palette {}", MODELS[studio.model].0, studio.tint + 1); }
    if let Ok(mut t) = page.single_mut() { **t = format!("{} clips  •  page {}/{}", studio.clips.len(), studio.page + 1, studio.clips.len().div_ceil(8).max(1)); }
    for (index, mut t) in &mut clips {
        **t = studio.clips.get(studio.page * 8 + index.0).map(|n| {
            format!("{} {}", if n == &studio.clip { "▶" } else { " " }, n)
        }).unwrap_or_default();
    }
    if let Ok(mut t) = status.single_mut() { **t = if studio.preview { format!("PLAYING  {}", studio.clip) }
        else if !studio.attack_clip.is_empty() { format!("ATTACK  {}", studio.attack_clip) }
        else { "READY  /  hit the wall".into() }; }
}

fn chips(time: Res<Time>, mut commands: Commands, mut chips: Query<(Entity, &mut Chip)>) {
    for (e, mut c) in &mut chips { c.life -= time.delta_secs(); if c.life <= 0.0 { commands.entity(e).despawn(); } }
}

fn wall_mesh(dents: &[(Vec2, f32, f32)]) -> Mesh {
    let mut positions = Vec::with_capacity((NX + 1) * (NY + 1));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());
    for y in 0..=NY { for x in 0..=NX {
        let px = -W * 0.5 + W * x as f32 / NX as f32;
        let py = H * y as f32 / NY as f32;
        let mut depth = 0.0;
        let mut dx = 0.0; let mut dy = 0.0;
        for (center, amount, radius) in dents {
            let a = (px - center.x) / radius; let b = (py - center.y) / radius;
            let g = (-3.5 * (a * a + b * b)).exp();
            depth += amount * g;
            dx += amount * g * -7.0 * a / radius;
            dy += amount * g * -7.0 * b / radius;
        }
        positions.push([px, py, FRONT + depth]);
        let n = Vec3::new(dx, dy, -1.0).normalize();
        normals.push(n.to_array());
        uvs.push([x as f32 / NX as f32, y as f32 / NY as f32]);
    }}
    let mut indices = Vec::with_capacity(NX * NY * 6);
    for y in 0..NY { for x in 0..NX {
        let i = (y * (NX + 1) + x) as u32;
        indices.extend_from_slice(&[i, i + NX as u32 + 1, i + 1, i + 1, i + NX as u32 + 1, i + NX as u32 + 2]);
    }}
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

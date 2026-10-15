//! The only UI: a small model picker (plain Bevy UI). Click a name or press 1..N to swap the possessed
//! (player) model instantly; its rig profile is loaded via rig::segments_for. Hidden in `--stand` runs.

use bevy::prelude::*;

use crate::body::{Bodies, SimRes, despawn_body, spawn_body};
use crate::rig;

#[derive(Component)]
struct PickButton(usize);

#[derive(Resource)]
pub struct Models(pub Vec<String>);

pub fn plugin(app: &mut App) {
    app.insert_resource(Models(rig::available_models()))
        .add_systems(Startup, spawn_ui)
        .add_systems(Update, (pick, highlight));
}

fn spawn_ui(mut commands: Commands, models: Res<Models>) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            right: px(8.0),
            top: px(8.0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4.0),
            ..default()
        })
        .with_children(|ui| {
            for (i, m) in models.0.iter().enumerate() {
                ui.spawn((
                    Button,
                    PickButton(i),
                    Node {
                        padding: UiRect::axes(px(8.0), px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.7)),
                ))
                .with_child((
                    Text::new(format!("{} {}", i + 1, m)),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(14.0),
                        ..default()
                    },
                ));
            }
        });
}

/// Swap the player body (index 0) to `model` in place: despawn, respawn at the same spot, keep list order.
pub fn swap_player(
    commands: &mut Commands,
    sim: &mut crate::body::SimRes,
    bodies: &mut Bodies,
    model: &str,
    at: Vec3,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) {
    if bodies.list.is_empty() {
        return;
    }
    rig::log_rig(model, &rig::segments_for(model));
    despawn_body(commands, &mut sim.sim, &bodies.list[0]);
    let mut p = spawn_body(
        commands,
        &mut sim.sim,
        &rig::segments_for(model),
        at,
        Some(crate::voxel::Style::for_model(model)),
        meshes,
        mats,
    );
    p.source = crate::source::SourceMotion::load(model).ok();
    p.model = model.to_string();
    bodies.list[0] = p;
    if let Some(state) = bodies.last.first_mut() {
        *state = sim.sim.read_state(bodies.list[0].id);
    }
    if let Some(ps) = bodies.procs.get_mut(0) {
        *ps = Default::default();
    }
    if let Some(boost) = bodies.boosts.get_mut(0) {
        *boost = Default::default();
    }
    if let Some(g) = bodies.gaits.get_mut(0) {
        *g = Default::default();
    }
    if let Some(v) = bodies.v_want.get_mut(0) {
        *v = Vec3::ZERO;
    }
    if let Some(h) = bodies.heading_want.get_mut(0) {
        *h = None;
    }
    if let Some(r) = bodies.rest_h.get_mut(0) {
        *r = None;
    }
    if let Some(a) = bodies.age.get_mut(0) {
        *a = 0;
    }
    if let Some(a) = bodies.settle.get_mut(0) {
        *a = Default::default();
    }
    if let Some(a) = bodies.limp.get_mut(0) {
        *a = false;
    }
    if let Some(g) = bodies.getup.get_mut(0) {
        *g = None;
    }
    if let Some(x) = bodies.pending.get_mut(0) {
        *x = None;
    }
}

fn pick(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    models: Res<Models>,
    clicks: Query<(&Interaction, &PickButton), Changed<Interaction>>,
    mut sim: ResMut<SimRes>,
    mut bodies: ResMut<Bodies>,
    mut actions: ResMut<crate::input::Actions>,
    pos: Query<&avian3d::prelude::Position>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    let mut want = DIGITS
        .iter()
        .position(|k| keys.just_pressed(*k))
        .filter(|&i| i < models.0.len());
    for (int, b) in &clicks {
        if *int == Interaction::Pressed {
            want = Some(b.0);
        }
    }
    let Some(i) = want else { return };
    let Some(p) = bodies.list.first() else { return };
    let at = pos
        .get(p.links[0])
        .map(|x| Vec3::new(x.0.x, 0.0, x.0.z))
        .unwrap_or(Vec3::ZERO);
    swap_player(
        &mut commands,
        &mut sim,
        &mut bodies,
        &models.0[i],
        at,
        &mut meshes,
        &mut mats,
    );
    *actions = Default::default();
}

fn highlight(
    bodies: Res<Bodies>,
    models: Res<Models>,
    mut q: Query<(&PickButton, &mut BackgroundColor)>,
) {
    let cur = bodies.list.first().map(|p| p.model.as_str()).unwrap_or("");
    for (b, mut c) in &mut q {
        c.0 = if models.0.get(b.0).map(|m| m == cur).unwrap_or(false) {
            Color::srgba(0.2, 0.45, 0.25, 0.85)
        } else {
            Color::srgba(0.1, 0.1, 0.12, 0.7)
        };
    }
}

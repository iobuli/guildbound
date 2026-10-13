//! Toon look (assets/shaders/toon.wgsl): every standard material in the world is swapped for a toon version as it
//! appears (same textures and colours, lighting banded into flat steps and a rim light). F7 switches between the
//! toon look and the standard one. The Creator preview keeps standard materials (its colour sliders edit them).
use std::collections::HashMap;

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Toon {}

impl MaterialExtension for Toon {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wgsl".into()
    }
}

pub type ToonMaterial = ExtendedMaterial<StandardMaterial, Toon>;

/// Leave this entity and everything under it standard.
#[derive(Component)]
pub struct NoToon;

/// The standard material a toon mesh came from (to switch back).
#[derive(Component)]
struct FromStandard(Handle<StandardMaterial>);

#[derive(Resource)]
pub struct ToonLook {
    pub on: bool,
    cache: HashMap<AssetId<StandardMaterial>, Handle<ToonMaterial>>,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<ToonMaterial>::default())
        .insert_resource(ToonLook { on: true, cache: HashMap::new() })
        .add_systems(PostUpdate, (toggle, sync_materials, convert).chain());
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<bevy_egui::input::EguiWantsInput>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    mut look: ResMut<ToonLook>,
    mut commands: Commands,
    toons: Query<(Entity, &FromStandard)>,
) {
    if window.focused && !wants.wants_any_keyboard_input() && keys.just_pressed(KeyCode::F7) {
        look.on = !look.on;
    }
    if !look.on {
        for (e, from) in &toons {
            commands.entity(e).remove::<(MeshMaterial3d<ToonMaterial>, FromStandard)>().insert(MeshMaterial3d(from.0.clone()));
        }
    }
}

fn sync_materials(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    mut look: ResMut<ToonLook>,
    standard: Res<Assets<StandardMaterial>>,
    mut toon: ResMut<Assets<ToonMaterial>>,
) {
    for event in events.read() {
        match event {
            AssetEvent::Modified { id } => {
                if let Some(handle) = look.cache.get(id)
                    && let Some(base) = standard.get(*id)
                    && let Some(mut material) = toon.get_mut(handle)
                {
                    material.base = base.clone();
                }
            }
            AssetEvent::Removed { id } => {
                look.cache.remove(id);
            }
            _ => {}
        }
    }
}

fn convert(
    mut commands: Commands,
    mut look: ResMut<ToonLook>,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>)>,
    parents: Query<&ChildOf>,
    excluded: Query<(), With<NoToon>>,
    standard: Res<Assets<StandardMaterial>>,
    mut toon: ResMut<Assets<ToonMaterial>>,
) {
    if !look.on {
        return;
    }
    for (e, m) in &meshes {
        if excluded.contains(e) || parents.iter_ancestors(e).any(|a| excluded.contains(a)) {
            continue;
        }
        let Some(base) = standard.get(&m.0) else { continue };
        if base.unlit {
            continue;
        }
        let handle = look.cache.entry(m.0.id()).or_insert_with(|| toon.add(ToonMaterial { base: base.clone(), extension: Toon {} })).clone();
        commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert((MeshMaterial3d(handle), FromStandard(m.0.clone())));
    }
}

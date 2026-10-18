//! Ground and weathering materials (`assets/shaders/ground.wgsl`, `weather.wgsl`).
//! - The terrain's standard material is swapped for `GroundMaterial` once it appears: the painted
//!   vertex colours get procedural macro drifts, triplanar rock strata on steep faces, grass tone
//!   breakup and mottled, pebbled worn ground (footpaths, the ruins: the vertex alpha). Debug view
//!   12 ("ground layers") shows the masks instead.
//! - Stone and wood (`Weathering`, the palette materials `world` lists) are swapped for
//!   `WeatherMaterial` wherever they are used: moss on top faces, grime streaks down the sides,
//!   pale wear specks. One weathered copy per source material, shared by every prop.
use super::debug::{DebugToggles, View};
use super::world::Terrain;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
pub struct GroundParams {
    pub look: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GroundExt {
    #[uniform(100)]
    pub ground: GroundParams,
}

impl MaterialExtension for GroundExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/ground.wgsl".into()
    }
}

pub type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundExt>;

#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default)]
pub struct WeatherParams {
    pub amount: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WeatherExt {
    #[uniform(100)]
    pub weather: WeatherParams,
}

impl MaterialExtension for WeatherExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/weather.wgsl".into()
    }
}

pub type WeatherMaterial = ExtendedMaterial<StandardMaterial, WeatherExt>;

/// Moss, grime and wear for stone; wood gets less moss and more grime.
const STONE: Vec4 = Vec4::new(0.75, 0.8, 0.7, 0.0);
const WOOD: Vec4 = Vec4::new(0.3, 1.0, 0.4, 0.0);
/// Detail strength on the ground.
const DETAIL: f32 = 1.0;

/// The palette materials that weather, and their weathered copies (made on first use).
#[derive(Resource, Default)]
pub struct Weathering {
    /// (source, is stone)
    sources: Vec<(Handle<StandardMaterial>, bool)>,
    made: HashMap<AssetId<StandardMaterial>, Handle<WeatherMaterial>>,
}

impl Weathering {
    /// Stone first (stone, rock, wall), then wood (wood, dark wood).
    pub fn new(handles: [Handle<StandardMaterial>; 5]) -> Self {
        let sources = handles.into_iter().enumerate().map(|(i, h)| (h, i < 3)).collect();
        Self { sources, made: HashMap::new() }
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((MaterialPlugin::<GroundMaterial>::default(), MaterialPlugin::<WeatherMaterial>::default()))
        .init_resource::<Weathering>()
        .add_systems(Update, (terrain, weather, layers));
}

/// Swap the terrain onto the ground material.
fn terrain(
    mut commands: Commands,
    terrain: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Terrain>>,
    standard: Res<Assets<StandardMaterial>>,
    mut grounds: ResMut<Assets<GroundMaterial>>,
) {
    for (e, m) in &terrain {
        let Some(base) = standard.get(&m.0).cloned() else { continue };
        let g = grounds.add(GroundMaterial { base, extension: GroundExt { ground: GroundParams { look: Vec4::new(0.0, DETAIL, 0.0, 0.0) } } });
        commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(g));
    }
}

/// Swap stone and wood props onto their weathered copies as they appear.
fn weather(
    mut commands: Commands,
    mut w: ResMut<Weathering>,
    props: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    standard: Res<Assets<StandardMaterial>>,
    mut weathered: ResMut<Assets<WeatherMaterial>>,
) {
    if w.sources.is_empty() {
        return;
    }
    for (e, m) in &props {
        let Some(stone) = w.sources.iter().find(|(h, _)| h.id() == m.0.id()).map(|(_, s)| *s) else { continue };
        let id = m.0.id();
        let handle = match w.made.get(&id) {
            Some(h) => h.clone(),
            None => {
                let Some(base) = standard.get(&m.0).cloned() else { continue };
                let amount = if stone { STONE } else { WOOD };
                let h = weathered.add(WeatherMaterial { base, extension: WeatherExt { weather: WeatherParams { amount } } });
                w.made.insert(id, h.clone());
                h
            }
        };
        commands.entity(e).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(handle));
    }
}

/// Debug view 12: the ground shows its masks.
fn layers(toggles: Res<DebugToggles>, terrain: Query<&MeshMaterial3d<GroundMaterial>>, mut grounds: ResMut<Assets<GroundMaterial>>) {
    if !toggles.is_changed() {
        return;
    }
    let show = if toggles.get(View::GroundLayers) { 1.0 } else { 0.0 };
    for m in &terrain {
        if let Some(mut g) = grounds.get_mut(&m.0) {
            g.extension.ground.look.x = show;
        }
    }
}

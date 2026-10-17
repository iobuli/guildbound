//! The glass surface: a UI material drawn over a panel's background colour (scanlines, a slow
//! bright sweep, grain, an inner glow at the edges; `shaders/gbui/glass.wgsl`). Every `Glass`
//! node shares one material whose parameters follow `Theme::glass`, and takes its parent's
//! corner radius so the surface stays inside the panel.
use crate::GLASS_SHADER;
use crate::style::Glass;
use crate::theme::{GlassLook, Theme};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct GlassMaterial {
    #[uniform(0)]
    pub params: GlassParams,
}

#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct GlassParams {
    /// Linear RGBA.
    pub tint: Vec4,
    /// Scanline alpha, scanline spacing (px), grain alpha, edge glow alpha.
    pub look: Vec4,
    /// Sweep alpha, sweep period (s), unused, unused.
    pub sweep: Vec4,
}

impl GlassParams {
    pub fn from_look(look: &GlassLook) -> Self {
        Self {
            tint: LinearRgba::from(look.tint).to_vec4(),
            look: Vec4::new(look.scanlines, look.spacing.max(1.0), look.grain, look.edge),
            sweep: Vec4::new(look.sweep, look.sweep_period.max(0.1), 0.0, 0.0),
        }
    }
}

impl UiMaterial for GlassMaterial {
    fn fragment_shader() -> ShaderRef {
        GLASS_SHADER.into()
    }
}

/// The one material every glass node uses.
#[derive(Resource, Clone, Debug)]
pub struct GlassHandle(pub Handle<GlassMaterial>);

pub fn init(mut commands: Commands, theme: Res<Theme>, mut materials: ResMut<Assets<GlassMaterial>>) {
    let handle = materials.add(GlassMaterial {
        params: GlassParams::from_look(&theme.glass),
    });
    commands.insert_resource(GlassHandle(handle));
}

pub fn attach(mut commands: Commands, handle: Option<Res<GlassHandle>>, added: Query<Entity, Added<Glass>>) {
    let Some(handle) = handle else { return };
    for entity in &added {
        commands.entity(entity).insert(MaterialNode(handle.0.clone()));
    }
}

pub fn follow_theme(theme: Res<Theme>, handle: Option<Res<GlassHandle>>, mut materials: ResMut<Assets<GlassMaterial>>) {
    if !theme.is_changed() {
        return;
    }
    let Some(handle) = handle else { return };
    let want = GlassParams::from_look(&theme.glass);
    if materials.get(&handle.0).is_some_and(|m| m.params != want)
        && let Some(mut m) = materials.get_mut(&handle.0)
    {
        m.params = want;
    }
}

#[allow(clippy::type_complexity)]
pub fn match_radius(mut glass: Query<(&ChildOf, &mut Node), With<Glass>>, parents: Query<&Node, Without<Glass>>) {
    for (child_of, mut node) in &mut glass {
        if let Ok(parent) = parents.get(child_of.parent())
            && node.border_radius != parent.border_radius
        {
            node.border_radius = parent.border_radius;
        }
    }
}

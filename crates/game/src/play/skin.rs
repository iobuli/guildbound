//! Character skin (`assets/shaders/skin.wgsl`): once a character's model is bound (`motion::Rig`),
//! its skinned body meshes swap their `StandardMaterial` for a `SkinMaterial` (the same standard
//! material extended), one copy per character, so each can be shaded on its own:
//! - Look: ink (toon bands, an ink silhouette line and shadow hatching) by default; debug view
//!   "alt skin shader" (`debug::View::ShaderMode`) switches every character to plain lighting.
//! - Accent: the shadow bands lean toward `Skin.accent` (per-character palette; `looks` sets it).
//! - Presence (enemies with `senses::Awareness`): a rim glow that warms from cool blue to amber to
//!   red as they notice you, and at the moment they are alerted, a white "blink" flash over the
//!   silhouette with a glint at the eyes.
//! Only skinned meshes are swapped, so weapons and props keep their own materials.
use super::debug::{DebugToggles, View};
use super::senses::Awareness;
use super::vfx::{self, Element, Vfx};
use crate::motion::Rig;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

const SHADER: &str = "shaders/skin.wgsl";
/// Alert flash length (s); eye height above the body centre (m).
const FLASH: f32 = 0.25;
const EYES: f32 = 0.62;

#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub struct SkinParams {
    pub rim: Vec4,
    pub look: Vec4,
    pub tint: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SkinExt {
    #[uniform(100)]
    pub skin: SkinParams,
}

impl MaterialExtension for SkinExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub type SkinMaterial = ExtendedMaterial<StandardMaterial, SkinExt>;

/// A character's skin materials and presence state (written here only, `accent` by `looks`).
#[derive(Component, Clone, Debug, Default)]
pub struct Skin {
    pub materials: Vec<Handle<SkinMaterial>>,
    /// Shadow accent: rgb, amount.
    pub accent: Vec4,
    flash: f32,
    alerted: bool,
    last: SkinParams,
}

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<SkinMaterial>::default()).add_systems(Update, (dress, presence).chain());
}

/// Give every newly bound character its own skin materials.
/// Swaps every skinned mesh under a rigged model that still has a `StandardMaterial`, so parts
/// that arrive later (a modelled look's clothes and hair, `appearance`) get the skin too. A root
/// being dressed by `appearance` waits until its look is applied, so the swap keeps the look's
/// recoloured material as its base.
#[allow(clippy::type_complexity)]
fn dress(
    mut commands: Commands,
    mut rigs: Query<(Entity, &Rig, Option<&mut Skin>, Option<&super::appearance::Appearance>, Option<&super::appearance::Dressed>)>,
    children: Query<&Children>,
    bodies: Query<&MeshMaterial3d<StandardMaterial>, With<SkinnedMesh>>,
    standard: Res<Assets<StandardMaterial>>,
    mut skins: ResMut<Assets<SkinMaterial>>,
) {
    for (e, rig, skin, look, dressed) in &mut rigs {
        if look.is_some() && dressed.is_none_or(|d| d.0 != rig.model) {
            continue;
        }
        let params = skin.as_ref().map_or(SkinParams::default(), |s| s.last);
        let mut made = Vec::new();
        for d in children.iter_descendants(rig.model) {
            let Ok(handle) = bodies.get(d) else { continue };
            let Some(base) = standard.get(&handle.0).cloned() else { continue };
            let made_skin = skins.add(SkinMaterial { base, extension: SkinExt { skin: params } });
            commands.entity(d).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(made_skin.clone()));
            made.push(made_skin);
        }
        if made.is_empty() {
            continue;
        }
        match skin {
            Some(mut s) => s.materials.extend(made),
            None => {
                commands.entity(e).try_insert(Skin { materials: made, last: SkinParams { look: Vec4::splat(-1.0), ..default() }, ..default() });
            }
        }
    }
}

/// The presence rim colour for an awareness level (0..1) and whether alerted.
pub fn rim(level: f32, alerted: bool) -> Vec4 {
    if alerted {
        return Vec4::new(1.0, 0.18, 0.12, 0.9);
    }
    let l = level.clamp(0.0, 1.0);
    let c = Vec3::new(0.3, 0.6, 1.0).lerp(Vec3::new(1.0, 0.7, 0.15), l);
    c.extend(0.9 * l)
}

/// Write each character's look, accent and presence into its materials (only when they change).
fn presence(
    time: Res<Time>,
    toggles: Res<DebugToggles>,
    vfx: Option<Res<Vfx>>,
    mut commands: Commands,
    mut skins: ResMut<Assets<SkinMaterial>>,
    mut characters: Query<(&Transform, &mut Skin, Option<&Awareness>)>,
) {
    let dt = time.delta_secs();
    let mode = if toggles.get(View::ShaderMode) { 0.0 } else { 2.0 };
    for (tf, mut skin, awareness) in &mut characters {
        let (level, alerted) = awareness.map_or((0.0, false), |a| (a.level, a.alerted));
        if alerted && !skin.alerted {
            skin.flash = FLASH;
            if let Some(vfx) = vfx.as_deref() {
                vfx::burst(&mut commands, vfx.burst(Element::Lightning), tf.translation + Vec3::Y * EYES + tf.rotation * Vec3::Z * 0.1, 0.35);
            }
        }
        skin.alerted = alerted;
        skin.flash = (skin.flash - dt).max(0.0);
        let params = SkinParams {
            rim: if awareness.is_some() { rim(level, alerted) } else { Vec4::ZERO },
            look: Vec4::new(skin.flash / FLASH, mode, 1.0, 0.0),
            tint: skin.accent,
        };
        // Small rim changes are not worth a re-upload.
        let close = (params.rim - skin.last.rim).abs().max_element() < 0.02 && params.look == skin.last.look && params.tint == skin.last.tint;
        if close {
            continue;
        }
        skin.last = params;
        for h in &skin.materials {
            if let Some(mut m) = skins.get_mut(h) {
                m.extension.skin = params;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rim_warms_with_awareness() {
        assert_eq!(rim(0.0, false).w, 0.0, "unaware: no rim");
        let mid = rim(0.5, false);
        assert!(mid.w > 0.3 && mid.x > rim(0.0, false).x, "warmer and stronger as it notices");
        let red = rim(0.2, true);
        assert!(red.x > 0.9 && red.y < 0.3, "alerted: red");
    }
}

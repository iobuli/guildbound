//! gbui: Guildbound's UI toolkit. LVGL's design (objects styled per state, themes, `lv_anim`-style
//! tweens, label effects, bars) rebuilt in Rust on Bevy UI, which already draws on the GPU.
//!
//! - `theme`: `Theme` resource (palette, classes with per-state styles, glass and ring looks),
//!   read from a JSON file and re-read when its mtime changes; `UiFonts`.
//! - `style`: `Styled(class)`, `Tint`, `UiFlags`, `Opacity`, `Glass`; `style::apply` resolves them
//!   into Bevy's colour, border, shadow and font components every frame.
//! - `anim`: `Tweens` (opacity, offset, scale, rotation, width, height) with easing, delay,
//!   repeat, yoyo; `Blink`.
//! - `term`: terminal text: `Typewriter`, `Decode` (scramble one text into another), `TermLog`
//!   written through `LogWrite` messages.
//! - `widgets`: `panel`, `label`, `bar` (`Bar` with a damage trail), `slot`, `Tooltip`.
//! - `glass`, `ring`: the UI materials (`shaders/gbui/glass.wgsl`, `shaders/gbui/ring.wgsl` under
//!   the app's asset root).
//!
//! Frame order (PostUpdate, before `UiSystems::Prepare`): `GbuiSet::Animate` (theme reload, logs,
//! text effects, tweens, bars, tooltip) then `GbuiSet::Style` (`style::apply`), so layout and
//! rendering see this frame's values. Animation runs on real time: hitstop and pause don't freeze
//! the UI.
//!
//! The app adds `GbuiPlugin { theme }` with the theme file's filesystem path. Nothing here knows
//! about the game; `cargo test -p gbui` runs headless.

pub mod anim;
pub mod glass;
pub mod ring;
pub mod style;
pub mod term;
pub mod theme;
pub mod widgets;

use bevy::prelude::*;
use bevy::ui::UiSystems;
use std::path::PathBuf;

pub mod prelude {
    pub use crate::GbuiSet;
    pub use crate::anim::{Blink, Done, Easing, Prop, Repeat, Tween, Tweens};
    pub use crate::ring::{RingMap, RingParams};
    pub use crate::style::{Glass, Opacity, Styled, Tint, UiFlags};
    pub use crate::term::{Decode, LineEffect, LogWrite, TermLog, Typewriter};
    pub use crate::theme::{Theme, UiFonts};
    pub use crate::widgets::{Bar, SlotCount, SlotKey, SlotLabel, Tooltip, bar, label, panel, slot};
}

/// Asset paths of the shaders, relative to the app's asset root.
pub const GLASS_SHADER: &str = "shaders/gbui/glass.wgsl";
pub const RING_SHADER: &str = "shaders/gbui/ring.wgsl";

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GbuiSet {
    Animate,
    Style,
}

pub struct GbuiPlugin {
    /// Filesystem path of the theme JSON.
    pub theme: PathBuf,
}

impl Plugin for GbuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiMaterialPlugin::<glass::GlassMaterial>::default(),
            UiMaterialPlugin::<ring::RingMap>::default(),
        ))
        .insert_resource(theme::ThemeFile::new(self.theme.clone()))
        .insert_resource(theme::Theme::load(&self.theme))
        .add_message::<term::LogWrite>()
        .configure_sets(
            PostUpdate,
            (GbuiSet::Animate, GbuiSet::Style).chain().before(UiSystems::Prepare),
        )
        .add_systems(Startup, (theme::init_fonts, glass::init, widgets::spawn_tooltip))
        .add_systems(
            PostUpdate,
            (
                (theme::reload, theme::load_fonts).chain(),
                (term::write_logs, term::age_logs, term::type_text, term::decode).chain(),
                (anim::run_tweens, anim::blink).chain(),
                widgets::drive_bars,
                widgets::tooltips,
            )
                .in_set(GbuiSet::Animate),
        )
        .add_systems(
            PostUpdate,
            (glass::attach, glass::follow_theme, glass::match_radius, style::apply)
                .chain()
                .in_set(GbuiSet::Style),
        );
    }
}

#[cfg(test)]
mod tests {
    /// Bevy compiles WGSL only at runtime; parse and validate ours here with the renderer's naga,
    /// standing in the two imported definitions.
    #[test]
    fn shaders_parse_and_validate() {
        const STUBS: &str = "
            struct Globals { time: f32, delta_time: f32, frame_count: u32, }
            struct UiVertexOutput {
                @location(0) uv: vec2<f32>,
                @location(1) border_widths: vec4<f32>,
                @location(2) border_radius: vec4<f32>,
                @location(3) @interpolate(flat) size: vec2<f32>,
                @builtin(position) position: vec4<f32>,
            };
        ";
        for path in [super::GLASS_SHADER, super::RING_SHADER] {
            let file = format!("{}/../../assets/{path}", env!("CARGO_MANIFEST_DIR"));
            let src = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file}: {e}"));
            let body: String = src.lines().filter(|l| !l.trim_start().starts_with("#import")).collect::<Vec<_>>().join("\n");
            let full = format!("{STUBS}\n{body}");
            let module = naga::front::wgsl::parse_str(&full).unwrap_or_else(|e| panic!("{path}: {}", e.emit_to_string(&full)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|e| panic!("{path}: {e:?}"));
            assert!(module.entry_points.iter().any(|e| e.name == "fragment"), "{path}: no fragment entry point");
        }
    }
}

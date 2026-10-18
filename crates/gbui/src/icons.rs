//! Sheet icons: show a sliced icon set in the HUD, and keep the animated parts animated.
//!
//! `tools/ui/sheet_slice.py` cuts a generated sheet into per-icon sprites and writes a
//! `manifest.json` beside a copy of the sheet. This module loads that manifest as one
//! `TextureAtlasLayout` over the sheet texture: every icon becomes an atlas cell, so all of
//! them share a single upload and the manifest stays the one source of truth for what each
//! cell is.
//!
//! Two rules make the art reusable rather than a picture of a finished screen:
//!
//!   * **States are not in the sprite.** A sprite is one glyph. The cast arc, the cooldown
//!     sweep, the pool gauge and the mastered star are child nodes above the icon, driven by
//!     `anim` tweens or a `ring`-style material, so a sheet with state rows baked in is only
//!     ever reference art.
//!   * **Alpha is the overlay control.** An icon's drawn alpha is its own `alpha` times the
//!     inherited `Opacity`, so any caller can place the same sprite as a translucent ghost, a
//!     pool-fill hint or a watermark without a second asset. That is why the sliced sprites must
//!     stay PNG with alpha: converting a sheet to JPEG throws the transparency away.
//!
//! Wiring a slot:
//! ```ignore
//! parent.spawn(widgets::slot("cast.button", 44.0, "Q"))
//!     .with_children(|p| p.spawn(icons::icon("abilities", "cinder_bolt_idle", 30.0)));
//! ```
//!

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use serde_json::Value;

use crate::style::Opacity;

/// What an icon wants drawn, before the inherited opacity is folded in.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IconTint {
    /// A normal, drawable icon.
    #[default]
    Full,
    /// Cannot be paid for: dim.
    Dim,
    /// Mastered.
    Mastered,
    /// The sprite's own colours, alpha only.
    Plain,
}

impl IconTint {
    pub fn alpha(self) -> f32 {
        match self {
            IconTint::Full => 0.9,
            IconTint::Dim => 0.35,
            IconTint::Mastered | IconTint::Plain => 1.0,
        }
    }
}

/// One icon in a slot: `sheet` is a folder under `assets/ui/icons/`, `name` a sprite in it.
#[derive(Component, Clone, Debug)]
pub struct Icon {
    pub sheet: String,
    pub name: String,
    pub tint: IconTint,
    /// Colour multiplied into the sprite (glyphs ship white, so the owner picks the colour).
    pub color: Color,
    /// Extra alpha for overlays: `Icon::overlay(.., 0.25)` draws the same sprite as a ghost.
    pub alpha: f32,
}

impl Icon {
    pub fn new(sheet: impl Into<String>, name: impl Into<String>) -> Self {
        Self { sheet: sheet.into(), name: name.into(), tint: IconTint::Full, color: Color::WHITE, alpha: 1.0 }
    }

    /// The same sprite at `alpha`, for a ghost, a trail or a watermark.
    pub fn overlay(sheet: impl Into<String>, name: impl Into<String>, alpha: f32) -> Self {
        Self { sheet: sheet.into(), name: name.into(), tint: IconTint::Plain, color: Color::WHITE, alpha: alpha.clamp(0.0, 1.0) }
    }

    pub fn tinted(mut self, tint: IconTint) -> Self {
        self.tint = tint;
        self
    }
}

/// Every loaded sheet: one texture and one atlas, keyed by sheet name.
#[derive(Resource, Default)]
pub struct IconSheets {
    sets: HashMap<String, IconSet>,
}

#[derive(Clone)]
pub struct IconSet {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub index: HashMap<String, usize>,
    pub file: String,
}

impl IconSheets {
    pub fn get(&self, sheet: &str) -> Option<&IconSet> {
        self.sets.get(sheet)
    }

    pub fn cell(&self, sheet: &str, name: &str) -> Option<(Handle<Image>, Handle<TextureAtlasLayout>, usize)> {
        let set = self.sets.get(sheet)?;
        Some((set.image.clone(), set.layout.clone(), *set.index.get(name)?))
    }

    /// Load `assets/ui/icons/<sheet>/`, where the slicer left `manifest.json` and `sheet.png`.
    /// The manifest is read the same way `theme.rs` reads the theme: plain JSON values, no
    /// extra derives, so gbui keeps needing only `serde_json`.
    pub fn load(
        &mut self,
        sheet: &str,
        assets: &AssetServer,
        layouts: &mut Assets<TextureAtlasLayout>,
    ) -> Result<(), String> {
        let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/ui/icons")).join(sheet);
        let text = std::fs::read_to_string(dir.join("manifest.json"))
            .map_err(|e| format!("{sheet}: manifest unreadable ({e})"))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{sheet}: bad manifest ({e})"))?;
        let grid = &v["grid"];
        let cell = UVec2::new(
            grid["cell_w"].as_u64().unwrap_or(1).max(1) as u32,
            grid["cell_h"].as_u64().unwrap_or(1).max(1) as u32,
        );
        let cols = grid["cols"].as_u64().unwrap_or(1).max(1) as u32;
        let rows = grid["rows"].as_u64().unwrap_or(1).max(1) as u32;
        let file = v["sheet"].as_str().unwrap_or(sheet).to_string();
        // reading order, so a cell's index is its position on the sheet
        let index = v["icons"]
            .as_array()
            .map(|a| {
                a.iter()
                    .enumerate()
                    .filter_map(|(i, e)| Some((e["name"].as_str()?.to_string(), i)))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let layout = layouts.add(TextureAtlasLayout::from_grid(cell, cols, rows, None, None));
        let image = assets.load(format!("ui/icons/{file}/sheet.png"));
        self.sets.insert(sheet.to_string(), IconSet { image, layout, index, file });
        Ok(())
    }
}

/// A standalone icon node, sized in px. Put it inside the slot it belongs to.
pub fn icon(sheet: &str, name: &str, size: f32) -> impl Bundle {
    (
        Icon::new(sheet, name),
        Node { width: px(size), height: px(size), ..default() },
        ImageNode::default(),
    )
}

/// Point every icon at its atlas cell and fold the inherited `Opacity` into its alpha.
///
/// This is the whole animated/overlay story on the icon's side: a caller tweens `Opacity` (or
/// sets `Icon.alpha`) and the sprite fades, at whatever alpha the theme's class wants. Anything
/// that moves *around* the glyph — arc, sweep, gauge — is a sibling layer, not this node.
pub fn drive_icons(
    sheets: Res<IconSheets>,
    parents: Query<&ChildOf>,
    opacities: Query<&Opacity>,
    mut icons: Query<(Entity, &Icon, &mut ImageNode)>,
) {
    for (entity, icon, mut node) in &mut icons {
        match sheets.cell(&icon.sheet, &icon.name) {
            Some((image, layout, index)) => {
                // Same sheet, another sprite (a slot rebound) moves the cell too.
                if node.image != image || node.texture_atlas.as_ref().is_none_or(|a| a.index != index) {
                    node.image = image;
                    node.texture_atlas = Some(TextureAtlas { layout, index });
                }
            }
            None => continue, // no sheet yet, or no such sprite: draw nothing rather than another cell
        }
        let inherited = crate::style::inherited_opacity(entity, &parents, &opacities);
        let c = icon.color.with_alpha(icon.tint.alpha() * icon.alpha * inherited);
        if node.color != c {
            node.color = c;
        }
    }
}

/// Boot-load the sheets listed here. Add a name per shipped sheet folder.
pub fn load_sheets(
    mut sheets: ResMut<IconSheets>,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for sheet in ["bag", "abilities"] {
        if let Err(e) = sheets.load(sheet, &assets, &mut layouts) {
            warn!("gbui: icon sheet {sheet}: {e}");
        }
    }
}

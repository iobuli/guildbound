//! Image-layer skins: a widget drawn as a stack of PNG layers whose opacity follows its state.
//!
//! A skin is data, `assets/ui/skins/<skin>/skin.json`, re-read whenever the file changes:
//! ```json
//! { "layers": [
//!   { "file": "glass.png", "slice": 12, "alpha": { "idle": 0.55, "hovered": 0.8 } },
//!   { "file": "wedge.png", "segments": 12, "drive": "progress", "off": 0.0,
//!     "alpha": { "idle": 0.0, "casting": 1.0 } },
//!   { "file": "star.png", "top": true, "alpha": { "idle": 0.0, "mastered": 1.0 } }
//! ] }
//! ```
//! Layers draw bottom to top under the widget's own children (`"top": true` draws above them).
//! - `alpha`: opacity per state. "idle" is the base; an active listed state overrides it, the
//!   highest in `PRECEDENCE` winning (JSON key order is not kept). States come from the widget
//!   (`hovered`, `pressed` from `Interaction`; `checked`, `disabled` from `UiFlags`) and from the
//!   owner (`Skinned::set`), e.g. `casting`, `dim`, `mastered`.
//! - `slice`: 9-slice border in px, so one frame PNG fits any size; 0 stretches the image.
//! - `segments`: N copies of the image rotated evenly about the centre (draw it at 12 o'clock);
//!   segment k is lit while `drive` (an owner value, 0..1) reaches it, unlit ones take `off` × alpha.
//! - `pulse`: seconds per breath of opacity (0.6..1.0 ×), for glows.
//! - `tint`: "#rrggbb" multiplied into the image.
//! - top-level `"dir"`: take the PNGs from another skin's folder, so two skins (say gold and
//!   violet tints of one ring) share one set of images.
//!
//! Art and code split cleanly: the PNGs carry the look (any artist or generator can replace
//! them, same names and sizes, straight alpha), the JSON carries the opacities, and the game only
//! sets states and drive values.
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;
use serde_json::Value;

use crate::style::{Opacity, UiFlags, inherited_opacity};

/// How often skin files are checked for changes (s, real time).
const RELOAD_EVERY: f32 = 1.0;

fn skin_dir(skin: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/ui/skins")).join(skin)
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerDef {
    pub file: String,
    pub slice: f32,
    pub segments: u32,
    pub drive: String,
    pub off: f32,
    pub pulse: f32,
    pub top: bool,
    pub tint: Color,
    /// (state, alpha) in file order; "idle" is the base.
    pub alpha: Vec<(String, f32)>,
}

impl LayerDef {
    /// Opacity for the active states: the highest-precedence active state the layer lists, else
    /// "idle" (else 1). A state not in `PRECEDENCE` ranks above all of them.
    pub fn alpha_for(&self, active: &[&str]) -> f32 {
        let rank = |s: &str| PRECEDENCE.iter().position(|p| *p == s).unwrap_or(PRECEDENCE.len());
        self.alpha
            .iter()
            .filter(|(s, _)| s != "idle" && active.contains(&s.as_str()))
            .max_by_key(|(s, _)| rank(s))
            .or_else(|| self.alpha.iter().find(|(s, _)| s == "idle"))
            .map_or(1.0, |e| e.1)
    }
}

/// State precedence, lowest first: when several active states set a layer's alpha, the latest in
/// this list wins (a dim slot stays dim while hovered; a cast shows over mastery).
pub const PRECEDENCE: [&str; 11] =
    ["idle", "hovered", "pressed", "checked", "open", "mastered", "casting", "over", "low", "dim", "disabled"];

/// Whether segment `k` of `n` is lit at drive value `v` (0..1): lit once `v` covers its middle.
pub fn lit(k: u32, n: u32, v: f32) -> bool {
    (k as f32 + 0.5) / n.max(1) as f32 <= v
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skin {
    /// Folder under `assets/ui/skins/` the images come from (default: the skin's own).
    pub dir: Option<String>,
    pub layers: Vec<LayerDef>,
}

impl Skin {
    pub fn parse(text: &str) -> Result<Skin, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let layers = v["layers"].as_array().ok_or("no \"layers\" array")?;
        let mut out = Vec::new();
        for (i, l) in layers.iter().enumerate() {
            let file = l["file"].as_str().ok_or(format!("layer {i}: no \"file\""))?.to_string();
            let alpha = l["alpha"]
                .as_object()
                .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_f64()? as f32))).collect())
                .unwrap_or_default();
            let tint = l["tint"]
                .as_str()
                .and_then(|h| Srgba::hex(h).ok())
                .map_or(Color::WHITE, Color::from);
            out.push(LayerDef {
                file,
                slice: l["slice"].as_f64().unwrap_or(0.0) as f32,
                segments: l["segments"].as_u64().unwrap_or(0) as u32,
                drive: l["drive"].as_str().unwrap_or("").to_string(),
                off: l["off"].as_f64().unwrap_or(0.0) as f32,
                pulse: l["pulse"].as_f64().unwrap_or(0.0) as f32,
                top: l["top"].as_bool().unwrap_or(false),
                tint,
                alpha,
            });
        }
        Ok(Skin { dir: v["dir"].as_str().map(str::to_string), layers: out })
    }
}

/// Loaded skins, re-read when their file changes; `version` bumps so widgets rebuild.
#[derive(Resource, Default)]
pub struct Skins {
    map: HashMap<String, (Skin, Option<SystemTime>, u32)>,
    since: f32,
}

impl Skins {
    fn ensure(&mut self, name: &str) -> Option<(&Skin, u32)> {
        if !self.map.contains_key(name) {
            let path = skin_dir(name).join("skin.json");
            let skin = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| Skin::parse(&t))
                .unwrap_or_else(|e| {
                    warn!("gbui: skin {name}: {e}");
                    Skin::default()
                });
            self.map.insert(name.to_string(), (skin, modified(&path), 1));
        }
        self.map.get(name).map(|(s, _, v)| (s, *v))
    }
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A widget drawn from a skin: the owner sets states and drive values.
#[derive(Component, Clone, Debug, Default)]
pub struct Skinned {
    pub skin: String,
    pub states: Vec<&'static str>,
    pub drives: Vec<(&'static str, f32)>,
    built: u32,
}

impl Skinned {
    pub fn new(skin: impl Into<String>) -> Self {
        Self { skin: skin.into(), ..default() }
    }
    /// Turn an owner state on or off.
    pub fn set(&mut self, state: &'static str, on: bool) {
        let has = self.states.contains(&state);
        if on && !has {
            self.states.push(state);
        } else if !on && has {
            self.states.retain(|s| *s != state);
        }
    }
    /// Set a drive value (0..1) read by segment layers.
    pub fn drive(&mut self, name: &'static str, value: f32) {
        let value = value.clamp(0.0, 1.0);
        match self.drives.iter_mut().find(|(n, _)| *n == name) {
            Some(d) => d.1 = value,
            None => self.drives.push((name, value)),
        }
    }
    fn value(&self, name: &str) -> f32 {
        self.drives.iter().find(|(n, _)| *n == name).map_or(0.0, |d| d.1)
    }
}

/// One drawn layer (or one segment of a segmented layer) of a `Skinned` widget.
#[derive(Component, Clone, Copy, Debug)]
pub struct LayerNode {
    layer: usize,
    segment: Option<(u32, u32)>,
}

/// Re-read changed skin files.
pub fn reload_skins(time: Res<Time<Real>>, mut skins: ResMut<Skins>) {
    skins.since += time.delta_secs();
    if skins.since < RELOAD_EVERY {
        return;
    }
    skins.since = 0.0;
    for (name, (skin, mtime, version)) in skins.map.iter_mut() {
        let path = skin_dir(name).join("skin.json");
        let now = modified(&path);
        if now != *mtime {
            *mtime = now;
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Skin::parse(&t)) {
                Ok(s) => {
                    *skin = s;
                    *version += 1;
                    info!("gbui: skin {name} reloaded");
                }
                Err(e) => warn!("gbui: skin {name}: {e} (kept the previous one)"),
            }
        }
    }
}

/// (Re)build a widget's layer nodes when it is new or its skin changed.
pub fn build_layers(
    mut commands: Commands,
    mut skins: ResMut<Skins>,
    assets: Res<AssetServer>,
    mut widgets: Query<(Entity, &mut Skinned, Option<&Children>)>,
    layer_nodes: Query<(), With<LayerNode>>,
) {
    for (entity, mut skinned, children) in &mut widgets {
        let Some((skin, version)) = skins.ensure(&skinned.skin) else { continue };
        if skinned.built == version {
            continue;
        }
        skinned.built = version;
        for child in children.into_iter().flatten() {
            if layer_nodes.contains(*child) {
                commands.entity(*child).despawn();
            }
        }
        let dir = format!("ui/skins/{}", skin.dir.as_deref().unwrap_or(&skinned.skin));
        commands.entity(entity).with_children(|p| {
            for (i, def) in skin.layers.iter().enumerate() {
                let image = assets.load(format!("{dir}/{}", def.file));
                let z = ZIndex(if def.top { 10 + i as i32 } else { -100 + i as i32 });
                let mode = if def.slice > 0.0 {
                    NodeImageMode::Sliced(TextureSlicer { border: BorderRect::all(def.slice), ..default() })
                } else {
                    NodeImageMode::Stretch
                };
                let n = def.segments;
                for k in 0..n.max(1) {
                    let rotation = if n > 0 { Rot2::radians(std::f32::consts::TAU * k as f32 / n as f32) } else { Rot2::IDENTITY };
                    p.spawn((
                        LayerNode { layer: i, segment: (n > 0).then_some((k, n)) },
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            right: px(0.0),
                            top: px(0.0),
                            bottom: px(0.0),
                            ..default()
                        },
                        ImageNode { image: image.clone(), image_mode: mode.clone(), color: def.tint.with_alpha(0.0), ..default() },
                        UiTransform { rotation, ..default() },
                        Pickable::IGNORE,
                        z,
                    ));
                }
            }
        });
    }
}

/// Set every layer's opacity from its widget's states and drives.
#[allow(clippy::type_complexity)]
pub fn drive_layers(
    time: Res<Time<Real>>,
    mut skins: ResMut<Skins>,
    widgets: Query<(&Skinned, Option<&Interaction>, Option<&UiFlags>)>,
    parents: Query<&ChildOf>,
    opacities: Query<&Opacity>,
    mut layers: Query<(Entity, &LayerNode, &ChildOf, &mut ImageNode)>,
) {
    let t = time.elapsed_secs();
    for (entity, node, parent, mut image) in &mut layers {
        let Ok((skinned, interaction, flags)) = widgets.get(parent.parent()) else { continue };
        let Some((skin, _)) = skins.ensure(&skinned.skin) else { continue };
        let Some(def) = skin.layers.get(node.layer) else { continue };
        let mut active: Vec<&str> = skinned.states.clone();
        match interaction {
            Some(Interaction::Hovered) => active.push("hovered"),
            Some(Interaction::Pressed) => active.extend(["hovered", "pressed"]),
            _ => {}
        }
        if let Some(f) = flags {
            if f.checked {
                active.push("checked");
            }
            if f.disabled {
                active.push("disabled");
            }
        }
        let mut a = def.alpha_for(&active);
        if let Some((k, n)) = node.segment
            && !lit(k, n, skinned.value(&def.drive))
        {
            a *= def.off;
        }
        if def.pulse > 0.0 {
            a *= 0.8 + 0.2 * (std::f32::consts::TAU * t / def.pulse).sin();
        }
        a *= inherited_opacity(entity, &parents, &opacities);
        let c = def.tint.with_alpha(a);
        if image.color != c {
            image.color = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKIN: &str = r##"{ "layers": [
        { "file": "glass.png", "slice": 12, "alpha": { "idle": 0.5, "hovered": 0.8, "dim": 0.2 } },
        { "file": "wedge.png", "segments": 12, "drive": "progress", "off": 0.1, "alpha": { "idle": 0.0, "casting": 1.0 } },
        { "file": "star.png", "top": true, "tint": "#FFC857", "alpha": { "mastered": 1.0, "idle": 0.0 } }
    ] }"##;

    #[test]
    fn skins_parse_and_states_override_in_order() {
        let s = Skin::parse(SKIN).unwrap();
        assert_eq!(s.layers.len(), 3);
        assert_eq!(s.layers[0].slice, 12.0);
        assert_eq!(s.layers[1].segments, 12);
        assert!(s.layers[2].top);
        let glass = &s.layers[0];
        assert_eq!(glass.alpha_for(&[]), 0.5);
        assert_eq!(glass.alpha_for(&["hovered"]), 0.8);
        // precedence, not file order: dim over hovered
        assert_eq!(glass.alpha_for(&["hovered", "dim"]), 0.2);
        // "idle" listed last still only acts as the base
        assert_eq!(s.layers[2].alpha_for(&["mastered"]), 1.0);
        assert_eq!(s.layers[2].alpha_for(&[]), 0.0);
        assert!(Skin::parse("{}").is_err());
    }

    #[test]
    fn segments_light_in_order_with_the_drive() {
        let lit_count = |v: f32| (0..12).filter(|k| lit(*k, 12, v)).count();
        assert_eq!(lit_count(0.0), 0);
        assert_eq!(lit_count(0.5), 6);
        assert_eq!(lit_count(1.0), 12);
        assert!(lit(0, 12, 0.05) && !lit(1, 12, 0.05));
    }

    #[test]
    fn skinned_states_and_drives() {
        let mut w = Skinned::new("ability");
        w.set("casting", true);
        w.set("casting", true);
        assert_eq!(w.states, vec!["casting"]);
        w.set("casting", false);
        assert!(w.states.is_empty());
        w.drive("progress", 1.7);
        assert_eq!(w.value("progress"), 1.0);
        assert_eq!(w.value("cover"), 0.0);
    }
}

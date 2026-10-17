//! Themes: a palette, named classes (a base `Style` plus per-state overrides), and the looks of
//! the glass and ring materials, read from JSON and re-read whenever the file's mtime changes, so
//! the look can be tuned while the game runs. A file that fails to parse keeps the old theme and
//! logs why.
//!
//! ```json
//! { "font": "fonts/x.ttf", "font_bold": "fonts/x-bold.ttf",
//!   "palette": { "accent": "#2EE6C5", "bg": "#04110F" },
//!   "glass": { "tint": "accent", "scanlines": 0.05 },
//!   "ring": { "ring": "accent@0.8" },
//!   "classes": {
//!     "panel": { "bg": "bg@0.55", "border": "accent@0.35", "border_width": 1, "radius": 3 },
//!     "slot":  { "extends": "panel", "hovered": { "border": "accent" }, "checked": { "glow": "accent2@0.4" } } } }
//! ```
//! - Colours: `#RRGGBB`, `#RRGGBBAA` or a palette name, optionally `@alpha` (multiplies alpha).
//! - Style keys: `bg`, `gradient` (colours, left to right), `border`, `border_width`, `radius`,
//!   `glow`, `glow_blur`, `glow_spread`, `text`, `shadow` (text shadow), `size`, `bold`.
//! - States layer over the base in the order checked, focused, hovered, pressed, disabled: a later
//!   state wins wherever it sets a field. `extends` copies another class first. Keys starting with
//!   `_` are comments. Unknown keys are errors, so typos show up in the log.
use bevy::prelude::*;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How often the theme file's mtime is checked (s, real time).
const POLL: f32 = 0.5;

/// State flags a class can restyle; bit `i` is `STATE_KEYS[i]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct States(pub u8);

pub const STATE_KEYS: [&str; 5] = ["checked", "focused", "hovered", "pressed", "disabled"];

impl States {
    pub const CHECKED: u8 = 1;
    pub const FOCUSED: u8 = 2;
    pub const HOVERED: u8 = 4;
    pub const PRESSED: u8 = 8;
    pub const DISABLED: u8 = 16;

    pub fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

/// One layer of style. `None` leaves the field to lower layers; after resolving, `None` means
/// the class doesn't set it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub bg: Option<Color>,
    pub gradient: Option<Vec<Color>>,
    pub border: Option<Color>,
    pub border_width: Option<f32>,
    pub radius: Option<f32>,
    pub glow: Option<Color>,
    pub glow_blur: Option<f32>,
    pub glow_spread: Option<f32>,
    pub text: Option<Color>,
    pub shadow: Option<Color>,
    pub size: Option<f32>,
    pub bold: Option<bool>,
}

impl Style {
    /// Fields `over` sets replace ours.
    pub fn layer(&mut self, over: &Style) {
        macro_rules! take {
            ($($f:ident),*) => {$(if over.$f.is_some() { self.$f = over.$f.clone(); })*};
        }
        take!(bg, gradient, border, border_width, radius, glow, glow_blur, glow_spread, text, shadow, size, bold);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Class {
    pub base: Style,
    /// Overrides per state, indexed like `STATE_KEYS`.
    pub states: [Style; 5],
}

impl Class {
    fn layer(&mut self, over: &Class) {
        self.base.layer(&over.base);
        for (mine, theirs) in self.states.iter_mut().zip(&over.states) {
            mine.layer(theirs);
        }
    }
}

/// The glass surface drawn over panel backgrounds (`glass.wgsl`).
#[derive(Clone, Debug, PartialEq)]
pub struct GlassLook {
    pub tint: Color,
    /// Scanline alpha and spacing (px).
    pub scanlines: f32,
    pub spacing: f32,
    /// Per-pixel grain alpha.
    pub grain: f32,
    /// Inner glow along the edges.
    pub edge: f32,
    /// A bright band that sweeps down every `sweep_period` seconds.
    pub sweep: f32,
    pub sweep_period: f32,
}

impl Default for GlassLook {
    fn default() -> Self {
        Self {
            tint: Color::srgb(0.18, 0.9, 0.77),
            scanlines: 0.05,
            spacing: 3.0,
            grain: 0.02,
            edge: 0.12,
            sweep: 0.05,
            sweep_period: 7.0,
        }
    }
}

/// The round map (`ring.wgsl`).
#[derive(Clone, Debug, PartialEq)]
pub struct RingLook {
    pub fill: Color,
    pub ring: Color,
    pub land: Color,
    pub water: Color,
    pub contour: Color,
    pub sweep: Color,
    /// Rim width (px), radar sweep turns per second, range-ring alpha.
    pub width: f32,
    pub sweep_speed: f32,
    pub grid: f32,
}

impl Default for RingLook {
    fn default() -> Self {
        Self {
            fill: Color::srgba(0.02, 0.07, 0.06, 0.7),
            ring: Color::srgba(0.18, 0.9, 0.77, 0.8),
            land: Color::srgba(0.1, 0.35, 0.3, 0.6),
            water: Color::srgba(0.3, 0.7, 0.95, 0.55),
            contour: Color::srgba(0.18, 0.9, 0.77, 0.45),
            sweep: Color::srgba(0.5, 1.0, 0.7, 0.18),
            width: 3.0,
            sweep_speed: 0.25,
            grid: 0.12,
        }
    }
}

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Theme {
    /// Font asset paths (empty: Bevy's built-in font).
    pub font: String,
    pub font_bold: String,
    pub palette: HashMap<String, Color>,
    /// Classes with `extends` already applied.
    pub classes: HashMap<String, Class>,
    pub glass: GlassLook,
    pub ring: RingLook,
}

impl Theme {
    /// Read and parse `path`; the built-in default (logged) if that fails.
    pub fn load(path: &Path) -> Theme {
        match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| Theme::parse(&s)) {
            Ok(theme) => theme,
            Err(err) => {
                warn!("[gbui] {}: {err}; using the built-in theme", path.display());
                Theme::default()
            }
        }
    }

    pub fn parse(json: &str) -> Result<Theme, String> {
        let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let root = root.as_object().ok_or("the theme must be a JSON object")?;
        let mut theme = Theme::default();
        if let Some(palette) = root.get("palette") {
            for (name, hex) in object(palette, "palette")? {
                if name.starts_with('_') {
                    continue;
                }
                let hex = hex.as_str().ok_or_else(|| format!("palette.{name}: expected a string"))?;
                let color = hex_color(hex).map_err(|e| format!("palette.{name}: {e}"))?;
                theme.palette.insert(name.clone(), color);
            }
        }
        for (key, value) in root {
            match key.as_str() {
                "palette" => {}
                "font" => theme.font = string(value, "font")?,
                "font_bold" => theme.font_bold = string(value, "font_bold")?,
                "glass" => theme.glass = theme.parse_glass(value)?,
                "ring" => theme.ring = theme.parse_ring(value)?,
                "classes" => theme.classes = theme.parse_classes(value)?,
                k if k.starts_with('_') => {}
                k => return Err(format!("unknown key '{k}'")),
            }
        }
        Ok(theme)
    }

    /// The style `class` has in `states`; `None` if the class isn't defined.
    pub fn resolve(&self, class: &str, states: States) -> Option<Style> {
        let c = self.classes.get(class)?;
        let mut style = c.base.clone();
        for (i, over) in c.states.iter().enumerate() {
            if states.has(1 << i) {
                style.layer(over);
            }
        }
        Some(style)
    }

    /// A colour string against this theme's palette.
    pub fn color(&self, s: &str) -> Result<Color, String> {
        let (base, alpha) = match s.split_once('@') {
            Some((b, a)) => {
                let a: f32 = a.trim().parse().map_err(|_| format!("bad alpha in '{s}'"))?;
                (b.trim(), a.clamp(0.0, 1.0))
            }
            None => (s.trim(), 1.0),
        };
        let color = if base.starts_with('#') {
            hex_color(base)?
        } else {
            *self.palette.get(base).ok_or_else(|| format!("unknown colour '{base}'"))?
        };
        Ok(color.with_alpha(color.alpha() * alpha))
    }

    fn parse_glass(&self, value: &Value) -> Result<GlassLook, String> {
        let mut look = GlassLook::default();
        for (key, v) in object(value, "glass")? {
            let at = format!("glass.{key}");
            match key.as_str() {
                "tint" => look.tint = self.color(&string(v, &at)?).map_err(|e| format!("{at}: {e}"))?,
                "scanlines" => look.scanlines = number(v, &at)?,
                "spacing" => look.spacing = number(v, &at)?,
                "grain" => look.grain = number(v, &at)?,
                "edge" => look.edge = number(v, &at)?,
                "sweep" => look.sweep = number(v, &at)?,
                "sweep_period" => look.sweep_period = number(v, &at)?,
                k if k.starts_with('_') => {}
                _ => return Err(format!("unknown key '{at}'")),
            }
        }
        Ok(look)
    }

    fn parse_ring(&self, value: &Value) -> Result<RingLook, String> {
        let mut look = RingLook::default();
        for (key, v) in object(value, "ring")? {
            let at = format!("ring.{key}");
            let color = || -> Result<Color, String> { self.color(&string(v, &at)?).map_err(|e| format!("{at}: {e}")) };
            match key.as_str() {
                "fill" => look.fill = color()?,
                "ring" => look.ring = color()?,
                "land" => look.land = color()?,
                "water" => look.water = color()?,
                "contour" => look.contour = color()?,
                "sweep" => look.sweep = color()?,
                "width" => look.width = number(v, &at)?,
                "sweep_speed" => look.sweep_speed = number(v, &at)?,
                "grid" => look.grid = number(v, &at)?,
                k if k.starts_with('_') => {}
                _ => return Err(format!("unknown key '{at}'")),
            }
        }
        Ok(look)
    }

    fn parse_classes(&self, value: &Value) -> Result<HashMap<String, Class>, String> {
        let mut raw: HashMap<String, (Option<String>, Class)> = HashMap::new();
        for (name, body) in object(value, "classes")? {
            if name.starts_with('_') {
                continue;
            }
            let mut class = Class::default();
            let mut extends = None;
            for (key, v) in object(body, name)? {
                let at = format!("{name}.{key}");
                if key == "extends" {
                    extends = Some(string(v, &at)?);
                } else if let Some(i) = STATE_KEYS.iter().position(|s| s == key) {
                    for (k, sv) in object(v, &at)? {
                        self.style_key(&mut class.states[i], k, sv, &format!("{at}.{k}"))?;
                    }
                } else {
                    self.style_key(&mut class.base, key, v, &at)?;
                }
            }
            raw.insert(name.clone(), (extends, class));
        }
        let mut out = HashMap::new();
        for name in raw.keys() {
            out.insert(name.clone(), flatten(name, &raw, 0)?);
        }
        Ok(out)
    }

    fn style_key(&self, style: &mut Style, key: &str, v: &Value, at: &str) -> Result<(), String> {
        let color = || -> Result<Option<Color>, String> { Ok(Some(self.color(&string(v, at)?).map_err(|e| format!("{at}: {e}"))?)) };
        match key {
            "bg" => style.bg = color()?,
            "border" => style.border = color()?,
            "glow" => style.glow = color()?,
            "text" => style.text = color()?,
            "shadow" => style.shadow = color()?,
            "gradient" => {
                let stops = v.as_array().ok_or_else(|| format!("{at}: expected an array of colours"))?;
                let mut colors = Vec::with_capacity(stops.len());
                for s in stops {
                    colors.push(self.color(&string(s, at)?).map_err(|e| format!("{at}: {e}"))?);
                }
                style.gradient = Some(colors);
            }
            "border_width" => style.border_width = Some(number(v, at)?),
            "radius" => style.radius = Some(number(v, at)?),
            "glow_blur" => style.glow_blur = Some(number(v, at)?),
            "glow_spread" => style.glow_spread = Some(number(v, at)?),
            "size" => style.size = Some(number(v, at)?),
            "bold" => style.bold = Some(v.as_bool().ok_or_else(|| format!("{at}: expected true/false"))?),
            k if k.starts_with('_') => {}
            _ => return Err(format!("unknown key '{at}'")),
        }
        Ok(())
    }
}

fn flatten(name: &str, raw: &HashMap<String, (Option<String>, Class)>, depth: usize) -> Result<Class, String> {
    if depth > 8 {
        return Err(format!("'{name}': extends goes around in a circle"));
    }
    let (extends, own) = raw.get(name).ok_or_else(|| format!("extends an unknown class '{name}'"))?;
    let mut class = match extends {
        Some(parent) => flatten(parent, raw, depth + 1)?,
        None => Class::default(),
    };
    class.layer(own);
    Ok(class)
}

fn hex_color(hex: &str) -> Result<Color, String> {
    Srgba::hex(hex.trim()).map(Color::Srgba).map_err(|_| format!("bad hex colour '{hex}'"))
}

fn object<'a>(v: &'a Value, at: &str) -> Result<&'a Map<String, Value>, String> {
    v.as_object().ok_or_else(|| format!("{at}: expected an object"))
}

fn string(v: &Value, at: &str) -> Result<String, String> {
    v.as_str().map(str::to_string).ok_or_else(|| format!("{at}: expected a string"))
}

fn number(v: &Value, at: &str) -> Result<f32, String> {
    v.as_f64().map(|n| n as f32).ok_or_else(|| format!("{at}: expected a number"))
}

/// Hot-reload bookkeeping for the theme file.
#[derive(Resource, Clone, Debug)]
pub struct ThemeFile {
    pub path: PathBuf,
    mtime: Option<SystemTime>,
    since: f32,
}

impl ThemeFile {
    pub fn new(path: PathBuf) -> Self {
        let mtime = modified(&path);
        Self { path, mtime, since: 0.0 }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Re-read the theme when its file changes. A broken file keeps the current theme.
pub fn reload(time: Res<Time<Real>>, mut file: ResMut<ThemeFile>, mut theme: ResMut<Theme>) {
    file.since += time.delta_secs();
    if file.since < POLL {
        return;
    }
    file.since = 0.0;
    let mtime = modified(&file.path);
    if mtime == file.mtime {
        return;
    }
    file.mtime = mtime;
    match std::fs::read_to_string(&file.path).map_err(|e| e.to_string()).and_then(|s| Theme::parse(&s)) {
        Ok(new) => {
            info!("[gbui] theme reloaded ({} classes)", new.classes.len());
            theme.set_if_neq(new);
        }
        Err(err) => warn!("[gbui] {}: {err}; keeping the current theme", file.path.display()),
    }
}

/// Font handles for the theme's regular and bold faces.
#[derive(Resource, Clone, Debug, Default)]
pub struct UiFonts {
    pub regular: Handle<Font>,
    pub bold: Handle<Font>,
    paths: (String, String),
}

fn font(assets: &AssetServer, path: &str) -> Handle<Font> {
    if path.is_empty() { Handle::default() } else { assets.load(path.to_string()) }
}

pub fn init_fonts(mut commands: Commands, assets: Res<AssetServer>, theme: Res<Theme>) {
    commands.insert_resource(UiFonts {
        regular: font(&assets, &theme.font),
        bold: font(&assets, &theme.font_bold),
        paths: (theme.font.clone(), theme.font_bold.clone()),
    });
}

/// Load the faces again when the theme names other files.
pub fn load_fonts(assets: Res<AssetServer>, theme: Res<Theme>, fonts: Option<ResMut<UiFonts>>) {
    let Some(mut fonts) = fonts else { return };
    if !theme.is_changed() || (fonts.paths.0 == theme.font && fonts.paths.1 == theme.font_bold) {
        return;
    }
    fonts.regular = font(&assets, &theme.font);
    fonts.bold = font(&assets, &theme.font_bold);
    fonts.paths = (theme.font.clone(), theme.font_bold.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
        "_note": "comments are ignored",
        "font": "fonts/a.ttf",
        "palette": { "accent": "#2EE6C5", "bg": "#04110F" },
        "glass": { "tint": "accent@0.5", "scanlines": 0.07 },
        "classes": {
            "panel": { "bg": "bg@0.5", "border": "accent", "border_width": 1, "radius": 3,
                       "hovered": { "border": "#FFFFFF" } },
            "slot": { "extends": "panel", "size": 14, "bold": true,
                      "checked": { "glow": "accent@0.4", "border": "#FF0000" },
                      "pressed": { "bg": "accent@0.25" } },
            "bar.fill": { "gradient": ["accent", "#7CFFB2"] }
        }
    }"##;

    fn close(a: Color, b: Color) -> bool {
        let (a, b) = (a.to_srgba(), b.to_srgba());
        (a.red - b.red).abs() < 1e-3 && (a.green - b.green).abs() < 1e-3 && (a.blue - b.blue).abs() < 1e-3 && (a.alpha - b.alpha).abs() < 1e-3
    }

    #[test]
    fn parses_palette_colours_and_alpha() {
        let t = Theme::parse(SAMPLE).unwrap();
        assert_eq!(t.font, "fonts/a.ttf");
        let accent = t.color("accent").unwrap();
        assert!(close(accent, Color::srgb_u8(0x2E, 0xE6, 0xC5)));
        assert!((t.color("accent@0.25").unwrap().alpha() - 0.25).abs() < 1e-6);
        assert!((t.color("#FF000080").unwrap().alpha() - 128.0 / 255.0).abs() < 1e-3);
        assert!((t.glass.tint.alpha() - 0.5).abs() < 1e-6 && (t.glass.scanlines - 0.07).abs() < 1e-6);
        assert!(t.color("nope").is_err() && t.color("#GG0000").is_err() && t.color("accent@x").is_err());
    }

    #[test]
    fn states_layer_in_order_and_extends_inherits() {
        let t = Theme::parse(SAMPLE).unwrap();
        let base = t.resolve("slot", States::default()).unwrap();
        assert_eq!(base.radius, Some(3.0), "inherited from panel");
        assert_eq!((base.size, base.bold), (Some(14.0), Some(true)));
        assert!(close(base.border.unwrap(), t.color("accent").unwrap()));
        assert_eq!(base.glow, None);
        let hovered = t.resolve("slot", States(States::HOVERED)).unwrap();
        assert!(close(hovered.border.unwrap(), Color::WHITE), "panel's hover reaches slot");
        // checked sets red, hovered (later) sets white: hovered wins where both set the border
        let both = t.resolve("slot", States(States::CHECKED | States::HOVERED)).unwrap();
        assert!(close(both.border.unwrap(), Color::WHITE));
        assert!(both.glow.is_some(), "checked's glow survives");
        let pressed = t.resolve("slot", States(States::HOVERED | States::PRESSED)).unwrap();
        assert!((pressed.bg.unwrap().alpha() - 0.25).abs() < 1e-6);
        assert_eq!(t.resolve("bar.fill", States::default()).unwrap().gradient.map(|g| g.len()), Some(2));
        assert!(t.resolve("missing", States::default()).is_none());
    }

    #[test]
    fn rejects_typos_cycles_and_unknown_parents() {
        assert!(Theme::parse(r##"{ "classes": { "a": { "bgg": "#000000" } } }"##).unwrap_err().contains("a.bgg"));
        assert!(Theme::parse(r#"{ "classes": { "a": { "hovered": { "size": "big" } } } }"#).is_err());
        assert!(Theme::parse(r#"{ "classes": { "a": { "extends": "b" }, "b": { "extends": "a" } } }"#).is_err());
        assert!(Theme::parse(r#"{ "classes": { "a": { "extends": "zzz" } } }"#).is_err());
        assert!(Theme::parse(r#"{ "colour": {} }"#).is_err());
        assert!(Theme::parse("[1, 2]").is_err());
    }

    #[test]
    fn the_shipped_theme_parses_and_defines_every_class_the_game_uses() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/ui/theme.json");
        let json = std::fs::read_to_string(path).expect("assets/ui/theme.json");
        let t = Theme::parse(&json).unwrap_or_else(|e| panic!("theme.json: {e}"));
        for class in [
            "panel", "bracket", "title", "text", "text.dim", "slot", "slot.key", "slot.label", "slot.count",
            "bar", "bar.tick", "tooltip", "tooltip.text", "log.line", "log.item", "log.warn", "log.speech",
            "log.system",
        ] {
            assert!(t.classes.contains_key(class), "theme.json lacks class '{class}'");
        }
    }
}

//! Live tuning page (F1 toggles): edits `Feel` and `motion::compose::MotionFeel` (13 knobs:
//! run speed, accel, turn rate, jump, exaggeration, hold, stepped fps on/off, spring hz,
//! spring damping, arm swing, lean, bounce, hitstop, ragdoll threshold). F5 saves `profiles/feel.json`, loaded
//! at startup if present.
//!
//! The panel sits on the right edge: drag a bar to set a value, buttons toggle 12 fps stepping
//! and the debug skeleton, save, or restore defaults. While it is open the cursor is released;
//! `TuningPanel.capturing` tells gameplay input to ignore mouse buttons over the panel.
//! The profile file holds `{ "feel": Feel, "motion": MotionFeel mirror }`; missing or
//! non-finite entries fall back to defaults, unknown keys are ignored.
use super::Feel;
use super::camera::PageToggles;
use crate::motion::compose::MotionFeel;
use crate::motion::timing::Style;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn plugin(app: &mut App) {
    app.init_resource::<Feel>().init_resource::<MotionFeel>();
    let path = profile_path();
    let note = if path.exists() {
        match read_profile(&path) {
            Ok(profile) => {
                let world = app.world_mut();
                *world.resource_mut::<Feel>() = profile.feel;
                profile.motion.apply(&mut world.resource_mut::<MotionFeel>());
                info!("[tuning] loaded {}", path.display());
                format!("Loaded {PROFILE_LABEL}.")
            }
            Err(e) => {
                warn!("[tuning] {}: {e}", path.display());
                format!("{PROFILE_LABEL} unreadable, using defaults.")
            }
        }
    } else {
        format!("Drag bars to tune. F5 saves {PROFILE_LABEL}.")
    };
    app.insert_resource(TuningPanel {
        note,
        ..default()
    })
    .add_systems(Startup, spawn_panel)
    .add_systems(
        Update,
        (hotkeys.in_set(PageToggles), drag_sliders, press_buttons, paint).chain(),
    );
}

/// Shown in notes instead of the absolute path.
const PROFILE_LABEL: &str = "profiles/feel.json";
/// Frame rate of the stepped ("on twos") toggle.
const STEPPED_FPS: f32 = 12.0;

/// `profiles/feel.json` next to the asset folder (repository root).
pub fn profile_path() -> PathBuf {
    Path::new(crate::ASSETS)
        .join("..")
        .join("profiles")
        .join("feel.json")
}

/// Panel state other modules may read.
#[derive(Resource, Debug, Default)]
pub struct TuningPanel {
    pub open: bool,
    /// Cursor over the open panel or dragging a slider: gameplay should ignore mouse buttons.
    pub capturing: bool,
    drag: Option<usize>,
    note: String,
}

/// One slider: label, unit, range, display precision and accessors into the two resources.
struct Knob {
    label: &'static str,
    unit: &'static str,
    min: f32,
    max: f32,
    decimals: usize,
    get: fn(&Feel, &MotionFeel) -> f32,
    set: fn(&mut Feel, &mut MotionFeel, f32),
}

impl Knob {
    fn fraction(&self, feel: &Feel, motion: &MotionFeel) -> f32 {
        let v = (self.get)(feel, motion);
        let f = (v - self.min) / (self.max - self.min);
        if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 }
    }

    fn label(&self, feel: &Feel, motion: &MotionFeel) -> String {
        let v = (self.get)(feel, motion);
        format!("{}  {:.*} {}", self.label, self.decimals, v, self.unit)
    }
}

/// Exaggeration range: above ~1.4 limbs fold through the body, below ~0.9 the style is gone.
const EXAGGERATION: (f32, f32) = (0.9, 1.4);
/// Bounce range: 0 = the clips' own vertical motion, 2 = rubbery.
const BOUNCE: (f32, f32) = (0.0, 2.0);

const KNOBS: [Knob; 13] = [
    Knob {
        label: "Run speed",
        unit: "m/s",
        min: 2.0,
        max: 9.0,
        decimals: 2,
        get: |f, _| f.run_speed,
        set: |f, _, v| f.run_speed = v,
    },
    Knob {
        label: "Acceleration",
        unit: "m/s2",
        min: 5.0,
        max: 80.0,
        decimals: 1,
        get: |f, _| f.accel,
        set: |f, _, v| f.accel = v,
    },
    Knob {
        label: "Turn rate",
        unit: "deg/s",
        min: 90.0,
        max: 1440.0,
        decimals: 0,
        get: |f, _| f.turn_rate,
        set: |f, _, v| f.turn_rate = v,
    },
    Knob {
        label: "Jump speed",
        unit: "m/s",
        min: 2.0,
        max: 12.0,
        decimals: 2,
        get: |f, _| f.jump_speed,
        set: |f, _, v| f.jump_speed = v,
    },
    Knob {
        label: "Hitstop",
        unit: "s",
        min: 0.0,
        max: 0.25,
        decimals: 3,
        get: |f, _| f.hitstop,
        set: |f, _, v| f.hitstop = v,
    },
    Knob {
        label: "Ragdoll threshold",
        unit: "m/s",
        min: 2.0,
        max: 20.0,
        decimals: 1,
        get: |f, _| f.ragdoll_threshold,
        set: |f, _, v| f.ragdoll_threshold = v,
    },
    Knob {
        label: "Exaggeration",
        unit: "x",
        min: EXAGGERATION.0,
        max: EXAGGERATION.1,
        decimals: 2,
        get: |_, m| m.style.exaggeration,
        set: |_, m, v| m.style.exaggeration = v,
    },
    Knob {
        label: "Key hold",
        unit: "",
        min: 0.0,
        max: 1.0,
        decimals: 2,
        get: |_, m| m.style.hold,
        set: |_, m, v| m.style.hold = v,
    },
    Knob {
        label: "Spring core",
        unit: "Hz",
        min: 1.0,
        max: 20.0,
        decimals: 1,
        get: |_, m| m.springs.core_hz,
        set: |_, m, v| m.springs.core_hz = v,
    },
    Knob {
        label: "Spring damping",
        unit: "",
        min: 0.1,
        max: 2.0,
        decimals: 2,
        get: |_, m| m.springs.damping,
        set: |_, m, v| m.springs.damping = v,
    },
    Knob {
        label: "Arm swing",
        unit: "x",
        min: 0.0,
        max: 2.0,
        decimals: 2,
        get: |_, m| m.springs.arm_swing,
        set: |_, m, v| m.springs.arm_swing = v,
    },
    Knob {
        label: "Lean",
        unit: "x",
        min: 0.0,
        max: 3.0,
        decimals: 2,
        get: |_, m| m.contact.lean,
        set: |_, m, v| m.contact.lean = v,
    },
    Knob {
        label: "Bounce",
        unit: "x",
        min: BOUNCE.0,
        max: BOUNCE.1,
        decimals: 2,
        get: |_, m| m.bounce,
        set: |_, m, v| m.bounce = v,
    },
];

/// On-disk profile: gameplay `Feel` plus a serde mirror of `MotionFeel`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Profile {
    feel: Feel,
    motion: MotionProfile,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            feel: Feel::default(),
            motion: MotionProfile::from(&MotionFeel::default()),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct MotionProfile {
    style: Style,
    springs: SpringProfile,
    contact: ContactProfile,
    bounce: f32,
    debug: bool,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct SpringProfile {
    core_hz: f32,
    limb_hz: f32,
    damping: f32,
    arm_swing: f32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
struct ContactProfile {
    lean: f32,
    max_lean: f32,
    reach_up: f32,
    reach_down: f32,
    pelvis_rate: f32,
}

impl From<&MotionFeel> for MotionProfile {
    fn from(m: &MotionFeel) -> Self {
        Self {
            style: m.style,
            springs: SpringProfile {
                core_hz: m.springs.core_hz,
                limb_hz: m.springs.limb_hz,
                damping: m.springs.damping,
                arm_swing: m.springs.arm_swing,
            },
            contact: ContactProfile {
                lean: m.contact.lean,
                max_lean: m.contact.max_lean,
                reach_up: m.contact.reach_up,
                reach_down: m.contact.reach_down,
                pelvis_rate: m.contact.pelvis_rate,
            },
            bounce: m.bounce,
            debug: m.debug,
        }
    }
}

impl MotionProfile {
    fn apply(&self, m: &mut MotionFeel) {
        m.style = self.style;
        m.style.exaggeration = self.style.exaggeration.clamp(EXAGGERATION.0, EXAGGERATION.1);
        m.springs.arm_swing = self.springs.arm_swing;
        m.springs.core_hz = self.springs.core_hz;
        m.springs.limb_hz = self.springs.limb_hz;
        m.springs.damping = self.springs.damping;
        m.contact.lean = self.contact.lean;
        m.contact.max_lean = self.contact.max_lean;
        m.contact.reach_up = self.contact.reach_up;
        m.contact.reach_down = self.contact.reach_down;
        m.contact.pelvis_rate = self.contact.pelvis_rate;
        m.bounce = self.bounce.clamp(BOUNCE.0, BOUNCE.1);
        m.debug = self.debug;
    }
}

fn read_profile(path: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_profile(&text)
}

/// Parse a profile over the defaults: missing keys keep defaults, non-finite numbers are
/// replaced by defaults, unknown keys are ignored.
fn parse_profile(text: &str) -> Result<Profile, String> {
    let file: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let defaults = serde_json::to_value(Profile::default()).map_err(|e| e.to_string())?;
    let mut merged = defaults.clone();
    merge(&mut merged, &file);
    sanitize(&mut merged, &defaults);
    serde_json::from_value(merged).map_err(|e| e.to_string())
}

fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                match base.get_mut(key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

/// Replace numbers that are not finite as f32 with the default at the same path.
fn sanitize(value: &mut Value, defaults: &Value) {
    match value {
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if let Some(d) = defaults.get(key) {
                    sanitize(v, d);
                }
            }
        }
        Value::Number(n) => {
            let finite = n.as_f64().is_some_and(|x| (x as f32).is_finite());
            if !finite && defaults.is_number() {
                *value = defaults.clone();
            }
        }
        _ => {}
    }
}

fn write_profile(path: &Path, feel: &Feel, motion: &MotionFeel) -> Result<(), String> {
    let profile = Profile {
        feel: feel.clone(),
        motion: MotionProfile::from(motion),
    };
    let text = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Save and return the note to show.
fn save(feel: &Feel, motion: &MotionFeel) -> String {
    let path = profile_path();
    match write_profile(&path, feel, motion) {
        Ok(()) => {
            info!("[tuning] saved {}", path.display());
            format!("Saved {PROFILE_LABEL}.")
        }
        Err(e) => {
            error!("[tuning] save {}: {e}", path.display());
            format!("Save failed: {e}")
        }
    }
}

#[derive(Component)]
struct PanelRoot;

#[derive(Component)]
struct Slider(usize);

#[derive(Component)]
struct Fill(usize);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PanelButton {
    Stepped,
    Debug,
    Save,
    Defaults,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PanelText {
    Knob(usize),
    Button(PanelButton),
    Note,
}

const PANEL_BG: Color = Color::srgba(0.035, 0.055, 0.075, 0.94);
const TRACK: Color = Color::srgb(0.13, 0.2, 0.25);
const TRACK_HOVER: Color = Color::srgb(0.17, 0.26, 0.32);
const FILL: Color = Color::srgb(0.1, 0.75, 0.6);
const BUTTON: Color = Color::srgb(0.13, 0.22, 0.29);
const BUTTON_HOVER: Color = Color::srgb(0.18, 0.3, 0.38);
const BUTTON_ON: Color = Color::srgb(0.1, 0.5, 0.42);
const PRESSED: Color = Color::srgb(0.08, 0.6, 0.5);
const TITLE: Color = Color::srgb(0.95, 0.85, 0.55);
const LABEL: Color = Color::WHITE;
const NOTE: Color = Color::srgb(0.7, 0.78, 0.85);

fn text(s: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            PanelRoot,
            Interaction::default(),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                right: px(12.0),
                top: px(12.0),
                width: px(300.0),
                padding: UiRect::all(px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            GlobalZIndex(50),
        ))
        .with_children(|ui| {
            ui.spawn(text("TUNING   F1 hide  |  F5 save", 15.0, TITLE));
            for i in 0..KNOBS.len() {
                ui.spawn((PanelText::Knob(i), text("", 12.0, LABEL)));
                ui.spawn((
                    Button,
                    Slider(i),
                    RelativeCursorPosition::default(),
                    Node {
                        width: percent(100.0),
                        height: px(14.0),
                        margin: UiRect::bottom(px(3.0)),
                        ..default()
                    },
                    BackgroundColor(TRACK),
                ))
                .with_child((
                    Fill(i),
                    Node {
                        width: percent(50.0),
                        height: percent(100.0),
                        ..default()
                    },
                    BackgroundColor(FILL),
                ));
            }
            ui.spawn(Node {
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6.0),
                row_gap: px(6.0),
                margin: UiRect::top(px(6.0)),
                ..default()
            })
            .with_children(|row| {
                for button in [
                    PanelButton::Stepped,
                    PanelButton::Debug,
                    PanelButton::Save,
                    PanelButton::Defaults,
                ] {
                    row.spawn((
                        Button,
                        button,
                        Node {
                            width: px(134.0),
                            padding: UiRect::axes(px(6.0), px(5.0)),
                            ..default()
                        },
                        BackgroundColor(BUTTON),
                    ))
                    .with_child((PanelText::Button(button), text("", 12.0, LABEL)));
                }
            });
            ui.spawn((PanelText::Note, text("", 11.0, NOTE)));
        });
}

/// F1 toggles the page (the camera frees the cursor while it is open), F5 saves.
fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<TuningPanel>,
    mut roots: Query<&mut Node, With<PanelRoot>>,
    feel: Res<Feel>,
    motion: Res<MotionFeel>,
) {
    if keys.just_pressed(KeyCode::F1) {
        panel.open = !panel.open;
        let display = if panel.open { Display::Flex } else { Display::None };
        for mut node in &mut roots {
            node.display = display;
        }
        if !panel.open {
            panel.drag = None;
            panel.capturing = false;
        }
    }
    if keys.just_pressed(KeyCode::F5) {
        panel.note = save(&feel, &motion);
    }
}

fn drag_sliders(
    mouse: Res<ButtonInput<MouseButton>>,
    mut panel: ResMut<TuningPanel>,
    sliders: Query<(&Slider, &Interaction, &RelativeCursorPosition)>,
    hovers: Query<&Interaction, Or<(With<PanelRoot>, With<PanelButton>)>>,
    mut feel: ResMut<Feel>,
    mut motion: ResMut<MotionFeel>,
) {
    if !panel.open {
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        panel.drag = None;
    }
    for (slider, interaction, cursor) in &sliders {
        if mouse.just_pressed(MouseButton::Left) && *interaction == Interaction::Pressed {
            panel.drag = Some(slider.0);
        }
        if panel.drag != Some(slider.0) {
            continue;
        }
        let Some(knob) = KNOBS.get(slider.0) else {
            continue;
        };
        if let Some(p) = cursor.normalized.filter(|p| p.is_finite()) {
            let value = knob.min + (knob.max - knob.min) * (p.x + 0.5).clamp(0.0, 1.0);
            if ((knob.get)(&feel, &motion) - value).abs() > 1e-6 {
                (knob.set)(&mut feel, &mut motion, value);
            }
        }
    }
    let hovered = sliders.iter().any(|(_, i, _)| *i != Interaction::None)
        || hovers.iter().any(|i| *i != Interaction::None);
    let capturing = hovered || panel.drag.is_some();
    if panel.capturing != capturing {
        panel.capturing = capturing;
    }
}

fn press_buttons(
    buttons: Query<(&PanelButton, &Interaction), Changed<Interaction>>,
    mut panel: ResMut<TuningPanel>,
    mut feel: ResMut<Feel>,
    mut motion: ResMut<MotionFeel>,
) {
    for (button, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            PanelButton::Stepped => {
                motion.style.stepped_fps = match motion.style.stepped_fps {
                    Some(_) => None,
                    None => Some(STEPPED_FPS),
                };
            }
            PanelButton::Debug => motion.debug = !motion.debug,
            PanelButton::Save => panel.note = save(&feel, &motion),
            PanelButton::Defaults => {
                *feel = Feel::default();
                *motion = MotionFeel::default();
                panel.note = "Defaults restored (F5 saves).".into();
            }
        }
    }
}

fn button_label(button: PanelButton, motion: &MotionFeel) -> String {
    let on_off = |on: bool| if on { "ON" } else { "off" };
    match button {
        PanelButton::Stepped => format!(
            "{} fps step: {}",
            STEPPED_FPS,
            on_off(motion.style.stepped_fps.is_some())
        ),
        PanelButton::Debug => format!("Debug bones: {}", on_off(motion.debug)),
        PanelButton::Save => "Save (F5)".into(),
        PanelButton::Defaults => "Defaults".into(),
    }
}

fn paint(
    panel: Res<TuningPanel>,
    feel: Res<Feel>,
    motion: Res<MotionFeel>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut texts: Query<(&PanelText, &mut Text)>,
    mut backgrounds: Query<
        (&Interaction, &mut BackgroundColor, Option<&PanelButton>),
        Or<(With<PanelButton>, With<Slider>)>,
    >,
) {
    if !panel.open {
        return;
    }
    for (fill, mut node) in &mut fills {
        let Some(knob) = KNOBS.get(fill.0) else {
            continue;
        };
        let width = percent(knob.fraction(&feel, &motion) * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
    for (kind, mut text) in &mut texts {
        let s = match *kind {
            PanelText::Knob(i) => KNOBS.get(i).map_or_else(String::new, |k| k.label(&feel, &motion)),
            PanelText::Button(b) => button_label(b, &motion),
            PanelText::Note => panel.note.clone(),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for (interaction, mut background, button) in &mut backgrounds {
        let on = match button {
            Some(PanelButton::Stepped) => motion.style.stepped_fps.is_some(),
            Some(PanelButton::Debug) => motion.debug,
            _ => false,
        };
        let color = match (*interaction, button.is_some()) {
            (Interaction::Pressed, _) => PRESSED,
            (Interaction::Hovered, true) => BUTTON_HOVER,
            (Interaction::Hovered, false) => TRACK_HOVER,
            (Interaction::None, true) if on => BUTTON_ON,
            (Interaction::None, true) => BUTTON,
            (Interaction::None, false) => TRACK,
        };
        background.set_if_neq(BackgroundColor(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_ranges_contain_defaults() {
        let feel = Feel::default();
        let motion = MotionFeel::default();
        for k in &KNOBS {
            let v = (k.get)(&feel, &motion);
            assert!(k.min < k.max, "{}", k.label);
            assert!(v >= k.min && v <= k.max, "{} default {v} outside {}..{}", k.label, k.min, k.max);
        }
    }

    #[test]
    fn knobs_set_what_they_get() {
        let mut feel = Feel::default();
        let mut motion = MotionFeel::default();
        for k in &KNOBS {
            let v = k.min + 0.37 * (k.max - k.min);
            (k.set)(&mut feel, &mut motion, v);
            assert!(((k.get)(&feel, &motion) - v).abs() < 1e-6, "{}", k.label);
            assert!((k.fraction(&feel, &motion) - 0.37).abs() < 1e-4, "{}", k.label);
        }
    }

    #[test]
    fn profile_round_trip() {
        let mut feel = Feel::default();
        feel.run_speed = 5.5;
        let mut motion = MotionFeel::default();
        motion.style.stepped_fps = Some(12.0);
        motion.springs.damping = 0.9;
        motion.contact.lean = 1.7;
        motion.bounce = 1.6;
        motion.debug = true;
        let profile = Profile {
            feel,
            motion: MotionProfile::from(&motion),
        };
        let text = serde_json::to_string(&profile).unwrap();
        let back = parse_profile(&text).unwrap();
        assert_eq!(back.feel.run_speed, 5.5);
        let mut applied = MotionFeel::default();
        back.motion.apply(&mut applied);
        assert_eq!(applied.style.stepped_fps, Some(12.0));
        assert_eq!(applied.springs.damping, 0.9);
        assert_eq!(applied.contact.lean, 1.7);
        assert_eq!(applied.bounce, 1.6);
        assert!(applied.debug);
    }

    #[test]
    fn panel_toggles_and_slider_drag_sets_feel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_plugins(plugin);
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<PanelRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(app.world().get::<Node>(root).unwrap().display, Display::None);

        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F1);
        app.update();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        assert!(app.world().resource::<TuningPanel>().open);
        assert_eq!(app.world().get::<Node>(root).unwrap().display, Display::Flex);

        // Grab the run-speed bar at 75 % of its width.
        let slider = app
            .world_mut()
            .query::<(Entity, &Slider)>()
            .iter(app.world())
            .find(|(_, s)| s.0 == 0)
            .map(|(e, _)| e)
            .unwrap();
        app.world_mut().entity_mut(slider).insert((
            Interaction::Pressed,
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::new(0.25, 0.0)),
            },
        ));
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
        app.update();
        let run = app.world().resource::<Feel>().run_speed;
        assert!((run - (2.0 + 7.0 * 0.75)).abs() < 1e-4, "{run}");
        assert!(app.world().resource::<TuningPanel>().capturing);
        let label = app
            .world_mut()
            .query::<(&PanelText, &Text)>()
            .iter(app.world())
            .find(|(k, _)| **k == PanelText::Knob(0))
            .map(|(_, t)| t.0.clone())
            .unwrap();
        assert!(label.contains("7.25"), "{label}");
    }

    #[test]
    fn partial_and_bad_profiles_fall_back_to_defaults() {
        let p = parse_profile(r#"{ "feel": { "run_speed": 6.0, "bogus": 1 }, "motion": { "springs": { "core_hz": 1e60 } } }"#)
            .unwrap();
        let d = Profile::default();
        assert_eq!(p.feel.run_speed, 6.0);
        assert_eq!(p.feel.accel, d.feel.accel);
        assert_eq!(p.motion.springs.core_hz, d.motion.springs.core_hz);
        assert_eq!(p.motion.style, d.motion.style);
        assert!(parse_profile("not json").is_err());
        assert!(parse_profile("{}").is_ok());
    }
}

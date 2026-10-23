//! The pause menu: Esc with no other page open stops the game (`Time<Virtual>` paused: physics,
//! animation and AI all wait) and shows a panel; Esc again, or Resume, carries on.
//! - **1** Resume · **2** Save now (`savegame::SaveNow`) · **3 / 4** mouse sensitivity − / + ·
//!   **5 / 6** volume − / + · **7** New game (forgets the save and quits; the next launch starts
//!   fresh) · **8** Save and quit. Every line can be clicked too.
//! - The panel also lists the controls (`CONTROLS`).
//! - **Settings** (`Settings`: mouse sensitivity and volume) persist in `saves/settings.json`;
//!   `camera` scales mouse look by `Settings::mouse`, the volume goes to `GlobalVolume`.
//!
//! `GB_PAUSE=<seconds>` (smoke runs) opens the menu once that much game time has passed (with
//! `GB_SCREENSHOT_AT` at the same value the shot shows it).
//!
//! Ownership: `Paused`, `Settings` (written here only); `camera` reads `Paused` as an open page.
use super::savegame::{SaveNow, SaveState};
use bevy::audio::{GlobalVolume, Volume};
use bevy::prelude::*;
use gbui::prelude::*;

/// Mouse sensitivity steps (× the camera's base) and its range.
const MOUSE_STEP: f32 = 0.1;
const MOUSE_RANGE: (f32, f32) = (0.2, 3.0);
/// Volume steps and range (linear).
const VOLUME_STEP: f32 = 0.1;
const VOLUME_RANGE: (f32, f32) = (0.0, 1.0);

/// The pause menu is up (and the game's clock stopped).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Paused(pub bool);

/// Player settings kept between runs.
#[derive(Resource, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// Mouse look × the camera's base sensitivity.
    pub mouse: f32,
    /// Master volume, 0..1.
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { mouse: 1.0, volume: 1.0 }
    }
}

pub fn settings_path() -> std::path::PathBuf {
    std::path::Path::new(crate::ASSETS).join("../saves/settings.json")
}

impl Settings {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str::<Settings>(&t).ok())
            .map(|s| s.clamped())
            .unwrap_or_default()
    }
    fn clamped(self) -> Self {
        let fin = |v: f32, d: f32| if v.is_finite() { v } else { d };
        Self {
            mouse: fin(self.mouse, 1.0).clamp(MOUSE_RANGE.0, MOUSE_RANGE.1),
            volume: fin(self.volume, 1.0).clamp(VOLUME_RANGE.0, VOLUME_RANGE.1),
        }
    }
}

/// The controls, as the pause panel lists them.
const CONTROLS: [&str; 23] = [
    "WASD  move   ·   Shift  sprint   ·   Alt  walk",
    "Space  jump   ·   Ctrl  dodge   ·   C  crouch",
    "LMB  attack (hold: heavy)   ·   RMB  guard",
    "Z  sword / fists   ·   Shift+Z  Trunks / Tapion",
    "Q  art   ·   E  spell   (hold to aim, let go to cast)",
    "T / Mouse 5  next spell   ·   Tab  enemy lock   ·   B  aim shoulder",
    "F  use / talk   ·   X  draw / sheathe   ·   G  grab",
    "I  bag   ·   K  stats and map   ·   R  back to spawn",
    "F5  quicksave   ·   Esc  this menu",
    "",
    "In the air after a throw: A / D to recover",
    "",
    "Testing keys",
    "F1  tuning   ·   F2  pose tweak   ·   F4  monitor",
    "F6  throw yourself   ·   F7  colliders",
    "F9  skip an hour   ·   F10  peaceful",
    "F11  god mode   ·   F12  debug views",
    "",
    "Gamepad: left stick move, right stick look,",
    "D-pad up art, D-pad right spell",
    "",
    "Saves go to your user folder and load when",
    "the game starts.",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Resume,
    Save,
    MouseDown,
    MouseUp,
    VolumeDown,
    VolumeUp,
    NewGame,
    Quit,
}

impl Choice {
    const KEYS: [(KeyCode, Choice); 8] = [
        (KeyCode::Digit1, Choice::Resume),
        (KeyCode::Digit2, Choice::Save),
        (KeyCode::Digit3, Choice::MouseDown),
        (KeyCode::Digit4, Choice::MouseUp),
        (KeyCode::Digit5, Choice::VolumeDown),
        (KeyCode::Digit6, Choice::VolumeUp),
        (KeyCode::Digit7, Choice::NewGame),
        (KeyCode::Digit8, Choice::Quit),
    ];
}

#[derive(Component)]
struct Panel;

/// A clickable line.
#[derive(Component)]
struct Line(Choice);

/// The text showing the current settings.
#[derive(Component)]
struct Readout;

pub fn plugin(app: &mut App) {
    // Smoke runs keep the defaults so a run never depends on the player's settings.
    let smoke = std::env::vars().any(|(k, _)| k.starts_with("GB_"));
    let settings = if smoke { Settings::default() } else { Settings::load(&settings_path()) };
    app.insert_resource(settings)
        .init_resource::<Paused>()
        .add_systems(Startup, (spawn_panel, apply_volume))
        // Before the camera decides the cursor, so the Esc that opens the menu also frees it.
        .add_systems(Update, (toggle, choose, show).chain().before(super::camera::capture_cursor));
}

fn apply_volume(settings: Res<Settings>, mut global: ResMut<GlobalVolume>) {
    global.volume = Volume::Linear(settings.volume);
}

fn spawn_panel(mut commands: Commands) {
    let line = |p: &mut ChildSpawnerCommands, choice: Choice, text: &str| {
        p.spawn((
            Line(choice),
            Button,
            Node { padding: UiRect::axes(px(8.0), px(3.0)), ..default() },
            Styled::new("row"),
            UiFlags::default(),
        ))
        .with_child(label(text.to_string(), "row.text"));
    };
    commands
        .spawn((
            Panel,
            panel(
                "stats",
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(18.0),
                    top: percent(16.0),
                    width: percent(64.0),
                    flex_direction: FlexDirection::Row,
                    padding: UiRect::all(px(18.0)),
                    column_gap: px(28.0),
                    display: Display::None,
                    ..default()
                },
            ),
            GlobalZIndex(80),
        ))
        .with_children(|root| {
            root.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4.0), ..default() }).with_children(|c| {
                c.spawn(label("▌CONTROLS", "title"));
                for line in CONTROLS {
                    c.spawn(label(line, "text.dim"));
                }
            });
            root.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6.0), min_width: px(260.0), ..default() }).with_children(|p| {
            p.spawn(label("▌PAUSED", "title"));
            line(p, Choice::Resume, "1.  Resume");
            line(p, Choice::Save, "2.  Save now");
            p.spawn((Node { column_gap: px(6.0), ..default() },)).with_children(|r| {
                line(r, Choice::MouseDown, "3.  Mouse −");
                line(r, Choice::MouseUp, "4.  Mouse +");
            });
            p.spawn((Node { column_gap: px(6.0), ..default() },)).with_children(|r| {
                line(r, Choice::VolumeDown, "5.  Volume −");
                line(r, Choice::VolumeUp, "6.  Volume +");
            });
            p.spawn((Readout, label("", "text.dim")));
            line(p, Choice::NewGame, "7.  New game (forget the save and quit)");
            line(p, Choice::Quit, "8.  Save and quit");
            });
        });
}

/// Esc opens the menu when no other page is open, and closes it.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    capture: Option<Res<super::camera::CursorCapture>>,
    mut paused: ResMut<Paused>,
    mut time: ResMut<Time<Virtual>>,
    mut smoke: Local<Option<Option<f32>>>,
) {
    let at = *smoke.get_or_insert_with(|| std::env::var("GB_PAUSE").ok().and_then(|s| s.trim().parse().ok()));
    if let Some(at) = at
        && time.elapsed_secs() >= at
    {
        *smoke = Some(None);
        set(&mut paused, &mut time, true);
        return;
    }
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // Esc on another page (bag, stats, talk) only closes that page.
    if !paused.0 && capture.is_some_and(|c| c.ui_mode) {
        return;
    }
    let on = !paused.0;
    set(&mut paused, &mut time, on);
}

fn set(paused: &mut Paused, time: &mut Time<Virtual>, on: bool) {
    paused.0 = on;
    if on {
        time.pause();
    } else {
        time.unpause();
    }
}

#[allow(clippy::too_many_arguments)]
fn choose(
    keys: Res<ButtonInput<KeyCode>>,
    lines: Query<(&Line, Ref<Interaction>)>,
    mut paused: ResMut<Paused>,
    mut time: ResMut<Time<Virtual>>,
    mut settings: ResMut<Settings>,
    mut global: ResMut<GlobalVolume>,
    mut slot: Option<ResMut<SaveState>>,
    mut save: MessageWriter<SaveNow>,
    mut exit: MessageWriter<AppExit>,
) {
    if !paused.0 {
        return;
    }
    let clicked = lines.iter().find(|(_, i)| i.is_changed() && **i == Interaction::Pressed).map(|(l, _)| l.0);
    let pressed = Choice::KEYS.iter().find(|(k, _)| keys.just_pressed(*k)).map(|(_, c)| *c);
    let Some(choice) = clicked.or(pressed) else { return };
    let before = *settings;
    match choice {
        Choice::Resume => set(&mut paused, &mut time, false),
        Choice::Save => {
            save.write(SaveNow);
        }
        Choice::MouseDown => settings.mouse -= MOUSE_STEP,
        Choice::MouseUp => settings.mouse += MOUSE_STEP,
        Choice::VolumeDown => settings.volume -= VOLUME_STEP,
        Choice::VolumeUp => settings.volume += VOLUME_STEP,
        Choice::NewGame => {
            if let Some(slot) = slot.as_mut() {
                slot.forget();
            }
            exit.write(AppExit::Success);
        }
        // Quitting saves (`savegame` saves on exit).
        Choice::Quit => {
            exit.write(AppExit::Success);
        }
    }
    let now = settings.clamped();
    if now != before {
        *settings = now;
        global.volume = Volume::Linear(now.volume);
        let path = settings_path();
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&path, serde_json::to_string_pretty(&now).unwrap_or_default()));
        if let Err(e) = written {
            error!("[pause] {}: {e}", path.display());
        }
    }
}

fn show(
    paused: Res<Paused>,
    settings: Res<Settings>,
    mut panels: Query<&mut Node, With<Panel>>,
    mut readout: Query<&mut Text, With<Readout>>,
) {
    if paused.is_changed() {
        for mut node in &mut panels {
            node.display = if paused.0 { Display::Flex } else { Display::None };
        }
    }
    if paused.is_changed() || settings.is_changed() {
        for mut text in &mut readout {
            text.0 = format!("     mouse {:.1}×   ·   volume {:.0}%", settings.mouse, settings.volume * 100.0);
        }
    }
}

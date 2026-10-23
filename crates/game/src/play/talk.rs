//! Talking to non-combat NPCs. A `Talkable` (name, role) in reach and in front of the player is
//! the `TalkFocus` (the prompt says "Talk to Mira"); F then talks instead of using an interactable
//! (`input` turns it into `Intent.talk_pressed`). While `Talking`:
//! - the camera moves into the player's eyes and frames the NPC's face across the gap (`view`,
//!   `PostUpdate` after `camera::follow`, eased by `Talking.blend`, a narrower lens and a little
//!   breathing sway); the player's body is hidden, so it reads first-person;
//! - the NPC stops its routine and turns its head to the player (`occupations`);
//! - a panel at the bottom shows the NPC's name and role, their line typed out, and numbered
//!   options (keys 1–7 or a click). Each pick is a `Chose` message; `guild` answers by rewriting
//!   `Dialogue` (or sending `EndTalk`). Esc or "Leave" ends the talk.
//! `Talking`, `TalkFocus` and the camera override are written here only; `Dialogue` by `guild`.
use super::camera::MainCamera;
use super::hud::log::{HudLog, LogKind};
use super::{HALF_HEIGHT, Intent, Player, PlaySet};
use crate::motion::Rig;
use crate::motion::bank::PoseBank;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use gbui::prelude::*;

/// Talk reach (m) and how squarely in front the NPC must be (cos).
const REACH: f32 = 2.6;
const IN_FRONT: f32 = 0.2;
/// Camera blend rate (1/s), the lens while talking (deg), how far in front of the eyes the
/// camera sits (m).
const BLEND_RATE: f32 = 6.0;
const TALK_FOV: f32 = 42.0;
const EYE_FORWARD: f32 = 0.1;
/// Most options shown.
pub const OPTIONS: usize = 7;
const PANEL_W: f32 = 680.0;
const LINE_RATE: f32 = 70.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Receptionist,
    Captain,
    Adventurer,
    Trainer,
    Villager,
    Guard,
}

/// Someone you can talk to.
#[derive(Component, Clone, Debug)]
pub struct Talkable {
    pub name: &'static str,
    pub role: Role,
    /// A few words under the name ("Guild receptionist").
    pub title: &'static str,
}

/// UI kept on screen during a talk (the dialogue panel, the Guild Card); every other top-level
/// UI node is hidden for a clean first-person view and shown again after.
#[derive(Component, Default)]
pub struct KeepInTalk;

/// The NPC F would talk to now.
#[derive(Resource, Default)]
pub struct TalkFocus(pub Option<Entity>);

/// The talk in progress (and the camera blend into it, which outlives it while easing out).
#[derive(Resource, Default)]
pub struct Talking {
    pub npc: Option<Entity>,
    pub blend: f32,
}

impl Talking {
    pub fn active(&self) -> bool {
        self.npc.is_some()
    }
}

/// One option on the talk panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub label: String,
    pub id: &'static str,
}

/// What the NPC says now and what the player may answer (written by `guild`).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct Dialogue {
    pub line: String,
    pub options: Vec<Choice>,
}

/// A talk began (so `guild` can open the menu).
#[derive(Message, Clone, Copy, Debug)]
pub struct TalkStarted {
    pub npc: Entity,
    pub role: Role,
}

/// The player picked an option.
#[derive(Message, Clone, Copy, Debug)]
pub struct Chose {
    pub npc: Entity,
    pub role: Role,
    pub id: &'static str,
}

/// End the talk (Leave, Esc, or `guild` closing it).
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct EndTalk;

pub fn plugin(app: &mut App) {
    app.init_resource::<TalkFocus>()
        .init_resource::<Talking>()
        .init_resource::<Dialogue>()
        .add_message::<TalkStarted>()
        .add_message::<Chose>()
        .add_message::<EndTalk>()
        .add_systems(Startup, spawn_ui)
        .add_systems(Update, focus.before(PlaySet::Input))
        .add_systems(Update, (smoke, start, choose, end, show_panel, prompt, declutter).chain().after(PlaySet::Input))
        .add_systems(PostUpdate, view.after(super::camera::follow).before(TransformSystems::Propagate));
}

/// The nearest talkable in reach and in front of the player (not while talking).
fn focus(
    talking: Res<Talking>,
    mut focus: ResMut<TalkFocus>,
    players: Query<&Transform, With<Player>>,
    npcs: Query<(Entity, &Transform, Option<&super::Fighter>), (With<Talkable>, Without<super::ai::Autopilot>)>,
) {
    let want = (!talking.active())
        .then(|| players.single().ok())
        .flatten()
        .and_then(|p| {
            let fwd = p.rotation * Vec3::Z;
            npcs.iter()
                .filter_map(|(e, t, f)| {
                    if f.is_some_and(|f| f.dead) { return None; }
                    let to = (t.translation - p.translation).with_y(0.0);
                    let d = to.length();
                    (d < REACH && to.normalize_or_zero().dot(fwd.with_y(0.0).normalize_or_zero()) > IN_FRONT).then_some((e, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(e, _)| e)
        });
    if focus.0 != want {
        focus.0 = want;
    }
}

/// `GB_TALK=<name>`: start talking to that NPC 2 s in (headless screenshots).
fn smoke(
    time: Res<Time>,
    mut done: Local<bool>,
    mut talking: ResMut<Talking>,
    npcs: Query<(Entity, &Talkable)>,
    mut started: MessageWriter<TalkStarted>,
) {
    if *done || time.elapsed_secs() < 2.0 {
        return;
    }
    *done = true;
    let Ok(want) = std::env::var("GB_TALK") else { return };
    if let Some((e, t)) = npcs.iter().find(|(_, t)| t.name.eq_ignore_ascii_case(want.trim())) {
        talking.npc = Some(e);
        started.write(TalkStarted { npc: e, role: t.role });
    }
}

fn start(
    focus: Res<TalkFocus>,
    mut talking: ResMut<Talking>,
    players: Query<&Intent, With<Player>>,
    npcs: Query<&Talkable>,
    mut started: MessageWriter<TalkStarted>,
) {
    let Ok(intent) = players.single() else { return };
    if !intent.talk_pressed || talking.active() {
        return;
    }
    let Some(npc) = focus.0 else { return };
    let Ok(t) = npcs.get(npc) else { return };
    talking.npc = Some(npc);
    started.write(TalkStarted { npc, role: t.role });
}

/// Keys 1–7, clicks on options, Esc.
fn choose(
    keys: Res<ButtonInput<KeyCode>>,
    talking: Res<Talking>,
    dialogue: Res<Dialogue>,
    npcs: Query<&Talkable>,
    rows: Query<(&OptionRow, Ref<Interaction>)>,
    mut chose: MessageWriter<Chose>,
    mut end: MessageWriter<EndTalk>,
) {
    let Some(npc) = talking.npc else { return };
    let Ok(t) = npcs.get(npc) else {
        end.write(EndTalk);
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        end.write(EndTalk);
        return;
    }
    const DIGITS: [KeyCode; OPTIONS] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
    ];
    let mut pick = DIGITS.iter().position(|k| keys.just_pressed(*k));
    for (row, i) in &rows {
        if i.is_changed() && *i == Interaction::Pressed {
            pick = Some(row.0);
        }
    }
    let Some(choice) = pick.and_then(|i| dialogue.options.get(i)) else { return };
    if choice.id == "leave" {
        end.write(EndTalk);
    } else {
        chose.write(Chose { npc, role: t.role, id: choice.id });
    }
}

fn end(mut ends: MessageReader<EndTalk>, mut talking: ResMut<Talking>, mut dialogue: ResMut<Dialogue>) {
    if ends.read().count() > 0 && talking.active() {
        talking.npc = None;
        *dialogue = Dialogue::default();
    }
}

#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Speaker;
#[derive(Component)]
struct Line;
#[derive(Component)]
struct OptionRow(usize);
#[derive(Component)]
struct OptionText(usize);
#[derive(Component)]
struct Prompt;

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            KeepInTalk,
            panel(
                "stats",
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: percent(50.0),
                    bottom: px(24.0),
                    margin: UiRect::left(px(-PANEL_W * 0.5)),
                    width: px(PANEL_W),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(14.0)),
                    row_gap: px(6.0),
                    ..default()
                },
            ),
            GlobalZIndex(50),
            Interaction::default(),
        ))
        .with_children(|p| {
            p.spawn((Speaker, label("", "title")));
            p.spawn((
                Line,
                label("", "row.text"),
                Typewriter::new("", LINE_RATE),
                Node {
                    width: px(PANEL_W - 28.0),
                    min_height: px(30.0),
                    ..default()
                },
            ));
            for i in 0..OPTIONS {
                p.spawn((
                    OptionRow(i),
                    Button,
                    Node {
                        height: px(21.0),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(8.0)),
                        ..default()
                    },
                    Styled::new("row"),
                    UiFlags::default(),
                ))
                .with_child((OptionText(i), label("", "row.text")));
            }
        });
    // "Talk to Mira [F]", shown while a talkable is in focus.
    commands.spawn((
        Prompt,
        label("", "text.accent"),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            left: percent(50.0),
            bottom: px(150.0),
            margin: UiRect::left(px(-90.0)),
            ..default()
        },
        GlobalZIndex(30),
    ));
}

/// The panel follows `Talking` and `Dialogue`.
#[allow(clippy::type_complexity)]
fn show_panel(
    talking: Res<Talking>,
    dialogue: Res<Dialogue>,
    npcs: Query<&Talkable>,
    mut root: Query<&mut Node, (With<Panel>, Without<OptionRow>)>,
    mut speaker: Query<&mut Text, (With<Speaker>, Without<OptionText>)>,
    mut line: Query<&mut Typewriter, With<Line>>,
    mut rows: Query<(&OptionRow, &mut Node), Without<Panel>>,
    mut texts: Query<(&OptionText, &mut Text), Without<Speaker>>,
) {
    let who = talking.npc.and_then(|e| npcs.get(e).ok());
    for mut node in &mut root {
        let want = if who.is_some() { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    let Some(who) = who else { return };
    if !(talking.is_changed() || dialogue.is_changed()) {
        return;
    }
    for mut t in &mut speaker {
        t.0 = format!("▌{}   ·   {}", who.name.to_uppercase(), who.title);
    }
    for mut tw in &mut line {
        tw.retype(&dialogue.line);
    }
    for (row, mut node) in &mut rows {
        let want = if row.0 < dialogue.options.len() { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (o, mut t) in &mut texts {
        t.0 = dialogue.options.get(o.0).map_or(String::new(), |c| format!("{}.  {}", o.0 + 1, c.label));
    }
}

/// While talking, hide the rest of the HUD (only what this hid comes back).
#[allow(clippy::type_complexity)]
fn declutter(
    talking: Res<Talking>,
    mut hidden: Local<Vec<Entity>>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>, Without<KeepInTalk>)>,
) {
    if talking.active() {
        for (e, mut v) in &mut roots {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
                hidden.push(e);
            }
        }
    } else if !hidden.is_empty() {
        for e in hidden.drain(..) {
            if let Ok((_, mut v)) = roots.get_mut(e) {
                *v = Visibility::Inherited;
            }
        }
    }
}

/// "Talk to Mira  [F]" while one is in focus.
fn prompt(focus: Res<TalkFocus>, npcs: Query<&Talkable>, mut q: Query<(&mut Node, &mut Text), With<Prompt>>) {
    let line = focus.0.and_then(|e| npcs.get(e).ok()).map(|t| format!("[F] Talk to {}", t.name));
    for (mut node, mut text) in &mut q {
        let want = if line.is_some() { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        if let Some(l) = &line
            && text.0 != *l
        {
            text.0 = l.clone();
        }
    }
}

/// World position of a character's head bone (fallback: the top of its capsule).
fn head(rig: Option<&Rig>, bank: Option<&PoseBank>, globals: &Query<&GlobalTransform>, tf: &Transform) -> Vec3 {
    rig.zip(bank)
        .and_then(|(r, b)| r.bones.get(b.bones.head).copied())
        .and_then(|e| globals.get(e).ok())
        .map(|g| g.translation())
        .filter(|p| p.is_finite())
        .unwrap_or(tf.translation + Vec3::Y * (HALF_HEIGHT * 0.75))
}

/// The first-person close-up: eyes to face, eased in and out, the player's body hidden.
#[allow(clippy::type_complexity)]
fn view(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut talking: ResMut<Talking>,
    mut cams: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    players: Query<(Entity, &Transform, Option<&Rig>, Option<&Visibility>), (With<Player>, Without<MainCamera>)>,
    mut commands: Commands,
    npcs: Query<(&Transform, Option<&Rig>), (Without<Player>, Without<MainCamera>)>,
    globals: Query<&GlobalTransform>,
    mut last_npc: Local<Option<Entity>>,
) {
    let dt = time.delta_secs();
    let want = if talking.active() { 1.0 } else { 0.0 };
    talking.blend += (want - talking.blend) * (BLEND_RATE * dt).min(1.0);
    if talking.blend < 1e-3 {
        talking.blend = 0.0;
    }
    if let Some(n) = talking.npc {
        *last_npc = Some(n);
    }
    let b = talking.blend;
    let Ok((player, ptf, prig, vis)) = players.single() else { return };
    // Hide the body for first person (the root may not carry a Visibility until now).
    let want_vis = if b > 0.5 { Visibility::Hidden } else { Visibility::Inherited };
    if vis != Some(&want_vis) && (vis.is_some() || b > 0.5) {
        commands.entity(player).try_insert(want_vis);
    }
    for (mut tf, mut proj) in &mut cams {
        if let Projection::Perspective(p) = &mut *proj {
            p.fov = (60.0 + (TALK_FOV - 60.0) * b).to_radians();
        }
        if b <= 0.0 {
            continue;
        }
        let Some((ntf, nrig)) = last_npc.and_then(|n| npcs.get(n).ok()) else { continue };
        let face = head(nrig, bank.as_deref(), &globals, ntf) + Vec3::Y * 0.02;
        let eyes = head(prig, bank.as_deref(), &globals, ptf);
        let toward = (face - eyes).normalize_or(ptf.rotation * Vec3::Z);
        let sway = Vec3::new(0.0, 0.006 * (time.elapsed_secs() * 1.3).sin(), 0.0);
        let eye = eyes + toward * EYE_FORWARD + sway;
        let close = Transform::from_translation(eye).looking_at(face, Vec3::Y);
        tf.translation = tf.translation.lerp(close.translation, b);
        tf.rotation = tf.rotation.slerp(close.rotation, b);
    }
}

/// A short log line from an NPC (chatter, greetings), when the player is near enough to hear.
pub fn say(log: &mut MessageWriter<HudLog>, who: &str, words: &str) {
    log.write(HudLog::new(LogKind::Speech, format!("{who}: {words}")).said(format!("{who}: {words}")));
}

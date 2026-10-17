//! The message log: a terminal column above the map. Anything can write a `HudLog` message; this
//! module alone turns them into `gbui` log lines (class per `LogKind`, typed or decoded). Lines
//! live `LIFE` seconds and fade, like a single-player chat. A `key` makes a message rewrite its
//! live line instead of adding one (pickup counts). Speech carries what the speaker `said`
//! (gibberish), which decodes into the text.
use super::MARGIN;
use bevy::prelude::*;
use gbui::prelude::*;

const WIDTH: f32 = 470.0;
/// Bottom edge: above the map and its readout.
const BOTTOM: f32 = MARGIN + super::minimap::SIZE + 34.0;
const MAX_LINES: usize = 9;
const LIFE: f32 = 14.0;
const FADE: f32 = 2.5;
/// Typing rate for info and system lines (chars/s); speech decode delay and length (s).
const TYPE_RATE: f32 = 75.0;
const SPEECH_HOLD: f32 = 0.9;
const SPEECH_DECODE: f32 = 1.3;

pub fn plugin(app: &mut App) {
    app.add_message::<HudLog>()
        .add_systems(Startup, (spawn, greet).chain())
        .add_systems(Update, forward);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    Info,
    Item,
    Warn,
    Speech,
    System,
}

#[derive(Message, Clone, Debug)]
pub struct HudLog {
    pub text: String,
    pub kind: LogKind,
    /// Rewrite the live line with this key instead of adding one.
    pub key: Option<u64>,
    /// Speech: the words as heard; the line shows them, then decodes them into `text`.
    pub said: Option<String>,
}

impl HudLog {
    pub fn new(kind: LogKind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind,
            key: None,
            said: None,
        }
    }
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }
    pub fn said(mut self, said: impl Into<String>) -> Self {
        self.said = Some(said.into());
        self
    }
}

/// The log column.
#[derive(Component)]
pub struct HudLogColumn;

fn spawn(mut commands: Commands) {
    commands.spawn((
        HudLogColumn,
        TermLog {
            max_lines: MAX_LINES,
            life: LIFE,
            fade: FADE,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(MARGIN),
            bottom: px(BOTTOM),
            width: px(WIDTH),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            row_gap: px(2.0),
            ..default()
        },
        GlobalZIndex(20),
    ));
}

fn greet(mut log: MessageWriter<HudLog>) {
    log.write(HudLog::new(LogKind::System, "guildbound // link established"));
    log.write(HudLog::new(LogKind::System, "[F] use   [Q] art   [E] spell   [MB5] next spell   [I] bag   [Esc] free cursor"));
}

fn forward(mut reads: MessageReader<HudLog>, column: Query<Entity, With<HudLogColumn>>, mut writes: MessageWriter<LogWrite>) {
    let Ok(column) = column.single() else {
        reads.clear();
        return;
    };
    for m in reads.read() {
        let (class, effect) = match m.kind {
            LogKind::Info => ("log.line", LineEffect::Type(TYPE_RATE)),
            LogKind::System => ("log.system", LineEffect::Type(TYPE_RATE)),
            LogKind::Item => ("log.item", LineEffect::Plain),
            LogKind::Warn => ("log.warn", LineEffect::Plain),
            LogKind::Speech => (
                "log.speech",
                match &m.said {
                    Some(said) => LineEffect::Decode {
                        from: said.clone(),
                        delay: SPEECH_HOLD,
                        duration: SPEECH_DECODE,
                    },
                    None => LineEffect::Type(TYPE_RATE),
                },
            ),
        };
        let mut w = LogWrite::new(column, m.text.clone()).class(class).effect(effect);
        if let Some(key) = m.key {
            w = w.key(key);
        }
        writes.write(w);
    }
}

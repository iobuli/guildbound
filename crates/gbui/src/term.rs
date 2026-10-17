//! Terminal text.
//! - `Typewriter`: reveals its text at `rate` characters per second behind a `▌` cursor.
//! - `Decode`: shows `from`, then scrambles it glyph by glyph into `to` (left to right with a
//!   little jitter): the "translation" effect for NPC speech.
//! - `TermLog`: a column of lines written through `LogWrite` messages. Lines slide in, live
//!   `life` seconds and fade over the last `fade`; past `max_lines` the oldest go. A write with a
//!   `key` rewrites the live line with the same key instead of adding one ("+3 Log" counting up).
//!
//! `Typewriter` and `Decode` own their entity's `Text`; give a text one of them at most.
use crate::style::{Opacity, Styled};
use bevy::prelude::*;
use std::borrow::Cow;

pub const CURSOR: char = '▌';
/// Seconds a new log line takes to fade in.
const APPEAR: f32 = 0.12;
/// Scramble frames per second while decoding.
const SCRAMBLE_HZ: f32 = 24.0;
/// How far ahead of locking a character starts scrambling (fraction of the decode).
const SCRAMBLE_LEAD: f32 = 0.22;

/// Glyphs a decode cycles through (all in JetBrains Mono).
pub const GLYPHS: &[char] = &[
    '#', '%', '&', '*', '+', '=', '/', '\\', '<', '>', '?', '$', '@', '0', '1', '~', '^', '░', '▒', '▓', '┼', '╱', '╲',
    '◇', '◆', 'λ', 'Σ', 'Ψ', 'Ω', 'Ж', 'Щ', 'Д', 'Ϟ', 'ξ',
];

fn hash(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

fn unit(a: u32, b: u32) -> f32 {
    (hash(a, b) & 0xFFFF) as f32 / 65535.0
}

/// How many of `len` characters a typewriter shows after `elapsed` seconds.
pub fn typed_chars(len: usize, elapsed: f32, rate: f32) -> usize {
    if rate <= 0.0 {
        return len;
    }
    ((elapsed.max(0.0) * rate) as usize).min(len)
}

/// The text of a decode at `progress` 0..1: positions lock to `to` left to right (jittered by
/// `seed`), scramble just before they lock, and show `from` before that. Spaces in the target
/// stay spaces while scrambling, so words keep their shape. 0 gives `from`, 1 gives `to`.
pub fn decode_text(from: &str, to: &str, progress: f32, seed: u32, tick: u32) -> String {
    if progress <= 0.0 {
        return from.to_string();
    }
    if progress >= 1.0 {
        return to.to_string();
    }
    let f: Vec<char> = from.chars().collect();
    let t: Vec<char> = to.chars().collect();
    let n = f.len().max(t.len()).max(1);
    let mut out = String::with_capacity(n * 2);
    for i in 0..n {
        let lock = 0.8 * i as f32 / n as f32 + 0.15 * unit(seed, i as u32);
        let c = if progress >= lock {
            t.get(i).copied()
        } else if progress >= lock - SCRAMBLE_LEAD {
            match t.get(i).or(f.get(i)) {
                Some(' ') => Some(' '),
                _ => Some(GLYPHS[hash(seed ^ tick.wrapping_mul(0x51ED), i as u32) as usize % GLYPHS.len()]),
            }
        } else {
            Some(f.get(i).copied().unwrap_or(' '))
        };
        if let Some(c) = c {
            out.push(c);
        }
    }
    out
}

#[derive(Component, Clone, Debug, Default)]
pub struct Typewriter {
    pub text: String,
    /// Shown in front, untyped.
    pub prefix: String,
    /// Characters per second (0: all at once).
    pub rate: f32,
    pub elapsed: f32,
}

impl Typewriter {
    pub fn new(text: impl Into<String>, rate: f32) -> Self {
        Self {
            text: text.into(),
            rate,
            ..default()
        }
    }
    /// Type `text` from the start (no-op if it is already the text).
    pub fn retype(&mut self, text: &str) {
        if self.text != text {
            self.text = text.to_string();
            self.elapsed = 0.0;
        }
    }
    pub fn render(&self) -> String {
        let len = self.text.chars().count();
        let shown = typed_chars(len, self.elapsed, self.rate);
        let mut s = self.prefix.clone();
        s.extend(self.text.chars().take(shown));
        if shown < len {
            s.push(CURSOR);
        }
        s
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Decode {
    pub from: String,
    pub to: String,
    pub prefix: String,
    /// Seconds `from` is shown untouched, then seconds the decode takes.
    pub delay: f32,
    pub duration: f32,
    pub elapsed: f32,
    pub seed: u32,
}

impl Decode {
    pub fn new(from: impl Into<String>, to: impl Into<String>, delay: f32, duration: f32) -> Self {
        let (from, to) = (from.into(), to.into());
        let seed = to.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
        Self {
            from,
            to,
            delay,
            duration,
            seed,
            ..default()
        }
    }
    pub fn render(&self) -> String {
        let p = ((self.elapsed - self.delay) / self.duration.max(1e-3)).clamp(0.0, 1.0);
        let tick = (self.elapsed * SCRAMBLE_HZ) as u32;
        format!("{}{}", self.prefix, decode_text(&self.from, &self.to, p, self.seed, tick))
    }
}

pub fn type_text(time: Res<Time<Real>>, mut texts: Query<(&mut Typewriter, &mut Text)>) {
    let dt = time.delta_secs().min(0.1);
    for (mut tw, mut text) in &mut texts {
        tw.elapsed += dt;
        let s = tw.render();
        if text.0 != s {
            text.0 = s;
        }
    }
}

pub fn decode(time: Res<Time<Real>>, mut texts: Query<(&mut Decode, &mut Text)>) {
    let dt = time.delta_secs().min(0.1);
    for (mut d, mut text) in &mut texts {
        if d.elapsed > d.delay + d.duration + 1.0 {
            continue;
        }
        d.elapsed += dt;
        let s = d.render();
        if text.0 != s {
            text.0 = s;
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct TermLog {
    pub max_lines: usize,
    /// Seconds a line lives (`f32::INFINITY`: forever) and fades over at the end.
    pub life: f32,
    pub fade: f32,
    pub prefix: String,
    /// Class of lines written without one.
    pub line_class: Cow<'static, str>,
}

impl Default for TermLog {
    fn default() -> Self {
        Self {
            max_lines: 8,
            life: 12.0,
            fade: 2.0,
            prefix: "› ".to_string(),
            line_class: "log.line".into(),
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct LogLine {
    pub log: Entity,
    pub key: Option<u64>,
    pub age: f32,
}

#[derive(Clone, Debug, Default)]
pub enum LineEffect {
    #[default]
    Plain,
    /// Typed at this many characters per second.
    Type(f32),
    /// Shows `from`, then decodes it into the line's text.
    Decode { from: String, delay: f32, duration: f32 },
}

#[derive(Message, Clone, Debug)]
pub struct LogWrite {
    pub log: Entity,
    pub text: String,
    pub class: Option<Cow<'static, str>>,
    pub key: Option<u64>,
    pub effect: LineEffect,
}

impl LogWrite {
    pub fn new(log: Entity, text: impl Into<String>) -> Self {
        Self {
            log,
            text: text.into(),
            class: None,
            key: None,
            effect: LineEffect::Plain,
        }
    }
    pub fn class(mut self, class: impl Into<Cow<'static, str>>) -> Self {
        self.class = Some(class.into());
        self
    }
    pub fn key(mut self, key: u64) -> Self {
        self.key = Some(key);
        self
    }
    pub fn effect(mut self, effect: LineEffect) -> Self {
        self.effect = effect;
        self
    }
}

pub fn write_logs(
    mut commands: Commands,
    mut writes: MessageReader<LogWrite>,
    logs: Query<&TermLog>,
    mut lines: Query<(Entity, &mut LogLine, &mut Text)>,
    mut fresh: Local<Vec<(Entity, u64, Entity)>>,
) {
    fresh.clear();
    for w in writes.read() {
        let Ok(log) = logs.get(w.log) else { continue };
        let text = format!("{}{}", log.prefix, w.text);
        if let Some(key) = w.key {
            if let Some(&(_, _, line)) = fresh.iter().find(|(l, k, _)| *l == w.log && *k == key) {
                commands.entity(line).insert(Text::new(text)).remove::<(Typewriter, Decode)>();
                continue;
            }
            if let Some((line, mut l, mut t)) = lines
                .iter_mut()
                .find(|(_, l, _)| l.log == w.log && l.key == Some(key) && l.age < log.life - log.fade)
            {
                l.age = APPEAR;
                t.0 = text;
                commands.entity(line).remove::<(Typewriter, Decode)>();
                continue;
            }
        }
        let class = w.class.clone().unwrap_or_else(|| log.line_class.clone());
        let mut line = commands.spawn((
            LogLine {
                log: w.log,
                key: w.key,
                age: 0.0,
            },
            Text::new(""),
            TextFont::default(),
            Styled(class),
            Opacity(0.0),
            ChildOf(w.log),
        ));
        match &w.effect {
            LineEffect::Plain => {
                line.insert(Text::new(text));
            }
            LineEffect::Type(rate) => {
                line.insert(Typewriter {
                    text: w.text.clone(),
                    prefix: log.prefix.clone(),
                    rate: *rate,
                    elapsed: 0.0,
                });
            }
            LineEffect::Decode { from, delay, duration } => {
                let mut d = Decode::new(from.clone(), w.text.clone(), *delay, *duration);
                d.prefix = log.prefix.clone();
                line.insert((Text::new(d.render()), d));
            }
        }
        if let Some(key) = w.key {
            fresh.push((w.log, key, line.id()));
        }
    }
}

pub fn age_logs(
    mut commands: Commands,
    time: Res<Time<Real>>,
    logs: Query<(&TermLog, &Children)>,
    mut lines: Query<(&mut LogLine, &mut Opacity)>,
) {
    let dt = time.delta_secs().min(0.1);
    for (log, children) in &logs {
        let live = children.iter().filter(|c| lines.contains(*c)).count();
        let mut excess = live.saturating_sub(log.max_lines.max(1));
        for child in children.iter() {
            let Ok((mut line, mut opacity)) = lines.get_mut(child) else { continue };
            line.age += dt;
            if excess > 0 || line.age >= log.life {
                excess = excess.saturating_sub(1);
                commands.entity(child).try_despawn();
                continue;
            }
            let appear = (line.age / APPEAR).min(1.0);
            let fade = if log.life.is_finite() { ((log.life - line.age) / log.fade.max(1e-3)).clamp(0.0, 1.0) } else { 1.0 };
            opacity.set_if_neq(Opacity(appear * fade));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typewriter_reveals_at_its_rate_behind_a_cursor() {
        assert_eq!(typed_chars(10, 0.0, 20.0), 0);
        assert_eq!(typed_chars(10, 0.26, 20.0), 5);
        assert_eq!(typed_chars(10, 9.0, 20.0), 10);
        assert_eq!(typed_chars(10, 0.0, 0.0), 10, "rate 0 shows everything");
        let mut tw = Typewriter::new("héllo", 10.0);
        tw.prefix = "> ".into();
        tw.elapsed = 0.25;
        assert_eq!(tw.render(), "> hé▌", "counts characters, not bytes");
        tw.elapsed = 1.0;
        assert_eq!(tw.render(), "> héllo");
        tw.retype("héllo");
        assert_eq!(tw.elapsed, 1.0, "same text keeps going");
        tw.retype("bye");
        assert_eq!(tw.elapsed, 0.0);
    }

    #[test]
    fn decode_runs_from_source_to_target() {
        let (from, to) = ("Vashen tul, oro kessa?", "Halt, traveller. Where to?");
        assert_eq!(decode_text(from, to, 0.0, 7, 0), from);
        assert_eq!(decode_text(from, to, 1.0, 7, 0), to);
        assert_eq!(decode_text(from, to, 0.5, 7, 3), decode_text(from, to, 0.5, 7, 3), "deterministic");
        // positions locked to the target never go back as progress grows
        let locked = |p: f32| -> usize {
            let n = from.chars().count().max(to.chars().count());
            (0..n).filter(|&i| p >= 0.8 * i as f32 / n as f32 + 0.15 * unit(7, i as u32)).count()
        };
        let mut last = 0;
        for k in 0..=20 {
            let now = locked(k as f32 / 20.0);
            assert!(now >= last);
            last = now;
        }
        assert_eq!(locked(0.96), to.chars().count().max(from.chars().count()), "all locked before the end");
        // target spaces stay spaces while scrambling
        let mid = decode_text("aaaa aaaa", "bbbb bbbb", 0.45, 1, 2);
        assert_eq!(mid.chars().nth(4), Some(' '));
        // a longer source shrinks to the target
        assert_eq!(decode_text("abcdefgh", "xy", 1.0, 3, 0), "xy");
        assert!(decode_text("abcdefgh", "xy", 0.999, 3, 0).chars().count() <= 8);
    }

    #[test]
    fn log_lines_merge_by_key_and_respect_the_line_limit() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<LogWrite>()
            .add_systems(Update, (write_logs, age_logs).chain());
        let log = app
            .world_mut()
            .spawn(TermLog {
                max_lines: 3,
                ..default()
            })
            .id();
        let send = |app: &mut App, w: LogWrite| {
            app.world_mut().write_message(w);
            app.update();
        };
        send(&mut app, LogWrite::new(log, "+1 Log").key(1));
        send(&mut app, LogWrite::new(log, "+2 Log").key(1));
        let texts = |app: &mut App| -> Vec<String> {
            let mut q = app.world_mut().query::<(&LogLine, &Text)>();
            q.iter(app.world()).map(|(_, t)| t.0.clone()).collect()
        };
        assert_eq!(texts(&mut app), vec!["› +2 Log".to_string()], "same key rewrites the line");
        for i in 0..4 {
            send(&mut app, LogWrite::new(log, format!("line {i}")));
        }
        app.update();
        let now = texts(&mut app);
        assert_eq!(now.len(), 3, "{now:?}");
        assert!(now.contains(&"› line 3".to_string()) && !now.contains(&"› +2 Log".to_string()));
    }
}

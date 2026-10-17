//! gbperf: a frame profiler that knows nothing about the game.
//! - Frame time (from `Time<Real>`): average, 99th percentile and worst over the last
//!   `WINDOW` frames, FPS, hitches (a frame over `HITCH_FACTOR` × the average and over
//!   `HITCH_MIN_MS`).
//! - Main-thread CPU per frame: wall time from `First` to `Last`.
//! - Spans: `app.perf_span(schedule, "name", set)` times the wall clock from just before `set`
//!   to just after it, summed over the frame (a `FixedUpdate` set runs 0..n times a frame). The
//!   start runs somewhere before the set, so a span can include unrelated systems scheduled in
//!   between; chained sets keep that small.
//! - Fixed ticks per frame (more than the usual 1–2 means the fixed loop is catching up).
//! - GPU / render CPU time per render pass from Bevy's `RenderDiagnosticsPlugin` (timestamp
//!   queries on DX12 / Vulkan; the busiest `GPU_ROWS` passes are shown).
//! - Entity count.
//!
//! F3 (`GbperfPlugin::key`) shows a `gbui` panel at the top right (classes `perf`,
//! `perf.text`). Every `LOG_EVERY` seconds, and once on exit, a summary line goes to
//! `GbperfPlugin::log` (rewritten each run), so every playtest leaves a baseline.
use bevy::app::AppExit;
use bevy::diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use gbui::prelude::*;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

/// Frames kept for the statistics.
const WINDOW: usize = 240;
/// A hitch: a frame longer than this many times the average, and longer than `HITCH_MIN_MS`.
const HITCH_FACTOR: f32 = 2.5;
const HITCH_MIN_MS: f32 = 25.0;
/// Seconds between log lines; panel refreshes per second.
const LOG_EVERY: f32 = 5.0;
const PANEL_HZ: f32 = 4.0;
/// GPU passes shown in the panel and the log.
const GPU_ROWS: usize = 6;

pub struct GbperfPlugin {
    /// Log file (filesystem path), rewritten at startup.
    pub log: PathBuf,
    /// Toggles the panel.
    pub key: KeyCode,
}

impl Plugin for GbperfPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            RenderDiagnosticsPlugin,
        ))
        .init_resource::<Perf>()
        .insert_resource(PerfConfig {
            log: self.log.clone(),
            key: self.key,
        })
        .add_systems(Startup, (start_log, spawn_panel))
        .add_systems(First, frame_start)
        .add_systems(FixedFirst, count_fixed_tick)
        .add_systems(Last, (frame_end, write_log, toggle, refresh).chain());
    }
}

#[derive(Resource, Clone, Debug)]
struct PerfConfig {
    log: PathBuf,
    key: KeyCode,
}

/// Rolling per-frame samples (ms or counts).
#[derive(Clone, Debug, Default)]
pub struct Samples(VecDeque<f32>);

impl Samples {
    pub fn push(&mut self, v: f32) {
        if self.0.len() == WINDOW {
            self.0.pop_front();
        }
        self.0.push_back(v);
    }
    pub fn avg(&self) -> f32 {
        if self.0.is_empty() { 0.0 } else { self.0.iter().sum::<f32>() / self.0.len() as f32 }
    }
    pub fn max(&self) -> f32 {
        self.0.iter().copied().fold(0.0, f32::max)
    }
    /// The `q` quantile (0..1).
    pub fn quantile(&self, q: f32) -> f32 {
        if self.0.is_empty() {
            return 0.0;
        }
        let mut v: Vec<f32> = self.0.iter().copied().collect();
        v.sort_by(f32::total_cmp);
        v[((v.len() - 1) as f32 * q.clamp(0.0, 1.0)).round() as usize]
    }
}

#[derive(Clone, Debug)]
struct Span {
    name: &'static str,
    open: Option<Instant>,
    /// This frame's total (ms).
    acc: f32,
    samples: Samples,
}

#[derive(Resource, Default)]
pub struct Perf {
    pub frame: Samples,
    pub cpu: Samples,
    pub fixed: Samples,
    pub hitches: u32,
    /// Worst hitch (ms) since the last log line.
    pub worst_hitch: f32,
    spans: Vec<Span>,
    frame_start: Option<Instant>,
    fixed_this_frame: u32,
    clock: f32,
    since_log: f32,
    since_panel: f32,
    open: bool,
}

impl Perf {
    fn open_span(&mut self, i: usize) {
        self.spans[i].open = Some(Instant::now());
    }
    fn close_span(&mut self, i: usize) {
        if let Some(t) = self.spans[i].open.take() {
            self.spans[i].acc += t.elapsed().as_secs_f32() * 1000.0;
        }
    }
}

pub trait PerfApp {
    /// Time `set` in `schedule` under `name` (see the module doc).
    fn perf_span<M>(
        &mut self,
        schedule: impl ScheduleLabel + Clone,
        name: &'static str,
        set: impl IntoSystemSet<M> + Clone,
    ) -> &mut Self;
}

impl PerfApp for App {
    fn perf_span<M>(
        &mut self,
        schedule: impl ScheduleLabel + Clone,
        name: &'static str,
        set: impl IntoSystemSet<M> + Clone,
    ) -> &mut Self {
        let i = {
            let mut perf = self.world_mut().get_resource_or_init::<Perf>();
            perf.spans.push(Span {
                name,
                open: None,
                acc: 0.0,
                samples: Samples::default(),
            });
            perf.spans.len() - 1
        };
        self.add_systems(schedule.clone(), (move |mut p: ResMut<Perf>| p.open_span(i)).before(set.clone()));
        self.add_systems(schedule, (move |mut p: ResMut<Perf>| p.close_span(i)).after(set))
    }
}

fn frame_start(mut perf: ResMut<Perf>) {
    perf.frame_start = Some(Instant::now());
}

fn count_fixed_tick(mut perf: ResMut<Perf>) {
    perf.fixed_this_frame += 1;
}

fn frame_end(time: Res<Time<Real>>, mut perf: ResMut<Perf>) {
    let perf = &mut *perf;
    let dt = time.delta_secs();
    perf.clock += dt;
    perf.since_log += dt;
    perf.since_panel += dt;
    let ms = dt * 1000.0;
    let avg = perf.frame.avg();
    if perf.frame.0.len() > 30 && ms > HITCH_MIN_MS && ms > avg * HITCH_FACTOR {
        perf.hitches += 1;
        perf.worst_hitch = perf.worst_hitch.max(ms);
    }
    perf.frame.push(ms);
    if let Some(t) = perf.frame_start.take() {
        perf.cpu.push(t.elapsed().as_secs_f32() * 1000.0);
    }
    perf.fixed.push(perf.fixed_this_frame as f32);
    perf.fixed_this_frame = 0;
    for span in &mut perf.spans {
        span.samples.push(span.acc);
        span.acc = 0.0;
    }
}

/// The busiest render passes: (name, GPU ms, render-CPU ms), by GPU time.
fn gpu_passes(store: &DiagnosticsStore) -> Vec<(String, f64, f64)> {
    let mut passes: Vec<(String, f64, f64)> = Vec::new();
    for d in store.iter() {
        let path = d.path().as_str();
        let Some(rest) = path.strip_prefix("render/") else { continue };
        let (name, kind) = match rest.rsplit_once('/') {
            Some(split) => split,
            None => continue,
        };
        let v = d.smoothed().unwrap_or(0.0);
        let entry = match passes.iter_mut().find(|p| p.0 == name) {
            Some(e) => e,
            None => {
                passes.push((name.to_string(), 0.0, 0.0));
                passes.last_mut().unwrap()
            }
        };
        match kind {
            "elapsed_gpu" => entry.1 = v,
            "elapsed_cpu" => entry.2 = v,
            _ => {}
        }
    }
    passes.sort_by(|a, b| b.1.total_cmp(&a.1).then(b.2.total_cmp(&a.2)));
    passes.truncate(GPU_ROWS);
    passes
}

fn entities(store: &DiagnosticsStore) -> f64 {
    store.get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).and_then(|d| d.value()).unwrap_or(0.0)
}

fn summary(perf: &Perf, store: &DiagnosticsStore) -> String {
    let f = &perf.frame;
    let avg = f.avg();
    let mut s = format!(
        "t={:.0} fps={:.0} frame avg={:.1} p99={:.1} max={:.1} cpu={:.1} fixed={:.2}/{:.0} hitches={} worst={:.0} ent={:.0} |",
        perf.clock,
        if avg > 0.0 { 1000.0 / avg } else { 0.0 },
        avg,
        f.quantile(0.99),
        f.max(),
        perf.cpu.avg(),
        perf.fixed.avg(),
        perf.fixed.max(),
        perf.hitches,
        perf.worst_hitch,
        entities(store),
    );
    for span in &perf.spans {
        let _ = write!(s, " {}={:.2}/{:.2}", span.name, span.samples.avg(), span.samples.max());
    }
    s.push_str(" | gpu");
    for (name, gpu, cpu) in gpu_passes(store) {
        let _ = write!(s, " {name}={gpu:.2}/{cpu:.2}");
    }
    s
}

fn start_log(config: Res<PerfConfig>) {
    if let Some(dir) = config.log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let header = "# gbperf: frame ms (avg p99 max), main-thread cpu ms, fixed ticks/frame (avg/max), hitches, entities | span avg/max ms | gpu pass gpu/cpu ms\n";
    if let Err(e) = std::fs::write(&config.log, header) {
        warn!("[gbperf] {}: {e}", config.log.display());
    }
}

fn append(config: &PerfConfig, line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&config.log) {
        let _ = writeln!(f, "{line}");
    }
}

fn write_log(
    config: Res<PerfConfig>,
    store: Res<DiagnosticsStore>,
    mut perf: ResMut<Perf>,
    mut exits: MessageReader<AppExit>,
) {
    let exiting = exits.read().count() > 0;
    if perf.since_log < LOG_EVERY && !exiting {
        return;
    }
    perf.since_log = 0.0;
    let mut line = summary(&perf, &store);
    if exiting {
        line.push_str(" (exit)");
    }
    append(&config, &line);
    perf.hitches = 0;
    perf.worst_hitch = 0.0;
}

#[derive(Component)]
struct PerfPanel;

#[derive(Component)]
struct PerfText;

fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            PerfPanel,
            panel(
                "perf",
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12.0),
                    right: px(12.0),
                    padding: UiRect::axes(px(10.0), px(7.0)),
                    min_width: px(300.0),
                    ..default()
                },
            ),
            Visibility::Hidden,
            GlobalZIndex(90),
        ))
        .with_child((PerfText, label("", "perf.text")));
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<PerfConfig>,
    mut perf: ResMut<Perf>,
    mut panels: Query<&mut Visibility, With<PerfPanel>>,
) {
    if !keys.just_pressed(config.key) {
        return;
    }
    perf.open = !perf.open;
    perf.since_panel = 1.0;
    for mut vis in &mut panels {
        vis.set_if_neq(if perf.open { Visibility::Inherited } else { Visibility::Hidden });
    }
}

fn refresh(store: Res<DiagnosticsStore>, mut perf: ResMut<Perf>, mut texts: Query<&mut Text, With<PerfText>>) {
    if !perf.open || perf.since_panel < 1.0 / PANEL_HZ {
        return;
    }
    perf.since_panel = 0.0;
    let f = &perf.frame;
    let avg = f.avg();
    let mut s = format!(
        "▌PERF  {:>4.0} fps\nframe  {:>5.1} avg {:>5.1} p99 {:>5.1} max\ncpu    {:>5.1} ms   fixed {:.1}/{:.0}\nhitch  {:>5} (worst {:.0} ms)\nents   {:>5.0}\n",
        if avg > 0.0 { 1000.0 / avg } else { 0.0 },
        avg,
        f.quantile(0.99),
        f.max(),
        perf.cpu.avg(),
        perf.fixed.avg(),
        perf.fixed.max(),
        perf.hitches,
        perf.worst_hitch,
        entities(&store),
    );
    let mut spans: Vec<&Span> = perf.spans.iter().collect();
    spans.sort_by(|a, b| b.samples.avg().total_cmp(&a.samples.avg()));
    s.push_str("── cpu spans (avg / max ms)\n");
    for span in spans {
        let _ = writeln!(s, "{:<12} {:>6.2} {:>6.2}", span.name, span.samples.avg(), span.samples.max());
    }
    let passes = gpu_passes(&store);
    if !passes.is_empty() {
        s.push_str("── render passes (gpu / cpu ms)\n");
        for (name, gpu, cpu) in passes {
            let short: String = name.chars().rev().take(22).collect::<Vec<_>>().into_iter().rev().collect();
            let _ = writeln!(s, "{short:<22} {gpu:>5.2} {cpu:>5.2}");
        }
    }
    for mut text in &mut texts {
        if text.0 != s {
            text.0 = s.clone();
        }
    }
}

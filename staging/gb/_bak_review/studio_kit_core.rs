//! Pure core of the Studio panel kit (no Bevy, no egui). Compiles standalone.
//! Edits are event-sourced: every Studio edit is an `Edit` appended to `EditLog`; undo/redo move a
//! cursor over the log instead of mutating state, and state is rebuilt by replaying [0, cursor).
//! TODO(event_log): `Edit` rows become `event_log::Event`s (with Tick + Seq) once event_log.rs lands;
//! undo then appends a compensating event rather than truncating, so replay stays the source of truth.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Tick(pub u64); // TODO(event_log): replace with event_log::Tick

/// The five Studio tabs, in daviy's order. Keys 1-5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab { #[default] Pose, Physics, Animation, Events, Controller }
pub const TABS: [Tab; 5] = [Tab::Pose, Tab::Physics, Tab::Animation, Tab::Events, Tab::Controller];
impl Tab {
    pub fn label(self) -> &'static str {
        match self { Tab::Pose => "Pose", Tab::Physics => "Physics", Tab::Animation => "Animation", Tab::Events => "Events", Tab::Controller => "Controller" }
    }
    pub fn key(self) -> &'static str { ["1", "2", "3", "4", "5"][self as usize] }
}

/// Every Studio shortcut, one table, so tooltips and the key handler cannot drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { OpenStudio, Tab(Tab), ToggleRagdoll, ToggleBalance, Pause, Step, Undo, Redo }
pub fn shortcut(a: Action) -> &'static str {
    match a {
        Action::OpenStudio => "F2", Action::Tab(t) => t.key(), Action::ToggleRagdoll => "R", Action::ToggleBalance => "B",
        Action::Pause => "Space", Action::Step => ".", Action::Undo => "Ctrl+Z", Action::Redo => "Ctrl+Y",
    }
}

/// Margin chip class: green > 3 cm, amber 0..3 cm, red < 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginClass { Good, Warn, Bad }
pub fn margin_class(margin_m: f32) -> MarginClass {
    if margin_m > 0.03 { MarginClass::Good } else if margin_m >= 0.0 { MarginClass::Warn } else { MarginClass::Bad }
}

/// A Studio edit. Values carry before/after so undo needs no snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    RagdollSet { on: bool },
    /// Balance toggle. Turning it on restarts the 60-tick ramp from this event's tick.
    BalanceSet { on: bool },
    BoneRot { bone: u16, before: [f32; 4], after: [f32; 4] },
    Param { key: &'static str, before: f32, after: f32 },
    Select { bone: Option<u16> },
}

#[derive(Clone, Debug, Default)]
pub struct EditLog {
    pub rows: Vec<(Tick, Edit)>,
    /// rows[..cursor] are applied; rows[cursor..] are redoable.
    pub cursor: usize,
}
impl EditLog {
    /// New edit: drops the redo tail (classic undo semantics), then appends.
    pub fn push(&mut self, t: Tick, e: Edit) {
        self.rows.truncate(self.cursor);
        self.rows.push((t, e));
        self.cursor = self.rows.len();
    }
    pub fn can_undo(&self) -> bool { self.cursor > 0 }
    pub fn can_redo(&self) -> bool { self.cursor < self.rows.len() }
    pub fn undo(&mut self) -> Option<&(Tick, Edit)> { if self.can_undo() { self.cursor -= 1; self.rows.get(self.cursor) } else { None } }
    pub fn redo(&mut self) -> Option<&(Tick, Edit)> { if self.can_redo() { self.cursor += 1; self.rows.get(self.cursor - 1) } else { None } }
    /// Timeline scrub: jump the cursor to just after row `i` (or 0).
    pub fn seek(&mut self, applied: usize) { self.cursor = applied.min(self.rows.len()); }
    pub fn applied(&self) -> &[(Tick, Edit)] { &self.rows[..self.cursor] }

    /// Replay [0, cursor) into the toggle state: (ragdoll, balance, balance ramp anchor tick).
    pub fn toggles(&self) -> (bool, bool, Option<Tick>) {
        let (mut rag, mut bal, mut anchor) = (false, false, None);
        for (t, e) in self.applied() {
            match e {
                Edit::RagdollSet { on } => { rag = *on; if *on && bal { anchor = Some(*t); } }
                Edit::BalanceSet { on } => { if *on && !bal { anchor = Some(*t); } bal = *on; if !*on { anchor = None; } }
                _ => {}
            }
        }
        (rag, bal, anchor)
    }
    /// Current rotation of a bone after replay (last applied BoneRot), else None.
    pub fn bone_rot(&self, bone: u16) -> Option<[f32; 4]> {
        self.applied().iter().rev().find_map(|(_, e)| match e { Edit::BoneRot { bone: b, after, .. } if *b == bone => Some(*after), _ => None })
    }
    pub fn param(&self, key: &str) -> Option<f32> {
        self.applied().iter().rev().find_map(|(_, e)| match e { Edit::Param { key: k, after, .. } if *k == key => Some(*after), _ => None })
    }
    pub fn selected(&self) -> Option<u16> {
        self.applied().iter().rev().find_map(|(_, e)| match e { Edit::Select { bone } => Some(*bone), _ => None }).flatten()
    }
}

/// Ticks since balance ramp anchor, for phys_balance::ramp.
pub fn ramp_ticks(now: Tick, anchor: Option<Tick>) -> Option<u64> { anchor.map(|a| now.0.saturating_sub(a.0)) }

/// One-line description for the Events list / timeline tooltip.
pub fn describe(e: &Edit) -> String {
    match e {
        Edit::RagdollSet { on } => format!("Ragdoll {}", if *on { "on" } else { "off" }),
        Edit::BalanceSet { on } => format!("Balance {}", if *on { "on (ramp restarts)" } else { "off" }),
        Edit::BoneRot { bone, .. } => format!("Rotate bone {bone}"),
        Edit::Param { key, before, after } => format!("{key}: {before:.3} -> {after:.3}"),
        Edit::Select { bone } => match bone { Some(b) => format!("Select bone {b}"), None => "Clear selection".into() },
    }
}

/// Bone tree row (depth-first order, fixed). Filter keeps matches plus their ancestors.
#[derive(Clone, Debug)]
pub struct BoneRow { pub bone: u16, pub name: String, pub depth: u16, pub driver: usize }
pub fn filter_tree(rows: &[BoneRow], query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    if q.is_empty() { return (0..rows.len()).collect(); }
    let mut keep = vec![false; rows.len()];
    for i in 0..rows.len() {
        if rows[i].name.to_lowercase().contains(&q) {
            keep[i] = true;
            let mut d = rows[i].depth;
            for j in (0..i).rev() { if rows[j].depth < d { keep[j] = true; d = rows[j].depth; if d == 0 { break; } } }
        }
    }
    (0..rows.len()).filter(|&i| keep[i]).collect()
}

// ---- Motion trails (display only; never logged as events; gizmo-only drawing)
pub const TRAIL_DEFAULT: usize = 120;
/// Hard cap per bone and kind: 600 samples = 5 s at 120 Hz. The slider picks how many are drawn.
pub const TRAIL_CAP: usize = 600;
pub const TRAIL_RANGE: (usize, usize) = (30, 600);
pub const DOT_EVERY: u64 = 4;
pub const DIVERGE_EVERY: u64 = 8;
pub const ONION_EVERY: u64 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Actual, Target }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample { pub tick: u64, pub pos: [f32; 3], pub kind: Kind, /// feet only: planted this tick
    pub planted: Option<bool> }

/// Fixed-capacity Vec ring of samples, oldest overwritten.
#[derive(Clone, Debug)]
pub struct Ring { buf: Vec<Sample>, head: usize, len: usize }
impl Default for Ring { fn default() -> Self { Ring::with_cap(TRAIL_CAP) } }
impl Ring {
    pub fn with_cap(n: usize) -> Self { Ring { buf: Vec::with_capacity(n.max(1)), head: 0, len: 0 } }
    fn cap(&self) -> usize { self.buf.capacity().max(1).min(TRAIL_CAP) }
    pub fn push(&mut self, s: Sample) {
        if self.buf.len() < self.cap() { self.buf.push(s); self.len = self.buf.len(); self.head = self.len % self.cap(); return; }
        self.buf[self.head] = s;
        self.head = (self.head + 1) % self.buf.len();
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    /// Oldest -> newest, last `n` only.
    pub fn last(&self, n: usize) -> Vec<Sample> {
        let full = self.buf.len();
        let start = if full < self.cap() { 0 } else { self.head };
        let all = (0..full).map(|i| self.buf[(start + i) % full.max(1)]);
        let skip = full.saturating_sub(n);
        all.skip(skip).collect()
    }
    pub fn at_tick(&self, t: u64) -> Option<Sample> { self.buf.iter().copied().find(|s| s.tick == t) }
    pub fn clear(&mut self) { self.buf.clear(); self.head = 0; self.len = 0; }
}

/// One bone's trails: actual (physics result) and target (anim_interp), plus clip key ticks.
#[derive(Clone, Debug, Default)]
pub struct BoneTrail { pub actual: Ring, pub target: Ring, pub keys: Vec<u64> }

/// All trails, in bone order (Vec index = bone index). COM is its own ring.
#[derive(Clone, Debug, Default)]
pub struct TrailData { pub bones: Vec<BoneTrail>, pub com: Ring }
impl TrailData {
    fn slot(&mut self, bone: u16) -> &mut BoneTrail {
        let i = bone as usize;
        if self.bones.len() <= i { self.bones.resize_with(i + 1, BoneTrail::default); }
        &mut self.bones[i]
    }
    pub fn push_actual(&mut self, bone: u16, tick: u64, pos: [f32; 3], planted: Option<bool>) {
        self.slot(bone).actual.push(Sample { tick, pos, kind: Kind::Actual, planted });
    }
    /// Animation's entry point (anim_interp target positions).
    pub fn push_target(&mut self, bone: u16, tick: u64, pos: [f32; 3]) {
        self.slot(bone).target.push(Sample { tick, pos, kind: Kind::Target, planted: None });
    }
    /// Clip key times as ticks (diamonds on the target trail). Kept sorted, de-duplicated, capped.
    pub fn set_keys(&mut self, bone: u16, mut ticks: Vec<u64>) {
        ticks.sort_unstable(); ticks.dedup(); let n = ticks.len().saturating_sub(TRAIL_CAP); ticks.drain(..n);
        self.slot(bone).keys = ticks;
    }
    pub fn push_com(&mut self, tick: u64, pos: [f32; 3]) { self.com.push(Sample { tick, pos, kind: Kind::Actual, planted: None }); }
    pub fn clear(&mut self) { self.bones.clear(); self.com.clear(); }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TrailSet { All, Selected, #[default] Preset }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TrailColor { #[default] Speed, Driver, Plain }

/// Default set: hands, feet, head. TODO(verify) against the rig's bone names.
pub fn in_preset(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("hand") && !["thumb", "index", "middle", "ring", "pinky"].iter().any(|f| n.contains(f)))
        || (n.contains("foot") && !n.contains("toe")) || (n.contains("head") && !n.contains("top") && !n.contains("end"))
}
pub fn is_foot(name: &str) -> bool { let n = name.to_ascii_lowercase(); n.contains("foot") && !n.contains("toe") }

/// Fade alpha oldest -> newest: 0.08 .. 1.0.
pub fn trail_alpha(i: usize, len: usize) -> f32 { 0.08 + 0.92 * (i as f32 + 1.0) / len.max(1) as f32 }
/// Dashed ghost: draw segment i only on even dash blocks of 3 samples.
pub fn dash_on(i: usize) -> bool { (i / 3) % 2 == 0 }
pub fn is_dot_tick(t: u64) -> bool { t % DOT_EVERY == 0 }
pub fn is_diverge_tick(t: u64) -> bool { t % DIVERGE_EVERY == 0 }
pub fn is_onion_tick(t: u64) -> bool { t % ONION_EVERY == 0 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrClass { Good, Warn, Bad }
/// Divergence colour class. Hands/others: green < 2 cm, amber 2..8, red > 8 cm.
/// Planted feet (phys_stance `Contact.planted`, the same state balance uses): green < 1 cm, amber 1..3, red > 3 cm.
pub fn err_class(err_m: f32) -> ErrClass { err_class_for(err_m, false) }
pub fn err_class_for(err_m: f32, planted_foot: bool) -> ErrClass {
    let (g, r) = if planted_foot { (0.01, 0.03) } else { (0.02, 0.08) };
    if err_m < g { ErrClass::Good } else if err_m <= r { ErrClass::Warn } else { ErrClass::Bad }
}
/// Index of the fastest sample (central difference over ticks) in an oldest->newest run; None if < 3.
pub fn peak_speed_index(s: &[Sample]) -> Option<(usize, f32)> {
    if s.len() < 3 { return None; }
    (1..s.len() - 1).map(|i| {
        let (a, b) = (s[i - 1], s[i + 1]);
        let dt = (b.tick.saturating_sub(a.tick)).max(1) as f32 / 120.0;
        let d = [b.pos[0] - a.pos[0], b.pos[1] - a.pos[1], b.pos[2] - a.pos[2]];
        (i, (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() / dt)
    }).fold(None, |best: Option<(usize, f32)>, x| match best { Some(b) if b.1 >= x.1 => Some(b), _ => Some(x) })
}
/// Segment budget for "All bones": draw at most `budget` segments total by striding samples.
pub fn stride_for(bones: usize, samples: usize, budget: usize) -> usize {
    let want = bones.max(1) * samples.max(1);
    want.div_ceil(budget.max(1)).max(1)
}
pub const SEGMENT_BUDGET: usize = 10_000;

/// Divergence connectors: (target pos, actual pos, error m) every 8 ticks where both exist.
/// Each item also carries the actual sample's `planted` flag (feet) for err_class_for.
pub fn divergence(bt: &BoneTrail, n: usize) -> Vec<([f32; 3], [f32; 3], f32, bool)> {
    bt.actual.last(n).into_iter().filter(|a| is_diverge_tick(a.tick)).filter_map(|a| {
        let t = bt.target.at_tick(a.tick)?;
        let d = [a.pos[0] - t.pos[0], a.pos[1] - t.pos[1], a.pos[2] - t.pos[2]];
        Some((t.pos, a.pos, (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(), a.planted == Some(true)))
    }).collect()
}
/// Contact marks: (pos, planted_now) where a foot's planted flag flips.
pub fn contact_marks(samples: &[Sample]) -> Vec<([f32; 3], bool)> {
    samples.windows(2).filter_map(|w| match (w[0].planted, w[1].planted) { (Some(a), Some(b)) if a != b => Some((w[1].pos, b)), _ => None }).collect()
}
/// Key markers: target positions at clip-key ticks within the shown window.
pub fn key_markers(bt: &BoneTrail, n: usize) -> Vec<[f32; 3]> {
    bt.target.last(n).into_iter().filter(|s| bt.keys.binary_search(&s.tick).is_ok()).map(|s| s.pos).collect()
}

/// Speed colour ramp (sRGB 0..1): cyan #5fd7ff at 0 -> amber #ffbe61 at `mid` -> red #ff385a at 2*mid (m/s).
pub fn speed_rgb(speed: f32, mid: f32) -> [f32; 3] {
    let c = [0x5f as f32 / 255.0, 0xd7 as f32 / 255.0, 1.0];
    let a = [1.0, 0xbe as f32 / 255.0, 0x61 as f32 / 255.0];
    let r = [1.0, 0x38 as f32 / 255.0, 0x5a as f32 / 255.0];
    let x = (speed / mid.max(1e-3)).clamp(0.0, 2.0);
    let (p, q, t) = if x <= 1.0 { (c, a, x) } else { (a, r, x - 1.0) };
    [0, 1, 2].map(|k| p[k] + (q[k] - p[k]) * t)
}

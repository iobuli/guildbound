//! Extends: crates/game/src/sandbox.rs (pure, Bevy-free half). Pose-order slot 6 (Avian/ragdoll):
//! torque split before the physics step; metrics are built from the final post-physics pose.
//! Pure part of the sandbox (no Bevy). Compiles standalone against phys_balance.
//! Fixed bone order, whole ticks, Vec only.

use super::pb;

// ---- Event log placeholders. TODO(event_log): replace with crate::event_log / guildbound_sim::event_log
// types once The Rust Expert's event_log.rs lands in crates/sim/src. Shapes follow the posted contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Tick(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Seq(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigProfile { Master, Mannequin }
/// TODO(event_log): `RagdollOn` is not in the posted event list yet; ask The Rust Expert to add it
/// (and `RagdollOff`) to `Event`.
#[derive(Clone, Debug, PartialEq)]
pub enum Event { RagdollOn { rig: RigProfile }, RagdollOff }
/// Minimal append-only log stand-in: (tick, seq, event), kept sorted by append order.
#[derive(Clone, Debug, Default)]
pub struct EventLog { pub rows: Vec<(Tick, Seq, Event)> }
impl EventLog {
    pub fn push(&mut self, tick: Tick, ev: Event) -> Seq {
        let seq = Seq(self.rows.iter().filter(|r| r.0 == tick).count() as u32);
        self.rows.push((tick, seq, ev));
        seq
    }
    /// Tick of the latest RagdollOn not followed by RagdollOff. The balance ramp counts from this,
    /// so replay of the log ramps exactly like the live run.
    pub fn ragdoll_on_tick(&self) -> Option<Tick> {
        let mut on = None;
        for (t, _, e) in &self.rows {
            match e { Event::RagdollOn { .. } => on = Some(*t), Event::RagdollOff => on = None }
        }
        on
    }
}

// ---- Bone roles from the 79-bone table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side { Left = 0, Right = 1 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneRole { Other, Foot(Side), StanceLeg(Side) }

/// Role from a bone name in the existing table. Matches Mixamo-style and Blender-style names.
/// TODO(verify): exact names in the repo's 79-bone table for master.glb and the mannequin.
pub fn role_from_name(name: &str) -> BoneRole {
    let n = name.to_ascii_lowercase();
    let side = if n.contains("left") || n.ends_with(".l") || n.ends_with("_l") { Some(Side::Left) }
        else if n.contains("right") || n.ends_with(".r") || n.ends_with("_r") { Some(Side::Right) } else { None };
    let Some(s) = side else { return BoneRole::Other };
    if n.contains("toe") || n.contains("foot") { BoneRole::Foot(s) }
    else if n.contains("upleg") || n.contains("thigh") || n.contains("leg") || n.contains("shin") || n.contains("calf") { BoneRole::StanceLeg(s) }
    else { BoneRole::Other }
}

// ---- Rig measurements, taken once at load and frozen.
/// Foot box in the foot bone's local frame, from that rig's foot mesh bounds (min/max, x and z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootBox { pub min: [f32; 2], pub max: [f32; 2], pub sole_y: f32 }

/// Union of local-space AABBs (x,z) of all mesh parts skinned to one foot (foot + toe bones).
pub fn measure_foot_box(aabbs: &[([f32; 3], [f32; 3])]) -> Option<FootBox> {
    let mut it = aabbs.iter();
    let &(mn, mx) = it.next()?;
    let mut b = FootBox { min: [mn[0], mn[2]], max: [mx[0], mx[2]], sole_y: mn[1] };
    for &(mn, mx) in it {
        b.min = [b.min[0].min(mn[0]), b.min[1].min(mn[2])];
        b.max = [b.max[0].max(mx[0]), b.max[1].max(mx[2])];
        b.sole_y = b.sole_y.min(mn[1]);
    }
    Some(b)
}

/// The four ground-plane corners of a foot box placed by the foot's world position and yaw.
pub fn foot_corners(b: &FootBox, pos: pb::V3, yaw: f32) -> [[f32; 2]; 4] {
    let (s, c) = yaw.sin_cos();
    let local = [[b.min[0], b.min[1]], [b.max[0], b.min[1]], [b.max[0], b.max[1]], [b.min[0], b.max[1]]];
    local.map(|[x, z]| [pos[0] + c * x + s * z, pos[2] - s * x + c * z])
}

/// Frozen per-rig numbers fed to balance_torque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigBalance { pub mass: f32, pub h: f32, pub feet: [FootBox; 2] }

/// Height of the whole-body COM above the mean ankle height, in the load-time T-pose.
pub fn measure_h(bones: &[pb::BoneMass], ankles: [pb::V3; 2]) -> (f32, f32) {
    let (m, com) = pb::body_com(bones);
    (m, (com[1] - 0.5 * (ankles[0][1] + ankles[1][1])).max(0.05))
}

/// Split a whole-body torque evenly over stance motors on planted sides, each clamped to its own cap.
/// Any leftover from a capped motor is passed to the remaining uncapped stance motors (up to their
/// own caps), repeating until all is placed or every motor is capped. Deterministic: bone order.
/// Input (bone, side, limit) -> output (bone, tau, limit), same order.
pub fn split_torque(tau: pb::V3, stance: &[(u16, Side, f32)], planted: [bool; 2]) -> Vec<(u16, pb::V3, f32)> {
    let mag = |t: pb::V3| (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    let total = mag(tau);
    let mut out: Vec<(u16, pb::V3, f32)> = stance.iter().map(|&(b, _, l)| (b, [0.0; 3], l)).collect();
    if total <= 0.0 { return out; }
    let dir = tau.map(|v| v / total);
    let mut given = vec![0.0f32; stance.len()];
    let mut open: Vec<usize> = (0..stance.len()).filter(|&i| planted[stance[i].1 as usize]).collect();
    let mut left = total;
    while left > 1e-6 && !open.is_empty() {
        let share = left / open.len() as f32;
        let mut still = Vec::with_capacity(open.len());
        for &i in &open {
            let room = stance[i].2 - given[i];
            let add = share.min(room.max(0.0));
            given[i] += add;
            left -= add;
            if given[i] < stance[i].2 - 1e-6 { still.push(i); }
        }
        if still.len() == open.len() { break; } // nobody capped this round: all placed
        open = still;
    }
    for (i, o) in out.iter_mut().enumerate() { o.1 = dir.map(|v| v * given[i]); }
    out
}

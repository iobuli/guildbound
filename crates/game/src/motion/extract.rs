//! Extreme extraction: reduce a dense `Clip` to the few poses that carry the action.
//! A frame is a candidate extreme when the weighted angular speed of the major bones is a local
//! minimum (motion reverses / holds), or at a foot/hand contact flip (lowest foot switches,
//! fastest hand peaks then stops). First and last frames are always keys. Candidates closer
//! than ~0.08 s merge. Loops get at least 4 keys (both foot contacts + both passing poses).
use super::retarget::{Clip, frame_time};
use super::{Bones, Pose, SAMPLE_HZ, Skeleton};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Start,
    Anticipation,
    /// Foot plant (locomotion) or strike contact.
    Contact,
    Passing,
    Apex,
    /// Fastest limb peak right before stop (attacks).
    Impact,
    Recoil,
    Hold,
    End,
}

#[derive(Clone, Debug)]
pub struct Key {
    /// Seconds in the source clip.
    pub time: f32,
    pub pose: Pose,
    pub tag: Tag,
}

/// The reduced, stylizable representation of a clip.
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    /// Sorted by time; never empty.
    pub keys: Vec<Key>,
    /// For locomotion loops: normalized phase (0..1) where the LEFT foot plants.
    pub left_contact: Option<f32>,
    /// For locomotion loops: ground distance covered by one full cycle (m), estimated from the
    /// planted foot's sweep relative to the pelvis (in-place clips) or root travel. 0 otherwise.
    pub stride: f32,
    /// The dense clip, kept for raw sampling, root travel and debugging.
    pub dense: Clip,
}

/// Candidates closer than this merge (seconds).
const MERGE_TIME: f32 = 0.08;
/// Interpolation error (weighted rad) above which a breakdown key is inserted.
const MAX_ERROR: f32 = 0.25;
/// Effector speed (m/s) that counts as a strike.
const STRIKE_SPEED: f32 = 3.0;
/// Strike ends once the effector slows below this share of its peak speed.
const STRIKE_STOP: f32 = 0.35;
/// Pelvis rise (m) over both ends that makes a peak an `Apex`.
const APEX_RISE: f32 = 0.1;
/// Root travel per cycle (m) above which stride comes from travel instead of the feet.
const TRAVEL_STRIDE: f32 = 0.2;

/// Major bones with (speed weight, error importance).
fn major_bones(b: &Bones) -> Vec<(usize, f32, f32)> {
    let mut v = vec![
        (b.pelvis, 2.0, 1.0),
        (b.spine[0], 1.0, 0.8),
        (b.spine[1], 1.0, 0.8),
        (b.spine[2], 1.0, 0.8),
        (b.neck, 0.5, 0.5),
        (b.head, 0.5, 0.5),
    ];
    for s in 0..2 {
        v.extend([
            (b.clavicle[s], 0.4, 0.5),
            (b.upperarm[s], 1.0, 1.0),
            (b.lowerarm[s], 0.8, 0.8),
            (b.hand[s], 0.4, 0.5),
            (b.thigh[s], 1.2, 1.0),
            (b.calf[s], 1.0, 0.8),
            (b.foot[s], 0.5, 0.5),
        ]);
    }
    v
}

/// Where a key came from; decides who wins a merge and the final tag.
#[derive(Clone, Copy, Debug)]
struct Pick {
    frame: usize,
    tag: Tag,
    priority: u8,
}

/// Per-clip analysis shared by the selection stages.
struct Analysis<'a> {
    clip: &'a Clip,
    n: usize,
    times: Vec<f32>,
    /// Smoothed weighted angular speed per frame (rad/s).
    speed: Vec<f32>,
    /// Model-space joint transforms per frame.
    fk: Vec<Vec<Transform>>,
    major: Vec<(usize, f32, f32)>,
}

impl<'a> Analysis<'a> {
    fn new(clip: &'a Clip, skeleton: &Skeleton, bones: &Bones) -> Self {
        let n = clip.frames.len();
        let times: Vec<f32> = (0..n).map(|i| frame_time(i, clip.duration)).collect();
        let fk: Vec<Vec<Transform>> = clip
            .frames
            .iter()
            .map(|p| skeleton.fk(p, bones.pelvis))
            .collect();
        let major = major_bones(bones);
        let weight_sum: f32 = major.iter().map(|m| m.1).sum::<f32>() + 3.0;
        // Segment speeds i → i+1.
        let segment: Vec<f32> = (0..n.saturating_sub(1))
            .map(|i| {
                let dt = (times[i + 1] - times[i]).max(1e-4);
                let (a, b) = (&clip.frames[i], &clip.frames[i + 1]);
                let angular: f32 = major
                    .iter()
                    .map(|&(j, w, _)| w * a.rot[j].angle_between(b.rot[j]))
                    .sum();
                let linear = 3.0 * fk[i][bones.pelvis]
                    .translation
                    .distance(fk[i + 1][bones.pelvis].translation);
                (angular + linear) / dt / weight_sum
            })
            .collect();
        let raw: Vec<f32> = (0..n)
            .map(|i| match (i.checked_sub(1).and_then(|p| segment.get(p)), segment.get(i)) {
                (Some(a), Some(b)) => 0.5 * (a + b),
                (Some(a), None) => *a,
                (None, Some(b)) if clip.looping && n > 2 => 0.5 * (b + segment[n - 2]),
                (None, Some(b)) => *b,
                (None, None) => 0.0,
            })
            .collect();
        let speed = (0..n)
            .map(|i| {
                let prev = if i > 0 { raw[i - 1] } else { raw[i] };
                let next = if i + 1 < n { raw[i + 1] } else { raw[i] };
                let s = 0.25 * prev + 0.5 * raw[i] + 0.25 * next;
                if s.is_finite() { s } else { 0.0 }
            })
            .collect();
        Self {
            clip,
            n,
            times,
            speed,
            fk,
            major,
        }
    }

    fn pos(&self, frame: usize, bone: usize) -> Vec3 {
        self.fk[frame][bone].translation
    }

    /// Central-difference speed (m/s) of a bone in model space.
    fn bone_speed(&self, bone: usize) -> Vec<f32> {
        (0..self.n)
            .map(|i| {
                let a = i.saturating_sub(1);
                let b = (i + 1).min(self.n - 1);
                let dt = (self.times[b] - self.times[a]).max(1e-4);
                self.pos(b, bone).distance(self.pos(a, bone)) / dt
            })
            .collect()
    }

    /// Weighted interpolation error of frame `i` against the slerp between frames `a` and `b`.
    fn error(&self, a: usize, b: usize, i: usize) -> f32 {
        let span = (self.times[b] - self.times[a]).max(1e-6);
        let u = ((self.times[i] - self.times[a]) / span).clamp(0.0, 1.0);
        let (pa, pb, pi) = (
            &self.clip.frames[a],
            &self.clip.frames[b],
            &self.clip.frames[i],
        );
        let angular = self
            .major
            .iter()
            .map(|&(j, _, imp)| imp * pa.rot[j].slerp(pb.rot[j], u).angle_between(pi.rot[j]))
            .fold(0.0, f32::max);
        let linear = 2.0 * pa.pelvis.lerp(pb.pelvis, u).distance(pi.pelvis);
        angular.max(linear)
    }

    /// Worst interior frame of segment `a..b` and its error.
    fn worst(&self, a: usize, b: usize) -> Option<(usize, f32)> {
        (a + 1..b)
            .map(|i| (i, self.error(a, b, i)))
            .max_by(|x, y| x.1.total_cmp(&y.1))
    }

    /// Prominent local minima of the angular speed: (frame, prominence).
    fn speed_minima(&self) -> Vec<(usize, f32)> {
        let (lo, hi) = self
            .speed
            .iter()
            .fold((f32::MAX, 0.0f32), |(lo, hi), &s| (lo.min(s), hi.max(s)));
        let range = (hi - lo).max(0.0);
        let window = (0.3 * SAMPLE_HZ).round() as usize;
        let mut out = Vec::new();
        for i in 1..self.n.saturating_sub(1) {
            let s = self.speed[i];
            if !(s < self.speed[i - 1] && s <= self.speed[i + 1]) {
                continue;
            }
            let left = self.speed[i.saturating_sub(window)..=i]
                .iter()
                .fold(0.0f32, |m, &x| m.max(x));
            let right = self.speed[i..(i + window + 1).min(self.n)]
                .iter()
                .fold(0.0f32, |m, &x| m.max(x));
            let prominence = left.min(right) - s;
            if prominence > 0.2 * range && prominence > 0.05 {
                out.push((i, prominence));
            }
        }
        out
    }
}

/// Foot gait analysis of a loop.
struct Gait {
    /// Touch-down frame per foot [left, right].
    contact: [usize; 2],
    /// Normalized phase of the left touch-down.
    left_phase: f32,
    stride: f32,
}

/// Cyclic runs of `true` in `mask` (len m): (start, length), longest first.
fn longest_run(mask: &[bool]) -> Option<(usize, usize)> {
    let m = mask.len();
    if m == 0 || mask.iter().all(|&x| !x) {
        return None;
    }
    if mask.iter().all(|&x| x) {
        return Some((0, m));
    }
    let mut best: Option<(usize, usize)> = None;
    for start in 0..m {
        // A run starts where the previous frame (cyclically) is not in the mask.
        if !mask[start] || mask[(start + m - 1) % m] {
            continue;
        }
        let mut len = 0;
        while len < m && mask[(start + len) % m] {
            len += 1;
        }
        if best.is_none_or(|(_, l)| len > l) {
            best = Some((start, len));
        }
    }
    best
}

fn analyze_gait(a: &Analysis, bones: &Bones) -> Option<Gait> {
    if !a.clip.looping || a.n < 6 {
        return None;
    }
    // Cycle frames: the last frame duplicates frame 0.
    let m = a.n - 1;
    let d = a.clip.duration;
    let unwrapped = |k: usize| {
        if k < m {
            a.times[k]
        } else {
            a.times[k - m] + d
        }
    };
    let height = |i: usize, s: usize| {
        a.pos(i, bones.foot[s])
            .y
            .min(a.pos(i, bones.ball[s]).y)
    };
    let mut contact = [0usize; 2];
    let mut runs = [(0usize, 0usize); 2];
    for s in 0..2 {
        let h: Vec<f32> = (0..m).map(|i| height(i, s)).collect();
        let lo = h.iter().copied().fold(f32::MAX, f32::min);
        let hi = h.iter().copied().fold(f32::MIN, f32::max);
        if hi - lo < 0.04 {
            return None;
        }
        let threshold = lo + (0.25 * (hi - lo)).max(0.02);
        let mask: Vec<bool> = h.iter().map(|&y| y < threshold).collect();
        let (start, len) = longest_run(&mask)?;
        if len < 2 || len >= m {
            return None;
        }
        contact[s] = start;
        runs[s] = (start, len);
    }
    let phase = |f: usize| a.times[f] / d;
    let separation = (phase(contact[1]) - phase(contact[0])).rem_euclid(1.0);
    if !(0.25..=0.75).contains(&separation) {
        return None;
    }

    let cycle = a.clip.travel.last().copied().unwrap_or_default()
        - a.clip.travel.first().copied().unwrap_or_default();
    let travel = Vec2::new(cycle.x, cycle.z).length();
    let stride = if travel > TRAVEL_STRIDE {
        travel
    } else {
        // Planted foot slides under the in-place pelvis at ground speed.
        let mut sum = 0.0;
        let mut count = 0;
        for s in 0..2 {
            let (start, len) = runs[s];
            let trim = len / 5;
            let (k0, k1) = (start + trim, start + len - 1 - trim);
            if k1 <= k0 {
                continue;
            }
            let rel = |k: usize| {
                let i = k % m;
                let v = a.pos(i, bones.foot[s]) - a.pos(i, bones.pelvis);
                Vec2::new(v.x, v.z)
            };
            let dt = unwrapped(k1) - unwrapped(k0);
            if dt > 1e-3 {
                sum += (rel(k1) - rel(k0)).length() / dt * d;
                count += 1;
            }
        }
        if count > 0 {
            sum / count as f32
        } else {
            // Fallback: twice the planted foot's peak-to-peak sweep along the dominant axis.
            let mut sweep = 0.0;
            for s in 0..2 {
                let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
                for i in 0..m {
                    let v = a.pos(i, bones.foot[s]) - a.pos(i, bones.pelvis);
                    lo = lo.min(Vec2::new(v.x, v.z));
                    hi = hi.max(Vec2::new(v.x, v.z));
                }
                sweep += (hi - lo).max_element();
            }
            sweep
        }
    };
    let ground_speed = stride / d.max(1e-3);
    let stride = if stride.is_finite() && (0.2..=15.0).contains(&ground_speed) {
        stride
    } else {
        0.0
    };
    Some(Gait {
        contact,
        left_phase: phase(contact[0]).rem_euclid(1.0),
        stride,
    })
}

/// Fastest effector stroke of a one-shot: (anticipation frame, peak frame, impact frame).
fn strike(a: &Analysis, bones: &Bones) -> Option<(usize, usize, usize)> {
    let effectors = [bones.hand[0], bones.hand[1], bones.foot[0], bones.foot[1]];
    let mut best: Option<(Vec<f32>, usize, f32)> = None;
    for &e in &effectors {
        let v = a.bone_speed(e);
        let (i, peak) = v
            .iter()
            .copied()
            .enumerate()
            .skip(1)
            .take(a.n.saturating_sub(2))
            .max_by(|x, y| x.1.total_cmp(&y.1))?;
        if best.as_ref().is_none_or(|b| peak > b.2) {
            best = Some((v, i, peak));
        }
    }
    let (v, peak_frame, peak) = best?;
    if !(peak > STRIKE_SPEED) {
        return None;
    }
    let impact = (peak_frame + 1..a.n)
        .find(|&i| v[i] < STRIKE_STOP * peak)
        .unwrap_or(a.n - 1);
    let mut wind = peak_frame;
    while wind > 0 && v[wind - 1] <= v[wind] + 1e-3 {
        wind -= 1;
    }
    Some((wind, peak_frame, impact))
}

pub fn extract(clip: &Clip, skeleton: &Skeleton, bones: &Bones) -> Track {
    let n = clip.frames.len();
    let mut track = Track {
        name: clip.track.clone(),
        duration: clip.duration,
        looping: clip.looping,
        keys: Vec::new(),
        left_contact: None,
        stride: 0.0,
        dense: clip.clone(),
    };
    if n == 0 {
        track.keys.push(Key {
            time: 0.0,
            pose: skeleton.rest_pose(),
            tag: Tag::Start,
        });
        return track;
    }
    if n < 3 || !(clip.duration > 0.0) {
        track.keys.push(Key {
            time: 0.0,
            pose: clip.frames[0].clone(),
            tag: Tag::Start,
        });
        if n > 1 {
            track.keys.push(Key {
                time: frame_time(n - 1, clip.duration),
                pose: clip.frames[n - 1].clone(),
                tag: Tag::End,
            });
        }
        return track;
    }

    let a = Analysis::new(clip, skeleton, bones);
    let merge = ((MERGE_TIME * SAMPLE_HZ).round() as usize).max(1);
    // Loop endpoints yield their tag to gait keys (the cycle seam is arbitrary).
    let end_priority = if clip.looping { 4 } else { 10 };
    let mut picks = vec![
        Pick {
            frame: 0,
            tag: Tag::Start,
            priority: end_priority,
        },
        Pick {
            frame: n - 1,
            tag: Tag::End,
            priority: end_priority,
        },
    ];

    let gait = analyze_gait(&a, bones);
    if let Some(g) = &gait {
        track.left_contact = Some(g.left_phase);
        track.stride = g.stride;
        let m = n - 1;
        let (l, r) = (g.contact[0], g.contact[1]);
        let mid = |from: usize, to: usize| (from + ((to + m - from) % m) / 2) % m;
        for (frame, tag, priority) in [
            (l, Tag::Contact, 9),
            (r, Tag::Contact, 9),
            (mid(l, r), Tag::Passing, 6),
            (mid(r, l), Tag::Passing, 6),
        ] {
            picks.push(Pick {
                frame,
                tag,
                priority,
            });
        }
    }

    let mut impact_frame = None;
    if !clip.looping
        && let Some((wind, _peak, impact)) = strike(&a, bones)
    {
        impact_frame = Some(impact);
        picks.push(Pick {
            frame: impact,
            tag: Tag::Impact,
            priority: 8,
        });
        if wind > 0 {
            picks.push(Pick {
                frame: wind,
                tag: Tag::Anticipation,
                priority: 7,
            });
        }
    }

    // Apex: a pelvis peak clearly above both ends (jumps).
    let heights: Vec<f32> = (0..n).map(|i| a.pos(i, bones.pelvis).y).collect();
    if let Some((top, &h)) = heights
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.total_cmp(y.1))
        && top > 0
        && top < n - 1
        && h > heights[0].max(heights[n - 1]) + APEX_RISE
    {
        picks.push(Pick {
            frame: top,
            tag: Tag::Apex,
            priority: 6,
        });
    }

    let mut keys = merge_picks(picks, merge, n);
    let mandatory = keys.len();
    let cap = mandatory.max(((clip.duration * 2.5).round() as usize + 2).clamp(3, 16));

    // Breakdowns where slerp between neighbouring keys misses the motion, snapped to a nearby
    // speed minimum when there is one.
    let minima = a.speed_minima();
    while keys.len() < cap {
        let worst = keys
            .windows(2)
            .filter_map(|w| a.worst(w[0].frame, w[1].frame))
            .filter(|&(i, _)| keys.iter().all(|k| k.frame.abs_diff(i) >= merge))
            .max_by(|x, y| x.1.total_cmp(&y.1));
        let Some((frame, err)) = worst else { break };
        if err < MAX_ERROR {
            break;
        }
        let snapped = minima
            .iter()
            .filter(|(m, _)| m.abs_diff(frame) <= 3)
            .filter(|(m, _)| keys.iter().all(|k| k.frame.abs_diff(*m) >= merge))
            .min_by_key(|(m, _)| m.abs_diff(frame))
            .map_or(frame, |(m, _)| *m);
        insert_pick(&mut keys, snapped, Tag::Passing, 2);
    }
    // Natural holds / reversals the motion pauses on.
    let (lo, hi) = a
        .speed
        .iter()
        .fold((f32::MAX, 0.0f32), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    let mut ranked = minima.clone();
    ranked.sort_by(|x, y| y.1.total_cmp(&x.1));
    for (frame, prominence) in ranked {
        if keys.len() >= cap {
            break;
        }
        if prominence > 0.35 * (hi - lo) && keys.iter().all(|k| k.frame.abs_diff(frame) >= merge) {
            insert_pick(&mut keys, frame, Tag::Hold, 3);
        }
    }
    // Loops keep at least four keys: split the widest gaps.
    while clip.looping && keys.len() < 4 {
        let Some(w) = keys
            .windows(2)
            .filter(|w| w[1].frame - w[0].frame >= 2)
            .max_by_key(|w| w[1].frame - w[0].frame)
        else {
            break;
        };
        let frame = (w[0].frame + w[1].frame) / 2;
        insert_pick(&mut keys, frame, Tag::Passing, 1);
    }

    retag(&mut keys, &a, impact_frame, gait.is_some(), lo, hi);
    track.keys = keys
        .iter()
        .map(|k| Key {
            time: a.times[k.frame],
            pose: clip.frames[k.frame].clone(),
            tag: k.tag,
        })
        .collect();
    track
}

/// Sort and merge picks closer than `merge` frames. Endpoints keep their frame (and take the
/// tag of a stronger neighbour); otherwise the stronger pick survives.
fn merge_picks(mut picks: Vec<Pick>, merge: usize, n: usize) -> Vec<Pick> {
    picks.retain(|p| p.frame < n);
    picks.sort_by_key(|p| (p.frame, std::cmp::Reverse(p.priority)));
    let endpoint = |f: usize| f == 0 || f == n - 1;
    let mut out: Vec<Pick> = Vec::with_capacity(picks.len());
    for p in picks {
        match out.last_mut() {
            Some(last) if p.frame - last.frame < merge => {
                let stronger = p.priority > last.priority;
                if p.frame != last.frame && endpoint(last.frame) && endpoint(p.frame) {
                    // Very short clip: keep both ends.
                    out.push(p);
                } else if endpoint(last.frame) {
                    if stronger {
                        last.tag = p.tag;
                        last.priority = p.priority;
                    }
                } else if endpoint(p.frame) {
                    let (tag, priority) = if stronger {
                        (p.tag, p.priority)
                    } else {
                        (last.tag, last.priority)
                    };
                    *last = Pick {
                        frame: p.frame,
                        tag,
                        priority,
                    };
                } else if stronger {
                    *last = p;
                }
            }
            _ => out.push(p),
        }
    }
    out
}

fn insert_pick(keys: &mut Vec<Pick>, frame: usize, tag: Tag, priority: u8) {
    let at = keys.partition_point(|k| k.frame < frame);
    if keys.get(at).is_some_and(|k| k.frame == frame) {
        return;
    }
    keys.insert(
        at,
        Pick {
            frame,
            tag,
            priority,
        },
    );
}

/// Give generic keys meaning from their context: recoil/hold after a strike, holds where the
/// motion is nearly still, the last breakdown before a strike becomes its anticipation.
fn retag(keys: &mut [Pick], a: &Analysis, impact: Option<usize>, gait: bool, lo: f32, hi: f32) {
    let still = lo + 0.15 * (hi - lo);
    let generic = |p: &Pick| p.priority <= 3;
    if let Some(impact) = impact {
        let mut after_impact = 0;
        for k in keys.iter_mut().filter(|k| k.frame > impact) {
            if generic(k) {
                k.tag = if after_impact == 0 {
                    Tag::Recoil
                } else {
                    Tag::Hold
                };
            }
            after_impact += 1;
        }
        let has_anticipation = keys.iter().any(|k| k.tag == Tag::Anticipation);
        if !has_anticipation
            && let Some(k) = keys
                .iter_mut()
                .filter(|k| k.frame < impact && k.frame > 0 && generic(k))
                .last()
        {
            k.tag = Tag::Anticipation;
        }
    }
    for k in keys.iter_mut().filter(|k| generic(k)) {
        if impact.is_some_and(|i| k.frame > i) || k.tag == Tag::Anticipation {
            continue;
        }
        k.tag = if a.speed[k.frame] <= still && !gait {
            Tag::Hold
        } else {
            Tag::Passing
        };
    }
}

#[cfg(test)]
mod tests {
    use super::super::donor::tests::lib;
    use super::super::retarget::{self, tests::canonical};
    use super::*;
    use std::sync::OnceLock;

    fn tracks() -> &'static Vec<Track> {
        static T: OnceLock<Vec<Track>> = OnceLock::new();
        T.get_or_init(|| {
            let (s, b) = canonical();
            ["ual1", "ual2", "sekiro"]
                .iter()
                .flat_map(|l| retarget::retarget(lib(l), s, b))
                .map(|c| extract(&c, s, b))
                .collect()
        })
    }

    fn track(name: &str) -> &'static Track {
        tracks().iter().find(|t| t.name == name).expect(name)
    }

    #[test]
    fn walk_loop_has_contacts_and_stride() {
        let t = track("ual1/Walk_Loop");
        assert!(t.looping);
        assert!(t.keys.len() >= 4, "{} keys", t.keys.len());
        let contacts = t.keys.iter().filter(|k| k.tag == Tag::Contact).count();
        assert!(contacts >= 2, "contacts {contacts}: {:?}", tags(t));
        assert!((0.8..2.5).contains(&t.stride), "stride {}", t.stride);
        let phase = t.left_contact.expect("left contact");
        assert!((0.0..1.0).contains(&phase));
    }

    #[test]
    fn keys_are_well_formed() {
        for t in tracks() {
            assert!(!t.keys.is_empty(), "{}", t.name);
            assert_eq!(t.keys[0].time, 0.0, "{}", t.name);
            assert!((t.keys.last().unwrap().time - t.duration).abs() < 1e-4, "{}", t.name);
            for w in t.keys.windows(2) {
                assert!(w[1].time > w[0].time, "{} keys sorted: {:?}", t.name, tags(t));
            }
            if t.looping {
                assert!(t.keys.len() >= 4, "{} loop keys {}", t.name, t.keys.len());
            }
            assert!(t.keys.len() <= 18, "{} has {} keys", t.name, t.keys.len());
            for k in &t.keys {
                assert!(k.pose.rot.iter().all(|q| q.is_finite()));
                assert!(k.pose.pelvis.is_finite());
            }
        }
    }

    #[test]
    fn strikes_get_impact_keys() {
        for name in ["ual1/Punch_Jab", "ual1/Punch_Cross", "ual2/Sword_Regular_A"] {
            let t = track(name);
            assert!(
                t.keys.iter().any(|k| k.tag == Tag::Impact),
                "{name}: {:?}",
                tags(t)
            );
        }
    }

    fn tags(t: &Track) -> Vec<(f32, Tag)> {
        t.keys.iter().map(|k| ((k.time * 100.0).round() / 100.0, k.tag)).collect()
    }

    /// `cargo test -p guildbound extract::tests::report -- --nocapture` prints key counts,
    /// strides and tags for a sanity check of the locomotion loops.
    #[test]
    fn report() {
        println!("{:<34} {:>5} {:>5} {:>6} {:>6}  keys", "track", "loop", "keys", "stride", "lphase");
        for t in tracks() {
            if !(t.looping || t.left_contact.is_some()) {
                continue;
            }
            println!(
                "{:<34} {:>5} {:>5} {:>6.2} {:>6}  {:?}",
                t.name,
                t.looping,
                t.keys.len(),
                t.stride,
                t.left_contact.map_or("-".into(), |p| format!("{p:.2}")),
                tags(t)
            );
        }
        let total: usize = tracks().iter().map(|t| t.keys.len()).sum();
        println!("{} tracks, {} keys", tracks().len(), total);
        for t in tracks().iter().filter(|t| !t.looping) {
            println!("{:<34} {:>5.2}s {:?}", t.name, t.duration, tags(t));
        }
    }
}

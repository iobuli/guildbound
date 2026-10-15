//! Physics: rig loader. Reads `phys_rig.json` (segment mass shares, per-rig totals, rest geometry and the
//! foot box) and builds the `api::RigProfile` the motor writer uses. Pure, no deps: a small JSON reader
//! sized for this file. Rest rotations are identity, so each joint's `offset` is its anchor minus the
//! parent's anchor and `com` is the segment centre minus its own anchor.
//! Geometry source: gb_lab/src/ragdoll.rs stand rig (bal/ro_5_0.3_12); masses: ragdoll.rs SEGMENTS shares.

use crate::api::{self, RigJoint, RigProfile};
use crate::math::{Mat3, Vec3};
use std::collections::BTreeMap;

/// The bundled rig file.
pub const RIG_JSON: &str = include_str!("phys_rig.json");

/// Fine-grained joint role. Side 0 = left, 1 = right. `is_arm` selects the strike Kp ×1.5 joints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointRole {
    Pelvis,
    Spine,
    Head,
    Shoulder(u8),
    Elbow(u8),
    Hip(u8),
    Knee(u8),
    Ankle(u8),
}

impl JointRole {
    pub fn is_arm(self) -> bool {
        matches!(self, JointRole::Shoulder(_) | JointRole::Elbow(_))
    }
    pub fn is_leg(self) -> bool {
        matches!(
            self,
            JointRole::Hip(_) | JointRole::Knee(_) | JointRole::Ankle(_)
        )
    }
    pub fn is_spine(self) -> bool {
        matches!(self, JointRole::Pelvis | JointRole::Spine | JointRole::Head)
    }
    pub fn side(self) -> Option<u8> {
        match self {
            JointRole::Shoulder(s)
            | JointRole::Elbow(s)
            | JointRole::Hip(s)
            | JointRole::Knee(s)
            | JointRole::Ankle(s) => Some(s),
            _ => None,
        }
    }
    /// Map to the motor's coarser `api::JointRole` (arms, spine and head become `Other`).
    pub fn to_api(self) -> api::JointRole {
        match self {
            JointRole::Pelvis => api::JointRole::Pelvis,
            JointRole::Hip(s) => api::JointRole::Hip(s),
            JointRole::Knee(s) => api::JointRole::Knee(s),
            JointRole::Ankle(s) => api::JointRole::Ankle(s),
            JointRole::Shoulder(s) | JointRole::Elbow(s) => api::JointRole::Arm(s),
            _ => api::JointRole::Other,
        }
    }
    fn parse(s: &str) -> Option<JointRole> {
        let side = |t: &str| match t {
            "l" => Some(0u8),
            "r" => Some(1u8),
            _ => None,
        };
        Some(match s.split_once('_') {
            None => match s {
                "pelvis" => JointRole::Pelvis,
                "spine" => JointRole::Spine,
                "head" => JointRole::Head,
                _ => return None,
            },
            Some((k, t)) => {
                let sd = side(t)?;
                match k {
                    "shoulder" => JointRole::Shoulder(sd),
                    "elbow" => JointRole::Elbow(sd),
                    "hip" => JointRole::Hip(sd),
                    "knee" => JointRole::Knee(sd),
                    "ankle" => JointRole::Ankle(sd),
                    _ => return None,
                }
            }
        })
    }
}

/// Foot box half extents (m), foot-local: half_length along forward (z), half_width across (x).
/// `half_length` is the sole edge d_edge for the ankle cap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootBox {
    pub half_length: f64,
    pub half_width: f64,
    pub half_height: f64,
}

/// Loaded rig: the motor's profile, the fine roles (same index order) and the foot box.
#[derive(Clone, Debug)]
pub struct LoadedRig {
    pub profile: RigProfile,
    pub roles: Vec<JointRole>,
    pub foot: FootBox,
    pub total_mass: f64,
}

impl LoadedRig {
    /// Sole edge (half_length, half_width), m.
    pub fn sole_edge(&self) -> (f64, f64) {
        (self.foot.half_length, self.foot.half_width)
    }
    /// Kp multiplier per joint for a strike boost b in [0,1]: 1 + (scale − 1)·b on arm joints, 1 elsewhere.
    pub fn kp_scale(&self, joint: usize, strike_boost: f64, scale: f64) -> f64 {
        if self.roles.get(joint).is_some_and(|r| r.is_arm()) {
            1.0 + (scale - 1.0) * strike_boost.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// Load `rig` ("master", "mannequin", "hero") from rig JSON text. Masses are the shares scaled to the
/// rig's total; every joint's torque_cap starts at `fault_lever_m`·M·g (spawn raises thighs to ≥ M·g·0.12).
pub fn load_rig(json: &str, rig: &str, fault_lever_m: f64) -> Result<LoadedRig, String> {
    let root = Json::parse(json)?;
    let total = root
        .get("rigs")
        .and_then(|r| r.get(rig))
        .and_then(Json::num)
        .ok_or(format!("rig '{rig}' not in rigs"))?;
    let fb = root.get("foot_box").ok_or("missing foot_box")?;
    let fnum = |k: &str| fb.get(k).and_then(Json::num).ok_or(format!("foot_box.{k}"));
    let foot = FootBox {
        half_length: fnum("half_length")?,
        half_width: fnum("half_width")?,
        half_height: fnum("half_height")?,
    };
    let rows = root
        .get("joints")
        .and_then(Json::arr)
        .ok_or("missing joints")?;
    let share_sum: f64 = rows
        .iter()
        .filter_map(|r| r.get("share").and_then(Json::num))
        .sum();
    if share_sum <= 0.0 {
        return Err("shares sum to 0".into());
    }
    let cap = fault_lever_m * total * api::G;
    let (mut joints, mut roles, mut anchors) = (Vec::new(), Vec::new(), Vec::<Vec3>::new());
    for (i, r) in rows.iter().enumerate() {
        let name = r
            .get("name")
            .and_then(Json::str)
            .ok_or(format!("joint {i}: name"))?
            .to_string();
        let parent = match r.get("parent") {
            Some(Json::Null) | None => None,
            Some(p) => Some(p.num().ok_or(format!("{name}: parent"))? as u16),
        };
        if parent.is_some_and(|p| p as usize >= i) {
            return Err(format!("{name}: parent must come first"));
        }
        if (i == 0) != parent.is_none() {
            return Err(format!("{name}: only joint 0 is the root"));
        }
        let role = r
            .get("role")
            .and_then(Json::str)
            .and_then(JointRole::parse)
            .ok_or(format!("{name}: role"))?;
        let v3 = |k: &str| -> Result<Vec3, String> {
            let a = r
                .get(k)
                .and_then(Json::arr)
                .filter(|a| a.len() == 3)
                .ok_or(format!("{name}: {k}"))?;
            let n: Vec<f64> = a.iter().filter_map(Json::num).collect();
            if n.len() != 3 {
                return Err(format!("{name}: {k}"));
            }
            Ok(Vec3::new(n[0], n[1], n[2]))
        };
        let (anchor, com_w) = (v3("anchor")?, v3("com")?);
        let mass = r
            .get("share")
            .and_then(Json::num)
            .ok_or(format!("{name}: share"))?
            * total
            / share_sum;
        let shape = r
            .get("shape")
            .and_then(Json::arr)
            .ok_or(format!("{name}: shape"))?;
        let inertia = shape_inertia(shape, mass).ok_or(format!("{name}: bad shape"))?;
        let offset = match parent {
            Some(p) => anchor - anchors[p as usize],
            None => Vec3::ZERO,
        };
        joints.push(RigJoint {
            name,
            parent,
            offset,
            mass,
            com: com_w - anchor,
            inertia,
            role: role.to_api(),
            torque_cap: cap,
        });
        roles.push(role);
        anchors.push(anchor);
    }
    Ok(LoadedRig {
        profile: RigProfile {
            joints,
            sole: api::SoleBox {
                half_length: foot.half_length,
                half_width: foot.half_width,
                half_height: foot.half_height,
            },
        },
        roles,
        foot,
        total_mass: total,
    })
}

/// Bundled rig file, by rig name.
pub fn default_rig(rig: &str) -> Result<LoadedRig, String> {
    load_rig(RIG_JSON, rig, 3.0)
}

/// Principal inertia about the segment COM. box [ex,ey,ez] full extents; ball r; capsule_{x,y} r, l
/// (cylinder part l plus two hemispheres approximated as a solid cylinder of length l + 2r [team approx]).
fn shape_inertia(s: &[Json], m: f64) -> Option<Mat3> {
    let k = s.first()?.str()?;
    let n: Vec<f64> = s[1..].iter().filter_map(Json::num).collect();
    Some(match (k, n.as_slice()) {
        ("box", [x, y, z]) => Mat3::diag(
            m * (y * y + z * z) / 12.0,
            m * (x * x + z * z) / 12.0,
            m * (x * x + y * y) / 12.0,
        ),
        ("ball", [r]) => {
            let i = 0.4 * m * r * r;
            Mat3::diag(i, i, i)
        }
        ("capsule_x" | "capsule_y", [r, l]) => {
            let h = l + 2.0 * r;
            let (ax, cr) = (0.5 * m * r * r, m * (3.0 * r * r + h * h) / 12.0);
            if k == "capsule_x" {
                Mat3::diag(ax, cr, cr)
            } else {
                Mat3::diag(cr, ax, cr)
            }
        }
        _ => return None,
    })
}

/// Minimal JSON value for the rig file (objects keep sorted keys; numbers are f64).
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn get(&self, k: &str) -> Option<&Json> {
        if let Json::Obj(m) = self {
            m.get(k)
        } else {
            None
        }
    }
    fn num(&self) -> Option<f64> {
        if let Json::Num(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    fn str(&self) -> Option<&str> {
        if let Json::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn arr(&self) -> Option<&[Json]> {
        if let Json::Arr(a) = self {
            Some(a)
        } else {
            None
        }
    }
    fn parse(s: &str) -> Result<Json, String> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = Self::val(b, &mut i)?;
        Self::ws(b, &mut i);
        if i != b.len() {
            return Err(format!("trailing data at {i}"));
        }
        Ok(v)
    }
    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    }
    fn val(b: &[u8], i: &mut usize) -> Result<Json, String> {
        Self::ws(b, i);
        let lit = |i: &mut usize, w: &str, v: Json| {
            if b[*i..].starts_with(w.as_bytes()) {
                *i += w.len();
                Ok(v)
            } else {
                Err(format!("bad literal at {i}"))
            }
        };
        match b.get(*i) {
            None => Err("unexpected end".into()),
            Some(b'n') => lit(i, "null", Json::Null),
            Some(b't') => lit(i, "true", Json::Bool(true)),
            Some(b'f') => lit(i, "false", Json::Bool(false)),
            Some(b'"') => Self::string(b, i).map(Json::Str),
            Some(b'[') => {
                *i += 1;
                let mut v = Vec::new();
                loop {
                    Self::ws(b, i);
                    if b.get(*i) == Some(&b']') {
                        *i += 1;
                        return Ok(Json::Arr(v));
                    }
                    if !v.is_empty() {
                        if b.get(*i) != Some(&b',') {
                            return Err(format!("expected , at {i}"));
                        }
                        *i += 1;
                    }
                    v.push(Self::val(b, i)?);
                }
            }
            Some(b'{') => {
                *i += 1;
                let mut m = BTreeMap::new();
                loop {
                    Self::ws(b, i);
                    if b.get(*i) == Some(&b'}') {
                        *i += 1;
                        return Ok(Json::Obj(m));
                    }
                    if !m.is_empty() {
                        if b.get(*i) != Some(&b',') {
                            return Err(format!("expected , at {i}"));
                        }
                        *i += 1;
                        Self::ws(b, i);
                    }
                    if b.get(*i) != Some(&b'"') {
                        return Err(format!("expected key at {i}"));
                    }
                    let k = Self::string(b, i)?;
                    Self::ws(b, i);
                    if b.get(*i) != Some(&b':') {
                        return Err(format!("expected : at {i}"));
                    }
                    *i += 1;
                    m.insert(k, Self::val(b, i)?);
                }
            }
            Some(_) => {
                let st = *i;
                while *i < b.len()
                    && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                {
                    *i += 1;
                }
                std::str::from_utf8(&b[st..*i])
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .map(Json::Num)
                    .ok_or(format!("bad number at {st}"))
            }
        }
    }
    fn string(b: &[u8], i: &mut usize) -> Result<String, String> {
        *i += 1;
        let st = *i;
        while *i < b.len() && b[*i] != b'"' {
            if b[*i] == b'\\' {
                return Err("escapes not supported".into());
            }
            *i += 1;
        }
        if *i >= b.len() {
            return Err("unterminated string".into());
        }
        let s = std::str::from_utf8(&b[st..*i])
            .map_err(|e| e.to_string())?
            .to_string();
        *i += 1;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_rig_loads_with_real_sole_edge() {
        let r = default_rig("master").unwrap();
        assert_eq!(r.profile.joints.len(), 13);
        assert_eq!(r.sole_edge(), (0.12, 0.05));
        let m: f64 = r.profile.joints.iter().map(|j| j.mass).sum();
        assert!((m - 75.0).abs() < 1e-9);
        assert!((default_rig("mannequin").unwrap().total_mass - 59.3).abs() < 1e-12);
        assert!(default_rig("nobody").is_err());
    }
    #[test]
    fn roles_and_arm_flag() {
        let r = default_rig("master").unwrap();
        let arms: Vec<&str> = r
            .roles
            .iter()
            .zip(&r.profile.joints)
            .filter(|(ro, _)| ro.is_arm())
            .map(|(_, j)| j.name.as_str())
            .collect();
        assert_eq!(
            arms,
            ["B_L_UpperArm", "B_L_Forearm", "B_R_UpperArm", "B_R_Forearm"]
        );
        let ankles: Vec<usize> = (0..13)
            .filter(|&i| matches!(r.profile.joints[i].role, api::JointRole::Ankle(_)))
            .collect();
        assert_eq!(ankles, [11, 12]);
        assert_eq!(r.profile.joints[7].role, api::JointRole::Hip(0));
        assert_eq!(r.kp_scale(3, 1.0, 1.5), 1.5);
        assert_eq!(r.kp_scale(3, 0.5, 1.5), 1.25);
        assert_eq!(r.kp_scale(7, 1.0, 1.5), 1.0);
    }
    #[test]
    fn geometry_offsets_and_inertia() {
        let r = default_rig("master").unwrap();
        let foot = &r.profile.joints[11];
        assert_eq!(foot.parent, Some(8));
        assert!((foot.offset.y - (0.08 - 0.52)).abs() < 1e-12 && (foot.com.z - 0.04).abs() < 1e-12);
        let ix = foot.inertia.0[0][0];
        assert!((ix - foot.mass * (0.06 * 0.06 + 0.24 * 0.24) / 12.0).abs() < 1e-12);
        let cap = 3.0 * 75.0 * api::G;
        assert!(
            r.profile
                .joints
                .iter()
                .all(|j| (j.torque_cap - cap).abs() < 1e-9)
        );
    }
    #[test]
    fn rejects_bad_files() {
        assert!(load_rig("{}", "master", 3.0).is_err());
        assert!(load_rig("{\"rigs\":{\"master\":75}", "master", 3.0).is_err());
        let bad = RIG_JSON.replace("\"parent\": 8", "\"parent\": 12");
        assert!(load_rig(&bad, "master", 3.0).is_err());
    }
}

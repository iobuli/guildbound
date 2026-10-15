//! Measurements baked beside a model. Missing profiles retain the established defaults.
use bevy::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone)]
pub struct TorsoSection {
    pub from: String,
    pub to: String,
    pub lateral: [String; 2],
    pub radius: Vec2,
    pub offset: Vec2,
}

#[derive(Clone, Debug)]
pub struct JointRule {
    pub from: String,
    pub joint: String,
    pub to: String,
    pub min: f32,
    pub max: f32,
}

#[derive(Component, Clone)]
pub struct RigProfile {
    pub rules: Vec<JointRule>,
    pub knee_max: f32,
    pub look: Vec2,
    pub mass_kg: f32,
    pub torso: Vec<TorsoSection>,
    pub limb_radii: HashMap<String, f32>,
}

impl Default for RigProfile {
    fn default() -> Self {
        Self {
            rules: crate::joints::RULES.iter().map(|&(a, j, b, min, max)| JointRule {
                from: a.into(), joint: j.into(), to: b.into(), min, max,
            }).collect(),
            knee_max: crate::knee::MAX_FLEX_DEG,
            look: Vec2::new(52f32.to_radians(), 46f32.to_radians()),
            mass_kg: 75.0,
            torso: Vec::new(), limb_radii: HashMap::new(),
        }
    }
}

impl RigProfile {
    pub fn load(path: &str) -> Self {
        let read = |suffix: &str| Some(crate::read_json(&format!("assets/{path}.{suffix}.json"))).filter(|v| !v.is_null());
        let mut out = read("rig").map(|v| Self::from_json(&v)).unwrap_or_default();
        if let Some(body) = read("body") { out.read_body(&body["sections"], &body["limbs"]); }
        out
    }

    fn from_json(v: &Value) -> Self {
        let mut out = Self::default();
        let number = |v: &Value, lo: f32, hi: f32| v.as_f64().map(|n| n as f32).filter(|n| n.is_finite() && (lo..=hi).contains(n));
        for row in v["rules"].as_array().into_iter().flatten() {
            let (Some(joint), Some(min), Some(max)) = (row["joint"].as_str(), number(&row["min_deg"], 0.0, 179.0), number(&row["max_deg"], 0.0, 179.0)) else { continue };
            if min >= max { continue; }
            if let Some(rule) = out.rules.iter_mut().find(|r| r.joint == joint && row["from"].as_str() == Some(r.from.as_str()) && row["to"].as_str() == Some(r.to.as_str())) {
                rule.min = min;
                rule.max = max;
            }
        }
        out.knee_max = number(&v["knee_max_flex_deg"], 45.0, 150.0).unwrap_or(out.knee_max);
        out.mass_kg = number(&v["mass_kg"], 10.0, 300.0).unwrap_or(out.mass_kg);
        out.look.x = number(&v["look"]["yaw_deg"], 0.0, 90.0).map(f32::to_radians).unwrap_or(out.look.x);
        out.look.y = number(&v["look"]["pitch_deg"], 0.0, 80.0).map(f32::to_radians).unwrap_or(out.look.y);
        out.read_body(&v["torso"], &Value::Null);
        out
    }

    fn read_body(&mut self, sections: &Value, limbs: &Value) {
        let n = |v: &Value| v.as_f64().map(|v| v as f32).filter(|v| v.is_finite());
        let mut torso = Vec::new();
        for s in sections.as_array().into_iter().flatten() {
            let (Some(from), Some(to), Some(l), Some(r), Some(a), Some(b)) = (s["from"].as_str(), s["to"].as_str(), s["lateral"][0].as_str(), s["lateral"][1].as_str(), n(&s["a"]), n(&s["b"])) else { continue };
            if !(0.02..0.5).contains(&a) || !(0.02..0.5).contains(&b) { continue; }
            torso.push(TorsoSection { from: from.into(), to: to.into(), lateral: [l.into(), r.into()],
                radius: Vec2::new(a, b), offset: Vec2::new(n(&s["cx"]).unwrap_or(0.0), n(&s["cy"]).unwrap_or(0.0)) });
        }
        if !torso.is_empty() { self.torso = torso; }
        for row in limbs.as_array().into_iter().flatten() {
            if let (Some(name), Some(radius)) = (row["from"].as_str(), n(&row["radius"])) && (0.005..0.2).contains(&radius) {
                self.limb_radii.insert(name.into(), radius);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn measured_ranges_override_defaults_without_accepting_broken_anatomy() {
        let p = RigProfile::from_json(&json!({"mass_kg": 58.9, "look": {"yaw_deg": 30}, "rules": [
            {"from":"B_Spine2", "joint":"B_Neck", "to":"B_Head", "min_deg":10, "max_deg":60},
            {"from":"wrong", "joint":"B_Spine", "to":"B_Spine1", "min_deg":0, "max_deg":20},
            {"from":"B_Spine", "joint":"B_Spine1", "to":"B_Spine2", "min_deg":90, "max_deg":20}
        ]}));
        assert_eq!(p.rules.iter().find(|r| r.joint == "B_Neck").unwrap().max, 60.0);
        assert_eq!(p.rules[0].max, 40.0);
        assert_eq!(p.rules[1].max, 35.0);
        assert_eq!(p.mass_kg, 58.9);
        assert!((p.look.x - 30f32.to_radians()).abs() < 1e-6);
    }
}

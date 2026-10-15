//! Timed poses from gb.possess. The machine evaluates bounded, seekable motion between LLM updates.
use std::collections::BTreeMap;
use bevy::prelude::*;
use serde_json::Value;
use crate::{motion_math::world_transform, procedural::RestPose};

#[derive(Clone, Debug)]
pub struct Key {
    time: f32,
    bones: BTreeMap<String, Quat>,
    pelvis: Vec3,
    grip: Option<[f32; 2]>,
    hands: BTreeMap<String, Vec3>,
}

#[derive(Clone, Debug)]
pub struct Motion {
    pub duration: f32,
    pub looping: bool,
    pub plants: [bool; 2],
    pub keys: Vec<Key>,
}

pub struct Sample {
    pub bones: BTreeMap<String, Quat>,
    pub pelvis: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub grip: Option<[f32; 2]>,
    pub hands: BTreeMap<String, Vec3>,
}

fn number(v: &Value) -> Result<f32, String> {
    v.as_f64().map(|n| n as f32).filter(|n| n.is_finite()).ok_or_else(|| "motion numbers must be finite".into())
}

fn vector(v: &Value) -> Result<Vec3, String> {
    let a = v.as_array().filter(|a| a.len() == 3).ok_or("expected [x,y,z]")?;
    Ok(Vec3::new(number(&a[0])?, number(&a[1])?, number(&a[2])?))
}

impl Motion {
    pub fn parse(v: &Value, known: &[String]) -> Result<Self, String> {
        let rows = v["keys"].as_array().filter(|a| (2..=240).contains(&a.len())).ok_or("motion needs 2-240 keys")?;
        let mut keys = Vec::<Key>::new();
        for row in rows {
            let time = number(&row["time"])?;
            if !(0.0..=30.0).contains(&time) || keys.last().is_some_and(|k| time - k.time < 1.0 / 240.0) {
                return Err("key times must increase, at least 1/240 s apart, within 30 s".into());
            }
            let mut bones = BTreeMap::new();
            for (name, value) in row["bones"].as_object().ok_or("each key needs bones")? {
                if !known.contains(name) || matches!(name.as_str(), "Motion" | "motion" | "root" | "Root_M") {
                    return Err(format!("unknown or motor-owned bone: {name}"));
                }
                let e = vector(value)?;
                if e.abs().max_element() > 180.0 { return Err("bone offsets must be within +/-180 degrees".into()); }
                bones.insert(name.clone(), Quat::from_euler(EulerRot::XYZ, e.x.to_radians(), e.y.to_radians(), e.z.to_radians()));
            }
            if bones.is_empty() { return Err("each key needs at least one bone".into()); }
            if keys.last().is_some_and(|k| !k.bones.keys().eq(bones.keys())) {
                return Err("every key must name the same bones".into());
            }
            let pelvis = if row.get("pelvis_m").is_some() { vector(&row["pelvis_m"])? } else { Vec3::ZERO };
            if pelvis.length() > 0.35 { return Err("pelvis offset exceeds 0.35 m".into()); }
            let grip = if let Some(v) = row.get("grip") {
                let a = v.as_array().filter(|a| a.len() == 2).ok_or("grip needs [left,right]")?;
                let g = [number(&a[0])?, number(&a[1])?];
                if g.iter().any(|g| !(0.0..=1.0).contains(g)) { return Err("grip must be within 0..1".into()); }
                Some(g)
            } else { None };
            if let Some(previous) = keys.last() {
                if previous.grip.is_some() != grip.is_some() { return Err("grip must be present at every key or none".into()); }
                let dt = time - previous.time;
                // Smoothstep's peak speed is 1.5 times its segment average; peak acceleration is 6 delta / dt^2.
                if bones.iter().any(|(n, q)| previous.bones[n].angle_between(*q).abs() * 1.5 / dt > 720f32.to_radians()) {
                    return Err("motion exceeds 720 deg/s bone speed".into());
                }
                let travel = pelvis.distance(previous.pelvis);
                if travel * 1.5 / dt > 2.0 || travel * 6.0 / (dt * dt) > 20.0 {
                    return Err("pelvis exceeds 2 m/s or 20 m/s2".into());
                }
            }
            let mut hands = BTreeMap::new();
            for (side, value) in row["hands_m"].as_object().into_iter().flatten() {
                let (s, hand) = match side.as_str() { "left" => ("L", "B_L_Forearm.001"), "right" => ("R", "B_R_Hand"), _ => return Err("hands_m keys must be left/right".into()) };
                if [format!("B_{s}_UpperArm"), format!("B_{s}_Forearm"), hand.into()].iter().any(|b| !known.contains(b)) {
                    return Err(format!("{side} hand target requires mapped arm bones"));
                }
                let point = vector(value)?;
                if point.length() > 2.5 { return Err("hand target exceeds 2.5 m from the stage origin".into()); }
                hands.insert(side.clone(), point);
            }
            if let Some(previous) = keys.last() {
                if !previous.hands.keys().eq(hands.keys()) { return Err("every key must name the same hand targets".into()); }
                let dt = time - previous.time;
                if hands.iter().any(|(n, p)| p.distance(previous.hands[n]) * 1.5 / dt > 10.0 || p.distance(previous.hands[n]) * 6.0 / (dt * dt) > 120.0) {
                    return Err("hand target exceeds 10 m/s or 120 m/s2".into());
                }
            }
            keys.push(Key { time, bones, pelvis, grip, hands });
        }
        if keys[0].time != 0.0 { return Err("first key must be at time 0".into()); }
        let looping = v["loop"].as_bool().unwrap_or(false);
        let first = &keys[0];
        let last = keys.last().unwrap();
        if looping && (first.pelvis.distance(last.pelvis) > 1e-5 || first.grip != last.grip
            || first.hands.iter().any(|(n, p)| p.distance(last.hands[n]) > 1e-5)
            || first.bones.iter().any(|(n, q)| q.dot(last.bones[n]).abs() < 1.0 - 1e-6)) {
            return Err("loop endpoints must match".into());
        }
        let mut plants = [false; 2];
        for side in v["plants"].as_array().into_iter().flatten() {
            let i = match side.as_str() { Some("left") => 0, Some("right") => 1, _ => return Err("plants must be left/right".into()) };
            plants[i] = true;
            let s = if i == 0 { "L" } else { "R" };
            if ["Thigh", "Calf", "Foot"].iter().any(|b| !known.contains(&format!("B_{s}_{b}"))) {
                return Err(format!("{side} foot plant requires the mapped leg bones"));
            }
        }
        Ok(Self { duration: last.time, looping, plants, keys })
    }

    pub fn sample(&self, time: f32) -> Sample {
        let t = if self.looping { time.rem_euclid(self.duration) } else { time.clamp(0.0, self.duration) };
        let i = self.keys.iter().position(|k| k.time >= t).unwrap_or(self.keys.len() - 1).max(1);
        let (a, b) = (&self.keys[i - 1], &self.keys[i]);
        let dt = b.time - a.time;
        let u = ((t - a.time) / dt).clamp(0.0, 1.0);
        let w = u * u * (3.0 - 2.0 * u);
        let delta = b.pelvis - a.pelvis;
        let moving = time >= 0.0 && (self.looping || time < self.duration);
        Sample {
            bones: a.bones.iter().map(|(n, q)| (n.clone(), q.slerp(b.bones[n], w).normalize())).collect(),
            pelvis: a.pelvis.lerp(b.pelvis, w),
            velocity: if moving { delta * (6.0 * u * (1.0 - u) / dt) } else { Vec3::ZERO },
            acceleration: if moving { delta * ((6.0 - 12.0 * u) / (dt * dt)) } else { Vec3::ZERO },
            grip: a.grip.zip(b.grip).map(|(a, b)| std::array::from_fn(|s| a[s] + (b[s] - a[s]) * w)),
            hands: a.hands.iter().map(|(n, p)| (n.clone(), p.lerp(b.hands[n], w))).collect(),
        }
    }
}

/// Replaces the stage's clip pose with bind-relative bone targets. Never changes gameplay position or scale.
pub fn apply(world: &mut World) {
    let p = world.resource::<crate::possession::Possession>();
    let s = world.resource::<crate::studio::Studio>();
    let Some(motion) = p.motion.as_ref().filter(|_| p.enabled && s.open) else { return };
    let Some(root) = s.entity else { return };
    let Some(rest) = world.get::<RestPose>(root).map(|r| (r.0.clone(), r.1.clone())) else { return };
    let sample = motion.sample(s.time);
    let rotation = world.get::<Transform>(root).map_or(Quat::IDENTITY, |t| t.rotation);
    let mut pelvis = None;
    for (&e, &q) in &rest.0 {
        let name = world.get::<Name>(e).map(|n| n.as_str().to_owned()).unwrap_or_default();
        if name == "B_Pelvis" { pelvis = Some(e); }
        if let Some(mut t) = world.get_mut::<Transform>(e) {
            t.rotation = q;
            if let Some(pos) = rest.1.get(&e) { t.translation = *pos; }
        }
    }
    // Bind anchors are independent of the order in which frames are requested.
    let anchors = ["L", "R"].map(|side| rest.0.keys().find(|&&e| world.get::<Name>(e).is_some_and(|n| n.as_str() == format!("B_{side}_Foot")))
        .and_then(|&e| world_transform(world, e)).map(|t| t.translation()));
    world.resource_mut::<crate::possession::Possession>().plant_anchors = anchors;
    for &e in rest.0.keys() {
        let target = world.get::<Name>(e).and_then(|n| sample.bones.get(n.as_str())).copied();
        if let Some(q) = target && let Some(mut t) = world.get_mut::<Transform>(e) {
            t.rotation = (rest.0[&e] * q).normalize();
        }
    }
    if let Some(e) = pelvis && let Some(parent) = world.get::<ChildOf>(e).map(|p| p.parent())
        && let Some(g) = world_transform(world, parent) && let Some(mut t) = world.get_mut::<Transform>(e) {
        t.translation += g.affine().inverse().transform_vector3(rotation * sample.pelvis);
    }
    let origin = world_transform(world, root).map_or(Vec3::ZERO, |g| g.translation());
    for (side, point) in &sample.hands {
        let (s, hand, sign) = if side == "left" { ("L", "B_L_Forearm.001", 1.0) } else { ("R", "B_R_Hand", -1.0) };
        let names = [format!("B_{s}_UpperArm"), format!("B_{s}_Forearm"), hand.into()];
        let joints: Option<Vec<_>> = names.iter().map(|n| rest.0.keys().copied().find(|&e| world.get::<Name>(e).is_some_and(|name| name.as_str() == n))).collect();
        let Some(joints) = joints else { continue };
        let joints = [joints[0], joints[1], joints[2]];
        let poses: Option<Vec<_>> = joints.iter().map(|&e| world_transform(world, e)).collect();
        let Some(poses) = poses else { continue };
        let lengths = [poses[0].translation().distance(poses[1].translation()), poses[1].translation().distance(poses[2].translation())];
        if lengths.iter().any(|l| !l.is_finite() || !(0.05..0.8).contains(l)) { continue; }
        let pole = rotation * Vec3::new(sign, -0.7, -0.2);
        crate::ik::apply(world, joints, origin + rotation * *point, pole, pole, lengths);
    }
}

pub fn plant_feet(world: &mut World) {
    let p = world.resource::<crate::possession::Possession>();
    let s = world.resource::<crate::studio::Studio>();
    let Some(motion) = p.motion.as_ref().filter(|_| p.enabled && s.open) else { return };
    let (plants, anchors) = (motion.plants, p.plant_anchors);
    let Some(root) = s.entity else { return };
    let Some(bones) = world.get::<crate::studio::StudioRig>(root).map(|r| r.bones.clone()) else { return };
    let forward = world.get::<Transform>(root).map_or(Vec3::Z, |t| t.rotation * Vec3::Z);
    for (i, side) in ["L", "R"].into_iter().enumerate() {
        let Some(target) = anchors[i].filter(|_| plants[i]) else { continue };
        let joints: Option<Vec<_>> = ["Thigh", "Calf", "Foot"].iter().map(|part| bones.iter().copied().find(|&e| world.get::<Name>(e).is_some_and(|n| n.as_str() == format!("B_{side}_{part}")))).collect();
        let Some(joints) = joints else { continue };
        let joints = [joints[0], joints[1], joints[2]];
        let poses: Option<Vec<_>> = joints.iter().map(|&e| world_transform(world, e)).collect();
        let Some(poses) = poses else { continue };
        let lengths = [poses[0].translation().distance(poses[1].translation()), poses[1].translation().distance(poses[2].translation())];
        if lengths.iter().any(|l| !l.is_finite() || !(0.1..0.8).contains(l)) { continue; }
        let foot_rotation = poses[2].to_scale_rotation_translation().1;
        crate::ik::apply(world, joints, target, forward, forward, lengths);
        crate::ik::rotate_world(world, joints[2], foot_rotation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn program() -> Value {
        json!({"plants":["left"], "keys":[
            {"time":0, "bones":{"B_Pelvis":[0,0,0]}, "pelvis_m":[0,0,0], "grip":[0,0]},
            {"time":1, "bones":{"B_Pelvis":[0,60,0]}, "pelvis_m":[0,-0.1,0], "grip":[1,1]}
        ]})
    }

    fn known() -> Vec<String> { ["B_Pelvis", "B_L_Thigh", "B_L_Calf", "B_L_Foot"].map(str::to_owned).to_vec() }

    #[test]
    fn sampled_pose_is_seekable_and_stops_with_zero_velocity() {
        let m = Motion::parse(&program(), &known()).unwrap();
        for hz in [30, 60, 144] {
            for frame in 0..=hz { m.sample(frame as f32 / hz as f32); }
            let s = m.sample(0.5);
            assert!((s.bones["B_Pelvis"].angle_between(Quat::IDENTITY) - 30f32.to_radians()).abs() < 1e-5);
            assert!((s.pelvis.y + 0.05).abs() < 1e-6);
            assert_eq!(s.grip, Some([0.5, 0.5]));
            assert!(m.sample(10.0).velocity == Vec3::ZERO);
        }
    }

    #[test]
    fn rejects_unknown_bones_fast_moves_and_unclosed_loops() {
        let mut v = program();
        v["loop"] = json!(true);
        assert!(Motion::parse(&v, &known()).unwrap_err().contains("endpoints"));
        v["loop"] = json!(false);
        v["keys"][1]["time"] = json!(0.01);
        assert!(Motion::parse(&v, &known()).is_err());
        assert!(Motion::parse(&program(), &[]).is_err());
    }

    fn bone(world: &mut World, name: &str, parent: Entity, at: Vec3, bones: &mut Vec<Entity>) -> Entity {
        let e = world.spawn((Name::new(name.to_owned()), Transform::from_translation(at), ChildOf(parent))).id();
        bones.push(e);
        e
    }

    fn stage_world() -> (World, Entity, Vec<Entity>) {
        let mut world = World::new();
        let root = world.spawn(Transform::from_translation(crate::studio::STAGE)).id();
        let mut bones = Vec::new();
        let pelvis = bone(&mut world, "B_Pelvis", root, Vec3::Y * 0.9, &mut bones);
        let spine = bone(&mut world, "B_Spine", pelvis, Vec3::Y * 0.15, &mut bones);
        let spine1 = bone(&mut world, "B_Spine1", spine, Vec3::Y * 0.15, &mut bones);
        let chest = bone(&mut world, "B_Spine2", spine1, Vec3::Y * 0.15, &mut bones);
        let neck = bone(&mut world, "B_Neck", chest, Vec3::Y * 0.15, &mut bones);
        bone(&mut world, "B_Head", neck, Vec3::Y * 0.12, &mut bones);
        for (side, x) in [("L", 0.15), ("R", -0.15)] {
            let hip = bone(&mut world, &format!("B_{side}_Thigh"), pelvis, Vec3::new(x, -0.02, 0.0), &mut bones);
            let knee = bone(&mut world, &format!("B_{side}_Calf"), hip, Vec3::new(0.0, -0.4, 0.12), &mut bones);
            let foot = bone(&mut world, &format!("B_{side}_Foot"), knee, Vec3::new(0.0, -0.4, -0.12), &mut bones);
            bone(&mut world, &format!("B_{side}_Toe0"), foot, Vec3::new(0.0, -0.04, 0.15), &mut bones);
        }
        let hand = bone(&mut world, "B_L_Forearm.001", chest, Vec3::new(0.4, 0.0, 0.0), &mut bones);
        for (finger, z) in [("index", 0.04), ("middle", 0.015), ("ring", -0.012), ("pinky", -0.035)] {
            let name = if finger == "middle" { "B_L_Finger2".into() } else { format!("{finger}_01_l") };
            let a = bone(&mut world, &name, hand, Vec3::new(0.11, 0.0, z), &mut bones);
            let b = bone(&mut world, &format!("{finger}_02_l"), a, Vec3::X * 0.035, &mut bones);
            bone(&mut world, &format!("{finger}_03_l"), b, Vec3::X * 0.03, &mut bones);
        }
        let a = bone(&mut world, "thumb_01_l", hand, Vec3::new(0.034, 0.027, 0.041), &mut bones);
        let b = bone(&mut world, "thumb_02_l", a, Vec3::new(0.03, 0.02, 0.015), &mut bones);
        bone(&mut world, "thumb_03_l", b, Vec3::new(0.028, 0.017, 0.014), &mut bones);
        let rest = RestPose(bones.iter().map(|&e| (e, world.get::<Transform>(e).unwrap().rotation)).collect(),
            bones.iter().map(|&e| (e, world.get::<Transform>(e).unwrap().translation)).collect());
        world.entity_mut(root).insert((rest, crate::studio::StudioRig { bones: bones.clone(), player: None, rules: true }));
        let mut studio = crate::studio::Studio::default();
        studio.entity = Some(root);
        studio.open = true;
        world.insert_resource(studio);
        world.insert_resource(crate::possession::Possession::default());
        world.insert_resource(crate::sim_bridge::Sim::new());
        (world, root, bones)
    }

    fn pose(world: &mut World, time: f32) {
        world.resource_mut::<crate::studio::Studio>().time = time;
        apply(world);
        crate::procedural::bone_lengths(world);
        crate::possession::apply(world);
        plant_feet(world);
        crate::joints::apply(world);
        crate::knee::apply(world);
        crate::studio::measure(world);
        crate::possession::measure(world);
    }

    #[test]
    fn final_pose_is_repeatable_with_planted_feet_and_manual_fingers() {
        let (mut world, _, bones) = stage_world();
        let mut v = program();
        v["plants"] = json!(["left", "right"]);
        crate::possession::command(&mut world, &json!({"motion":v, "breath":true, "look":"wander",
            "set":{"index_02_l":[0,0,12]}, "absolute":true})).unwrap();
        pose(&mut world, 0.5);
        let first: Vec<_> = bones.iter().map(|&e| *world.get::<Transform>(e).unwrap()).collect();
        let expected_com = world.resource::<crate::possession::Possession>().readout.com;
        for t in [0.5, 0.5, 0.9, 0.0, 0.5] { pose(&mut world, t); }
        for (&e, expected) in bones.iter().zip(first) {
            assert_eq!(*world.get::<Transform>(e).unwrap(), expected, "render history changed the pose");
        }
        assert_eq!(world.resource::<crate::possession::Possession>().readout.com, expected_com);
        let p = world.resource::<crate::possession::Possession>();
        for (i, side) in ["L", "R"].into_iter().enumerate() {
            let foot = bones.iter().copied().find(|&e| world.get::<Name>(e).unwrap().as_str() == format!("B_{side}_Foot")).unwrap();
            assert!(world_transform(&world, foot).unwrap().translation().distance(p.plant_anchors[i].unwrap()) < 0.002);
        }
        let finger = bones.iter().copied().find(|&e| world.get::<Name>(e).unwrap().as_str() == "index_02_l").unwrap();
        assert!(world.get::<Transform>(finger).unwrap().rotation.angle_between(Quat::from_rotation_z(12f32.to_radians())).abs() < 1e-4);
    }

    #[test]
    fn paused_edits_refresh_measurements_and_invalid_motion_is_atomic() {
        let (mut world, _, _) = stage_world();
        crate::possession::command(&mut world, &json!({"motion":program()})).unwrap();
        pose(&mut world, 0.2);
        pose(&mut world, 0.3);
        assert!(world.resource::<crate::possession::Possession>().readout.valid);
        let before = world.resource::<crate::possession::Possession>().readout.com;
        crate::possession::command(&mut world, &json!({"set":{"B_Neck":[30,0,0]}, "absolute":true})).unwrap();
        pose(&mut world, 0.3);
        let after = &world.resource::<crate::possession::Possession>().readout;
        assert!(after.com.distance(before) > 1e-4);
        assert!(!after.valid, "editing a paused pose is not a physical impulse");
        assert!(crate::possession::command(&mut world, &json!({"motion":{"keys":[]}, "clear":true})).is_err());
        assert!(world.resource::<crate::possession::Possession>().overrides.contains_key("B_Neck"));
        assert!(world.resource::<crate::possession::Possession>().motion.is_some());
    }
}

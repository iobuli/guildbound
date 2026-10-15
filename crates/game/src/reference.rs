//! Aggregate motion measurements only. RPG clips never enter the playback graph.
use bevy::prelude::*;
use gltf::animation::util::ReadOutputs;

#[derive(Resource, Clone)]
pub(crate) struct Reference {
    pub stride_ratio: f32,
    pub duty: f32,
    pub knee_peak: f32,
    pub elbow_bend: f32,
    pub strike_peak: f32,
    pub strike_start: f32,
    pub strike_end: f32,
    pub strike_extension: f32,
    pub run_period: f32,
    pub note: String,
}
impl Default for Reference {
    fn default() -> Self {
        match Self::read() {
            Ok(reference) => reference,
            Err(e) => Self {
                stride_ratio: 2.0,
                duty: 0.48,
                knee_peak: 1.7,
                elbow_bend: 0.85,
                strike_peak: 0.42,
                strike_start: 0.34,
                strike_end: 0.58,
                strike_extension: 0.9,
                run_period: 0.8,
                note: format!("RPG data unavailable: {e}"),
            },
        }
    }
}
impl Reference {
    fn read() -> Result<Self, String> {
        let base = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/packs/hero/fighter.glb"
        );
        let metadata: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{base}.motion.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let glb = gltf::Gltf::open(base).map_err(|e| e.to_string())?;
        let clips_indexed = glb.animations().count();
        let blob = glb
            .blob
            .as_deref()
            .ok_or("RPG reference has no binary buffer")?;
        let nodes: Vec<_> = glb.nodes().collect();
        let mut parents = vec![None; nodes.len()];
        for n in &nodes {
            for child in n.children() {
                parents[child.index()] = Some(n.index());
            }
        }
        let rest: Vec<Transform> = nodes
            .iter()
            .map(|n| {
                let (t, q, s) = n.transform().decomposed();
                Transform {
                    translation: Vec3::from(t),
                    rotation: Quat::from_array(q).normalize(),
                    scale: Vec3::from(s),
                }
            })
            .collect();
        let find = |name: &str| {
            nodes
                .iter()
                .find(|n| n.name() == Some(name))
                .map(|n| n.index())
                .ok_or_else(|| format!("RPG missing {name}"))
        };
        let leg = [find("B_L_Thigh")?, find("B_L_Calf")?, find("B_L_Foot")?];
        let arm = [
            find("B_R_UpperArm")?,
            find("B_R_Forearm")?,
            find("B_R_Hand")?,
        ];
        fn matrices(local: &[Transform], parents: &[Option<usize>]) -> Vec<Mat4> {
            fn solve(
                i: usize,
                l: &[Transform],
                p: &[Option<usize>],
                cache: &mut [Option<Mat4>],
            ) -> Mat4 {
                if let Some(m) = cache[i] {
                    return m;
                }
                let m = p[i]
                    .map(|j| solve(j, l, p, cache))
                    .unwrap_or(Mat4::IDENTITY)
                    * l[i].to_matrix();
                cache[i] = Some(m);
                m
            }
            let mut cache = vec![None; local.len()];
            (0..local.len())
                .map(|i| solve(i, local, parents, &mut cache))
                .collect()
        }
        let bind = matrices(&rest, &parents);
        let p = leg.map(|i| bind[i].transform_point3(Vec3::ZERO));
        let leg_length = p[0].distance(p[1]) + p[1].distance(p[2]);
        let mut knee = Vec::new();
        let mut elbows = Vec::new();
        let mut run_period = 0.8;
        let mut sources = Vec::new();
        for name in [
            "nc_run_forward",
            "nc_walk_forward",
            "hp_straight_right_A",
            "ual_roll",
        ] {
            let Some(animation) = glb.animations().find(|a| a.name() == Some(name)) else {
                continue;
            };
            let mut tracks = Vec::new();
            let mut duration = 0.0f32;
            for channel in animation.channels() {
                if channel.sampler().interpolation() == gltf::animation::Interpolation::CubicSpline
                {
                    continue;
                }
                let reader = channel
                    .reader(|b| matches!(b.source(), gltf::buffer::Source::Bin).then_some(blob));
                let Some(inputs) = reader.read_inputs() else {
                    continue;
                };
                let times: Vec<_> = inputs.collect();
                if times.is_empty() {
                    continue;
                }
                duration = duration.max(*times.last().unwrap());
                let values = match reader.read_outputs() {
                    Some(ReadOutputs::Rotations(v)) => TrackValues::Rotation(
                        v.into_f32()
                            .map(|q| Quat::from_array(q).normalize())
                            .collect(),
                    ),
                    Some(ReadOutputs::Translations(v)) => {
                        TrackValues::Translation(v.map(Vec3::from).collect())
                    }
                    _ => continue,
                };
                tracks.push((
                    channel.target().node().index(),
                    times,
                    values,
                    channel.sampler().interpolation() == gltf::animation::Interpolation::Step,
                ));
            }
            if name == "nc_run_forward" {
                run_period = duration;
            }
            for sample in 0..=48 {
                let time = duration * sample as f32 / 48.0;
                let mut local = rest.clone();
                for (node, times, values, step) in &tracks {
                    let lo = times.partition_point(|&t| t <= time).saturating_sub(1);
                    let hi = (lo + 1).min(times.len() - 1);
                    let span = times[hi] - times[lo];
                    let u = if *step || span < 1e-6 {
                        0.0
                    } else {
                        ((time - times[lo]) / span).clamp(0.0, 1.0)
                    };
                    match values {
                        TrackValues::Rotation(v) => local[*node].rotation = v[lo].slerp(v[hi], u),
                        TrackValues::Translation(v) => {
                            local[*node].translation = v[lo].lerp(v[hi], u)
                        }
                    }
                }
                let pose = matrices(&local, &parents);
                let bend = |j: [usize; 3]| {
                    let p = j.map(|i| pose[i].transform_point3(Vec3::ZERO));
                    (p[1] - p[0])
                        .normalize_or(Vec3::NEG_Y)
                        .angle_between((p[2] - p[1]).normalize_or(Vec3::NEG_Y))
                };
                if name == "nc_run_forward" {
                    knee.push(bend(leg));
                    elbows.push(bend(arm));
                }
            }
            sources.push(serde_json::json!({"clip":name,"duration_s":duration,"samples":49}));
        }
        knee.sort_by(f32::total_cmp);
        elbows.sort_by(f32::total_cmp);
        let quantile = |v: &[f32], q: f32, fallback: f32| {
            if v.is_empty() {
                fallback
            } else {
                v[((v.len() - 1) as f32 * q) as usize]
            }
        };
        let contacts = metadata["foot_contacts"]["Unarmed-Run-Forward"]
            .as_array()
            .ok_or("RPG run contact data missing")?;
        let planted = contacts
            .iter()
            .flat_map(|v| v.as_array().into_iter().flatten())
            .filter(|v| v.as_f64().unwrap_or(0.0) > 0.55)
            .count();
        let duty = (planted as f32 / (contacts.len() * 2).max(1) as f32).clamp(0.35, 0.65);
        let stride = metadata["strides_m"]["Unarmed-Run-Forward"]
            .as_f64()
            .unwrap_or(2.0) as f32;
        let strike = metadata["strikes"]["Unarmed-Attack-R1"]["contact_peak"]
            .as_f64()
            .unwrap_or(0.42) as f32;
        let strike_data = &metadata["strikes"]["Unarmed-Attack-R1"];
        let reference = Self {
            stride_ratio: (stride / leg_length.max(0.3)).clamp(1.6, 2.7),
            duty,
            knee_peak: quantile(&knee, 0.9, 1.7).clamp(0.8, 2.2),
            elbow_bend: quantile(&elbows, 0.5, 0.85).clamp(0.35, 1.4),
            strike_peak: strike.clamp(0.3, 0.6),
            strike_start: strike_data["contact_start"].as_f64().unwrap_or(0.34) as f32,
            strike_end: strike_data["contact_end"].as_f64().unwrap_or(0.58) as f32,
            strike_extension: strike_data["extension_m"].as_f64().unwrap_or(0.9) as f32,
            run_period,
            note: format!(
                "RPG library: {clips_indexed} per-clip analogues; {} feature anchors measured",
                sources.len()
            ),
        };
        let report = serde_json::json!({"source":base,"metadata":format!("{base}.motion.json"),"clips":sources,"source_leg_m":leg_length,
            "run_stride_per_leg":reference.stride_ratio,"stance_fraction":reference.duty,"knee_p90_rad":reference.knee_peak,"elbow_median_rad":reference.elbow_bend,
            "strike_peak_phase":reference.strike_peak,"strike_start":reference.strike_start,"strike_end":reference.strike_end,
            "strike_extension_m":reference.strike_extension,"clips_indexed":clips_indexed,"run_period_s":reference.run_period,
            "method":"whole-library indexing, kinematic feature extraction, selected runtime reconstruction; no neural training"});
        if std::fs::create_dir_all("profiles").is_ok() {
            let _ = std::fs::write(
                "profiles/rpg_reference.json",
                serde_json::to_string_pretty(&report).unwrap(),
            );
        }
        println!("[reference] {}", reference.note);
        Ok(reference)
    }
}
enum TrackValues {
    Rotation(Vec<Quat>),
    Translation(Vec<Vec3>),
}

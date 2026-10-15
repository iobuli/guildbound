//! gb_lab: headless Bevy + Avian test bench for Guildbound ideas.
//! Scenes: lie (rung 0 gate), stand, sway, clearance. Deterministic, fixed 1/120 s.
mod phys_modal;
mod ragdoll;
mod sway;
mod voxel;

use std::collections::HashMap;

pub const DT: f32 = 1.0 / 120.0;

#[derive(Clone, Debug)]
pub struct Args {
    pub scene: String,
    pub ticks: u32,
    pub kp: f32,
    pub kd: f32,
    pub k: f32,
    pub out: Option<String>,
    pub unpaired: bool,
    pub exact: bool,
    pub leg_floor: f32,
    pub no_gff: bool,
    pub no_filter: bool,
    pub balance: bool,
    pub rest_offset: bool,
    pub no_clamp: bool,
    pub pin_pelvis: bool,
    pub one_leg: bool,
    pub scaled: bool,
    pub cp: bool,
    pub ired_scalar: bool,
    pub damp_rel: bool,
    pub foot_damp: f32,
    pub anchor_k: f32,
    pub bal_rate: f32,
    pub bal_lp: f32,
    pub substeps: u32,
    pub flat_feet: bool,
    pub friction: f32,
    pub old_gff: bool,
    pub geom_edge: bool,
    pub bal_sign: f32,
    pub kb: f32,
    pub kbd: f32,
    pub hip_offset_deg: f32,
}

fn parse_args() -> Args {
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut m: HashMap<String, String> = HashMap::new();
    let mut flags = Vec::new();
    let mut scene = String::from("stand");
    let mut i = 0;
    while i < v.len() {
        let a = &v[i];
        if let Some(key) = a.strip_prefix("--") {
            if matches!(key, "unpaired" | "exact" | "no_gff" | "no_filter" | "balance" | "rest_offset_off" | "no_clamp" | "pin_pelvis" | "one_leg" | "scaled" | "cp" | "geom_edge" | "old_gff" | "flat_feet" | "ired_scalar" | "damp_rel") {
                flags.push(key.to_string());
            } else if i + 1 < v.len() {
                m.insert(key.to_string(), v[i + 1].clone());
                i += 1;
            }
        } else {
            scene = a.clone();
        }
        i += 1;
    }
    let f = |k: &str, d: f32| m.get(k).and_then(|s| s.parse().ok()).unwrap_or(d);
    let default_ticks = match scene.as_str() { "lie" => 240, "stand" => 4800, _ => 1200 };
    Args {
        ticks: m.get("ticks").and_then(|s| s.parse().ok()).unwrap_or(default_ticks),
        kp: f("kp", 400.0),
        kd: f("kd", 40.0),
        k: f("k", 1.2),
        out: m.get("out").cloned(),
        unpaired: flags.iter().any(|s| s == "unpaired"),
        exact: flags.iter().any(|s| s == "exact"),
        leg_floor: f("leg_floor", 0.0),
        no_gff: flags.iter().any(|s| s == "no_gff"),
        no_filter: flags.iter().any(|s| s == "no_filter"),
        balance: flags.iter().any(|s| s == "balance"),
        rest_offset: !flags.iter().any(|s| s == "rest_offset_off"),
        no_clamp: flags.iter().any(|s| s == "no_clamp"),
        pin_pelvis: flags.iter().any(|s| s == "pin_pelvis"),
        old_gff: flags.iter().any(|s| s == "old_gff"),
        friction: f("friction", 0.8),
        flat_feet: flags.iter().any(|s| s == "flat_feet"),
        substeps: m.get("substeps").and_then(|s| s.parse().ok()).unwrap_or(6),
        foot_damp: f("foot_damp", 0.0),
        anchor_k: f("anchor_k", 0.0),
        bal_rate: f("bal_rate", 0.0),
        bal_lp: f("bal_lp", 1.0),
        ired_scalar: flags.iter().any(|s| s == "ired_scalar"),
        damp_rel: flags.iter().any(|s| s == "damp_rel"),
        cp: flags.iter().any(|s| s == "cp"),
        geom_edge: flags.iter().any(|s| s == "geom_edge"),
        bal_sign: f("bal_sign", 1.0),
        kb: f("kb", 3.0),
        kbd: f("kbd", 0.6),
        scaled: flags.iter().any(|s| s == "scaled"),
        one_leg: flags.iter().any(|s| s == "one_leg"),
        hip_offset_deg: f("hip_offset_deg", 0.0),
        scene,
    }
}

fn main() {
    let args = parse_args();
    let summary = match args.scene.as_str() {
        "lie" => ragdoll::run(&args, true),
        "stand" => ragdoll::run(&args, false),
        "sway" => sway::run(&args),
        "clearance" => voxel::run(&args),
        s => {
            eprintln!("unknown scene '{s}'. use: lie | stand | sway | clearance");
            std::process::exit(2);
        }
    };
    println!("{summary}");
}

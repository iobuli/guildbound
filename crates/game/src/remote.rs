//! Claude's side of the interface: `gb.*` methods on the Bevy Remote Protocol (JSON-RPC 2.0 over HTTP,
//! 127.0.0.1:15702). The built-in `world.*` methods stay available too. Client: Tools/gb.py.

use bevy::prelude::*;
use bevy::remote::{BrpError, BrpResult, RemotePlugin, error_codes};
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use guildbound_sim as gs;
use serde_json::{Value, json};

use crate::anim::{AnimLibrary, AnimRequests};
use crate::feed::DevFeed;
use crate::sim_bridge::{OpponentMode, Scripted, Sim};

#[derive(Resource, Default)]
pub struct RemoteActivity {
    pub calls: u64,
    pub last_method: String,
    pub last_at: f64,
}

pub fn plugin() -> RemotePlugin {
    RemotePlugin::default()
        .with_method_main("gb.state", gb_state)
        .with_method_main("gb.control", gb_control)
        .with_method_main("gb.input", gb_input)
        .with_method_main("gb.tuning", gb_tuning)
        .with_method_main("gb.say", gb_say)
        .with_method_main("gb.feed", gb_feed)
        .with_method_main("gb.screenshot", gb_screenshot)
        .with_method_main("gb.clips", gb_clips)
        .with_method_main("gb.anim", gb_anim)
}

fn gb_clips(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.clips");
    let names: Vec<String> = world.get_resource::<AnimLibrary>().map(|l| l.clips.iter().map(|(n, _)| n.clone()).collect()).unwrap_or_default();
    Ok(json!({ "count": names.len(), "clips": names }))
}

/// {clip, fighter?: 0}. Plays the clip once, then locomotion resumes.
fn gb_anim(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.anim");
    let p = p.ok_or_else(|| bad("params required"))?;
    let clip = p.get("clip").and_then(Value::as_str).ok_or_else(|| bad("clip required"))?.to_owned();
    let fighter = p.get("fighter").and_then(Value::as_u64).unwrap_or(0) as usize;
    if world.get_resource::<AnimLibrary>().is_none_or(|l| l.node(&clip).is_none()) {
        return Err(bad(format!("unknown clip {clip} (see gb.clips)")));
    }
    world.resource_mut::<AnimRequests>().0.push((fighter, clip.clone()));
    Ok(json!({ "fighter": fighter, "clip": clip }))
}

fn touch(world: &mut World, method: &str) -> Value {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let mut a = world.resource_mut::<RemoteActivity>();
    a.calls += 1;
    a.last_method = method.into();
    a.last_at = now;
    Value::Null
}

fn bad(message: impl Into<String>) -> BrpError {
    BrpError { code: error_codes::INVALID_PARAMS, message: message.into(), data: None }
}

fn f32_of(p: &Value, key: &str) -> Option<f32> {
    p.get(key).and_then(Value::as_f64).map(|v| v as f32)
}

fn state_json(sim: &Sim) -> Value {
    let t = &sim.world.tuning;
    json!({
        "tick": sim.world.tick,
        "seconds": sim.world.tick as f32 * gs::STEP,
        "paused": sim.paused,
        "speed": sim.speed,
        "hash": format!("{:016x}", sim.world.state_hash()),
        "opponent": format!("{:?}", sim.opponent).to_lowercase(),
        "fighters": sim.world.fighters.iter().enumerate().map(|(i, f)| json!({
            "index": i,
            "position": [f.position.x, f.position.z],
            "velocity": [f.velocity.x, f.velocity.z],
            "facing": [f.facing.x, f.facing.z],
            "hp": f.hp, "max_hp": f.max_hp,
            "lock_target": f.lock_target,
            "scripted_ticks_left": sim.scripted[i].map(|s| s.ticks_left),
        })).collect::<Vec<_>>(),
        "tuning": {
            "walk_speed": t.walk_speed, "run_speed": t.run_speed, "turn_rate": t.turn_rate,
            "body_radius": t.body_radius, "arena_half_size": t.arena_half_size,
        },
    })
}

fn gb_state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.state");
    Ok(state_json(world.resource::<Sim>()))
}

/// {pause?: bool, step?: n, speed?: f, reset?: bool, opponent?: "idle"|"chase", lock?: bool}
fn gb_control(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.control");
    let p = p.unwrap_or_default();
    let mut sim = world.resource_mut::<Sim>();
    if p.get("reset").and_then(Value::as_bool) == Some(true) {
        sim.reset();
    }
    if let Some(v) = p.get("pause").and_then(Value::as_bool) {
        sim.paused = v;
    }
    if let Some(v) = f32_of(&p, "speed") {
        sim.speed = v.clamp(0.05, 4.0);
    }
    if let Some(v) = p.get("opponent").and_then(Value::as_str) {
        sim.opponent = match v {
            "idle" => OpponentMode::Idle,
            "chase" => OpponentMode::Chase,
            _ => return Err(bad("opponent must be idle or chase")),
        };
    }
    if let Some(v) = p.get("lock").and_then(Value::as_bool) {
        sim.world.fighters[0].lock_target = v.then_some(1);
    }
    if let Some(n) = p.get("step").and_then(Value::as_u64) {
        if !sim.paused {
            return Err(bad("step needs the sim paused (send pause: true first)"));
        }
        sim.pending_steps += n as u32;
    }
    Ok(state_json(&sim))
}

/// {fighter?: 0, move: [x, z], run?: false, ticks?: 60}. Overrides that fighter's intent for N sim ticks.
fn gb_input(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.input");
    let p = p.ok_or_else(|| bad("params required"))?;
    let i = p.get("fighter").and_then(Value::as_u64).unwrap_or(0) as usize;
    let m = p.get("move").and_then(Value::as_array).ok_or_else(|| bad("move: [x, z] required"))?;
    let (x, z) = match (m.first().and_then(Value::as_f64), m.get(1).and_then(Value::as_f64)) {
        (Some(x), Some(z)) => (x as f32, z as f32),
        _ => return Err(bad("move: [x, z] must be numbers")),
    };
    let ticks = p.get("ticks").and_then(Value::as_u64).unwrap_or(60).max(1) as u32;
    let run = p.get("run").and_then(Value::as_bool).unwrap_or(false);
    let mut sim = world.resource_mut::<Sim>();
    let slot = sim.scripted.get_mut(i).ok_or_else(|| bad(format!("no fighter {i}")))?;
    *slot = Some(Scripted { intent: gs::Intent { move_dir: gs::V2::new(x, z), run }, ticks_left: ticks });
    Ok(json!({ "fighter": i, "ticks": ticks }))
}

/// Any subset of {walk_speed, run_speed, turn_rate, body_radius, arena_half_size}, or {defaults: true}.
fn gb_tuning(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.tuning");
    let p = p.unwrap_or_default();
    let mut sim = world.resource_mut::<Sim>();
    let t = &mut sim.world.tuning;
    if p.get("defaults").and_then(Value::as_bool) == Some(true) {
        *t = gs::Tuning::default();
    }
    for (key, field) in [
        ("walk_speed", &mut t.walk_speed),
        ("run_speed", &mut t.run_speed),
        ("turn_rate", &mut t.turn_rate),
        ("body_radius", &mut t.body_radius),
        ("arena_half_size", &mut t.arena_half_size),
    ] {
        if let Some(v) = f32_of(&p, key) {
            *field = v;
        }
    }
    Ok(state_json(&sim)["tuning"].clone())
}

/// {text, who?: "claude"}. Shows in the panel feed and collab/devfeed.log.
fn gb_say(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.say");
    let p = p.ok_or_else(|| bad("params required"))?;
    let text = p.get("text").and_then(Value::as_str).ok_or_else(|| bad("text required"))?.to_owned();
    let who = p.get("who").and_then(Value::as_str).unwrap_or("claude").to_owned();
    let tick = world.resource::<Sim>().world.tick;
    let mut feed = world.resource_mut::<DevFeed>();
    feed.post(&who, &text, tick);
    Ok(json!({ "index": feed.entries.len() - 1 }))
}

/// {since?: n}. Entries from index n on, plus `next` to pass as `since` next time.
fn gb_feed(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.feed");
    let since = p.as_ref().and_then(|p| p.get("since")).and_then(Value::as_u64).unwrap_or(0) as usize;
    let feed = world.resource::<DevFeed>();
    let entries: Vec<Value> = feed.entries.iter().enumerate().skip(since)
        .map(|(i, e)| json!({ "index": i, "who": e.who, "text": e.text, "tick": e.tick, "stamp": e.stamp }))
        .collect();
    Ok(json!({ "entries": entries, "next": feed.entries.len() }))
}

/// {path?: "Documentation/checks/live/latest.png"} relative to the repo root. Written a frame or two later.
fn gb_screenshot(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.screenshot");
    let rel = p.as_ref().and_then(|p| p.get("path")).and_then(Value::as_str).unwrap_or("Documentation/checks/live/latest.png").to_owned();
    let root = std::path::Path::new(crate::ROOT).canonicalize().map_err(BrpError::internal)?;
    let path = root.join(&rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(BrpError::internal)?;
    }
    let _ = std::fs::remove_file(&path);
    world.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
    Ok(json!({ "path": rel, "absolute": path.to_string_lossy() }))
}

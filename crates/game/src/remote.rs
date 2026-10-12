//! Claude's side of the interface: `gb.*` methods on the Bevy Remote Protocol (JSON-RPC 2.0 over HTTP,
//! 127.0.0.1:15702). The built-in `world.*` methods stay available too. Client: Tools/gb.py.

use bevy::prelude::*;
use bevy::remote::{BrpError, BrpResult, RemotePlugin, error_codes};
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use guildbound_sim as gs;
use serde_json::{Value, json};

use crate::anim::{AnimLibrary, AnimRequests, Libraries};
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
        .with_method_main("gb.library", gb_library)
        .with_method_main("gb.presentation", gb_presentation)
        .with_method_main("gb.menu_pointer", gb_menu_pointer)
        .with_method_main("gb.physics", gb_physics)
        .with_method_main("gb.support", gb_support)
        .with_method_main("gb.guard", gb_guard)
        .with_method_main("gb.studio", gb_studio)
        .with_method_main("gb.possess", gb_possess)
        .with_method_main("gb.loot", gb_loot)
}

/// Studio state and the boss's notes/corrections/patterns. Optional {open, spawn, clip, time, hero_clips, rules, select}.
fn gb_studio(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.studio");
    if let Some(p) = p {
        let open = p.get("open").and_then(Value::as_bool);
        {
            let mut s = world.resource_mut::<crate::studio::Studio>();
            if let Some(v) = p.get("spawn").and_then(Value::as_str) { s.request_spawn(v.to_owned()); }
            if let Some(v) = p.get("clip").and_then(Value::as_str) { s.clip = Some(v.to_owned()); s.time = 0.0; }
            if let Some(v) = p.get("time").and_then(Value::as_f64) { s.time = v as f32; s.playing = false; }
            if let Some(v) = p.get("hero_clips").and_then(Value::as_bool) { s.hero_clips = v; }
            if let Some(v) = p.get("rules").and_then(Value::as_bool) { s.rules = v; }
            if let Some(v) = p.get("select").and_then(Value::as_str) { s.selected = Some(v.to_owned()); }
        }
        if let Some(o) = open && o != world.resource::<crate::studio::Studio>().open {
            world.resource_mut::<crate::studio::Studio>().toggle_request = true;
        }
    }
    Ok(crate::studio::snapshot(world))
}

/// Possession: bone overrides/aims, fixed-fps stepping, life layers, physics. Params (all optional):
/// {enable, fixed, fps, step, frame, set:{bone:[x,y,z]}, absolute, aim:{bone:[x,y,z]}, clear: true|[bones],
///  grip:[l,r]|null, look:"off"|"camera"|"wander"|[x,y,z], lean, breath, mass_kg, save:"correction", pose:NAME, bones:true}
fn gb_possess(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.possess");
    let p = p.unwrap_or(Value::Null);
    let status = crate::possession::command(world, &p);
    let mut out = crate::possession::snapshot(world, p.get("bones").and_then(Value::as_bool).unwrap_or(false));
    out["status"] = Value::String(status);
    Ok(out)
}

fn gb_guard(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.guard");
    Ok(crate::guard_ik::snapshot(world))
}

/// Player kit, pickups and chests.
fn gb_loot(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    use guildbound_sim::rpg::ITEMS;
    touch(world, "gb.loot");
    let w = &world.resource::<crate::sim_bridge::Sim>().world;
    let name = |id: &u8| ITEMS[*id as usize].name;
    let f = &w.fighters[0];
    let st = f.kit.stats();
    Ok(serde_json::json!({
        "bag": f.kit.bag.iter().map(name).collect::<Vec<_>>(),
        "worn": f.kit.worn.iter().map(|w| w.as_ref().map(name)).collect::<Vec<_>>(),
        "stats": {"power": st.power, "armor": st.armor, "vitality": st.vitality, "hp": f.hp, "max_hp": f.max_hp},
        "pickups": w.pickups.iter().map(|p| serde_json::json!({"item": name(&p.item), "at": [p.position.x, p.position.z]})).collect::<Vec<_>>(),
        "chests": w.chests.iter().map(|c| serde_json::json!({"at": [c.position.x, c.position.z], "open": c.open, "loot": c.loot.iter().map(name).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "reachable": format!("{:?}", w.reachable(0)),
    }))
}

fn gb_support(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.support");
    if let Some(enabled) = p.as_ref().and_then(|p| p["enabled"].as_bool()) { world.resource_mut::<crate::support::SupportSettings>().enabled = enabled; }
    Ok(crate::support::snapshot(world))
}

fn gb_physics(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.physics");
    Ok(crate::ragdoll::snapshot(world))
}

/// Exercise the actual menu Pointer<Over/Out/Click> observers, not a state shortcut.
/// {button?: displayed name, event?: "over"|"out"|"click"}; no button lists actions.
fn gb_menu_pointer(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    use bevy::picking::{backend::HitData, pointer::{Location, PointerButton, PointerId}};
    touch(world, "gb.menu_pointer");
    let mut query = world.query_filtered::<(Entity, &Name), With<crate::menu::MenuButton>>();
    let buttons: Vec<_> = query.iter(world).map(|(e, n)| (e, n.to_string())).collect();
    if let Some(name) = p.as_ref().and_then(|p| p["button"].as_str()) {
        let entity = buttons.iter().find(|(_, n)| n == name).map(|(e, _)| *e).ok_or_else(|| bad("button is not in the active menu"))?;
        let event = p.as_ref().and_then(|p| p["event"].as_str()).unwrap_or("over");
        if name == "Quit" && event == "click" { return Err(bad("Quit click is excluded from the live regression probe")); }
        let window = world.query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().iter(world).next().ok_or_else(|| bad("no primary window"))?;
        let camera = world.query_filtered::<Entity, With<bevy_lunex::UiSourceCamera<0>>>().iter(world).next().ok_or_else(|| bad("no menu camera"))?;
        let location = Location { target: bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)).normalize(Some(window)).ok_or_else(|| bad("no window target"))?, position: Vec2::ZERO };
        let hit = HitData::new(camera, 0.0, None, None);
        match event {
            "over" => world.trigger(Pointer::new(PointerId::Mouse, location, Over { hit }, entity)),
            "out" => world.trigger(Pointer::new(PointerId::Mouse, location, Out { hit }, entity)),
            "click" => world.trigger(Pointer::new(PointerId::Mouse, location, Click { button: PointerButton::Primary, hit, duration: std::time::Duration::from_millis(80), count: 1 }, entity)),
            _ => return Err(bad("event must be over, out or click")),
        }
        world.flush();
    }
    Ok(json!({"buttons": buttons.iter().map(|(_, name)| name).collect::<Vec<_>>()}))
}

/// Read-only animation/pose inspection for controller smoke checks.
fn gb_presentation(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.presentation");
    Ok(crate::anim::snapshot(world))
}

/// {select?: name or index}. Lists the converted packs; selecting one swaps every fighter's model and clips.
fn gb_library(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.library");
    let mut libs = world.resource_mut::<Libraries>();
    if let Some(sel) = p.as_ref().and_then(|p| p.get("select")) {
        let i = match sel {
            Value::Number(n) => n.as_u64().map(|n| n as usize),
            Value::String(name) => libs.list.iter().position(|l| &l.name == name),
            _ => None,
        }
        .filter(|&i| i < libs.list.len())
        .ok_or_else(|| bad("select must be a pack name or index from the list"))?;
        libs.requested = Some(i);
    }
    let names: Vec<&str> = libs.list.iter().map(|l| l.name.as_str()).collect();
    Ok(json!({ "packs": names, "selected": libs.selected, "requested": libs.requested }))
}

fn gb_clips(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.clips");
    let Some(lib) = world.get_resource::<AnimLibrary>() else { return Ok(json!({ "count": 0, "clips": [] })) };
    let names: Vec<&String> = lib.clips.iter().map(|(n, _)| n).collect();
    let r = &lib.roles;
    let clusters: Vec<Value> = lib.clusters.iter().map(|(n, limb, c)| json!({ "cluster": n, "limb": limb, "clips": c })).collect();
    Ok(json!({
        "count": names.len(), "clips": names,
        "roles": { "idle": r.idle, "walk": r.walk, "run": r.run, "back": r.back, "hit": r.hit, "knockdown": r.knockdown, "getup": r.getup },
        "clusters": clusters,
    }))
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
        "projectiles": sim.world.projectiles.iter().filter(|p| p.alive).map(|p| json!({"owner": p.owner, "spell": gs::SPELLS[p.spell].name, "position": [p.position.x, p.position.z], "charge": p.charge})).collect::<Vec<_>>(),
        "fighters": sim.world.fighters.iter().enumerate().map(|(i, f)| json!({
            "index": i,
            "position": [f.position.x, f.position.z],
            "velocity": [f.velocity.x, f.velocity.z],
            "facing": [f.facing.x, f.facing.z],
            "hp": f.hp, "max_hp": f.max_hp,
            "height": f.height, "vertical": f.vertical, "guard": f.guard, "mana": f.mana,
            "action": crate::panel::action_name(f), "action_elapsed": f.action.elapsed,
            "chain": f.chain, "heavy_charge": f.heavy_charge,
            "lock_target": f.lock_target,
            "scripted_ticks_left": sim.scripted[i].map(|s| s.ticks_left),
        })).collect::<Vec<_>>(),
        "tuning": {
            "walk_speed": t.walk_speed, "run_speed": t.run_speed, "turn_rate": t.turn_rate,
            "body_radius": t.body_radius, "arena_half_size": t.arena_half_size,
            "accel": t.accel, "decel": t.decel, "reverse_brake": t.reverse_brake,
            "model": format!("{:?}", t.model).to_lowercase(), "drag_accel": t.drag_accel, "coast_half_life": t.coast_half_life,
        },
    })
}

fn gb_state(In(_): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.state");
    let mut state = state_json(world.resource::<Sim>());
    state["app_state"] = json!(format!("{:?}", world.resource::<State<crate::menu::AppState>>().get()));
    state["dev_panel"] = json!(!world.resource::<crate::panel::PanelState>().hidden);
    Ok(state)
}

/// {pause?: bool, step?: n, speed?: f, reset?: bool, opponent?: "idle"|"chase", lock?: bool}
fn gb_control(In(p): In<Option<Value>>, world: &mut World) -> BrpResult {
    touch(world, "gb.control");
    let p = p.unwrap_or_default();
    match p.get("state").and_then(Value::as_str) {
        Some("play") => world.resource_mut::<NextState<crate::menu::AppState>>().set(crate::menu::AppState::Playing),
        Some("menu") => world.resource_mut::<NextState<crate::menu::AppState>>().set(crate::menu::AppState::MainMenu),
        Some(_) => return Err(bad("state must be play or menu")),
        None => {}
    }
    if let Some(v) = p.get("blend_space").and_then(Value::as_bool) {
        world.resource_mut::<crate::anim::AnimSettings>().blend_space = v;
    }
    if let Some(v) = p.get("joint_rule").and_then(Value::as_bool) {
        world.get_resource_or_insert_with(crate::joints::JointAuthority::default).enabled = v;
    }
    if let Some(v) = p.get("knee_rule").and_then(Value::as_bool) {
        world.get_resource_or_insert_with(crate::knee::KneeStats::default).enabled = v;
    }
    if let Some(hidden) = p.get("dev_panel").and_then(Value::as_bool) {
        world.resource_mut::<crate::panel::PanelState>().hidden = !hidden;
    }
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
            "spar" => OpponentMode::Spar,
            _ => return Err(bad("opponent must be idle or chase")),
        };
    }
    if let Some(v) = p.get("lock").and_then(Value::as_bool) {
        sim.world.fighters[0].lock_target = v.then_some(1);
    }
    // after any reset, so the new mobs survive it
    if let Some(n) = p.get("spawn_mobs").and_then(Value::as_u64) {
        for j in 0..n {
            let at = gs::V2::new(4.0 + j as f32 * 1.4, -4.0 + (j % 2) as f32 * 1.6);
            sim.spawn_mob(at);
        }
    }
    if let Some(n) = p.get("step").and_then(Value::as_u64) {
        if !sim.paused {
            return Err(bad("step needs the sim paused (send pause: true first)"));
        }
        sim.pending_steps += n as u32;
    }
    Ok(state_json(&sim))
}

/// {fighter?: 0, move: [x, z], run?, light?, heavy?, step?, guard?, ticks?: 60}. Overrides that fighter's intent for N sim ticks; presses fire on the first tick.
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
    let flag = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
    let cast = p.get("cast").and_then(Value::as_u64).map(|s| s as u8);
    if cast.is_some_and(|s| s as usize >= gs::SPELLS.len()) { return Err(bad("cast must be a spell slot 0..3")); }
    let intent = gs::Intent { move_dir: gs::V2::new(x, z), run, light: flag("light"), heavy: flag("heavy"), heavy_held: flag("heavy_held"), step: flag("step"), jump: flag("jump"), roll: flag("roll"), guard: flag("guard"), guard_pressed: flag("guard"), cast, cast_held: flag("cast_held"), ..Default::default() };
    *slot = Some(Scripted { intent, ticks_left: ticks });
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
    match p.get("model").and_then(Value::as_str) {
        Some("drag") => t.model = gs::MotorModel::Drag,
        Some("linear") => t.model = gs::MotorModel::Linear,
        Some(_) => return Err(bad("model must be drag or linear")),
        None => {}
    }
    for (key, field) in [
        ("walk_speed", &mut t.walk_speed),
        ("run_speed", &mut t.run_speed),
        ("turn_rate", &mut t.turn_rate),
        ("body_radius", &mut t.body_radius),
        ("arena_half_size", &mut t.arena_half_size),
        ("accel", &mut t.accel),
        ("decel", &mut t.decel),
        ("reverse_brake", &mut t.reverse_brake),
        ("drag_accel", &mut t.drag_accel),
        ("coast_half_life", &mut t.coast_half_life),
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

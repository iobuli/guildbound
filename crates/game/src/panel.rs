//! The boss's dev panel: sim controls, fighters, tuning, view toggles, Claude link, shared feed.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::anim::{AnimLibrary, AnimRequests, AnimSettings, Libraries};
use crate::camera::OrbitCamera;
use crate::feed::DevFeed;
use crate::remote::RemoteActivity;
use crate::sim_bridge::{OpponentMode, Sim};

#[derive(Resource)]
pub struct PanelState {
    /// Logical pixels, so the camera can ignore the mouse over the panel.
    pub width: f32,
    pub show_grid: bool,
    pub show_velocity: bool,
    pub show_bodies: bool,
    pub show_capsules: bool,
    /// Tab: hide the dev panel (play view); I: inventory window.
    pub hidden: bool,
    pub show_inventory: bool,
    draft: String,
    clip: String,
    clip_target: usize,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            width: 0.0,
            show_grid: false,
            show_velocity: false,
            show_bodies: false,
            show_capsules: false,
            hidden: true,
            show_inventory: false,
            draft: String::new(),
            clip: "hp_straight_left_A".into(),
            clip_target: 0,
        }
    }
}

pub fn panel(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut feed: ResMut<DevFeed>,
    mut state: ResMut<PanelState>,
    remote: Res<RemoteActivity>,
    time: Res<Time<Real>>,
    mut cam: Single<&mut OrbitCamera>,
    view: Single<(&Camera, &GlobalTransform), With<OrbitCamera>>,
    lib: Option<Res<AnimLibrary>>,
    mut anim: ResMut<AnimSettings>,
    mut requests: ResMut<AnimRequests>,
    mut libs: ResMut<Libraries>,
    app_state: Res<State<crate::menu::AppState>>,
    mut feedback: ResMut<crate::feedback::FeedbackSettings>,
    mut sfx: ResMut<crate::sfx::SfxSettings>,
    hud: crate::hud::HudParams,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if *app_state.get() == crate::menu::AppState::MainMenu {
        // the menu owns the screen: no HUD, no dev panel
        state.width = 0.0;
        return Ok(());
    }
    let left = if state.hidden { 0.0 } else { state.width };
    crate::hud::overhead_bars(&ctx, &sim, view.0, view.1);
    crate::hud::damage_numbers(&ctx, &sim, view.0, view.1);
    if !hud.studio.open && crate::hud::draw(&ctx, &sim, left, hud.icons.as_deref(), &hud.layout) {
        sim.reset();
        sim.opponent = OpponentMode::Spar;
        sim.world.fighters[0].lock_target = Some(1);
        sim.paused = false;
    }
    if state.show_inventory {
        let mut open = true;
        egui::Window::new("Inventory").open(&mut open).default_pos([state.width + 20.0, 80.0]).show(&ctx, |ui| {
            if let Some(action) = inventory_ui(ui, &sim) {
                sim.edges.bag = Some(action);
            }
        });
        state.show_inventory = open;
    }
    if state.hidden {
        state.width = 0.0;
        return Ok(());
    }
    let mut root = egui::Ui::new(
        ctx.clone(),
        "gb_root".into(),
        egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()),
    );
    let width = egui::Panel::left("gb_dev")
        .default_size(340.0)
        .resizable(true)
        .show_inside(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Guildbound dev");
                ui.label("WASD move, Shift run, Ctrl step, Shift+Ctrl homing dash (8 ki) | LMB rush, RMB guard/parry, F/Mouse4 smash (hold to charge; W/S rising/sweep), Mouse5 lock");
                ui.label("Camera: middle-drag orbit, wheel zoom | P pause, . step | Tab hide panel, I inventory | Space jump, C roll | 1 fireball (hold = charge), 2 ki blast, 3 lightning, 4 burst, Q charge ki | E pick up / open");
                ui.separator();
                sim_section(ui, &mut sim);
                ui.separator();
                fighters_section(ui, &mut sim, &mut cam);
                ui.separator();
                tuning_section(ui, &mut sim);
                ui.separator();
                ui.label(egui::RichText::new("View").strong());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.show_grid, "grid");
                    ui.checkbox(&mut state.show_velocity, "velocity");
                    ui.checkbox(&mut state.show_bodies, "bodies");
                    ui.checkbox(&mut state.show_capsules, "capsules");
                });
                ui.add(egui::Slider::new(&mut feedback.shake_scale, 0.0..=1.0).text("camera shake"));
                ui.add(egui::Slider::new(&mut sfx.volume, 0.0..=1.0).text("sound volume"));
                ui.separator();
                anim_section(ui, &mut libs, lib.as_deref(), &mut anim, &mut requests, &mut state);
                ui.separator();
                link_section(ui, &remote, time.elapsed_secs_f64());
                ui.separator();
                feed_section(ui, &mut feed, &mut state.draft, sim.world.tick);
            });
        })
        .response
        .rect
        .width();
    state.width = width;
    Ok(())
}

fn sim_section(ui: &mut egui::Ui, sim: &mut Sim) {
    ui.label(egui::RichText::new("Simulation (1/120 s fixed step)").strong());
    ui.horizontal(|ui| {
        if ui.button(if sim.paused { "Resume" } else { "Pause" }).clicked() {
            sim.paused = !sim.paused;
        }
        ui.add_enabled_ui(sim.paused, |ui| {
            if ui.button("Step 1").clicked() {
                sim.pending_steps += 1;
            }
            if ui.button("Step 10").clicked() {
                sim.pending_steps += 10;
            }
        });
        if ui.button("Reset").clicked() {
            sim.reset();
        }
    });
    ui.add(egui::Slider::new(&mut sim.speed, 0.05..=2.0).text("speed").logarithmic(true));
    ui.monospace(format!("tick {}   t {:.2} s", sim.world.tick, sim.world.tick as f32 * guildbound_sim::STEP));
    ui.monospace(format!("state hash {:016x}", sim.world.state_hash()));
}

fn fighters_section(ui: &mut egui::Ui, sim: &mut Sim, cam: &mut OrbitCamera) {
    ui.label(egui::RichText::new("Fighters").strong());
    ui.horizontal(|ui| {
        ui.label("Opponent:");
        ui.radio_value(&mut sim.opponent, OpponentMode::Idle, "idle");
        ui.radio_value(&mut sim.opponent, OpponentMode::Chase, "chase");
        ui.radio_value(&mut sim.opponent, OpponentMode::Spar, "spar (fights back)");
    });
    let mut lock = sim.world.fighters[0].lock_target.is_some();
    if ui.checkbox(&mut lock, "Player locks on opponent").changed() {
        sim.world.fighters[0].lock_target = lock.then_some(1);
    }
    ui.checkbox(&mut cam.follow_player, "Camera follows player");
    egui::Grid::new("fighters").striped(true).show(ui, |ui| {
        for h in ["#", "pos x", "pos z", "speed", "action", "hp", "guard"] {
            ui.label(h);
        }
        ui.end_row();
        for (i, f) in sim.world.fighters.iter().enumerate() {
            ui.label(fighter_name(i));
            ui.monospace(format!("{:+.2}", f.position.x));
            ui.monospace(format!("{:+.2}", f.position.z));
            ui.monospace(format!("{:.2}", f.velocity.length()));
            ui.monospace(action_name(f));
            ui.monospace(format!("{:.0}/{:.0}", f.hp, f.max_hp));
            ui.monospace(format!("{:.0}", f.guard));
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Spawn mob").clicked() {
            let k = sim.world.fighters.len() as f32;
            let at = guildbound_sim::V2::new((k * 2.3).sin() * 6.0, (k * 1.7).cos() * 6.0);
            sim.spawn_mob(at);
        }
        if ui.button("Spawn pack (3)").clicked() {
            for j in 0..3 {
                let at = guildbound_sim::V2::new(5.0 + j as f32, -5.0 + (j % 2) as f32 * 1.5);
                sim.spawn_mob(at);
            }
        }
        if ui.button("Heal all").clicked() {
            for f in &mut sim.world.fighters {
                f.hp = f.max_hp;
                f.guard = sim.world.combat.guard_max;
            }
        }
    });
    egui::CollapsingHeader::new("Contacts (latest last)").default_open(true).show(ui, |ui| {
        for (c, _) in &sim.recent {
            let who = fighter_name;
            ui.monospace(format!("t{} {} {} -> {} {:?}", c.tick, who(c.attacker), source_name(c.source), who(c.defender), c.outcome));
        }
    });
}

pub fn source_name(s: guildbound_sim::Source) -> &'static str {
    match s {
        guildbound_sim::Source::Move(m) => guildbound_sim::MOVES[m].name,
        guildbound_sim::Source::Spell(sp) => guildbound_sim::SPELLS[sp].name,
    }
}

pub fn fighter_name(i: usize) -> String {
    match i {
        0 => "you".into(),
        1 => "opp".into(),
        n => format!("mob {}", n - 1),
    }
}

pub fn action_name(f: &guildbound_sim::Fighter) -> String {
    use guildbound_sim::ActionKind as K;
    match f.action.kind {
        K::None => "free".into(),
        K::Strike(m) => format!("{} ({:?})", guildbound_sim::MOVES[m].name, f.action.phase()).to_lowercase(),
        K::HeavyCharge(m) => format!("smash {} {:.0}%", guildbound_sim::MOVES[m].name, f.heavy_charge * 100.0),
        K::Cast(s) => format!("cast {}", guildbound_sim::SPELLS[s].name),
        K::Charge(s) => format!("charging {} {:.0}%", guildbound_sim::SPELLS[s].name, f.charge * 100.0),
        k => format!("{k:?}").to_lowercase(),
    }
}

fn tuning_section(ui: &mut egui::Ui, sim: &mut Sim) {
    ui.label(egui::RichText::new("Tuning (live)").strong());
    let t = &mut sim.world.tuning;
    egui::Grid::new("tuning").show(ui, |ui| {
        ui.label("walk m/s");
        ui.add(egui::DragValue::new(&mut t.walk_speed).speed(0.05).range(0.1..=10.0));
        ui.end_row();
        ui.label("run m/s");
        ui.add(egui::DragValue::new(&mut t.run_speed).speed(0.05).range(0.1..=15.0));
        ui.end_row();
        ui.label("turn deg/s");
        ui.add(egui::DragValue::new(&mut t.turn_rate).speed(5.0).range(30.0..=2000.0));
        ui.end_row();
        ui.label("body radius m");
        ui.add(egui::DragValue::new(&mut t.body_radius).speed(0.01).range(0.1..=1.0));
        ui.end_row();
        ui.label("arena half m");
        ui.add(egui::DragValue::new(&mut t.arena_half_size).speed(0.1).range(2.0..=80.0));
        ui.end_row();
        ui.label("motor");
        ui.horizontal(|ui| {
            ui.radio_value(&mut t.model, guildbound_sim::MotorModel::Drag, "drag (Overgrowth-style)");
            ui.radio_value(&mut t.model, guildbound_sim::MotorModel::Linear, "linear");
        });
        ui.end_row();
        ui.label("drag push m/s2");
        ui.add(egui::DragValue::new(&mut t.drag_accel).speed(0.5).range(2.0..=150.0));
        ui.end_row();
        ui.label("coast half-life s");
        ui.add(egui::DragValue::new(&mut t.coast_half_life).speed(0.005).range(0.02..=2.0));
        ui.end_row();
        ui.label("linear accel m/s2");
        ui.add(egui::DragValue::new(&mut t.accel).speed(0.2).range(1.0..=80.0));
        ui.end_row();
        ui.label("decel (slide) m/s2");
        ui.add(egui::DragValue::new(&mut t.decel).speed(0.2).range(1.0..=80.0));
        ui.end_row();
        ui.label("reverse brake m/s2");
        ui.add(egui::DragValue::new(&mut t.reverse_brake).speed(0.2).range(1.0..=120.0));
        ui.end_row();
    });
    if ui.button("Defaults").clicked() {
        sim.world.tuning = guildbound_sim::Tuning::default();
    }
}

fn anim_section(
    ui: &mut egui::Ui,
    libs: &mut Libraries,
    lib: Option<&AnimLibrary>,
    anim: &mut AnimSettings,
    requests: &mut AnimRequests,
    state: &mut PanelState,
) {
    // header: "Animation [<pack v>]" - the dropdown is the model picker (model + its fitted clips)
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Animation [").strong());
        let current = libs.current().map_or("no packs converted".to_owned(), |l| l.name.clone());
        egui::ComboBox::from_id_salt("library").selected_text(current).show_ui(ui, |ui| {
            for (i, entry) in libs.list.iter().enumerate() {
                if ui.selectable_label(i == libs.selected, &entry.name).clicked() {
                    libs.requested = Some(i);
                }
            }
        });
        ui.label(egui::RichText::new("]").strong());
    });
    let Some(lib) = lib.filter(|l| !l.clips.is_empty()) else {
        ui.label("loading model and clips ...");
        return;
    };
    let r = &lib.roles;
    let show = |o: &Option<String>| o.clone().unwrap_or_else(|| "-".into());
    ui.label(format!("{} clips | stance {} | walk {} | run {} | back {}", lib.clips.len(), show(&r.idle), show(&r.walk), show(&r.run), show(&r.back)));
    egui::CollapsingHeader::new("Combat clusters (keys rotate variants)").show(ui, |ui| {
        for ((name, limb, clips), spec) in lib.clusters.iter().zip(crate::anim::CLUSTER_SPECS) {
            let list = if clips.is_empty() { "none in this pack".to_owned() } else { clips.join(", ") };
            ui.label(format!("{} {name} ({limb}): {list}", spec.2));
        }
    });
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("clip").selected_text(&state.clip).width(170.0).show_ui(ui, |ui| {
            for (name, _) in &lib.clips {
                ui.selectable_value(&mut state.clip, name.clone(), name);
            }
        });
        ui.radio_value(&mut state.clip_target, 0, "you");
        ui.radio_value(&mut state.clip_target, 1, "opp");
    });
    if ui.button("Play clip once").clicked() {
        requests.0.push((state.clip_target, state.clip.clone()));
    }
    ui.horizontal(|ui| {
        ui.label("stance");
        egui::ComboBox::from_id_salt("stance").selected_text(&anim.stance_idle).show_ui(ui, |ui| {
            for (name, _) in lib.clips.iter().filter(|(n, _)| n.to_lowercase().contains("idle")) {
                ui.selectable_value(&mut anim.stance_idle, name.clone(), name);
            }
        });
    });
    ui.checkbox(&mut anim.inertialize, "smooth transitions (inertialize from last pose)");
    ui.add(egui::Slider::new(&mut anim.reference_speed, 0.5..=4.0).text("walk clip m/s at 1x"));
    ui.add(egui::Slider::new(&mut anim.run_reference_speed, 1.0..=8.0).text("run clip m/s at 1x"));
    ui.add(egui::Slider::new(&mut anim.blend_seconds, 0.02..=0.5).text("blend s"));
    ui.add(egui::Slider::new(&mut anim.lean_per_accel, 0.0..=0.05).text("lean rad per m/s2"));
    ui.add(egui::Slider::new(&mut anim.lean_per_speed, 0.0..=0.06).text("lean rad per m/s (run lean)"));
}

fn link_section(ui: &mut egui::Ui, remote: &RemoteActivity, now: f64) {
    ui.label(egui::RichText::new("Claude link").strong());
    ui.monospace("http://127.0.0.1:15702  (Tools/gb.py)");
    if remote.calls == 0 {
        ui.label("no remote calls yet");
    } else {
        ui.label(format!("{} calls, last: {} ({:.0} s ago)", remote.calls, remote.last_method, now - remote.last_at));
    }
}

fn feed_section(ui: &mut egui::Ui, feed: &mut DevFeed, draft: &mut String, tick: u64) {
    ui.label(egui::RichText::new("Feed (boss <-> Claude)").strong());
    egui::ScrollArea::vertical().id_salt("feed").max_height(260.0).stick_to_bottom(true).show(ui, |ui| {
        for e in &feed.entries {
            let color = match e.who.as_str() {
                "boss" => egui::Color32::from_rgb(255, 210, 120),
                "claude" => egui::Color32::from_rgb(150, 200, 255),
                _ => egui::Color32::GRAY,
            };
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(color, format!("{} {}:", &e.stamp[..5], e.who));
                ui.label(&e.text);
            });
        }
    });
    let mut send = false;
    ui.horizontal(|ui| {
        let edit = ui.add(egui::TextEdit::singleline(draft).hint_text("message to Claude").desired_width(220.0));
        send |= edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        send |= ui.button("Send").clicked();
    });
    if send && !draft.trim().is_empty() {
        feed.post("boss", draft.trim(), tick);
        draft.clear();
    }
}

/// Equipment slots, totals and the bag. Returns the click as a sim action (applied on the next tick).
fn inventory_ui(ui: &mut egui::Ui, sim: &Sim) -> Option<guildbound_sim::rpg::BagAction> {
    use guildbound_sim::rpg::{BAG_SIZE, BagAction, ITEMS, ItemKind, SLOTS};
    let f = &sim.world.fighters[0];
    let st = f.kit.stats();
    let mut act = None;
    ui.label(egui::RichText::new("Equipped").strong());
    egui::Grid::new("worn").num_columns(2).show(ui, |ui| {
        for slot in SLOTS {
            ui.label(format!("{slot:?}"));
            match f.kit.worn_in(slot) {
                Some(id) => {
                    if ui.button(ITEMS[id as usize].name).on_hover_text("click to unequip").clicked() {
                        act = Some(BagAction::Unequip(slot));
                    }
                }
                None => { ui.weak("-"); }
            }
            ui.end_row();
        }
    });
    ui.label(format!("Power +{:.0}%   Armor {:.0}   Health {:.0} / {:.0}", st.power, st.armor, f.hp, f.max_hp));
    ui.separator();
    ui.label(egui::RichText::new(format!("Bag  {} / {BAG_SIZE}", f.kit.bag.len())).strong());
    if f.kit.bag.is_empty() {
        ui.weak("Empty. Walk over items and press E.");
    }
    for (k, &id) in f.kit.bag.iter().enumerate() {
        let def = &ITEMS[id as usize];
        ui.horizontal(|ui| {
            let (verb, info) = match def.kind {
                ItemKind::Consumable { heal, ki } => ("Use", if heal > 0.0 { format!("+{heal:.0} hp") } else { format!("+{ki:.0} ki") }),
                ItemKind::Gear(slot) => ("Equip", format!("{slot:?}  pow {:.0} arm {:.0} vit {:.0}", def.mods.power, def.mods.armor, def.mods.vitality)),
            };
            ui.label(def.name);
            ui.weak(info);
            if ui.small_button(verb).clicked() {
                act = Some(BagAction::Use(k as u8));
            }
            if ui.small_button("Drop").clicked() {
                act = Some(BagAction::Drop(k as u8));
            }
        });
    }
    act
}

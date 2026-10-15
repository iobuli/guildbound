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
    /// What the panel edits: 0 game rules, 1 the world, 2 debug views.
    mode: usize,
    pub show_inventory: bool,
    pub inventory: crate::inventory::InventoryUi,
    draft: String,
    clip: String,
    clip_target: usize,
}

impl PanelState {
    pub fn editing_world(&self) -> bool {
        !self.hidden && self.mode == 1
    }
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
            mode: 0,
            show_inventory: false,
            inventory: Default::default(),
            draft: String::new(),
            clip: "hp_straight_left_A".into(),
            clip_target: 0,
        }
    }
}

/// Settings and the world editor, bundled to stay within the system parameter limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Tools<'w, 's> {
    commands: Commands<'w, 's>,
    assets: Res<'w, AssetServer>,
    edit: ResMut<'w, crate::editor::WorldEdit>,
    feedback: ResMut<'w, crate::feedback::FeedbackSettings>,
    sfx: ResMut<'w, crate::sfx::SfxSettings>,
    toon: ResMut<'w, crate::toon::ToonLook>,
}

const MODES: [&str; 3] = ["Game", "World", "Debug"];
const MODE_HELP: [&str; 3] = [
    "Editing the game rules: the simulation, the fighters and how everyone moves.",
    "Editing the world: fly the camera freely and place objects on the ground.",
    "Debug views: overlays, the animation library, the link to Claude and the message feed.",
];

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
    mut tools: Tools,
    hud: crate::hud::HudParams,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    crate::ui_theme::style(&ctx);
    if *app_state.get() == crate::menu::AppState::MainMenu || hud.studio.open {
        // the menu owns the screen: no HUD, no dev panel
        state.width = 0.0;
        return Ok(());
    }
    let left = if state.hidden { 0.0 } else { state.width };
    if !state.show_inventory {
        crate::hud::overhead_bars(&ctx, &sim, view.0, view.1);
        crate::hud::damage_numbers(&ctx, &sim, view.0, view.1);
        if crate::hud::draw(&ctx, &sim, left, hud.icons.as_deref(), &hud.layout) {
            sim.reset();
            sim.opponent = OpponentMode::Spar;
            sim.world.fighters[0].lock_target = Some(1);
            sim.paused = false;
        }
        egui::Area::new("player_tools".into()).anchor(egui::Align2::RIGHT_TOP, [-20.0, 20.0]).show(&ctx, |ui| {
            ui.horizontal(|ui| {
                for (tab, label) in [(0, "Inventory  [I]"), (1, "Combos  [H]")] {
                    if ui.button(label).clicked() {
                        state.inventory.tab = tab;
                        state.show_inventory = true;
                        sim.clear_player_input();
                    }
                }
            });
        });
    }
    if state.show_inventory {
        let (action, close) = crate::inventory::show(&ctx, &sim, &mut state.inventory, hud.icons.as_deref());
        if let Some(action) = action {
            sim.edges.bag = Some(action);
            if sim.paused { sim.pending_steps += 1; }
        }
        if close { state.show_inventory = false; sim.clear_player_input(); }
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
                crate::ui_theme::tabs(ui, &MODES, &mut state.mode);
                ui.label(egui::RichText::new(MODE_HELP[state.mode.min(2)]).strong());
                ui.separator();
                match state.mode {
                    0 => {
                        ui.label("WASD move, Shift run, Ctrl step, Shift+Ctrl homing dash | LMB rush, RMB guard/parry, F/Mouse4 smash (hold to charge), Mouse5 lock | Space jump, C roll | 1-5 spells, R/T items | E pick up / open | P pause, . step | Tab this panel, I inventory");
                        ui.separator();
                        sim_section(ui, &mut sim);
                        ui.separator();
                        fighters_section(ui, &mut sim, &mut cam);
                        ui.separator();
                        tuning_section(ui, &mut sim);
                    }
                    1 => {
                        let was = cam.noclip;
                        let props = hud.studio.props();
                        crate::editor::ui(ui, &mut tools.edit, &mut cam.noclip, &props, &mut tools.commands, &tools.assets);
                        if cam.noclip && !was {
                            sim.clear_player_input();
                        }
                    }
                    _ => {
                        ui.label(egui::RichText::new("View").strong());
                        ui.checkbox(&mut tools.toon.on, "Toon lighting (F7)");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut state.show_grid, "grid");
                            ui.checkbox(&mut state.show_velocity, "movement arrows").on_hover_text("Show arrows for where each character is moving.");
                            ui.checkbox(&mut state.show_bodies, "physics shapes").on_hover_text("Show the physics shapes of the bodies.");
                            ui.checkbox(&mut state.show_capsules, "collision capsules").on_hover_text("Show the capsules used for character collisions.");
                        });
                        ui.add(egui::Slider::new(&mut tools.feedback.shake_scale, 0.0..=1.0).text("camera shake")).on_hover_text("How much the camera shakes when hits land.");
                        ui.add(egui::Slider::new(&mut tools.sfx.volume, 0.0..=1.0).text("sound volume")).on_hover_text("Volume of the game sounds.");
                        ui.separator();
                        anim_section(ui, &mut libs, lib.as_deref(), &mut anim, &mut requests, &mut state);
                        ui.separator();
                        link_section(ui, &remote, time.elapsed_secs_f64());
                        ui.separator();
                        feed_section(ui, &mut feed, &mut state.draft, sim.world.tick);
                    }
                }
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

pub fn tuning_section(ui: &mut egui::Ui, sim: &mut Sim) {
    ui.label(egui::RichText::new("Tuning (live)").strong());
    let t = &mut sim.world.tuning;
    egui::Grid::new("tuning").show(ui, |ui| {
        ui.label("walk m/s");
        ui.add(egui::DragValue::new(&mut t.walk_speed).speed(0.05).range(0.1..=10.0));
        ui.end_row();
        ui.label("run m/s");
        ui.add(egui::DragValue::new(&mut t.run_speed).speed(0.05).range(0.1..=15.0));
        ui.end_row();
        ui.label("sprint m/s");
        ui.add(egui::DragValue::new(&mut t.sprint_speed).speed(0.05).range(0.1..=20.0));
        ui.end_row();
        ui.label("relaxed out of combat");
        ui.checkbox(&mut t.relaxed, "relaxed movement").on_hover_text("Walk normally. Shift runs; sustained forward momentum builds into sprint outside combat.");
        ui.end_row();
        ui.label("turn smoothness 1/s");
        ui.add(egui::DragValue::new(&mut t.turn_stiffness).speed(0.2).range(2.0..=60.0));
        ui.end_row();
        ui.label("travel follows facing");
        ui.add(egui::Slider::new(&mut t.steer_follow, 0.0..=1.0));
        ui.end_row();
        ui.label("sprint buildup at run speed s");
        ui.add(egui::DragValue::new(&mut t.sprint_after_s).speed(0.05).range(0.0..=5.0));
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
    anim_tuning(ui, anim);
}

/// Animation tuning: shared by the dev panel and the Studio's Tuning tab.
pub fn anim_tuning(ui: &mut egui::Ui, anim: &mut AnimSettings) {
    ui.checkbox(&mut anim.inertialize, "smooth transitions").on_hover_text("Ease out of the old pose when the animation changes, instead of snapping.");
    ui.checkbox(&mut anim.blend_space, "blend walking directions").on_hover_text("Mix the walk animations for directions in between (off: use the closest single animation).");
    ui.add(egui::Slider::new(&mut anim.reference_speed, 0.5..=4.0).text("walk animation speed")).on_hover_text("The ground speed the walk animation was made for; keeps the feet from sliding.");
    ui.add(egui::Slider::new(&mut anim.run_reference_speed, 1.0..=8.0).text("run animation speed")).on_hover_text("The ground speed the run animation was made for; keeps the feet from sliding.");
    ui.add(egui::Slider::new(&mut anim.blend_seconds, 0.02..=0.5).text("fade-in time (s)")).on_hover_text("Seconds to fade from the previous animation into this one.");
    ui.add(egui::Slider::new(&mut anim.lean_per_accel, 0.0..=0.05).text("lean when speeding up")).on_hover_text("Extra lean while speeding up or slowing down.");
    ui.add(egui::Slider::new(&mut anim.lean_per_speed, 0.0..=0.06).text("run lean")).on_hover_text("How far a running character leans for each unit of speed.");
    ui.add(egui::Slider::new(&mut anim.lean_max, 0.0..=0.5).text("maximum lean")).on_hover_text("The furthest the body may lean (0.3 is about 17 degrees).");
    if ui.button("Animation defaults").clicked() {
        *anim = AnimSettings::default();
    }
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

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
    draft: String,
    clip: String,
    clip_target: usize,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            width: 340.0,
            show_grid: true,
            show_velocity: true,
            show_bodies: false,
            show_capsules: false,
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
    lib: Option<Res<AnimLibrary>>,
    mut anim: ResMut<AnimSettings>,
    mut requests: ResMut<AnimRequests>,
    mut libs: ResMut<Libraries>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let mut root = egui::Ui::new(
        ctx.clone(),
        "gb_root".into(),
        egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()),
    );
    let width = egui::Panel::left("gb_dev")
        .default_size(340.0)
        .resizable(true)
        .show(&mut root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Guildbound dev");
                ui.label("WASD move (camera-relative), Shift run, right-drag orbit, wheel zoom, Space pause, . step");
                ui.label("J jab, K cross, U hook, I uppercut, L roundhouse, O side kick, Y run punch | opp: H hit, N knockdown, G get up");
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
    });
    let mut lock = sim.world.fighters[0].lock_target.is_some();
    if ui.checkbox(&mut lock, "Player locks on opponent").changed() {
        sim.world.fighters[0].lock_target = lock.then_some(1);
    }
    ui.checkbox(&mut cam.follow_player, "Camera follows player");
    egui::Grid::new("fighters").striped(true).show(ui, |ui| {
        for h in ["#", "pos x", "pos z", "speed", "facing", "hp", "script"] {
            ui.label(h);
        }
        ui.end_row();
        for (i, f) in sim.world.fighters.iter().enumerate() {
            ui.label(if i == 0 { "you" } else { "opp" });
            ui.monospace(format!("{:+.2}", f.position.x));
            ui.monospace(format!("{:+.2}", f.position.z));
            ui.monospace(format!("{:.2}", f.velocity.length()));
            ui.monospace(format!("{:+.0} deg", f.facing.x.atan2(f.facing.z).to_degrees()));
            ui.monospace(format!("{:.0}/{:.0}", f.hp, f.max_hp));
            ui.monospace(sim.scripted[i].map_or("-".into(), |s| format!("{} t", s.ticks_left)));
            ui.end_row();
        }
    });
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
        ui.add(egui::DragValue::new(&mut t.arena_half_size).speed(0.1).range(2.0..=20.0));
        ui.end_row();
        ui.label("accel m/s2");
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

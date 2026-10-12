//! Player-facing sparring HUD. Reads live rules; returns a rematch request to
//! the input owner instead of changing simulation data from presentation.
use bevy_egui::egui;
use crate::sim_bridge::Sim;

const INK: egui::Color32 = egui::Color32::from_rgb(18, 25, 27);
const PAPER: egui::Color32 = egui::Color32::from_rgb(236, 230, 212);
const GOLD: egui::Color32 = egui::Color32::from_rgb(242, 194, 88);
const BLUE: egui::Color32 = egui::Color32::from_rgb(112, 181, 225);

fn frame() -> egui::Frame {
    egui::Frame::new().fill(INK.gamma_multiply(0.92)).inner_margin(12.0).corner_radius(4.0)
}

pub fn draw(ctx: &egui::Context, sim: &Sim, left: f32) -> bool {
    let available = (ctx.viewport_rect().width() - left).max(240.0);
    let width = ((available - 72.0) * 0.5).clamp(100.0, 280.0);
    let player = &sim.world.fighters[0];
    let target = player.lock_target.filter(|&i| i != 0 && i < sim.world.fighters.len()).unwrap_or(1);
    egui::Area::new("fight_vitals".into()).anchor(egui::Align2::CENTER_TOP, [left * 0.5, 18.0]).interactable(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (i, f) in [(0, player), (target, &sim.world.fighters[target])] {
                frame().show(ui, |ui| {
                    ui.vertical(|ui| {
                    ui.set_width(width);
                    let name = if i == 0 { "YOU".to_owned() } else if i == 1 { "SPARRING PARTNER".to_owned() } else { format!("OPPONENT {}", i - 1) };
                    ui.label(egui::RichText::new(name).size(14.0).strong().color(if i == 0 { BLUE } else { GOLD }));
                    ui.add(egui::ProgressBar::new((f.hp / f.max_hp.max(0.01)).clamp(0.0, 1.0)).desired_width(width).desired_height(18.0).fill(egui::Color32::from_rgb(170, 65, 52)).text(format!("HEALTH  {:.0} / {:.0}", f.hp, f.max_hp)));
                    ui.add(egui::ProgressBar::new((f.guard / sim.world.combat.guard_max.max(0.01)).clamp(0.0, 1.0)).desired_width(width).desired_height(12.0).fill(BLUE).text("GUARD"));
                    let pk = (f.posture / sim.world.posture.max.max(0.01)).clamp(0.0, 1.0);
                    ui.add(egui::ProgressBar::new(pk).desired_width(width).desired_height(10.0).fill(posture_color(pk)).text(if f.action.kind == guildbound_sim::ActionKind::Dizzy { "POSTURE BROKEN" } else { "POSTURE" }));
                    ui.add(egui::ProgressBar::new((f.ki / sim.world.magic.ki_max.max(0.01)).clamp(0.0, 1.0)).desired_width(width).desired_height(12.0).fill(GOLD).text(format!("ENERGY  {:.0}", f.ki)));
                    });
                });
                if i == 0 { ui.add_space(16.0); }
            }
        });
    });
    egui::Area::new("fight_tools".into()).anchor(egui::Align2::CENTER_BOTTOM, [left * 0.5, -18.0]).interactable(false).show(ctx, |ui| {
        frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, spell) in guildbound_sim::SPELLS.iter().enumerate() {
                    let ready = player.ki >= spell.cost && player.spell_cooldown <= 0.0;
                    let text = format!("[{}] {}", i + 1, spell.name.to_uppercase());
                    ui.label(egui::RichText::new(text).size(13.0).color(if ready { PAPER } else { egui::Color32::GRAY }));
                    if i < 3 { ui.add_space(12.0); }
                }
            });
            ui.label(egui::RichText::new("LMB rush   F / M4 smash (hold)   W/S + smash finisher   RMB guard / parry").size(12.0).color(BLUE));
            ui.label(egui::RichText::new("WASD move   SHIFT run   CTRL step   SHIFT+CTRL dash (8 ki)   SPACE jump   C roll   ESC menu").size(11.0).color(PAPER));
        });
    });
    let now = sim.world.tick as f32 * guildbound_sim::STEP;
    let status = if player.counter_left > 0.0 {
        Some("COUNTER OPEN - PRESS LIGHT")
    } else if sim.paused {
        Some("PAUSED - P TO RESUME")
    } else {
        sim.recent.iter().rev().find(|(c, at)| (c.attacker == 0 || c.defender == 0) && now - at < 0.65).map(|(c, _)| crate::feedback::profile(c.outcome).label)
    };
    if let Some(r) = sim.world.reachable(0) {
        use guildbound_sim::rpg::{ITEMS, Reachable};
        let what = match r {
            Reachable::Pickup(k) => format!("[E] PICK UP  {}", ITEMS[sim.world.pickups[k].item as usize].name.to_uppercase()),
            Reachable::Chest(_) => "[E] OPEN CHEST".to_owned(),
        };
        egui::Area::new("interact_prompt".into()).anchor(egui::Align2::CENTER_CENTER, [left * 0.5, 120.0]).interactable(false).show(ctx, |ui| {
            frame().show(ui, |ui| { ui.label(egui::RichText::new(what).strong().size(15.0).color(PAPER)); });
        });
    }
    egui::Area::new("fight_status".into()).anchor(egui::Align2::CENTER_BOTTOM, [left * 0.5, -112.0]).interactable(false).show(ctx, |ui| {
        ui.set_width(300.0);
        if let Some(status) = status {
            frame().show(ui, |ui| { ui.label(egui::RichText::new(status).strong().size(16.0).color(GOLD)); });
        }
        if matches!(player.action.kind, guildbound_sim::ActionKind::Charge(_)) {
            ui.add(egui::ProgressBar::new(player.charge).desired_width(220.0).fill(GOLD).text(format!("CHARGING  {:.0}%", player.charge * 100.0)));
        }
        if let guildbound_sim::ActionKind::Drink(id) = player.action.kind {
            let k = (player.action.elapsed / player.action.length.max(0.01)).clamp(0.0, 1.0);
            ui.add(egui::ProgressBar::new(k).desired_width(220.0).fill(egui::Color32::from_rgb(90, 200, 120)).text(format!("DRINKING  {}", guildbound_sim::rpg::ITEMS[id as usize].name.to_uppercase())));
        }
        if matches!(player.action.kind, guildbound_sim::ActionKind::HeavyCharge(_)) {
            ui.add(egui::ProgressBar::new(player.heavy_charge).desired_width(260.0).fill(GOLD).text(format!("SMASH  {:.0}%", player.heavy_charge * 100.0)));
        } else if let guildbound_sim::ActionKind::Strike(m) = player.action.kind {
            let label = if guildbound_sim::MOVES[m].heavy { "FINISHER".to_owned() } else { format!("RUSH {} / 4", (player.chain + 1).min(4)) };
            ui.label(egui::RichText::new(format!("{label} - {}", crate::panel::action_name(player))).color(PAPER));
        }
    });
    let mut restart = false;
    if player.hp <= 0.0 || sim.world.fighters[1].hp <= 0.0 {
        egui::Area::new("fight_result".into()).anchor(egui::Align2::CENTER_CENTER, [left * 0.5, 0.0]).show(ctx, |ui| {
            ui.set_width(330.0);
            frame().show(ui, |ui| {
                ui.label(egui::RichText::new(if player.hp <= 0.0 { "YOU WERE DOWNED" } else { "SPAR COMPLETE" }).size(26.0).strong().color(PAPER));
                ui.label(egui::RichText::new("Reset the floor and try the exchange again.").color(BLUE));
                restart = ui.button("Spar again").clicked();
            });
        });
    }
    restart
}

/// Health bars over every other living fighter's head (projected from the 3D camera).
pub fn overhead_bars(ctx: &egui::Context, sim: &Sim, camera: &bevy::prelude::Camera, cam_tf: &bevy::prelude::GlobalTransform) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    for f in sim.world.fighters.iter().skip(1) {
        if f.hp <= 0.0 { continue; }
        let head = bevy::prelude::Vec3::new(f.position.x, f.height + 2.15, f.position.z);
        let Ok(p) = camera.world_to_viewport(cam_tf, head) else { continue };
        let (w, h) = (70.0, 7.0);
        let r = egui::Rect::from_center_size(egui::pos2(p.x, p.y), egui::vec2(w, h));
        painter.rect_filled(r.expand(1.5), 2.0, INK.gamma_multiply(0.85));
        let k = (f.hp / f.max_hp.max(0.01)).clamp(0.0, 1.0);
        painter.rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(w * k, h)), 2.0, egui::Color32::from_rgb(190, 70, 55));
        // posture: grows from the centre outwards, Sekiro style
        let pk = (f.posture / sim.world.posture.max.max(0.01)).clamp(0.0, 1.0);
        if pk > 0.01 {
            let pr = egui::Rect::from_center_size(r.center_bottom() + egui::vec2(0.0, 5.0), egui::vec2(w * pk, 3.0));
            painter.rect_filled(pr, 1.0, posture_color(pk));
        }
        // awareness: a '?' that fills while suspicious, a red '!' when alert
        if let Some(n) = f.notice {
            use guildbound_sim::NoticeState;
            let at = r.center_top() - egui::vec2(0.0, 3.0);
            match n.state {
                NoticeState::Alert => { painter.text(at, egui::Align2::CENTER_BOTTOM, "!", egui::FontId::proportional(18.0), egui::Color32::from_rgb(230, 60, 50)); }
                NoticeState::Suspicious => { painter.text(at, egui::Align2::CENTER_BOTTOM, "?", egui::FontId::proportional(16.0), GOLD.gamma_multiply(0.4 + 0.6 * (n.level - 1.0).clamp(0.0, 1.0))); }
                NoticeState::Unaware if n.level > 0.02 => { painter.text(at, egui::Align2::CENTER_BOTTOM, "?", egui::FontId::proportional(13.0), PAPER.gamma_multiply(n.level.clamp(0.0, 1.0))); }
                NoticeState::Unaware => {}
            }
        }
    }
}

fn posture_color(k: f32) -> egui::Color32 {
    let k = k.clamp(0.0, 1.0);
    egui::Color32::from_rgb(240, (200.0 - 150.0 * k) as u8, (80.0 - 60.0 * k) as u8)
}

/// Damage numbers at each contact point: rise and fade; crits are red, larger and punch in scale
/// instead of rising (UE5 melee showcase idea). Presentation only.
pub fn damage_numbers(ctx: &egui::Context, sim: &Sim, camera: &bevy::prelude::Camera, cam_tf: &bevy::prelude::GlobalTransform) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let now = sim.world.tick as f32 * guildbound_sim::STEP;
    for (c, at) in &sim.recent {
        let age = now - at;
        if !(0.0..0.9).contains(&age) || (c.damage <= 0.0 && c.outcome != guildbound_sim::Outcome::Deathblow) {
            continue;
        }
        let Some(d) = sim.world.fighters.get(c.defender) else { continue };
        let rise = if c.crit { 0.0 } else { 0.6 * age };
        let p3 = bevy::prelude::Vec3::new(c.point.x, d.height + 1.7 + rise, c.point.z);
        let Ok(p) = camera.world_to_viewport(cam_tf, p3) else { continue };
        let fade = (1.0 - age / 0.9).clamp(0.0, 1.0);
        let (text, size, color) = match c.outcome {
            guildbound_sim::Outcome::Deathblow => ("DEATHBLOW".to_owned(), 26.0, egui::Color32::from_rgb(220, 30, 30)),
            _ if c.crit => (format!("{:.0}!", c.damage), 22.0 * (1.0 + 0.5 * (1.0 - (age / 0.12).min(1.0))), egui::Color32::from_rgb(235, 45, 40)),
            _ => (format!("{:.0}", c.damage), 15.0, PAPER),
        };
        let pos = egui::pos2(p.x, p.y);
        painter.text(pos + egui::vec2(1.5, 1.5), egui::Align2::CENTER_CENTER, &text, egui::FontId::proportional(size), egui::Color32::BLACK.gamma_multiply(fade));
        painter.text(pos, egui::Align2::CENTER_CENTER, &text, egui::FontId::proportional(size), color.gamma_multiply(fade));
    }
}

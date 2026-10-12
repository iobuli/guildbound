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
    egui::Area::new("fight_status".into()).anchor(egui::Align2::CENTER_BOTTOM, [left * 0.5, -112.0]).interactable(false).show(ctx, |ui| {
        ui.set_width(300.0);
        if let Some(status) = status {
            frame().show(ui, |ui| { ui.label(egui::RichText::new(status).strong().size(16.0).color(GOLD)); });
        }
        if matches!(player.action.kind, guildbound_sim::ActionKind::Charge(_)) {
            ui.add(egui::ProgressBar::new(player.charge).desired_width(220.0).fill(GOLD).text(format!("CHARGING  {:.0}%", player.charge * 100.0)));
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

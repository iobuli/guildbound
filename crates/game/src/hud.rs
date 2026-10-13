//! Player-facing sparring HUD. Reads live rules; returns a rematch request to
//! the input owner instead of changing simulation data from presentation.
use bevy_egui::egui;
use crate::sim_bridge::Sim;

use crate::ui_theme::{BORDER, CARD, GLASS, MUTED, PAPER};

const INK: egui::Color32 = egui::Color32::from_rgb(10, 10, 12);
const GOLD: egui::Color32 = egui::Color32::from_rgb(242, 194, 88);
const BLUE: egui::Color32 = egui::Color32::from_rgb(112, 181, 225);

fn frame() -> egui::Frame {
    egui::Frame::new().fill(GLASS).stroke(egui::Stroke::new(1.0_f32, BORDER)).inner_margin(12.0).corner_radius(8.0)
}

/// Spell and potion icons (boss's UI kit), registered with egui once.
#[derive(bevy::prelude::Resource)]
pub struct HudIcons {
    pub spells: [egui::TextureId; guildbound_sim::magic::SPELL_COUNT],
    pub potion_red: egui::TextureId,
    pub potion_blue: egui::TextureId,
}

pub fn load_icons(mut commands: bevy::prelude::Commands, mut contexts: bevy_egui::EguiContexts, assets: bevy::prelude::Res<bevy::prelude::AssetServer>) {
    let mut add = |name: &str| contexts.add_image(bevy_egui::EguiTextureHandle::Strong(assets.load(format!("packs/_sources/ui/hud/{name}.png"))));
    let spells = ["fireball", "chain", "nova", "blink", "heal"].map(&mut add);
    commands.insert_resource(HudIcons { spells, potion_red: add("potion_red"), potion_blue: add("potion_blue") });
}

const TEAL: egui::Color32 = crate::ui_theme::TEAL;
const RED: egui::Color32 = egui::Color32::from_rgb(206, 58, 52);
const MANA: egui::Color32 = egui::Color32::from_rgb(70, 140, 240);
const UV: egui::Rect = egui::Rect { min: egui::pos2(0.0, 0.0), max: egui::pos2(1.0, 1.0) };

/// A flat bar: label and value above a thin rounded track.
fn bar(p: &egui::Painter, rect: egui::Rect, k: f32, fill: egui::Color32, label: &str, value: &str) {
    let track = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 6.0), rect.max);
    if !label.is_empty() || !value.is_empty() {
        p.text(rect.left_top(), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(11.0), MUTED);
        p.text(rect.right_top(), egui::Align2::RIGHT_TOP, value, egui::FontId::monospace(11.0), PAPER);
    }
    p.rect_filled(track, 3.0, CARD);
    let w = track.width() * k.clamp(0.0, 1.0);
    if w > 0.5 {
        p.rect_filled(egui::Rect::from_min_size(track.min, egui::vec2(w, track.height())), 3.0, fill);
    }
}

/// Everything the HUD needs beyond the sim, bundled so the panel system stays within its parameter limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct HudParams<'w> {
    pub icons: Option<bevy::prelude::Res<'w, HudIcons>>,
    pub layout: bevy::prelude::Res<'w, crate::studio::HudLayout>,
    pub studio: bevy::prelude::Res<'w, crate::studio::Studio>,
}

pub fn draw(ctx: &egui::Context, sim: &Sim, left: f32, icons: Option<&HudIcons>, layout: &crate::studio::HudLayout) -> bool {
    let screen = ctx.viewport_rect();
    let player = &sim.world.fighters[0];
    let w = &sim.world;
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, "hud".into()));
    let in_combat = w.in_combat(0);

    // vitals card, bottom left
    let sc = layout.scale.clamp(0.5, 2.0);
    let card = egui::Rect::from_min_size(egui::pos2(left + 18.0 + layout.vitals[0], screen.bottom() - 150.0 * sc + layout.vitals[1]), egui::vec2(330.0, 132.0) * sc);
    p.rect(card, 10.0, GLASS, egui::Stroke::new(1.0_f32, BORDER), egui::StrokeKind::Inside);
    let mode = if in_combat { ("Combat", RED) } else { ("Exploring", TEAL) };
    p.text(card.min + egui::vec2(16.0, 12.0), egui::Align2::LEFT_TOP, "Ranger", egui::FontId::proportional(14.0), PAPER);
    let tag = p.layout_no_wrap(mode.0.to_owned(), egui::FontId::proportional(11.0), mode.1);
    let pill = egui::Rect::from_min_size(egui::pos2(card.right() - 16.0 - tag.size().x - 16.0, card.top() + 11.0), tag.size() + egui::vec2(16.0, 4.0));
    p.rect(pill, 9.0, mode.1.gamma_multiply(0.15), egui::Stroke::new(1.0_f32, mode.1.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    p.galley(pill.center() - tag.size() * 0.5, tag, mode.1);
    let row = |k: f32| egui::Rect::from_min_size(card.min + egui::vec2(16.0, 36.0 + k * 22.0) * sc, egui::vec2(card.width() - 32.0 * sc, 20.0 * sc));
    bar(&p, row(0.0), player.hp / player.max_hp.max(1.0), RED, "HEALTH", &format!("{:.0} / {:.0}", player.hp, player.max_hp));
    bar(&p, row(1.0), player.mana / w.magic.mana_max.max(1.0), MANA, "MANA", &format!("{:.0} / {:.0}", player.mana, w.magic.mana_max));
    bar(&p, row(2.0), player.guard / w.combat.guard_max.max(1.0), TEAL, "GUARD", "");
    let pk = (player.posture / w.posture.max.max(1.0)).clamp(0.0, 1.0);
    bar(&p, row(3.0), pk, posture_color(pk), if player.action.kind == guildbound_sim::ActionKind::Dizzy { "POSTURE BROKEN" } else { "POSTURE" }, "");

    // skill bar + quick slots, bottom centre
    let cell = 58.0 * sc;
    let gap = 8.0 * sc;
    let n = guildbound_sim::magic::SPELL_COUNT as f32;
    let total = n * cell + (n - 1.0) * gap + 2.0 * (cell + gap) + 14.0;
    let x0 = left + (screen.width() - left - total) * 0.5 + layout.skills[0];
    let y0 = screen.bottom() - cell - 26.0 + layout.skills[1];
    for (s, spell) in guildbound_sim::SPELLS.iter().enumerate() {
        let r = egui::Rect::from_min_size(egui::pos2(x0 + s as f32 * (cell + gap), y0), egui::vec2(cell, cell));
        let cd = player.cooldowns[s];
        let ready = player.mana >= spell.cost && cd <= 0.0;
        let charging = matches!(player.action.kind, guildbound_sim::ActionKind::Charge(c) | guildbound_sim::ActionKind::Cast(c) if c == s);
        p.rect(r, 8.0, GLASS, egui::Stroke::new(if charging { 1.5_f32 } else { 1.0 }, if charging { GOLD } else { BORDER }), egui::StrokeKind::Inside);
        if let Some(i) = icons {
            p.image(i.spells[s], r.shrink(7.0), UV, if ready { egui::Color32::WHITE } else { egui::Color32::from_gray(90) });
        }
        if cd > 0.0 {
            let k = (cd / spell.cooldown.max(0.01)).clamp(0.0, 1.0);
            p.rect_filled(egui::Rect::from_min_max(egui::pos2(r.left() + 5.0, r.bottom() - 5.0 - (r.height() - 10.0) * k), r.max - egui::vec2(5.0, 5.0)), 4.0, egui::Color32::from_black_alpha(150));
            p.text(r.center(), egui::Align2::CENTER_CENTER, format!("{cd:.1}"), egui::FontId::proportional(15.0), PAPER);
        }
        p.text(r.left_top() + egui::vec2(6.0, 4.0), egui::Align2::LEFT_TOP, format!("{}", s + 1), egui::FontId::proportional(12.0), GOLD);
        p.text(r.center_bottom() + egui::vec2(0.0, 4.0), egui::Align2::CENTER_TOP, format!("{:.0}", spell.cost), egui::FontId::proportional(10.0), if player.mana >= spell.cost { MANA } else { RED });
        if charging && player.charge > 0.0 {
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(r.left(), r.top() - 7.0), egui::vec2(cell * player.charge, 4.0)), 2.0, GOLD);
        }
    }
    let count = |id: guildbound_sim::rpg::ItemId| player.kit.bag.iter().filter(|&&b| b == id).count();
    for (k, (id, key)) in [(guildbound_sim::rpg::HEALING_DRAUGHT, "R"), (guildbound_sim::rpg::MANA_TONIC, "T")].into_iter().enumerate() {
        let r = egui::Rect::from_min_size(egui::pos2(x0 + n * (cell + gap) + 14.0 + k as f32 * (cell + gap), y0), egui::vec2(cell, cell));
        let c = count(id);
        p.rect(r, 8.0, GLASS, egui::Stroke::new(1.0_f32, BORDER), egui::StrokeKind::Inside);
        if let Some(i) = icons {
            p.image(if k == 0 { i.potion_red } else { i.potion_blue }, r.shrink(10.0), UV, if c > 0 { egui::Color32::WHITE } else { egui::Color32::from_gray(80) });
        }
        p.text(r.left_top() + egui::vec2(6.0, 4.0), egui::Align2::LEFT_TOP, key, egui::FontId::proportional(12.0), GOLD);
        p.text(r.right_bottom() - egui::vec2(6.0, 4.0), egui::Align2::RIGHT_BOTTOM, format!("{c}"), egui::FontId::proportional(13.0), PAPER);
    }
    p.text(egui::pos2(x0 + total * 0.5, screen.bottom() - 8.0), egui::Align2::CENTER_BOTTOM,
        "WASD move  SHIFT sprint  MOUSE5 lock-on  LMB rush  F smash  RMB guard (tap = perfect parry)  CTRL step  C roll  E interact",
        egui::FontId::proportional(11.0), MUTED.gamma_multiply(0.8));

    // target card, top centre, while fighting
    let target = player.lock_target.filter(|&i| i != 0 && i < w.fighters.len()).or_else(|| in_combat.then_some(1).filter(|&i| i < w.fighters.len()));
    if let Some(t) = target {
        let f = &w.fighters[t];
        let r = egui::Rect::from_center_size(egui::pos2(left + (screen.width() - left) * 0.5 + layout.target[0], 46.0 + layout.target[1]), egui::vec2(380.0, 64.0) * sc);
        p.rect(r, 10.0, GLASS, egui::Stroke::new(1.0_f32, BORDER), egui::StrokeKind::Inside);
        let name = if t == 1 { "Sparring partner".to_owned() } else { format!("Hostile {}", t - 1) };
        p.text(r.left_top() + egui::vec2(16.0, 10.0), egui::Align2::LEFT_TOP, name, egui::FontId::proportional(13.0), PAPER);
        let hb = egui::Rect::from_min_size(r.min + egui::vec2(16.0, 28.0), egui::vec2(r.width() - 32.0, 18.0));
        bar(&p, hb, f.hp / f.max_hp.max(1.0), RED, "", &format!("{:.0}", f.hp));
        let tk = (f.posture / w.posture.max.max(1.0)).clamp(0.0, 1.0);
        let pr = egui::Rect::from_center_size(hb.center_bottom() + egui::vec2(0.0, 7.0), egui::vec2(hb.width() * tk, 4.0));
        p.rect_filled(pr, 2.0, posture_color(tk));
    }
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

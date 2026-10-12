//! One UI system for every screen (HUD, Studio, dev panel), derived from the boss's Wenrexa hologram kit:
//! dark glass panels, a teal accent, the kit's button textures for normal/hover/active, section headers and
//! stat rows. Screens compose these pieces instead of styling egui by hand.
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};

pub const TEAL: egui::Color32 = egui::Color32::from_rgb(88, 220, 200);
pub const TEAL_DIM: egui::Color32 = egui::Color32::from_rgb(40, 110, 102);
pub const GLASS: egui::Color32 = egui::Color32::from_rgba_premultiplied(8, 18, 20, 232);
pub const PAPER: egui::Color32 = egui::Color32::from_rgb(222, 236, 232);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(130, 160, 156);
pub const WARN: egui::Color32 = egui::Color32::from_rgb(242, 194, 88);
pub const BAD: egui::Color32 = egui::Color32::from_rgb(232, 84, 72);
pub const GOOD: egui::Color32 = egui::Color32::from_rgb(110, 220, 130);

#[derive(Resource)]
pub struct Kit {
    pub btn: [egui::TextureId; 3],
    pub dock: egui::TextureId,
}

pub fn load(mut commands: Commands, mut contexts: EguiContexts, assets: Res<AssetServer>) {
    let mut add = |name: &str| contexts.add_image(EguiTextureHandle::Strong(assets.load(format!("packs/_sources/ui/hud/{name}.png"))));
    commands.insert_resource(Kit { btn: [add("btn_normal"), add("btn_hover"), add("btn_active")], dock: add("dock") });
}

/// Apply the hologram style to the egui context (once is enough; cheap to repeat).
pub fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = GLASS;
    v.window_fill = GLASS;
    v.window_stroke = egui::Stroke::new(1.0_f32, TEAL_DIM);
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.selection.bg_fill = TEAL_DIM;
    v.selection.stroke = egui::Stroke::new(1.0_f32, TEAL);
    v.hyperlink_color = TEAL;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(28, 60, 58));
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, PAPER);
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(16, 34, 36);
    v.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(16, 34, 36);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, PAPER);
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(22, 62, 60);
    v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(22, 62, 60);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, TEAL);
    v.widgets.active.bg_fill = TEAL_DIM;
    v.widgets.active.weak_bg_fill = TEAL_DIM;
    v.extreme_bg_color = egui::Color32::from_rgb(6, 12, 14);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
    });
}

/// A dock frame: glass fill with the kit's card texture painted behind the content.
pub fn dock(kit: Option<&Kit>) -> egui::Frame {
    let _ = kit;
    egui::Frame::new().fill(GLASS).stroke(egui::Stroke::new(1.0_f32, TEAL_DIM)).inner_margin(10.0).corner_radius(6.0)
}

/// Paint the kit's dock card behind a rect (call before drawing content into it).
pub fn dock_backdrop(ui: &egui::Ui, kit: Option<&Kit>, rect: egui::Rect) {
    if let Some(k) = kit {
        ui.painter().image(k.dock, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::from_white_alpha(90));
    }
}

/// Section header: small caps title with a teal rule.
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(title.to_uppercase()).size(11.0).strong().color(TEAL));
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.left()..=r.right(), r.top(), egui::Stroke::new(1.0_f32, TEAL_DIM));
    ui.add_space(4.0);
}

/// Label/value row, value coloured.
pub fn stat(ui: &mut egui::Ui, label: &str, value: impl Into<String>, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value.into()).size(12.0).monospace().color(color));
        });
    });
}

/// Kit-textured button (normal / hover / pressed states from the Wenrexa kit).
pub fn button(ui: &mut egui::Ui, kit: Option<&Kit>, text: &str, selected: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.5), PAPER);
    let size = egui::vec2((galley.size().x + 28.0).max(84.0), 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let state = if resp.is_pointer_button_down_on() || selected { 2 } else if resp.hovered() { 1 } else { 0 };
    if let Some(k) = kit {
        ui.painter().image(k.btn[state], rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    } else {
        ui.painter().rect_filled(rect, 4.0, if state > 0 { TEAL_DIM } else { egui::Color32::from_rgb(16, 34, 36) });
    }
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, PAPER);
    resp
}

//! One UI system for every screen (HUD, Studio, dev panel): a flat, neutral design-tool look (zinc surfaces,
//! 1 px borders, 6 px radius, one teal accent used sparingly), after the shadcn/gpui-kit style the boss picked.
//! Screens compose these pieces (tabs, sections, stat rows, buttons, badges, cards) instead of styling egui by hand.
use bevy_egui::egui;

pub const TEAL: egui::Color32 = egui::Color32::from_rgb(45, 212, 191);
pub const TEAL_DIM: egui::Color32 = egui::Color32::from_rgb(19, 78, 74);
/// Panel surface.
pub const GLASS: egui::Color32 = egui::Color32::from_rgba_premultiplied(10, 10, 12, 246);
/// Raised surface (cards, inputs, tab strip).
pub const CARD: egui::Color32 = egui::Color32::from_rgb(24, 24, 27);
pub const BORDER: egui::Color32 = egui::Color32::from_rgb(39, 39, 42);
pub const PAPER: egui::Color32 = egui::Color32::from_rgb(244, 244, 245);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(161, 161, 170);
pub const WARN: egui::Color32 = egui::Color32::from_rgb(250, 204, 21);
pub const BAD: egui::Color32 = egui::Color32::from_rgb(248, 113, 113);
pub const GOOD: egui::Color32 = egui::Color32::from_rgb(74, 222, 128);

pub fn style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    let r = egui::CornerRadius::same(6);
    v.panel_fill = GLASS;
    v.window_fill = GLASS;
    v.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.window_corner_radius = egui::CornerRadius::same(8);
    v.window_shadow = egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: egui::Color32::from_black_alpha(110) };
    v.popup_shadow = v.window_shadow;
    v.menu_corner_radius = r;
    v.selection.bg_fill = TEAL_DIM;
    v.selection.stroke = egui::Stroke::new(1.0_f32, TEAL);
    v.hyperlink_color = TEAL;
    v.faint_bg_color = CARD;
    v.extreme_bg_color = egui::Color32::from_rgb(9, 9, 11);
    v.code_bg_color = CARD;
    v.widgets.noninteractive.bg_fill = CARD;
    v.widgets.noninteractive.weak_bg_fill = CARD;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, PAPER);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = BORDER;
    v.widgets.inactive.weak_bg_fill = CARD;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, PAPER);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 52, 58);
    v.widgets.hovered.weak_bg_fill = BORDER;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(82, 82, 91));
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, PAPER);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = egui::Color32::from_rgb(63, 63, 70);
    v.widgets.active.weak_bg_fill = egui::Color32::from_rgb(63, 63, 70);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, TEAL);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, PAPER);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open = v.widgets.active;
    v.slider_trailing_fill = true;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.slider_rail_height = 4.0;
        s.spacing.indent = 14.0;
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    });
}

/// A docked panel: flat surface, hairline border.
pub fn dock() -> egui::Frame {
    egui::Frame::new().fill(GLASS).stroke(egui::Stroke::new(1.0_f32, BORDER)).inner_margin(12.0)
}

/// A raised card for grouping controls.
pub fn card() -> egui::Frame {
    egui::Frame::new().fill(CARD).stroke(egui::Stroke::new(1.0_f32, BORDER)).inner_margin(10.0).corner_radius(8.0)
}

/// Section header: small muted caps over a hairline.
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title.to_uppercase()).size(10.5).strong().color(MUTED));
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.left()..=r.right(), r.top(), egui::Stroke::new(1.0_f32, BORDER));
    ui.add_space(6.0);
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

/// Button: secondary (zinc, bordered) normally, primary (light fill, dark text) when selected.
pub fn button(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    let width = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.5), PAPER).size().x;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width + 24.0, 28.0), egui::Sense::click());
    let (fill, stroke, ink) = if selected {
        (PAPER, PAPER, egui::Color32::from_rgb(24, 24, 27))
    } else if resp.is_pointer_button_down_on() {
        (egui::Color32::from_rgb(63, 63, 70), egui::Color32::from_rgb(82, 82, 91), PAPER)
    } else if resp.hovered() {
        (BORDER, egui::Color32::from_rgb(63, 63, 70), PAPER)
    } else {
        (CARD, BORDER, PAPER)
    };
    ui.painter().rect(rect, 6.0, fill, egui::Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.5), ink);
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Tab strip: a muted track with the active tab raised.
pub fn tabs(ui: &mut egui::Ui, labels: &[&str], at: &mut usize) {
    egui::Frame::new().fill(CARD).stroke(egui::Stroke::new(1.0_f32, BORDER)).corner_radius(8.0).inner_margin(3.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.horizontal(|ui| {
            for (i, t) in labels.iter().enumerate() {
                let on = *at == i;
                let galley = ui.painter().layout_no_wrap((*t).to_owned(), egui::FontId::proportional(12.5), PAPER);
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(galley.size().x + 20.0, 24.0), egui::Sense::click());
                if on {
                    ui.painter().rect(rect, 6.0, egui::Color32::from_rgb(9, 9, 11), egui::Stroke::new(1.0_f32, BORDER), egui::StrokeKind::Inside);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 6.0, BORDER);
                }
                let ink = if on { PAPER } else { MUTED };
                let galley = ui.painter().layout_no_wrap((*t).to_owned(), egui::FontId::proportional(12.5), ink);
                ui.painter().galley(rect.center() - galley.size() * 0.5, galley, ink);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    *at = i;
                }
            }
        });
    });
}

/// Small pill label (status, verdicts, counts).
pub fn badge(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.16)).stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.55))).corner_radius(10.0).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).size(11.0).color(color));
    });
}

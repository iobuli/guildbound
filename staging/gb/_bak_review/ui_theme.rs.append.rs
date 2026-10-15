// ---- Shared panel API (Studio tabs, Physics and Animation panels build against this) ----
pub const ACCENT: egui::Color32 = TEAL;
pub const SPACE: [f32; 3] = [4.0, 8.0, 14.0];
pub const FONT: [f32; 3] = [11.5, 12.5, 14.0];
/// [animated, controller, IK, motor]
pub const DRIVER_COLORS: [egui::Color32; 4] = [
    egui::Color32::from_rgb(140, 150, 145),
    egui::Color32::from_rgb(240, 196, 80),
    egui::Color32::from_rgb(80, 190, 255),
    egui::Color32::from_rgb(245, 105, 95),
];
pub fn driver_index(state: &str) -> usize {
    match state { "controller" => 1, "IK" => 2, s if s.contains("motor") => 3, _ => 0 }
}
pub fn driver_gizmo(state: &str) -> bevy::prelude::Color {
    let c = DRIVER_COLORS[driver_index(state)];
    bevy::prelude::Color::srgb_u8(c.r(), c.g(), c.b())
}
pub fn hover_gizmo() -> bevy::prelude::Color { bevy::prelude::Color::srgb(1.0, 1.0, 1.0) }
pub fn panel<R>(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.add_space(SPACE[1]);
    card().show(ui, |ui| {
        ui.label(egui::RichText::new(title).size(FONT[2]).strong().color(PAPER));
        ui.add_space(SPACE[0]);
        body(ui)
    }).inner
}
pub fn icon_button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    button(ui, &format!("{icon}  {label}"), selected)
}
pub fn danger_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(format!("{icon}  {label}")).size(FONT[1]).strong().color(egui::Color32::WHITE)).fill(egui::Color32::from_rgb(140, 44, 36)).corner_radius(6))
}
pub fn driver_legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (i, n) in ["animated", "controller", "IK", "motor"].iter().enumerate() {
            ui.label(egui::RichText::new(format!("● {n}")).size(FONT[0]).color(DRIVER_COLORS[i]));
        }
    });
}

use super::theme::{TEXT_DIM, TS_LABEL};
use eframe::egui;

pub(super) fn inspector_field(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(label).size(TS_LABEL).color(TEXT_DIM));
    content(ui);
    ui.add_space(4.0);
}

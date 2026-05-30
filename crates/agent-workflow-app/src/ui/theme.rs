#![allow(deprecated)]

use eframe::egui;

pub(super) const ACCENT: egui::Color32 = egui::Color32::from_rgb(76, 148, 255);
pub(super) const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(40, 80, 160);
pub(super) const SUCCESS: egui::Color32 = egui::Color32::from_rgb(34, 176, 125);
pub(super) const DANGER: egui::Color32 = egui::Color32::from_rgb(219, 72, 72);
pub(super) const SURFACE_0: egui::Color32 = egui::Color32::from_rgb(10, 14, 20);
pub(super) const SURFACE_1: egui::Color32 = egui::Color32::from_rgb(15, 20, 29);
pub(super) const SURFACE_2: egui::Color32 = egui::Color32::from_rgb(22, 28, 40);
pub(super) const SURFACE_3: egui::Color32 = egui::Color32::from_rgb(30, 38, 55);
pub(super) const BORDER: egui::Color32 = egui::Color32::from_rgb(40, 52, 75);
pub(super) const FLOATING_SURFACE: egui::Color32 = SURFACE_1;
#[allow(dead_code)]
pub(super) const FLOATING_SURFACE_SOFT: egui::Color32 = SURFACE_2;
pub(super) const FLOATING_BORDER: egui::Color32 = BORDER;
pub(super) const FLOATING_RULE: egui::Color32 = egui::Color32::from_rgb(33, 45, 68);
pub(super) const FLOATING_INPUT_BG: egui::Color32 = egui::Color32::from_rgb(8, 12, 20);
pub(super) const FLOATING_INPUT_BORDER: egui::Color32 = egui::Color32::from_rgb(52, 78, 122);
pub(super) const FLOATING_SHADOW: egui::epaint::Shadow = egui::epaint::Shadow {
    offset: [0, 18],
    blur: 36,
    spread: 0,
    color: egui::Color32::from_black_alpha(90),
};
pub(super) const TEXT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(220, 230, 245);
pub(super) const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(120, 135, 160);
pub(super) const CHAT_COMPOSER_BG: egui::Color32 = SURFACE_1;
pub(super) const CHAT_COMPOSER_BORDER: egui::Color32 = BORDER;
pub(super) const CHAT_COMPOSER_RADIUS: u8 = 18;
pub(super) const CHAT_COMPOSER_MIN_HEIGHT: f32 = 74.0;
pub(super) const CHAT_COMPOSER_MAX_WIDTH: f32 = 980.0;

pub(super) const TS_TITLE: f32 = 13.0;
pub(super) const TS_SECTION: f32 = 11.0;
pub(super) const TS_LABEL: f32 = 10.0;
pub(super) const TS_BODY: f32 = 12.0;

pub(super) fn apply(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals {
        window_fill: SURFACE_0,
        panel_fill: SURFACE_1,
        extreme_bg_color: egui::Color32::from_rgb(7, 10, 15),
        faint_bg_color: SURFACE_2,
        widgets: egui::style::Widgets {
            noninteractive: egui::style::WidgetVisuals {
                bg_fill: SURFACE_1,
                weak_bg_fill: SURFACE_2,
                bg_stroke: egui::Stroke::new(1.0, BORDER),
                corner_radius: egui::CornerRadius::same(4),
                fg_stroke: egui::Stroke::new(1.0, TEXT_DIM),
                expansion: 0.0,
            },
            inactive: egui::style::WidgetVisuals {
                bg_fill: SURFACE_3,
                weak_bg_fill: SURFACE_2,
                bg_stroke: egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 70, 100)),
                corner_radius: egui::CornerRadius::same(5),
                fg_stroke: egui::Stroke::new(1.0, TEXT_BRIGHT),
                expansion: 0.0,
            },
            hovered: egui::style::WidgetVisuals {
                bg_fill: SURFACE_3,
                weak_bg_fill: SURFACE_3,
                bg_stroke: egui::Stroke::new(1.5, ACCENT),
                corner_radius: egui::CornerRadius::same(5),
                fg_stroke: egui::Stroke::new(1.5, TEXT_BRIGHT),
                expansion: 1.0,
            },
            active: egui::style::WidgetVisuals {
                bg_fill: ACCENT_DIM,
                weak_bg_fill: ACCENT_DIM,
                bg_stroke: egui::Stroke::new(1.0, ACCENT),
                corner_radius: egui::CornerRadius::same(5),
                fg_stroke: egui::Stroke::new(2.0, egui::Color32::WHITE),
                expansion: 1.0,
            },
            open: egui::style::WidgetVisuals {
                bg_fill: SURFACE_3,
                weak_bg_fill: SURFACE_3,
                bg_stroke: egui::Stroke::new(1.0, ACCENT),
                corner_radius: egui::CornerRadius::same(5),
                fg_stroke: egui::Stroke::new(1.0, TEXT_BRIGHT),
                expansion: 0.0,
            },
        },
        override_text_color: Some(TEXT_BRIGHT),
        ..egui::Visuals::dark()
    });
    ctx.style_mut(|style| {
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.interact_size.y = 24.0;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_surface_matches_app_palette() {
        assert_eq!(FLOATING_SURFACE, SURFACE_1);
        assert_eq!(FLOATING_SURFACE_SOFT, SURFACE_2);
        assert_eq!(FLOATING_BORDER, BORDER);
    }

    #[test]
    fn floating_shadow_matches_reference_card_depth() {
        assert_eq!(FLOATING_SHADOW.offset, [0, 18]);
        assert_eq!(FLOATING_SHADOW.blur, 36);
        assert_eq!(FLOATING_SHADOW.spread, 0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn chat_composer_tokens_match_chatgpt_style_contract() {
        assert_eq!(CHAT_COMPOSER_RADIUS, 18);
        assert_eq!(CHAT_COMPOSER_MIN_HEIGHT, 74.0);
        assert_eq!(CHAT_COMPOSER_MAX_WIDTH, 980.0);
        assert_eq!(CHAT_COMPOSER_BG, SURFACE_1);
        assert_eq!(CHAT_COMPOSER_BORDER, BORDER);
    }
}

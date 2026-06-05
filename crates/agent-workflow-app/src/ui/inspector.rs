#![allow(clippy::suboptimal_flops)]

#[allow(clippy::wildcard_imports)]
use super::theme::*;
use crate::settings_store::AppSettings;
use crate::state::AppState;
use eframe::egui;
use egui_phosphor::regular as ph;

const INSPECTOR_GAP: f32 = 12.0;
const FLOATING_INSPECTOR_WIDTH: f32 = 340.0;
const FLOATING_INSPECTOR_MARGIN: f32 = 20.0;
const FLOATING_INSPECTOR_MIN_WIDTH: f32 = 280.0;
const FLOATING_INSPECTOR_BOTTOM_CLEARANCE: f32 = 20.0;
const FLOATING_INSPECTOR_MIN_HEIGHT: f32 = 520.0;
const ICON_BTN_SIZE: f32 = 30.0;
#[cfg(test)]
const RUN_BUTTON_WIDTH: f32 = ICON_BTN_SIZE;
#[cfg(test)]
const DELETE_BUTTON_WIDTH: f32 = ICON_BTN_SIZE;
const FLAT_SECTION_LABEL_GAP: f32 = 6.0;
const FLAT_SECTION_BOTTOM_GAP: f32 = 14.0;
const PILL_ICON_SIZE: f32 = 32.0;
const PILL_BOTTOM_MARGIN: f32 = 24.0;
const PILL_INNER_PAD_X: f32 = 8.0;
const PILL_INNER_PAD_Y: f32 = 6.0;
const PILL_APPROX_W: f32 = PILL_ICON_SIZE + PILL_INNER_PAD_X * 2.0;
const PILL_APPROX_H: f32 = PILL_ICON_SIZE + PILL_INNER_PAD_Y * 2.0;

fn floating_inspector_width(available_width: f32) -> f32 {
    let max_width =
        (available_width - (FLOATING_INSPECTOR_MARGIN * 2.0)).max(FLOATING_INSPECTOR_MIN_WIDTH);
    FLOATING_INSPECTOR_WIDTH.min(max_width)
}

fn floating_inspector_anchor_offset() -> egui::Vec2 {
    egui::vec2(-FLOATING_INSPECTOR_MARGIN, FLOATING_INSPECTOR_MARGIN)
}

fn floating_inspector_height(available_height: f32) -> f32 {
    let max_height = (available_height
        - (FLOATING_INSPECTOR_MARGIN + FLOATING_INSPECTOR_BOTTOM_CLEARANCE))
        .max(0.0);
    FLOATING_INSPECTOR_MIN_HEIGHT.min(max_height)
}

#[allow(clippy::struct_excessive_bools)]
pub(super) struct InspectorOutput {
    pub begin_link: bool,
    pub delete_node: bool,
    pub apply_schema: bool,
    pub run_workflow: bool,
}

pub(super) struct GraphPillOutput {
    pub add_agent: bool,
}

pub(super) fn show_floating_inspector(
    ctx: &egui::Context,
    state: &mut AppState,
    settings: &AppSettings,
) -> InspectorOutput {
    let available_width = ctx.content_rect().width();
    let available_height = ctx.content_rect().height();
    let panel_width = floating_inspector_width(available_width);
    let panel_height = floating_inspector_height(available_height);

    egui::Area::new(egui::Id::new("floating_inspector"))
        .anchor(egui::Align2::RIGHT_TOP, floating_inspector_anchor_offset())
        .order(egui::Order::Foreground)
        .default_width(panel_width)
        .show(ctx, |ui| {
            ui.set_width(panel_width);
            egui::Frame::new()
                .fill(FLOATING_SURFACE)
                .stroke(egui::Stroke::new(1.0, FLOATING_BORDER))
                .corner_radius(egui::CornerRadius::same(20))
                .shadow(FLOATING_SHADOW)
                .inner_margin(egui::Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_min_height(panel_height - 32.0);
                    ui.set_width(panel_width - 36.0);
                    show_inspector_panel(ui, state, settings)
                })
                .inner
        })
        .inner
}

#[allow(clippy::too_many_lines)]
pub(super) fn show_inspector_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    settings: &AppSettings,
) -> InspectorOutput {
    let mut out = InspectorOutput {
        begin_link: false,
        delete_node: false,
        apply_schema: false,
        run_workflow: false,
    };

    egui::Frame::new().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.set_height(ICON_BTN_SIZE);
            let gap = ui.spacing().item_spacing.x;
            let controls_width = ICON_BTN_SIZE + gap;
            let label_width = (ui.available_width() - controls_width).max(96.0);

            if let Some(node) = state.selected_node_mut() {
                ui.add_sized(
                    [label_width, ICON_BTN_SIZE],
                    egui::TextEdit::singleline(&mut node.label)
                        .font(egui::FontId::proportional(TS_BODY))
                        .desired_width(f32::INFINITY),
                );
            } else {
                ui.add_sized(
                    [label_width, ICON_BTN_SIZE],
                    egui::Label::new(
                        egui::RichText::new("No node selected")
                            .size(TS_SECTION)
                            .color(TEXT_DIM),
                    ),
                );
            }

            if ui
                .add_sized(
                    [ICON_BTN_SIZE, ICON_BTN_SIZE],
                    egui::Button::new(egui::RichText::new(ph::PLAY).size(TS_TITLE).color(ACCENT))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                )
                .on_hover_text("Run workflow (⌘↩)")
                .clicked()
            {
                out.run_workflow = true;
            }
            if ui
                .add_sized(
                    [ICON_BTN_SIZE, ICON_BTN_SIZE],
                    egui::Button::new(egui::RichText::new(ph::TRASH).size(TS_TITLE))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                )
                .on_hover_text("Delete node")
                .clicked()
            {
                out.delete_node = true;
            }
        });
    });

    flat_rule(ui);
    ui.add_space(INSPECTOR_GAP);

    // Link mode banner
    if state.link_from_node_id.is_some() {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(20, 30, 50))
            .stroke(egui::Stroke::new(1.0, FLOATING_BORDER))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Click a node on the canvas to connect →")
                        .size(TS_LABEL)
                        .color(ACCENT),
                );
                for node in state.workflow.nodes.clone() {
                    if state.link_from_node_id.as_ref() != Some(&node.id)
                        && ui
                            .add(
                                egui::Button::new(format!("→ {}", node.label))
                                    .fill(ACCENT_DIM)
                                    .corner_radius(egui::CornerRadius::same(5)),
                            )
                            .clicked()
                    {
                        state.connect_link_to(node.id.clone());
                    }
                }
            });
        ui.add_space(INSPECTOR_GAP);
    }

    egui::ScrollArea::vertical()
        .id_salt("inspector_scroll")
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            prop_section(ui, "Role", |ui| {
                text_edit_frame(ui, |ui| {
                    if let Some(node) = state.selected_node_mut() {
                        ui.add(
                            egui::TextEdit::multiline(&mut node.agent.system_prompt)
                                .frame(egui::Frame::NONE)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("Describe the agent's role and behavior..."),
                        );
                    }
                });
            });
            flat_rule(ui);

            prop_section(ui, "Task", |ui| {
                text_edit_frame(ui, |ui| {
                    if let Some(node) = state.selected_node_mut() {
                        ui.add(
                            egui::TextEdit::multiline(&mut node.agent.task_prompt)
                                .frame(egui::Frame::NONE)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY)
                                .hint_text("What this agent should do..."),
                        );
                    }
                });
            });
            flat_rule(ui);

            flat_section(ui, "Routing", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Model")
                            .size(TS_SECTION)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(node) = state.selected_node_mut() {
                            egui::ComboBox::from_id_salt("model_combo")
                                .selected_text(&node.agent.model)
                                .width(150.0)
                                .show_ui(ui, |ui| {
                                    for m in settings.active_models() {
                                        ui.selectable_value(&mut node.agent.model, m.clone(), m);
                                    }
                                });
                        }
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Link to")
                            .size(TS_SECTION)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(ph::LINK).size(TS_TITLE))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE),
                            )
                            .on_hover_text("Link from selected node")
                            .clicked()
                        {
                            out.begin_link = true;
                        }
                    });
                });
            });
            flat_rule(ui);

            flat_section(ui, "Run Behavior", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Auto-start")
                            .size(TS_SECTION)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let node_id = state.selected_node().map(|n| n.id.clone());
                        if let Some(node_id) = node_id {
                            let mut auto_start =
                                *state.node_auto_start.get(&node_id).unwrap_or(&true);
                            let prev = auto_start;
                            ui.add(egui::Checkbox::new(&mut auto_start, "Run automatically"));
                            if auto_start != prev {
                                state.set_node_auto_start(&node_id, auto_start);
                            }
                        }
                    });
                });
            });
            flat_rule(ui);

            flat_section(ui, "Advanced", |ui| {
                egui::CollapsingHeader::new(
                    egui::RichText::new("Response Schema")
                        .size(TS_SECTION)
                        .color(TEXT_DIM),
                )
                .default_open(false)
                .show(ui, |ui| {
                    text_edit_frame(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut state.schema_editor_text)
                                .frame(egui::Frame::NONE)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY)
                                .font(egui::FontId::monospace(TS_LABEL)),
                        );
                    });
                    ui.add_space(8.0);
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(ph::CHECK).size(TS_TITLE))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                        )
                        .on_hover_text("Apply schema")
                        .clicked()
                    {
                        out.apply_schema = true;
                    }
                });
                ui.add_space(8.0);
                egui::CollapsingHeader::new(
                    egui::RichText::new("Entrypoint")
                        .size(TS_SECTION)
                        .color(TEXT_DIM),
                )
                .default_open(false)
                .show(ui, |ui| {
                    text_edit_frame(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut state.entrypoint_text)
                                .frame(egui::Frame::NONE)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY)
                                .hint_text("Describe what the first agent should do..."),
                        );
                    });
                });
            });

            // Keep bottom breathing room
            ui.add_space(2.0);
        });

    out
}

fn prop_section(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    flat_section(ui, label, body);
}

fn flat_section(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.label(
        egui::RichText::new(label)
            .size(TS_SECTION)
            .color(TEXT_BRIGHT)
            .monospace(),
    );
    ui.add_space(FLAT_SECTION_LABEL_GAP);
    body(ui);
    ui.add_space(FLAT_SECTION_BOTTOM_GAP);
}

fn flat_rule(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 10.0), egui::Sense::hover());
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.center().y,
        egui::Stroke::new(1.0, FLOATING_RULE),
    );
}

pub(super) fn show_graph_pill(ctx: &egui::Context, canvas_rect: egui::Rect) -> GraphPillOutput {
    let mut out = GraphPillOutput { add_agent: false };

    let pill_pos = egui::pos2(
        canvas_rect.center().x - PILL_APPROX_W / 2.0,
        canvas_rect.bottom() - PILL_BOTTOM_MARGIN - PILL_APPROX_H,
    );

    egui::Area::new(egui::Id::new("graph_pill"))
        .fixed_pos(pill_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FLOATING_SURFACE)
                .stroke(egui::Stroke::new(1.0, FLOATING_BORDER))
                .corner_radius(egui::CornerRadius::same(20))
                .shadow(FLOATING_SHADOW)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(
                                [PILL_ICON_SIZE, PILL_ICON_SIZE],
                                egui::Button::new(
                                    egui::RichText::new(ph::PLUS)
                                        .size(TS_TITLE)
                                        .color(TEXT_BRIGHT),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                            )
                            .on_hover_text("Add agent")
                            .clicked()
                        {
                            out.add_agent = true;
                        }
                    });
                });
        });

    out
}

fn text_edit_frame(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(FLOATING_INPUT_BG)
        .stroke(egui::Stroke::new(1.0, FLOATING_INPUT_BORDER))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(10, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn floating_width_uses_desktop_width_when_space_allows() {
        assert_eq!(floating_inspector_width(1280.0), 340.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn floating_width_leaves_breathing_room_on_narrow_windows() {
        assert_eq!(floating_inspector_width(360.0), 320.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn floating_anchor_offsets_from_top_right() {
        assert_eq!(floating_inspector_anchor_offset(), egui::vec2(-20.0, 20.0));
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn floating_width_never_drops_below_minimum_contract() {
        assert_eq!(floating_inspector_width(100.0), 280.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn floating_height_tracks_available_space_toward_bottom_panel() {
        assert_eq!(floating_inspector_height(900.0), 520.0);
        assert_eq!(floating_inspector_height(560.0), 520.0);
        assert_eq!(floating_inspector_height(500.0), 460.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn flat_section_spacing_is_smaller_than_old_card_stack() {
        assert_eq!(INSPECTOR_GAP, 12.0);
        assert_eq!(FLAT_SECTION_LABEL_GAP, 6.0);
        assert_eq!(FLAT_SECTION_BOTTOM_GAP, 14.0);
    }

    #[test]
    fn compact_action_widths_fit_floating_panel() {
        let controls_width = RUN_BUTTON_WIDTH + DELETE_BUTTON_WIDTH;
        assert!(controls_width < FLOATING_INSPECTOR_WIDTH);
    }
}

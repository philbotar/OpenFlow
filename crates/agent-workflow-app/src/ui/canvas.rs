#![allow(deprecated, clippy::wildcard_imports)]

use super::theme::*;
use crate::canvas_math::{edge_anchor_points, NODE_HEIGHT, NODE_WIDTH};
use crate::settings_store::AppSettings;
use crate::state::{AgentStatus, AppState, TraceStatus};
use eframe::egui;
use egui_phosphor::regular as ph;
use std::collections::BTreeSet;
use workflow_core::{ChatRole, NodeId};

const DOCK_DEFAULT_HEIGHT: f32 = 210.0;
const DOCK_MIN_HEIGHT: f32 = 36.0;
const DOCK_MAX_HEIGHT_RATIO: f32 = 0.6;
const DOCK_MAX_HEIGHT_FALLBACK: f32 = 520.0;
const DOCK_HEADER_ROW_HEIGHT: f32 = 24.0;
const DOCK_INSET_X: i8 = 12;
const DOCK_HEADER_INSET_Y: i8 = 4;
const CHAT_COMPOSER_RESERVED_HEIGHT: f32 = 112.0;
const CHAT_ERROR_RESERVED_HEIGHT: f32 = 52.0;
const CHAT_ROLE_LABEL_WIDTH: f32 = 78.0;
const CHAT_ROW_GAP: f32 = 10.0;
const TRACE_ROW_MIN_HEIGHT: f32 = 42.0;

const SKILL_OPTIONS: &[&str] = &[
    "brainstorming",
    "test-driven-development",
    "systematic-debugging",
    "writing-plans",
    "executing-plans",
    "verification-before-completion",
    "requesting-code-review",
    "receiving-code-review",
    "github:gh-fix-ci",
    "github:gh-address-comments",
    "browser",
    "documents",
];

pub(super) fn show_canvas_panel(ctx: &egui::Context, state: &mut AppState) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(SURFACE_0)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::click());
            let rect = response.rect;

            draw_dot_grid(&painter, rect);

            let node_size = (NODE_WIDTH, NODE_HEIGHT);

            for edge in &state.workflow.edges {
                let from = state.workflow.nodes.iter().find(|n| n.id == edge.from);
                let to = state.workflow.nodes.iter().find(|n| n.id == edge.to);
                if let (Some(from), Some(to)) = (from, to) {
                    let (s, e) = edge_anchor_points(
                        (from.position.x, from.position.y),
                        (to.position.x, to.position.y),
                        node_size,
                    );
                    let start = rect.left_top() + egui::vec2(s.0, s.1);
                    let end = rect.left_top() + egui::vec2(e.0, e.1);
                    let offset = ((end.x - start.x).abs() * 0.5).max(60.0);
                    draw_cubic_bezier(
                        &painter,
                        start,
                        egui::pos2(start.x + offset, start.y),
                        egui::pos2(end.x - offset, end.y),
                        end,
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(55, 72, 108)),
                    );
                    // Arrow tip at end
                    let dir = (end - egui::pos2(end.x - offset, end.y)).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            end,
                            end - dir * 8.0 + perp * 4.0,
                            end - dir * 8.0 - perp * 4.0,
                        ],
                        egui::Color32::from_rgb(55, 72, 108),
                        egui::Stroke::NONE,
                    ));
                }
            }

            // Collect a lightweight snapshot so we can mutate state inside the loop
            // without holding a borrow over workflow.nodes.
            let node_display: Vec<(NodeId, String, egui::Rect, AgentStatus, bool)> = state
                .workflow
                .nodes
                .iter()
                .map(|n| {
                    let min = rect.left_top() + egui::vec2(n.position.x, n.position.y);
                    let node_rect =
                        egui::Rect::from_min_size(min, egui::vec2(node_size.0, node_size.1));
                    let status = state
                        .status_by_node
                        .get(&n.id)
                        .copied()
                        .unwrap_or(AgentStatus::Idle);
                    let selected = state.selected_node_id.as_ref() == Some(&n.id);
                    (n.id.clone(), n.label.clone(), node_rect, status, selected)
                })
                .collect();

            for (node_id, node_label, node_rect, status, selected) in &node_display {
                let node_rect = *node_rect;
                let status = *status;
                let selected = *selected;

                let status_color = match status {
                    AgentStatus::Idle => BORDER,
                    AgentStatus::Queued => egui::Color32::from_rgb(120, 120, 220),
                    AgentStatus::Started => ACCENT,
                    AgentStatus::AwaitingInput => egui::Color32::from_rgb(255, 193, 7),
                    AgentStatus::Completed => SUCCESS,
                    AgentStatus::Failed => DANGER,
                };

                let corner_r = 8.0;

                // Card background
                painter.rect_filled(
                    node_rect,
                    corner_r,
                    if selected { SURFACE_3 } else { SURFACE_2 },
                );

                // Card border
                painter.rect_stroke(
                    node_rect,
                    corner_r,
                    egui::Stroke::new(
                        if selected { 1.5 } else { 0.5 },
                        if selected { ACCENT } else { SURFACE_3 },
                    ),
                    egui::StrokeKind::Inside,
                );

                // Running pulse
                if matches!(status, AgentStatus::Started) {
                    for (grow, alpha) in [(6.0_f32, 12u8), (3.0, 25u8)] {
                        painter.rect_filled(
                            node_rect.expand(grow),
                            corner_r + grow,
                            egui::Color32::from_rgba_unmultiplied(76, 148, 255, alpha),
                        );
                    }
                }

                // Awaiting-input blink
                if matches!(status, AgentStatus::AwaitingInput) {
                    for (grow, alpha) in [(6.0_f32, 12u8), (3.0, 25u8)] {
                        painter.rect_filled(
                            node_rect.expand(grow),
                            corner_r + grow,
                            egui::Color32::from_rgba_unmultiplied(255, 193, 7, alpha),
                        );
                    }
                }

                // Status indicator dot
                painter.circle_filled(
                    egui::pos2(
                        node_rect.left() + 14.0,
                        node_rect.height().mul_add(0.5, node_rect.top()),
                    ),
                    5.0,
                    status_color,
                );

                // Node label
                let text_x = node_rect.left() + 32.0;
                let label_y = node_rect.height().mul_add(0.36, node_rect.top());
                painter.text(
                    egui::pos2(text_x, label_y),
                    egui::Align2::LEFT_CENTER,
                    node_label,
                    egui::FontId::proportional(TS_BODY),
                    TEXT_BRIGHT,
                );

                // "Agent" subtitle
                painter.text(
                    egui::pos2(text_x, label_y + 14.0),
                    egui::Align2::LEFT_CENTER,
                    "Agent",
                    egui::FontId::proportional(TS_LABEL),
                    TEXT_DIM,
                );

                // Status chip text
                let status_label = match status {
                    AgentStatus::Idle => None,
                    AgentStatus::Queued => Some(("QUEUED", egui::Color32::from_rgb(120, 120, 220))),
                    AgentStatus::Started => Some(("RUNNING", ACCENT)),
                    AgentStatus::AwaitingInput => {
                        Some(("WAITING", egui::Color32::from_rgb(255, 193, 7)))
                    }
                    AgentStatus::Completed => Some(("DONE", SUCCESS)),
                    AgentStatus::Failed => Some(("FAILED", DANGER)),
                };
                if let Some((text, color)) = status_label {
                    let chip_y = node_rect.top() + node_rect.height() - 10.0;
                    painter.text(
                        egui::pos2(node_rect.right() - 8.0, chip_y),
                        egui::Align2::RIGHT_CENTER,
                        text,
                        egui::FontId::proportional(TS_LABEL),
                        color,
                    );
                }

                let node_response = ui.interact(
                    node_rect,
                    egui::Id::new(("node", node_id.clone())),
                    egui::Sense::click_and_drag(),
                );
                if node_response.dragged() {
                    let delta = node_response.drag_motion();
                    state.move_node_by_delta(
                        node_id,
                        delta.x,
                        delta.y,
                        (rect.width(), rect.height()),
                        node_size,
                    );
                }
                if node_response.clicked() {
                    state.select_node(node_id.clone());
                }
            }

            if let Some(error) = &state.last_error.clone() {
                let max_w = (rect.width() - 64.0).clamp(200.0, 520.0);
                egui::Area::new(egui::Id::new("error_toast"))
                    .fixed_pos(egui::pos2(rect.center().x - max_w / 2.0, rect.top() + 12.0))
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(60, 20, 20))
                            .stroke(egui::Stroke::new(1.0, DANGER))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.set_max_width(max_w);
                                ui.label(egui::RichText::new(error).size(TS_SECTION).color(DANGER));
                            });
                    });
            }

            rect
        })
        .inner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BottomPanelTab {
    Chat,
    RunTrace,
}

impl BottomPanelTab {
    const fn label(self) -> &'static str {
        match self {
            Self::Chat => "Chat",
            Self::RunTrace => "Run Trace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ReasoningLevel {
    Low,
    #[default]
    Medium,
    High,
}

impl ReasoningLevel {
    const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ChatCapability {
    #[default]
    AssistNode,
    DebugRun,
    DraftWorkflow,
    ReviewSchema,
}

impl ChatCapability {
    const fn label(self) -> &'static str {
        match self {
            Self::AssistNode => "Assist selected node",
            Self::DebugRun => "Debug run trace",
            Self::DraftWorkflow => "Draft workflow change",
            Self::ReviewSchema => "Review output schema",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ChatContextScope {
    #[default]
    SelectedNode,
    WholeWorkflow,
    CurrentRun,
    SelectedTrace,
}

impl ChatContextScope {
    const fn label(self) -> &'static str {
        match self {
            Self::SelectedNode => "Selected node",
            Self::WholeWorkflow => "Whole workflow",
            Self::CurrentRun => "Current run",
            Self::SelectedTrace => "Selected trace event",
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ChatDockState {
    skill_search: String,
    selected_skills: BTreeSet<String>,
    reasoning_level: ReasoningLevel,
    capability: ChatCapability,
    context_scope: ChatContextScope,
}

#[allow(clippy::struct_excessive_bools)]
pub(super) struct BottomPanelOutput {
    pub clear_run: bool,
    pub send_chat: bool,
    pub retry_run: bool,
    pub run_workflow: bool,
}

pub(super) struct BottomPanelInput<'a> {
    pub settings: &'a mut AppSettings,
    pub input_buffer: &'a mut String,
    pub active_tab: &'a mut BottomPanelTab,
    pub dock_state: &'a mut ChatDockState,
    pub api_key_ready: bool,
    pub execution_running: bool,
}

struct ChatContentInput<'a> {
    settings: &'a mut AppSettings,
    input_buffer: &'a mut String,
    dock_state: &'a mut ChatDockState,
    api_key_ready: bool,
    execution_running: bool,
}

#[derive(Debug, Clone, Copy)]
struct ChatRolePresentation {
    label: &'static str,
    label_color: egui::Color32,
    text_color: egui::Color32,
    monospace: bool,
}

#[derive(Debug, Clone, Copy)]
struct ChatSectionHeights {
    history: f32,
    composer: f32,
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn show_bottom_panel(
    ctx: &egui::Context,
    state: &mut AppState,
    input: BottomPanelInput<'_>,
) -> BottomPanelOutput {
    let mut out = BottomPanelOutput {
        clear_run: false,
        send_chat: false,
        retry_run: false,
        run_workflow: false,
    };

    let dock_max_height = (ctx.content_rect().height() * DOCK_MAX_HEIGHT_RATIO)
        .clamp(DOCK_DEFAULT_HEIGHT, DOCK_MAX_HEIGHT_FALLBACK);

    egui::TopBottomPanel::bottom("console_panel")
        .resizable(true)
        .default_height(DOCK_DEFAULT_HEIGHT)
        .min_height(DOCK_MIN_HEIGHT)
        .max_height(dock_max_height)
        .frame(
            egui::Frame::new()
                .fill(SURFACE_1)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            ui.take_available_space();
            let top_y = ui.clip_rect().top();
            ui.painter().hline(
                ui.clip_rect().left()..=ui.clip_rect().right(),
                top_y,
                egui::Stroke::new(1.0, BORDER),
            );
            let header_content_size = dock_header_content_size(ui.available_width());
            dock_header_frame().show(ui, |ui| {
                ui.set_min_width(header_content_size.x);
                show_bottom_header(
                    ui,
                    state,
                    input.active_tab,
                    &mut out,
                    input.execution_running,
                );
            });

            match *input.active_tab {
                BottomPanelTab::Chat => {
                    show_chat_content(
                        ui,
                        state,
                        ChatContentInput {
                            settings: input.settings,
                            input_buffer: input.input_buffer,
                            dock_state: input.dock_state,
                            api_key_ready: input.api_key_ready,
                            execution_running: input.execution_running,
                        },
                        &mut out,
                    );
                }
                BottomPanelTab::RunTrace => {
                    show_run_trace_content(ui, state);
                }
            }
        });

    out
}

fn draw_tab_button(ui: &mut egui::Ui, selected: bool, label: &str) -> bool {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(TS_LABEL)
                .color(if selected { TEXT_BRIGHT } else { TEXT_DIM })
                .monospace(),
        )
        .fill(if selected {
            SURFACE_3
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(
            if selected { 1.0 } else { 0.0 },
            if selected { ACCENT } else { BORDER },
        ))
        .corner_radius(egui::CornerRadius::same(5)),
    )
    .clicked()
}

fn dock_header_content_size(available_width: f32) -> egui::Vec2 {
    egui::vec2(
        f32::from(DOCK_INSET_X)
            .mul_add(-2.0, available_width)
            .max(0.0),
        DOCK_HEADER_ROW_HEIGHT,
    )
}

fn dock_header_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE_1)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin::symmetric(DOCK_INSET_X, DOCK_HEADER_INSET_Y))
}

fn show_bottom_header(
    ui: &mut egui::Ui,
    state: &AppState,
    active_tab: &mut BottomPanelTab,
    out: &mut BottomPanelOutput,
    execution_running: bool,
) {
    ui.horizontal_wrapped(|ui| {
        if draw_tab_button(
            ui,
            *active_tab == BottomPanelTab::Chat,
            BottomPanelTab::Chat.label(),
        ) {
            *active_tab = BottomPanelTab::Chat;
        }
        if draw_tab_button(
            ui,
            *active_tab == BottomPanelTab::RunTrace,
            BottomPanelTab::RunTrace.label(),
        ) {
            *active_tab = BottomPanelTab::RunTrace;
        }

        if *active_tab == BottomPanelTab::RunTrace {
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let selected_node = state
                .selected_node()
                .map_or("No node selected", |node| node.label.as_str());
            let selected_trace = state
                .selected_trace_event()
                .map_or("No trace event selected", |event| event.node_label.as_str());
            let run_text = if execution_running {
                "Run active"
            } else if state.last_run.is_some() {
                "Run complete"
            } else {
                "No run"
            };

            chip(
                ui,
                ph::FLOW_ARROW,
                state.workflow.name.as_str(),
                TEXT_DIM,
                SURFACE_1,
            );
            chip(ui, ph::TARGET, selected_node, TEXT_DIM, SURFACE_1);
            chip(
                ui,
                ph::LIST_MAGNIFYING_GLASS,
                selected_trace,
                TEXT_DIM,
                SURFACE_1,
            );
            chip(
                ui,
                if execution_running {
                    ph::CIRCLE_NOTCH
                } else {
                    ph::CHECK_CIRCLE
                },
                run_text,
                if execution_running { ACCENT } else { TEXT_DIM },
                SURFACE_1,
            );

            if !state.run_trace.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_text_button(
                        ui,
                        ph::ARROWS_COUNTER_CLOCKWISE,
                        "Clear",
                        "Clear run trace",
                    )
                    .clicked()
                    {
                        out.clear_run = true;
                    }
                });
            }
        }
    });
}

fn show_run_trace_content(ui: &mut egui::Ui, state: &mut AppState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if state.run_trace.is_empty() {
                show_trace_empty_state(ui);
            } else {
                let entries = state.run_trace.clone();
                for (index, event) in entries.iter().enumerate() {
                    let selected = state.selected_trace_index == Some(index);
                    let (status_text, dot_color) = trace_status_meta(event.status);
                    let fill = if selected { SURFACE_3 } else { SURFACE_1 };
                    egui::Frame::new()
                        .fill(fill)
                        .stroke(egui::Stroke::new(
                            if selected { 1.0 } else { 0.5 },
                            if selected { ACCENT } else { BORDER },
                        ))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_min_height(TRACE_ROW_MIN_HEIGHT);
                            ui.horizontal_wrapped(|ui| {
                                let (dot_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(12.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    egui::pos2(dot_rect.left() + 4.0, dot_rect.center().y),
                                    4.0,
                                    dot_color,
                                );
                                if ui
                                    .add(
                                        egui::Button::selectable(
                                            selected,
                                            egui::RichText::new(&event.node_label)
                                                .size(TS_SECTION)
                                                .color(TEXT_BRIGHT),
                                        )
                                        .fill(egui::Color32::TRANSPARENT),
                                    )
                                    .on_hover_text("Use this trace event as chat context")
                                    .clicked()
                                {
                                    state.select_trace_event(index);
                                }
                                status_chip(ui, status_text, dot_color);
                                ui.label(
                                    egui::RichText::new(&event.message)
                                        .size(TS_SECTION)
                                        .color(TEXT_DIM),
                                );
                                if event.status == TraceStatus::Failed
                                    && icon_only_button(ui, ph::COPY, "Copy error").clicked()
                                {
                                    ui.ctx().copy_text(event.message.clone());
                                }
                            });
                            if let Some(output) = &event.output {
                                ui.add_space(4.0);
                                egui::CollapsingHeader::new(
                                    egui::RichText::new("Output").size(TS_LABEL).color(TEXT_DIM),
                                )
                                .default_open(false)
                                .show(ui, |ui| {
                                    code_block(ui, &pretty_json(output));
                                });
                            }
                        });
                    ui.add_space(6.0);
                }
            }
        });
}

#[allow(clippy::needless_pass_by_value)]
fn show_chat_content(
    ui: &mut egui::Ui,
    state: &mut AppState,
    input: ChatContentInput<'_>,
    out: &mut BottomPanelOutput,
) {
    let selected_node_id = state.selected_node_id.clone();
    let selected_status = selected_node_id
        .as_ref()
        .and_then(|id| state.status_by_node.get(id))
        .copied()
        .unwrap_or(AgentStatus::Idle);
    let has_paused_node =
        selected_node_id.is_some() && selected_status == AgentStatus::AwaitingInput;

    // Clone data needed by the history closure up-front so state isn't
    // borrowed across both allocate_ui_with_layout calls.
    let messages = selected_node_id
        .as_ref()
        .and_then(|id| state.chat_logs.get(id))
        .cloned()
        .unwrap_or_default();
    let selected_node_label = selected_node_id
        .as_ref()
        .and_then(|id| {
            state
                .workflow
                .nodes
                .iter()
                .find(|node| &node.id == id)
                .map(|node| node.label.clone())
        })
        .unwrap_or_else(|| "Assistant".to_string());
    let last_error = state.last_error.clone();

    let available_h = ui.available_height();
    let available_w = ui.available_width();
    let heights = chat_section_heights(available_h, last_error.is_some());
    let history_h = heights.history;
    let composer_h = heights.composer;

    // History section — takes exactly history_h so the composer gets
    // exactly composer_h, keeping the panel's min_rect == panel height.
    ui.allocate_ui_with_layout(
        egui::vec2(available_w, history_h),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if messages.is_empty() {
                        show_chat_empty_state(ui, has_paused_node);
                    } else {
                        for msg in &messages {
                            show_chat_message(
                                ui,
                                &msg.role,
                                &msg.content,
                                Some(selected_node_label.as_str()),
                            );
                            ui.add_space(4.0);
                        }
                    }
                });
        },
    );

    // Composer section — takes exactly composer_h.
    ui.allocate_ui_with_layout(
        egui::vec2(available_w, composer_h),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            if let Some(error) = &last_error {
                show_chat_error(ui, error, out);
            }
            show_chat_composer(
                ui,
                state,
                input.settings,
                input.input_buffer,
                input.dock_state,
                input.api_key_ready,
                has_paused_node,
                input.execution_running,
                out,
            );
        },
    );
}

const fn chat_input_enabled(
    has_paused_node: bool,
    execution_running: bool,
    api_key_ready: bool,
) -> bool {
    has_paused_node && execution_running && api_key_ready
}

fn chat_send_enabled(
    has_paused_node: bool,
    execution_running: bool,
    api_key_ready: bool,
    text: &str,
) -> bool {
    chat_input_enabled(has_paused_node, execution_running, api_key_ready) && !text.trim().is_empty()
}

const fn chat_status_text(api_key_ready: bool, has_paused_node: bool) -> &'static str {
    if !api_key_ready {
        "API key missing"
    } else if !has_paused_node {
        "Paused node needed"
    } else {
        "Ready"
    }
}

const fn chat_role_presentation(role: &ChatRole) -> ChatRolePresentation {
    match role {
        ChatRole::User => ChatRolePresentation {
            label: "You:",
            label_color: ACCENT,
            text_color: TEXT_BRIGHT,
            monospace: false,
        },
        ChatRole::Assistant => ChatRolePresentation {
            label: "Assistant:",
            label_color: SUCCESS,
            text_color: TEXT_BRIGHT,
            monospace: false,
        },
        ChatRole::System => ChatRolePresentation {
            label: "System:",
            label_color: TEXT_DIM,
            text_color: TEXT_DIM,
            monospace: false,
        },
        ChatRole::Thinking => ChatRolePresentation {
            label: "Thinking:",
            label_color: ACCENT,
            text_color: ACCENT,
            monospace: true,
        },
    }
}

fn chat_section_heights(available_height: f32, has_error: bool) -> ChatSectionHeights {
    let reserved = CHAT_COMPOSER_RESERVED_HEIGHT
        + if has_error {
            CHAT_ERROR_RESERVED_HEIGHT
        } else {
            0.0
        };
    let composer = reserved.min(available_height);

    ChatSectionHeights {
        history: (available_height - composer).max(0.0),
        composer,
    }
}

fn selected_skill_summary(selected_skills: &BTreeSet<String>) -> String {
    match selected_skills.len() {
        0 => "Skills".to_string(),
        1 => selected_skills
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "Skills".to_string()),
        count => format!("{count} skills"),
    }
}

fn show_chat_empty_state(ui: &mut egui::Ui, can_send: bool) {
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(if can_send {
                "Send a message to continue."
            } else {
                "Run a workflow or choose a paused node."
            })
            .size(TS_BODY)
            .color(TEXT_DIM),
        );
    });
}

fn show_chat_message(
    ui: &mut egui::Ui,
    role: &ChatRole,
    content: &str,
    assistant_label: Option<&str>,
) {
    let presentation = chat_role_presentation(role);
    let role_label = if matches!(role, ChatRole::Assistant) {
        format!("{}:", assistant_label.unwrap_or("Assistant"))
    } else {
        presentation.label.to_string()
    };
    let available_width = ui.available_width();
    let text_width = (available_width - CHAT_ROLE_LABEL_WIDTH - CHAT_ROW_GAP).max(120.0);

    ui.horizontal_top(|ui| {
        ui.add_sized(
            [CHAT_ROLE_LABEL_WIDTH, 0.0],
            egui::Label::new(
                egui::RichText::new(role_label)
                    .size(TS_LABEL)
                    .color(presentation.label_color)
                    .strong(),
            ),
        );
        ui.add_space(CHAT_ROW_GAP);

        let text = egui::RichText::new(content)
            .size(TS_BODY)
            .color(presentation.text_color);
        let label = if presentation.monospace {
            egui::Label::new(text.monospace())
        } else {
            egui::Label::new(text)
        }
        .wrap()
        .selectable(true);

        ui.add_sized([text_width, 0.0], label);
    });
}

fn show_chat_error(ui: &mut egui::Ui, error: &str, out: &mut BottomPanelOutput) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(54, 24, 28))
        .stroke(egui::Stroke::new(1.0, DANGER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(ph::WARNING_CIRCLE)
                        .size(TS_TITLE)
                        .color(DANGER),
                );
                ui.label(
                    egui::RichText::new(error)
                        .size(TS_SECTION)
                        .color(TEXT_BRIGHT),
                );
                if icon_text_button(ui, ph::COPY, "Copy", "Copy error").clicked() {
                    ui.ctx().copy_text(error.to_string());
                }
                if icon_text_button(ui, ph::ARROWS_CLOCKWISE, "Retry run", "Run workflow again")
                    .clicked()
                {
                    out.retry_run = true;
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn show_chat_composer(
    ui: &mut egui::Ui,
    state: &mut AppState,
    settings: &AppSettings,
    input_buffer: &mut String,
    dock_state: &mut ChatDockState,
    api_key_ready: bool,
    has_paused_node: bool,
    execution_running: bool,
    out: &mut BottomPanelOutput,
) {
    let available_width = ui.available_width();
    let composer_width = available_width.min(CHAT_COMPOSER_MAX_WIDTH);
    ui.vertical_centered(|ui| {
        ui.set_width(composer_width);
        egui::Frame::new()
            .fill(CHAT_COMPOSER_BG)
            .stroke(egui::Stroke::new(1.0, CHAT_COMPOSER_BORDER))
            .corner_radius(egui::CornerRadius::same(CHAT_COMPOSER_RADIUS))
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_min_height(CHAT_COMPOSER_MIN_HEIGHT);
                let text_response = ui.add_enabled(
                    has_paused_node && execution_running && api_key_ready,
                    egui::TextEdit::multiline(input_buffer)
                        .frame(egui::Frame::NONE)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text("Message paused node"),
                );

                if text_response.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                    && chat_send_enabled(
                        has_paused_node,
                        execution_running,
                        api_key_ready,
                        input_buffer,
                    )
                {
                    out.send_chat = true;
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    compact_status_button(ui, api_key_ready, has_paused_node);
                    compact_model_menu(ui, state, settings);
                    compact_reasoning_menu(ui, dock_state);
                    compact_skill_menu(ui, dock_state);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let send_enabled = chat_send_enabled(
                            has_paused_node,
                            execution_running,
                            api_key_ready,
                            input_buffer,
                        );
                        if ui
                            .add_enabled(
                                send_enabled,
                                egui::Button::new(
                                    egui::RichText::new(ph::ARROW_UP)
                                        .size(TS_TITLE)
                                        .color(TEXT_BRIGHT),
                                )
                                .fill(if send_enabled { ACCENT_DIM } else { SURFACE_2 })
                                .min_size(egui::vec2(34.0, 34.0))
                                .corner_radius(egui::CornerRadius::same(17)),
                            )
                            .on_hover_text(if send_enabled {
                                "Send"
                            } else {
                                "Run must pause for human input before chat can send"
                            })
                            .clicked()
                        {
                            out.send_chat = true;
                        }

                        if !execution_running
                            && ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(ph::PLAY)
                                            .size(TS_TITLE)
                                            .color(TEXT_BRIGHT),
                                    )
                                    .fill(SURFACE_2)
                                    .min_size(egui::vec2(34.0, 34.0))
                                    .corner_radius(egui::CornerRadius::same(17)),
                                )
                                .on_hover_text("Run workflow")
                                .clicked()
                        {
                            out.run_workflow = true;
                        }
                    });
                });
            });
    });
}

fn compact_status_button(ui: &mut egui::Ui, api_key_ready: bool, has_paused_node: bool) {
    let color = if api_key_ready && has_paused_node {
        SUCCESS
    } else if api_key_ready {
        TEXT_DIM
    } else {
        DANGER
    };
    ui.add(
        egui::Button::new(
            egui::RichText::new(format!(
                "{} {}",
                if api_key_ready {
                    ph::CHECK_CIRCLE
                } else {
                    ph::WARNING_CIRCLE
                },
                chat_status_text(api_key_ready, has_paused_node)
            ))
            .size(TS_LABEL)
            .color(color),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE),
    )
    .on_hover_text("Chat readiness");
}

fn compact_model_menu(ui: &mut egui::Ui, state: &mut AppState, settings: &AppSettings) {
    let label = state
        .selected_node()
        .map_or_else(|| "Model".to_string(), |node| node.agent.model.clone());
    egui::ComboBox::from_id_salt("chat_compact_model")
        .selected_text(label)
        .width(132.0)
        .show_ui(ui, |ui| {
            if let Some(node) = state.selected_node_mut() {
                for model in settings.active_models() {
                    ui.selectable_value(&mut node.agent.model, model.clone(), model);
                }
            } else {
                ui.label(
                    egui::RichText::new("No node selected")
                        .size(TS_LABEL)
                        .color(TEXT_DIM),
                );
            }
        });
}

fn compact_reasoning_menu(ui: &mut egui::Ui, dock_state: &mut ChatDockState) {
    egui::ComboBox::from_id_salt("chat_compact_reasoning")
        .selected_text(dock_state.reasoning_level.label())
        .width(112.0)
        .show_ui(ui, |ui| {
            for level in [
                ReasoningLevel::Low,
                ReasoningLevel::Medium,
                ReasoningLevel::High,
            ] {
                ui.selectable_value(&mut dock_state.reasoning_level, level, level.label());
            }
            ui.separator();
            for capability in [
                ChatCapability::AssistNode,
                ChatCapability::DebugRun,
                ChatCapability::DraftWorkflow,
                ChatCapability::ReviewSchema,
            ] {
                ui.selectable_value(&mut dock_state.capability, capability, capability.label());
            }
            ui.separator();
            for scope in [
                ChatContextScope::SelectedNode,
                ChatContextScope::WholeWorkflow,
                ChatContextScope::CurrentRun,
                ChatContextScope::SelectedTrace,
            ] {
                ui.selectable_value(&mut dock_state.context_scope, scope, scope.label());
            }
        });
}

fn compact_skill_menu(ui: &mut egui::Ui, dock_state: &mut ChatDockState) {
    egui::ComboBox::from_id_salt("chat_compact_skills")
        .selected_text(selected_skill_summary(&dock_state.selected_skills))
        .width(132.0)
        .show_ui(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut dock_state.skill_search)
                    .desired_width(f32::INFINITY)
                    .hint_text("Search skills"),
            );
            ui.separator();
            let query = dock_state.skill_search.trim().to_lowercase();
            for skill in SKILL_OPTIONS {
                if !query.is_empty() && !skill.to_lowercase().contains(&query) {
                    continue;
                }
                let mut selected = dock_state.selected_skills.contains(*skill);
                if ui.checkbox(&mut selected, *skill).changed() {
                    if selected {
                        dock_state.selected_skills.insert((*skill).to_string());
                    } else {
                        dock_state.selected_skills.remove(*skill);
                    }
                }
            }
        });
}

fn show_trace_empty_state(ui: &mut egui::Ui) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} No run trace yet", ph::LIST_CHECKS))
                        .size(TS_TITLE)
                        .color(TEXT_BRIGHT),
                );
                ui.label(
                    egui::RichText::new(
                        "Run the workflow to see queued, running, paused, failed, and completed events in order.",
                    )
                    .size(TS_SECTION)
                    .color(TEXT_DIM),
                );
            });
        });
}

fn chip(ui: &mut egui::Ui, icon: &str, text: &str, color: egui::Color32, fill: egui::Color32) {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(0.5, BORDER))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("{icon} {text}"))
                    .size(TS_LABEL)
                    .color(color),
            );
        });
}

fn status_chip(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(SURFACE_0)
        .stroke(egui::Stroke::new(0.75, color))
        .corner_radius(egui::CornerRadius::same(5))
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(TS_LABEL)
                    .color(color)
                    .monospace(),
            );
        });
}

fn icon_text_button(ui: &mut egui::Ui, icon: &str, label: &str, hover: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(format!("{icon} {label}"))
                .size(TS_LABEL)
                .color(TEXT_BRIGHT),
        )
        .fill(SURFACE_2),
    )
    .on_hover_text(hover)
}

fn icon_only_button(ui: &mut egui::Ui, icon: &str, hover: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(icon).size(TS_TITLE))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE),
    )
    .on_hover_text(hover)
}

const fn trace_status_meta(status: TraceStatus) -> (&'static str, egui::Color32) {
    match status {
        TraceStatus::Queued => ("queued", egui::Color32::from_rgb(120, 120, 220)),
        TraceStatus::Running => ("running", ACCENT),
        TraceStatus::Paused => ("paused", egui::Color32::from_rgb(255, 193, 7)),
        TraceStatus::Failed => ("failed", DANGER),
        TraceStatus::Completed => ("completed", SUCCESS),
    }
}

fn pretty_json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn code_block(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(SURFACE_0)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(TS_LABEL)
                    .color(TEXT_DIM)
                    .monospace(),
            );
        });
}

#[allow(clippy::while_float)]
fn draw_dot_grid(painter: &egui::Painter, rect: egui::Rect) {
    let spacing = 24.0;
    let dot_color = egui::Color32::from_rgba_unmultiplied(40, 52, 75, 80);
    let mut x = (rect.left() / spacing).ceil() * spacing;
    while x <= rect.right() {
        let mut y = (rect.top() / spacing).ceil() * spacing;
        while y <= rect.bottom() {
            painter.circle_filled(egui::pos2(x, y), 0.75, dot_color);
            y += spacing;
        }
        x += spacing;
    }
}

#[allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]
fn draw_cubic_bezier(
    painter: &egui::Painter,
    p0: egui::Pos2,
    p1: egui::Pos2,
    p2: egui::Pos2,
    p3: egui::Pos2,
    stroke: egui::Stroke,
) {
    const STEPS: usize = 20;
    let mut prev = p0;
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        let u = 1.0 - t;
        let pt = egui::pos2(
            u * u * u * p0.x + 3.0 * u * u * t * p1.x + 3.0 * u * t * t * p2.x + t * t * t * p3.x,
            u * u * u * p0.y + 3.0 * u * u * t * p1.y + 3.0 * u * t * t * p2.y + t * t * t * p3.y,
        );
        painter.line_segment([prev, pt], stroke);
        prev = pt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_input_requires_ready_paused_node() {
        assert!(chat_input_enabled(true, true, true));
        assert!(!chat_input_enabled(false, true, true));
        assert!(!chat_input_enabled(true, false, true));
        assert!(!chat_input_enabled(true, true, false));
    }

    #[test]
    fn chat_send_requires_input_ready_and_text() {
        assert!(chat_send_enabled(true, true, true, "hello"));
        assert!(!chat_send_enabled(false, true, true, "hello"));
        assert!(!chat_send_enabled(true, false, true, "hello"));
        assert!(!chat_send_enabled(true, true, false, "hello"));
        assert!(!chat_send_enabled(true, true, true, "   "));
    }

    #[test]
    fn chat_status_text_is_short() {
        assert_eq!(chat_status_text(true, true), "Ready");
        assert_eq!(chat_status_text(false, true), "API key missing");
        assert_eq!(chat_status_text(true, false), "Paused node needed");
    }

    #[test]
    fn chat_role_presentations_match_inline_transcript_contract() {
        let system = chat_role_presentation(&ChatRole::System);
        assert_eq!(system.label, "System:");
        assert_eq!(system.text_color, TEXT_DIM);
        assert!(!system.monospace);

        let thinking = chat_role_presentation(&ChatRole::Thinking);
        assert_eq!(thinking.label, "Thinking:");
        assert_eq!(thinking.text_color, ACCENT);
        assert!(thinking.monospace);

        let assistant = chat_role_presentation(&ChatRole::Assistant);
        assert_eq!(assistant.label, "Assistant:");
        assert_eq!(assistant.text_color, TEXT_BRIGHT);
        assert!(!assistant.monospace);

        let user = chat_role_presentation(&ChatRole::User);
        assert_eq!(user.label, "You:");
        assert_eq!(user.text_color, TEXT_BRIGHT);
        assert!(!user.monospace);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn chat_section_heights_keep_composer_inside_available_height() {
        let heights = chat_section_heights(300.0, false);

        assert_eq!(heights.composer, CHAT_COMPOSER_RESERVED_HEIGHT);
        assert_eq!(heights.history, 300.0 - CHAT_COMPOSER_RESERVED_HEIGHT);
        assert!(heights.history + heights.composer <= 300.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn chat_section_heights_reserve_error_space_inside_composer_section() {
        let heights = chat_section_heights(300.0, true);

        assert_eq!(
            heights.composer,
            CHAT_COMPOSER_RESERVED_HEIGHT + CHAT_ERROR_RESERVED_HEIGHT
        );
        assert_eq!(
            heights.history,
            300.0 - CHAT_COMPOSER_RESERVED_HEIGHT - CHAT_ERROR_RESERVED_HEIGHT
        );
        assert!(heights.history + heights.composer <= 300.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn chat_section_heights_never_exceed_small_available_height() {
        let heights = chat_section_heights(72.0, true);

        assert_eq!(heights.history, 0.0);
        assert_eq!(heights.composer, 72.0);
    }

    #[test]
    fn chat_selected_skill_summary_is_compact() {
        let none = BTreeSet::new();
        assert_eq!(selected_skill_summary(&none), "Skills");

        let mut one = BTreeSet::new();
        one.insert("writing-plans".to_string());
        assert_eq!(selected_skill_summary(&one), "writing-plans");

        let mut many = BTreeSet::new();
        many.insert("brainstorming".to_string());
        many.insert("writing-plans".to_string());
        assert_eq!(selected_skill_summary(&many), "2 skills");
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn dock_header_content_size_fills_parent_after_margins() {
        let content_size = dock_header_content_size(1024.0);

        assert_eq!(content_size.x, 1000.0);
        assert_eq!(content_size.y, DOCK_HEADER_ROW_HEIGHT);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn dock_header_content_size_never_goes_negative() {
        let content_size = dock_header_content_size(12.0);

        assert_eq!(content_size.x, 0.0);
        assert_eq!(content_size.y, DOCK_HEADER_ROW_HEIGHT);
    }

    #[test]
    fn dock_header_frame_blends_into_dock_surface_without_nested_border() {
        let frame = dock_header_frame();

        assert_eq!(frame.fill, SURFACE_1);
        assert_eq!(frame.stroke, egui::Stroke::NONE);
        assert_eq!(frame.inner_margin.left, DOCK_INSET_X);
        assert_eq!(frame.inner_margin.top, DOCK_HEADER_INSET_Y);
    }

    #[test]
    fn bottom_tab_labels_are_plain_title_case() {
        assert_eq!(BottomPanelTab::Chat.label(), "Chat");
        assert_eq!(BottomPanelTab::RunTrace.label(), "Run Trace");
    }
}

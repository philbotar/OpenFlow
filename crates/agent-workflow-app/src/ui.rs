use crate::canvas_math::{edge_anchor_points, NODE_HEIGHT, NODE_WIDTH};
use crate::state::{AgentStatus, AppState};
use crate::storage::FileWorkflowStore;
use eframe::egui;
use openai_client::OpenAiResponsesClient;
use std::env;
use workflow_core::{RunEventKind, WorkflowRunner};

const ACCENT: egui::Color32 = egui::Color32::from_rgb(76, 148, 255);
const ACCENT_DIM: egui::Color32 = egui::Color32::from_rgb(50, 100, 200);
const SUCCESS: egui::Color32 = egui::Color32::from_rgb(34, 176, 125);
const DANGER: egui::Color32 = egui::Color32::from_rgb(219, 72, 72);
const SURFACE_0: egui::Color32 = egui::Color32::from_rgb(10, 14, 20);
const SURFACE_1: egui::Color32 = egui::Color32::from_rgb(15, 20, 29);
const SURFACE_2: egui::Color32 = egui::Color32::from_rgb(22, 28, 40);
const SURFACE_3: egui::Color32 = egui::Color32::from_rgb(30, 38, 55);
const BORDER: egui::Color32 = egui::Color32::from_rgb(40, 52, 75);
const TEXT_BRIGHT: egui::Color32 = egui::Color32::from_rgb(220, 230, 245);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(120, 135, 160);

pub struct WorkflowApp {
    state: AppState,
    store: FileWorkflowStore,
    runtime: tokio::runtime::Runtime,
}

impl WorkflowApp {
    pub fn new() -> Self {
        let store = FileWorkflowStore::new(FileWorkflowStore::default_path());
        let workflow = store
            .load()
            .ok()
            .and_then(|mut workflows| workflows.pop())
            .unwrap_or_else(|| AppState::new().workflow);

        Self {
            state: AppState {
                workflow,
                ..AppState::new()
            },
            store,
            runtime: tokio::runtime::Runtime::new().expect("tokio runtime starts"),
        }
    }

    fn run_current_workflow(&mut self) {
        if let Err(error) = self.state.validate() {
            self.state.last_error = Some(error.to_string());
            return;
        }

        let env_key = env::var("OPENAI_API_KEY").ok();
        let api_key = match self.state.resolve_api_key(env_key.as_deref()) {
            Some(value) => value,
            _ => {
                self.state.last_error =
                    Some("OpenAI API key missing (UI field and OPENAI_API_KEY empty)".to_string());
                return;
            }
        };

        self.state.last_error = None;
        let client = OpenAiResponsesClient::new(api_key);
        let runner = WorkflowRunner::new(client);
        let entrypoint = self.state.entrypoint_text.trim();
        let entrypoint = (!entrypoint.is_empty()).then_some(entrypoint);
        match self
            .runtime
            .block_on(runner.run_with_entrypoint(&self.state.workflow, entrypoint))
        {
            Ok(report) => {
                self.state.set_run_report(report);
                self.state.refresh_statuses_from_report();
            }
            Err(error) => self.state.last_error = Some(error.to_string()),
        }
    }

    fn save_workflow(&mut self) {
        match self.store.save(std::slice::from_ref(&self.state.workflow)) {
            Ok(()) => self.state.last_error = None,
            Err(error) => self.state.last_error = Some(format!("save failed: {error}")),
        }
    }
}

impl Default for WorkflowApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for WorkflowApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    #[allow(deprecated)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    bg_fill: SURFACE_2,
                    weak_bg_fill: SURFACE_2,
                    bg_stroke: egui::Stroke::new(1.0, BORDER),
                    corner_radius: egui::CornerRadius::same(5),
                    fg_stroke: egui::Stroke::new(1.0, TEXT_BRIGHT),
                    expansion: 0.0,
                },
                hovered: egui::style::WidgetVisuals {
                    bg_fill: SURFACE_3,
                    weak_bg_fill: SURFACE_3,
                    bg_stroke: egui::Stroke::new(1.0, ACCENT),
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

        let run_shortcut =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter));
        if run_shortcut {
            self.run_current_workflow();
        }
        let save_shortcut =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S));
        if save_shortcut {
            self.save_workflow();
        }

        // ── Toolbar ─────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_2)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .stroke(egui::Stroke::new(1.0, BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Left pill: graph mutations
                    egui::Frame::new()
                        .fill(SURFACE_1)
                        .corner_radius(egui::CornerRadius::same(6))
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .inner_margin(egui::Margin::symmetric(4, 2))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            if ui
                                .add(
                                    egui::Button::new("✚  Node")
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(4)),
                                )
                                .on_hover_text("Add agent node")
                                .clicked()
                            {
                                self.state.add_agent_node();
                            }
                            ui.add(egui::Separator::default().vertical());
                            if ui
                                .add(
                                    egui::Button::new("🔗  Link")
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(4)),
                                )
                                .on_hover_text("Begin linking from selected node")
                                .clicked()
                            {
                                self.state.begin_link_from_selected();
                            }
                            ui.add(egui::Separator::default().vertical());
                            if ui
                                .add(
                                    egui::Button::new("🗑  Delete")
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(4)),
                                )
                                .on_hover_text("Delete selected node")
                                .clicked()
                            {
                                self.state.remove_selected_node();
                            }
                        });

                    ui.add_space(8.0);

                    // Middle pill: workflow execution
                    egui::Frame::new()
                        .fill(SURFACE_1)
                        .corner_radius(egui::CornerRadius::same(6))
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .inner_margin(egui::Margin::symmetric(4, 2))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            if ui
                                .add(
                                    egui::Button::new("▶  Run")
                                        .fill(ACCENT)
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .min_size(egui::vec2(64.0, 0.0)),
                                )
                                .on_hover_text("Run workflow (⌘↵)")
                                .clicked()
                            {
                                self.run_current_workflow();
                            }
                            ui.add(egui::Separator::default().vertical());
                            if ui
                                .add(
                                    egui::Button::new("💾  Save")
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(4)),
                                )
                                .on_hover_text("Save workflow (⌘S)")
                                .clicked()
                            {
                                self.save_workflow();
                            }
                            ui.add(egui::Separator::default().vertical());
                            if ui
                                .add(
                                    egui::Button::new("⟳  Clear")
                                        .fill(egui::Color32::TRANSPARENT)
                                        .corner_radius(egui::CornerRadius::same(4)),
                                )
                                .on_hover_text("Clear last run output")
                                .clicked()
                            {
                                self.state.last_run = None;
                                self.state.refresh_statuses_from_report();
                            }
                        });

                    // Right: API key + error
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(error) = &self.state.last_error.clone() {
                            ui.colored_label(DANGER, error);
                            ui.add_space(8.0);
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.state.openai_api_key_input)
                                .password(true)
                                .hint_text("sk-…")
                                .desired_width(180.0),
                        );
                        ui.label(egui::RichText::new("🔑").color(TEXT_DIM).size(12.0));
                    });
                });
            });

        // ── Left sidebar ─────────────────────────────────────────────────────
        egui::SidePanel::left("nodes")
            .resizable(true)
            .default_width(220.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // NODES header
                    egui::Frame::new()
                        .fill(SURFACE_2)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("NODES")
                                        .size(10.0)
                                        .color(TEXT_DIM)
                                        .monospace(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new("✚")
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .small(),
                                            )
                                            .on_hover_text("Add node")
                                            .clicked()
                                        {
                                            self.state.add_agent_node();
                                        }
                                    },
                                );
                            });
                        });

                    for node in self.state.workflow.nodes.clone() {
                        let selected =
                            self.state.selected_node_id.as_ref() == Some(&node.id);
                        let status = self
                            .state
                            .status_by_node
                            .get(&node.id)
                            .copied()
                            .unwrap_or(AgentStatus::Idle);
                        let status_color = match status {
                            AgentStatus::Idle => egui::Color32::from_gray(120),
                            AgentStatus::Queued => egui::Color32::from_rgb(120, 120, 220),
                            AgentStatus::Started => ACCENT,
                            AgentStatus::Completed => SUCCESS,
                            AgentStatus::Failed => DANGER,
                        };

                        let available_width = ui.available_width();
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(available_width, 28.0),
                            egui::Sense::click(),
                        );

                        let bg = if selected {
                            SURFACE_3
                        } else if response.hovered() {
                            egui::Color32::from_rgba_premultiplied(30, 38, 55, 180)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, 0.0, bg);
                        ui.painter().circle_filled(
                            egui::pos2(rect.left() + 14.0, rect.center().y),
                            3.5,
                            status_color,
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 26.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &node.label,
                            egui::TextStyle::Body.resolve(ui.style()),
                            if selected { TEXT_BRIGHT } else { TEXT_DIM },
                        );

                        if self.state.link_from_node_id.is_some()
                            && self.state.link_from_node_id.as_ref() != Some(&node.id)
                        {
                            let btn_rect = egui::Rect::from_center_size(
                                egui::pos2(rect.right() - 16.0, rect.center().y),
                                egui::vec2(24.0, 18.0),
                            );
                            if ui
                                .put(
                                    btn_rect,
                                    egui::Button::new("→")
                                        .fill(ACCENT_DIM)
                                        .small(),
                                )
                                .clicked()
                            {
                                self.state.connect_link_to(node.id.clone());
                            }
                        }

                        if response.clicked() {
                            self.state.select_node(node.id.clone());
                        }
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // EDGES header
                    egui::Frame::new()
                        .fill(SURFACE_2)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("EDGES")
                                    .size(10.0)
                                    .color(TEXT_DIM)
                                    .monospace(),
                            );
                        });

                    for row in self.state.edge_rows() {
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(18, 4))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("→ {row}"))
                                        .size(12.0)
                                        .color(TEXT_DIM),
                                );
                            });
                    }
                });
            });

        // ── Inspector ────────────────────────────────────────────────────────
        egui::SidePanel::right("inspector")
            .resizable(true)
            .default_width(300.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    egui::Frame::new()
                        .fill(SURFACE_2)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("INSPECTOR")
                                    .size(10.0)
                                    .color(TEXT_DIM)
                                    .monospace(),
                            );
                        });

                    ui.add_space(4.0);

                    egui::CollapsingHeader::new(
                        egui::RichText::new("Agent Config")
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        if let Some(node) = self.state.selected_node_mut() {
                            inspector_row(ui, "Label", |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut node.label)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            inspector_row(ui, "Model", |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut node.agent.model)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            inspector_row_tall(ui, "System", |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut node.agent.system_prompt)
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            inspector_row_tall(ui, "Task", |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut node.agent.task_prompt)
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                        } else {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("No node selected")
                                    .size(12.0)
                                    .color(TEXT_DIM)
                                    .italics(),
                            );
                            ui.add_space(6.0);
                        }
                    });

                    ui.add_space(4.0);

                    egui::CollapsingHeader::new(
                        egui::RichText::new("Output Schema")
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.state.schema_editor_text)
                                .desired_rows(8)
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(4.0);
                        if ui
                            .add(egui::Button::new("Apply Schema").fill(SURFACE_3))
                            .clicked()
                        {
                            self.state.apply_schema_editor();
                        }
                    });

                    ui.add_space(4.0);

                    egui::CollapsingHeader::new(
                        egui::RichText::new("Entrypoint")
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.state.entrypoint_text)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("Describe what the first agent should do…"),
                        );
                    });
                });
            });

        // ── Canvas ───────────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(SURFACE_0).inner_margin(egui::Margin::same(0)))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("⬡").color(ACCENT).size(14.0));
                            ui.label(
                                egui::RichText::new(&self.state.workflow.name)
                                    .size(13.0)
                                    .color(TEXT_BRIGHT),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("WORKFLOW")
                                            .size(10.0)
                                            .color(TEXT_DIM)
                                            .monospace(),
                                    );
                                },
                            );
                        });
                    });

                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::click());
                let rect = response.rect;
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, BORDER),
                    egui::StrokeKind::Inside,
                );

                let node_size = (NODE_WIDTH, NODE_HEIGHT);
                for edge in &self.state.workflow.edges {
                    let from = self
                        .state
                        .workflow
                        .nodes
                        .iter()
                        .find(|node| node.id == edge.from);
                    let to = self
                        .state
                        .workflow
                        .nodes
                        .iter()
                        .find(|node| node.id == edge.to);
                    if let (Some(from), Some(to)) = (from, to) {
                        let (start_offset, end_offset) = edge_anchor_points(
                            (from.position.x, from.position.y),
                            (to.position.x, to.position.y),
                            node_size,
                        );
                        let start =
                            rect.left_top() + egui::vec2(start_offset.0, start_offset.1);
                        let end = rect.left_top() + egui::vec2(end_offset.0, end_offset.1);
                        painter.line_segment([start, end], egui::Stroke::new(1.5, BORDER));
                    }
                }

                for node in self.state.workflow.nodes.clone() {
                    let min = rect.left_top() + egui::vec2(node.position.x, node.position.y);
                    let node_rect =
                        egui::Rect::from_min_size(min, egui::vec2(node_size.0, node_size.1));
                    let selected = self.state.selected_node_id.as_ref() == Some(&node.id);
                    let fill = if selected { SURFACE_3 } else { SURFACE_2 };
                    painter.rect_filled(node_rect, 6.0, fill);
                    painter.rect_stroke(
                        node_rect,
                        6.0,
                        egui::Stroke::new(
                            if selected { 1.5 } else { 1.0 },
                            if selected { ACCENT } else { BORDER },
                        ),
                        egui::StrokeKind::Inside,
                    );

                    let node_id = egui::Id::new(("node", node.id.clone()));
                    let node_response =
                        ui.interact(node_rect, node_id, egui::Sense::click_and_drag());
                    if node_response.dragged() {
                        let delta = node_response.drag_motion();
                        self.state.move_node_by_delta(
                            &node.id,
                            delta.x,
                            delta.y,
                            (rect.width(), rect.height()),
                            node_size,
                        );
                    }
                    if node_response.clicked() {
                        self.state.select_node(node.id.clone());
                    }

                    let status = self
                        .state
                        .status_by_node
                        .get(&node.id)
                        .copied()
                        .unwrap_or(AgentStatus::Idle);
                    let (status_text, status_color) = match status {
                        AgentStatus::Idle => ("IDLE", egui::Color32::from_gray(120)),
                        AgentStatus::Queued => {
                            ("QUEUED", egui::Color32::from_rgb(120, 120, 220))
                        }
                        AgentStatus::Started => ("RUNNING", ACCENT),
                        AgentStatus::Completed => ("DONE", SUCCESS),
                        AgentStatus::Failed => ("FAILED", DANGER),
                    };
                    painter.circle_filled(
                        node_rect.left_top() + egui::vec2(12.0, 12.0),
                        3.5,
                        status_color,
                    );
                    painter.text(
                        node_rect.left_top() + egui::vec2(22.0, 7.0),
                        egui::Align2::LEFT_TOP,
                        status_text,
                        egui::TextStyle::Small.resolve(ui.style()),
                        status_color,
                    );
                    painter.text(
                        node_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &node.label,
                        egui::TextStyle::Button.resolve(ui.style()),
                        TEXT_BRIGHT,
                    );
                }
            });

        // ── Run trace ────────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("run_trace")
            .resizable(true)
            .default_height(160.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(SURFACE_2)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("RUN TRACE")
                                    .size(10.0)
                                    .color(TEXT_DIM)
                                    .monospace(),
                            );
                            if self.state.last_run.is_some() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new("⟳  Clear")
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .small(),
                                            )
                                            .clicked()
                                        {
                                            self.state.last_run = None;
                                            self.state.refresh_statuses_from_report();
                                        }
                                    },
                                );
                            }
                        });
                    });

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if let Some(report) = &self.state.last_run.clone() {
                            for event in &report.events {
                                let (status_text, dot_color) = match event.kind {
                                    RunEventKind::Queued => {
                                        ("queued", egui::Color32::from_rgb(120, 120, 220))
                                    }
                                    RunEventKind::Started => ("running", ACCENT),
                                    RunEventKind::Completed => ("done", SUCCESS),
                                    RunEventKind::Failed => ("failed", DANGER),
                                };
                                let node_label = self
                                    .state
                                    .workflow
                                    .nodes
                                    .iter()
                                    .find(|n| n.id == event.node_id)
                                    .map(|n| n.label.as_str())
                                    .unwrap_or(event.node_id.as_str());

                                egui::Frame::new()
                                    .inner_margin(egui::Margin::symmetric(12, 4))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let (dot_rect, _) = ui.allocate_exact_size(
                                                egui::vec2(12.0, 16.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().circle_filled(
                                                egui::pos2(
                                                    dot_rect.left() + 4.0,
                                                    dot_rect.center().y,
                                                ),
                                                4.0,
                                                dot_color,
                                            );
                                            ui.label(
                                                egui::RichText::new(node_label)
                                                    .size(12.0)
                                                    .color(TEXT_BRIGHT),
                                            );
                                            ui.label(
                                                egui::RichText::new(status_text)
                                                    .size(11.0)
                                                    .color(dot_color),
                                            );
                                            ui.label(
                                                egui::RichText::new(&event.message)
                                                    .size(11.0)
                                                    .color(TEXT_DIM),
                                            );
                                        });
                                        if let Some(output) = &event.output {
                                            egui::Frame::new()
                                                .fill(SURFACE_0)
                                                .corner_radius(egui::CornerRadius::same(4))
                                                .inner_margin(egui::Margin::symmetric(8, 4))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        egui::RichText::new(
                                                            output.to_string(),
                                                        )
                                                        .size(11.0)
                                                        .color(TEXT_DIM)
                                                        .monospace(),
                                                    );
                                                });
                                        }
                                    });
                            }
                        } else {
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(12, 12))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            "No run yet — press ▶ Run to execute the workflow.",
                                        )
                                        .size(12.0)
                                        .color(TEXT_DIM)
                                        .italics(),
                                    );
                                });
                        }
                    });
            });
    }
}

fn inspector_row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::vec2(80.0, 16.0),
            egui::Label::new(egui::RichText::new(label).size(11.0).color(TEXT_DIM)),
        );
        content(ui);
    });
    ui.add_space(2.0);
}

fn inspector_row_tall(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.label(egui::RichText::new(label).size(11.0).color(TEXT_DIM));
    content(ui);
    ui.add_space(2.0);
}

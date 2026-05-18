use crate::canvas_math::{edge_anchor_points, NODE_HEIGHT, NODE_WIDTH};
use crate::state::{AgentStatus, AppState};
use crate::storage::FileWorkflowStore;
use eframe::egui;
use openai_client::OpenAiResponsesClient;
use std::env;
use workflow_core::{RunEventKind, WorkflowRunner};

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
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Add node").clicked() {
                    self.state.add_agent_node();
                }
                if ui.button("Link from selected").clicked() {
                    self.state.begin_link_from_selected();
                }
                if ui.button("Run").clicked() {
                    self.run_current_workflow();
                }
                if ui.button("Save").clicked() {
                    self.save_workflow();
                }
                ui.separator();
                ui.label("OpenAI key");
                ui.add(
                    egui::TextEdit::singleline(&mut self.state.openai_api_key_input)
                        .password(true)
                        .hint_text("sk-...")
                        .desired_width(220.0),
                );
                if let Some(error) = &self.state.last_error {
                    ui.colored_label(egui::Color32::from_rgb(190, 40, 40), error);
                }
            });
        });

        egui::SidePanel::left("nodes")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Nodes");
                for node in self.state.workflow.nodes.clone() {
                    let selected = self.state.selected_node_id.as_ref() == Some(&node.id);
                    if ui.selectable_label(selected, &node.label).clicked() {
                        self.state.select_node(node.id.clone());
                    }
                    if self.state.link_from_node_id.is_some()
                        && self.state.link_from_node_id.as_ref() != Some(&node.id)
                        && ui.button(format!("Connect to {}", node.label)).clicked()
                    {
                        self.state.connect_link_to(node.id.clone());
                    }
                }

                ui.separator();
                ui.heading("Edges");
                for row in self.state.edge_rows() {
                    ui.label(row);
                }
            });

        egui::SidePanel::right("inspector")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                if let Some(node) = self.state.selected_node_mut() {
                    ui.label("Label");
                    ui.text_edit_singleline(&mut node.label);
                    ui.label("Model");
                    ui.text_edit_singleline(&mut node.agent.model);
                    ui.label("System prompt");
                    ui.text_edit_multiline(&mut node.agent.system_prompt);
                    ui.label("Task prompt");
                    ui.text_edit_multiline(&mut node.agent.task_prompt);
                }

                ui.label("Output JSON Schema");
                ui.text_edit_multiline(&mut self.state.schema_editor_text);
                if ui.button("Apply schema").clicked() {
                    self.state.apply_schema_editor();
                }

                ui.separator();
                ui.label("Entrypoint input for root agents");
                ui.add(
                    egui::TextEdit::multiline(&mut self.state.entrypoint_text)
                        .desired_rows(4)
                        .hint_text("Describe what the first agent should do"),
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.state.workflow.name);
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::click());
            let rect = response.rect;
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
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
                    let start = rect.left_top() + egui::vec2(start_offset.0, start_offset.1);
                    let end = rect.left_top() + egui::vec2(end_offset.0, end_offset.1);
                    painter.line_segment(
                        [start, end],
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 110, 130)),
                    );
                }
            }

            for node in self.state.workflow.nodes.clone() {
                let min = rect.left_top() + egui::vec2(node.position.x, node.position.y);
                let node_rect =
                    egui::Rect::from_min_size(min, egui::vec2(node_size.0, node_size.1));
                let selected = self.state.selected_node_id.as_ref() == Some(&node.id);
                let fill = if selected {
                    egui::Color32::from_rgb(235, 246, 255)
                } else {
                    egui::Color32::from_rgb(245, 245, 245)
                };
                painter.rect_filled(node_rect, 6.0, fill);
                painter.rect_stroke(
                    node_rect,
                    6.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 90, 105)),
                    egui::StrokeKind::Inside,
                );

                let node_id = egui::Id::new(("node", node.id.clone()));
                let node_response = ui.interact(node_rect, node_id, egui::Sense::click_and_drag());
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
                    AgentStatus::Queued => ("QUEUED", egui::Color32::from_rgb(120, 120, 220)),
                    AgentStatus::Started => ("RUNNING", egui::Color32::from_rgb(76, 148, 255)),
                    AgentStatus::Completed => ("DONE", egui::Color32::from_rgb(34, 176, 125)),
                    AgentStatus::Failed => ("FAILED", egui::Color32::from_rgb(219, 72, 72)),
                };
                painter.text(
                    node_rect.left_top() + egui::vec2(10.0, 10.0),
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
                    egui::Color32::from_rgb(20, 24, 30),
                );
            }
        });

        egui::TopBottomPanel::bottom("run_trace")
            .resizable(true)
            .default_height(180.0)
            .show(ctx, |ui| {
                ui.heading("Run trace");
                if let Some(report) = &self.state.last_run {
                    for event in &report.events {
                        let status = match event.kind {
                            RunEventKind::Queued => "queued",
                            RunEventKind::Started => "started",
                            RunEventKind::Completed => "completed",
                            RunEventKind::Failed => "failed",
                        };
                        ui.label(format!(
                            "{} | {} | {}",
                            event.node_id, status, event.message
                        ));
                        if let Some(output) = &event.output {
                            ui.monospace(output.to_string());
                        }
                    }
                }
            });
    }
}

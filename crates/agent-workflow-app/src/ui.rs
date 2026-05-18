use crate::state::AppState;
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

        let api_key = match env::var("OPENAI_API_KEY") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                self.state.last_error = Some("OPENAI_API_KEY is not configured".to_string());
                return;
            }
        };

        let client = OpenAiResponsesClient::new(api_key);
        let runner = WorkflowRunner::new(client);
        match self.runtime.block_on(runner.run(&self.state.workflow)) {
            Ok(report) => self.state.set_run_report(report),
            Err(error) => self.state.last_error = Some(error.to_string()),
        }
    }

    fn save_workflow(&mut self) {
        match self.store.save(&[self.state.workflow.clone()]) {
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
                for edge in &self.state.workflow.edges {
                    ui.label(format!("{} -> {}", edge.from, edge.to));
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
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.state.workflow.name);
            let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click());
            let rect = response.rect;
            painter.rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
                egui::StrokeKind::Inside,
            );

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
                    let start = rect.left_top() + egui::vec2(from.position.x + 140.0, from.position.y + 32.0);
                    let end = rect.left_top() + egui::vec2(to.position.x, to.position.y + 32.0);
                    painter.line_segment(
                        [start, end],
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 110, 130)),
                    );
                }
            }

            for node in &self.state.workflow.nodes {
                let min = rect.left_top() + egui::vec2(node.position.x, node.position.y);
                let node_rect = egui::Rect::from_min_size(min, egui::vec2(140.0, 64.0));
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
                        ui.label(format!("{} | {} | {}", event.node_id, status, event.message));
                        if let Some(output) = &event.output {
                            ui.monospace(output.to_string());
                        }
                    }
                }
            });
    }
}

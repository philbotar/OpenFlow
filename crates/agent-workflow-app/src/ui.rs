use crate::canvas_math::{edge_anchor_points, NODE_HEIGHT, NODE_WIDTH};
use crate::state::{AgentStatus, AppState};
use crate::storage::FileWorkflowStore;
use eframe::egui;
use std::env;
use workflow_core::{RunEventKind, Workflow, WorkflowRunner};

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
    workflows: Vec<Workflow>,
    active: usize,
    state: AppState,
    store: FileWorkflowStore,
    runtime: tokio::runtime::Runtime,
    show_settings: bool,
}

impl WorkflowApp {
    pub fn new() -> Self {
        let store = FileWorkflowStore::new(FileWorkflowStore::default_path());
        let mut workflows = store.load().unwrap_or_default();
        if workflows.is_empty() {
            workflows.push(AppState::new().workflow);
        }
        let state = state_from_workflow(workflows[0].clone(), String::new());
        Self {
            workflows,
            active: 0,
            state,
            store,
            runtime: tokio::runtime::Runtime::new().expect("tokio runtime"),
            show_settings: false,
        }
    }

    fn sync_active(&mut self) {
        if self.active < self.workflows.len() {
            self.workflows[self.active] = self.state.workflow.clone();
        }
    }

    fn switch_to(&mut self, index: usize) {
        if index == self.active {
            return;
        }
        self.sync_active();
        self.active = index;
        let workflow = self.workflows[index].clone();
        let api_key = self.state.openai_api_key_input.clone();
        self.state = state_from_workflow(workflow, api_key);
    }

    fn new_workflow(&mut self) {
        self.sync_active();
        let count = self.workflows.len() + 1;
        let mut fresh = AppState::new();
        fresh.workflow.name = format!("Workflow {count}");
        self.workflows.push(fresh.workflow.clone());
        let api_key = self.state.openai_api_key_input.clone();
        self.active = self.workflows.len() - 1;
        fresh.openai_api_key_input = api_key;
        self.state = fresh;
    }

    fn run_current_workflow(&mut self) {
        if let Err(error) = self.state.validate() {
            self.state.last_error = Some(error.to_string());
            return;
        }
        let env_key = env::var("OPENAI_API_KEY").ok();
        let api_key = match self.state.resolve_api_key(env_key.as_deref()) {
            Some(v) => v,
            None => {
                self.state.last_error =
                    Some("OpenAI API key missing (set in Settings or OPENAI_API_KEY)".into());
                return;
            }
        };
        self.state.last_error = None;
        let client = openai_client::OpenAiResponsesClient::new(api_key);
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

    fn save_all(&mut self) {
        self.sync_active();
        match self.store.save(&self.workflows) {
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
        // Global theme
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

        // Keyboard shortcuts
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
            self.run_current_workflow();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save_all();
        }

        // ── Left nav: workflow list ───────────────────────────────────────────
        let mut switch_to: Option<usize> = None;
        let mut do_new = false;
        let mut do_save = false;
        let do_run = false;
        let mut toggle_settings = false;

        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(220.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                ui.set_height(ui.available_height());

                // Header
                egui::Frame::new()
                    .fill(SURFACE_2)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("WORKFLOWS")
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
                                        .on_hover_text("New workflow")
                                        .clicked()
                                    {
                                        do_new = true;
                                    }
                                },
                            );
                        });
                    });

                // Workflow list
                egui::ScrollArea::vertical()
                    .id_salt("nav_scroll")
                    .max_height(ui.available_height() - 48.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for (i, workflow) in self.workflows.iter().enumerate() {
                            let is_active = i == self.active;
                            let available_width = ui.available_width();
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(available_width, 36.0),
                                egui::Sense::click(),
                            );
                            let bg = if is_active {
                                SURFACE_3
                            } else if response.hovered() {
                                egui::Color32::from_rgba_premultiplied(30, 38, 55, 160)
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(rect, 0.0, bg);

                            // Active indicator bar
                            if is_active {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        rect.left_top(),
                                        egui::vec2(2.0, rect.height()),
                                    ),
                                    0.0,
                                    ACCENT,
                                );
                            }

                            ui.painter().text(
                                egui::pos2(rect.left() + 16.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &workflow.name,
                                egui::TextStyle::Body.resolve(ui.style()),
                                if is_active { TEXT_BRIGHT } else { TEXT_DIM },
                            );

                            if response.clicked() && !is_active {
                                switch_to = Some(i);
                            }
                        }
                    });

                // Bottom: Settings pinned
                let bottom_rect = ui.available_rect_before_wrap();
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_max(
                        egui::pos2(bottom_rect.left(), bottom_rect.bottom() - 48.0),
                        bottom_rect.right_bottom(),
                    ),
                    |ui| {
                        ui.add(egui::Separator::default().horizontal());
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            let settings_fill = if self.show_settings { SURFACE_3 } else { egui::Color32::TRANSPARENT };
                            if ui
                                .add(
                                    egui::Button::new("⚙  Settings")
                                        .fill(settings_fill)
                                        .corner_radius(egui::CornerRadius::same(5)),
                                )
                                .clicked()
                            {
                                toggle_settings = true;
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(8.0);
                                if ui
                                    .add(
                                        egui::Button::new("↓")
                                            .fill(egui::Color32::TRANSPARENT)
                                            .small(),
                                    )
                                    .on_hover_text("Save all (⌘S)")
                                    .clicked()
                                {
                                    do_save = true;
                                }
                            });
                        });
                    },
                );
            });

        // Apply nav actions (deferred to avoid borrow conflicts)
        if let Some(i) = switch_to {
            self.switch_to(i);
        }
        if do_new {
            self.new_workflow();
        }
        if do_save {
            self.save_all();
        }
        if toggle_settings {
            self.show_settings = !self.show_settings;
        }
        if do_run {
            self.run_current_workflow();
        }

        // ── Right panel: Inspector or Settings ───────────────────────────────
        let mut add_node = false;
        let mut begin_link = false;
        let mut delete_node = false;
        let mut apply_schema = false;

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
                if self.show_settings {
                    show_settings_panel(ui, &mut self.state);
                } else {
                    show_inspector_panel(
                        ui,
                        &mut self.state,
                        &mut add_node,
                        &mut begin_link,
                        &mut delete_node,
                        &mut apply_schema,
                    );
                }
            });

        if add_node {
            self.state.add_agent_node();
        }
        if begin_link {
            self.state.begin_link_from_selected();
        }
        if delete_node {
            self.state.remove_selected_node();
        }
        if apply_schema {
            self.state.apply_schema_editor();
        }

        // ── Run trace ─────────────────────────────────────────────────────────
        let mut clear_run = false;
        egui::TopBottomPanel::bottom("run_trace")
            .resizable(true)
            .default_height(150.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE_1)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(SURFACE_2)
                    .inner_margin(egui::Margin::symmetric(12, 5))
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
                                            clear_run = true;
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
                                    .inner_margin(egui::Margin::symmetric(12, 3))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let (dot_rect, _) = ui.allocate_exact_size(
                                                egui::vec2(12.0, 16.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().circle_filled(
                                                egui::pos2(dot_rect.left() + 4.0, dot_rect.center().y),
                                                4.0,
                                                dot_color,
                                            );
                                            ui.label(egui::RichText::new(node_label).size(12.0).color(TEXT_BRIGHT));
                                            ui.label(egui::RichText::new(status_text).size(11.0).color(dot_color));
                                            ui.label(egui::RichText::new(&event.message).size(11.0).color(TEXT_DIM));
                                        });
                                        if let Some(output) = &event.output {
                                            egui::Frame::new()
                                                .fill(SURFACE_0)
                                                .corner_radius(egui::CornerRadius::same(4))
                                                .inner_margin(egui::Margin::symmetric(8, 4))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        egui::RichText::new(output.to_string())
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
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("No run yet — press ▶ Run to execute the workflow.")
                                            .size(12.0)
                                            .color(TEXT_DIM)
                                            .italics(),
                                    );
                                });
                        }
                    });
            });

        if clear_run {
            self.state.last_run = None;
            self.state.refresh_statuses_from_report();
        }

        // ── Canvas (CentralPanel) ─────────────────────────────────────────────
        let mut run_clicked = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(SURFACE_0).inner_margin(egui::Margin::same(0)))
            .show(ctx, |ui| {
                let canvas_rect = ui.available_rect_before_wrap();
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

                // Edges
                for edge in &self.state.workflow.edges {
                    let from = self.state.workflow.nodes.iter().find(|n| n.id == edge.from);
                    let to = self.state.workflow.nodes.iter().find(|n| n.id == edge.to);
                    if let (Some(from), Some(to)) = (from, to) {
                        let (s, e) = edge_anchor_points(
                            (from.position.x, from.position.y),
                            (to.position.x, to.position.y),
                            node_size,
                        );
                        painter.line_segment(
                            [
                                rect.left_top() + egui::vec2(s.0, s.1),
                                rect.left_top() + egui::vec2(e.0, e.1),
                            ],
                            egui::Stroke::new(1.5, BORDER),
                        );
                    }
                }

                // Nodes
                for node in self.state.workflow.nodes.clone() {
                    let min = rect.left_top() + egui::vec2(node.position.x, node.position.y);
                    let node_rect =
                        egui::Rect::from_min_size(min, egui::vec2(node_size.0, node_size.1));
                    let selected = self.state.selected_node_id.as_ref() == Some(&node.id);
                    painter.rect_filled(node_rect, 6.0, if selected { SURFACE_3 } else { SURFACE_2 });
                    painter.rect_stroke(
                        node_rect,
                        6.0,
                        egui::Stroke::new(
                            if selected { 1.5 } else { 1.0 },
                            if selected { ACCENT } else { BORDER },
                        ),
                        egui::StrokeKind::Inside,
                    );

                    let node_response =
                        ui.interact(node_rect, egui::Id::new(("node", node.id.clone())), egui::Sense::click_and_drag());
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

                    let status = self.state.status_by_node.get(&node.id).copied().unwrap_or(AgentStatus::Idle);
                    let (status_text, status_color) = match status {
                        AgentStatus::Idle => ("IDLE", egui::Color32::from_gray(100)),
                        AgentStatus::Queued => ("QUEUED", egui::Color32::from_rgb(120, 120, 220)),
                        AgentStatus::Started => ("RUNNING", ACCENT),
                        AgentStatus::Completed => ("DONE", SUCCESS),
                        AgentStatus::Failed => ("FAILED", DANGER),
                    };
                    painter.circle_filled(node_rect.left_top() + egui::vec2(12.0, 12.0), 3.5, status_color);
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

                // Error toast (if any)
                if let Some(error) = &self.state.last_error {
                    let toast_w = (error.len() as f32 * 7.5).clamp(200.0, canvas_rect.width() - 32.0);
                    let toast_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            canvas_rect.center().x - toast_w / 2.0,
                            canvas_rect.top() + 12.0,
                        ),
                        egui::vec2(toast_w, 32.0),
                    );
                    painter.rect_filled(toast_rect, 6.0, egui::Color32::from_rgb(60, 20, 20));
                    painter.rect_stroke(toast_rect, 6.0, egui::Stroke::new(1.0, DANGER), egui::StrokeKind::Inside);
                    painter.text(
                        toast_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        error,
                        egui::TextStyle::Body.resolve(ui.style()),
                        DANGER,
                    );
                }

                // Floating ▶ Run button (bottom-right of canvas)
                let run_size = egui::vec2(88.0, 34.0);
                let run_pos = egui::pos2(
                    canvas_rect.right() - run_size.x - 16.0,
                    canvas_rect.bottom() - run_size.y - 16.0,
                );
                let run_rect = egui::Rect::from_min_size(run_pos, run_size);
                if ui.put(run_rect, egui::Button::new("▶  Run").fill(ACCENT).corner_radius(egui::CornerRadius::same(8))).clicked() {
                    run_clicked = true;
                }
            });

        if run_clicked {
            self.run_current_workflow();
        }
    }
}

fn state_from_workflow(workflow: Workflow, api_key: String) -> AppState {
    let selected_node_id = workflow.nodes.first().map(|n| n.id.clone());
    let schema_editor_text = workflow
        .nodes
        .first()
        .map(|n| serde_json::to_string_pretty(&n.agent.output_schema).unwrap_or_default())
        .unwrap_or_default();
    let status_by_node = workflow.nodes.iter().map(|n| (n.id.clone(), AgentStatus::Idle)).collect();
    AppState {
        workflow,
        selected_node_id,
        link_from_node_id: None,
        schema_editor_text,
        openai_api_key_input: api_key,
        entrypoint_text: String::new(),
        status_by_node,
        last_run: None,
        last_error: None,
    }
}

fn show_inspector_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    add_node: &mut bool,
    begin_link: &mut bool,
    delete_node: &mut bool,
    apply_schema: &mut bool,
) {
    // Node action bar at top
    egui::Frame::new()
        .fill(SURFACE_2)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("INSPECTOR").size(10.0).color(TEXT_DIM).monospace());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new("🗑").fill(egui::Color32::TRANSPARENT).small())
                        .on_hover_text("Delete selected node")
                        .clicked()
                    {
                        *delete_node = true;
                    }
                    if ui
                        .add(egui::Button::new("🔗").fill(egui::Color32::TRANSPARENT).small())
                        .on_hover_text("Link from selected")
                        .clicked()
                    {
                        *begin_link = true;
                    }
                    if ui
                        .add(egui::Button::new("✚").fill(egui::Color32::TRANSPARENT).small())
                        .on_hover_text("Add node")
                        .clicked()
                    {
                        *add_node = true;
                    }
                });
            });
        });

    // Link-mode connect targets
    if state.link_from_node_id.is_some() {
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(20, 30, 50))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Click a node in the list to connect →").size(11.0).color(ACCENT));
                for node in state.workflow.nodes.clone() {
                    if state.link_from_node_id.as_ref() != Some(&node.id) {
                        if ui
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
                }
            });
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add_space(4.0);

        egui::CollapsingHeader::new(egui::RichText::new("Agent Config").size(12.0).color(TEXT_BRIGHT))
            .default_open(true)
            .show(ui, |ui| {
                if let Some(node) = state.selected_node_mut() {
                    inspector_row(ui, "Label", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut node.label).desired_width(f32::INFINITY));
                    });
                    inspector_row(ui, "Model", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut node.agent.model).desired_width(f32::INFINITY));
                    });
                    inspector_row_tall(ui, "System", |ui| {
                        ui.add(egui::TextEdit::multiline(&mut node.agent.system_prompt).desired_rows(3).desired_width(f32::INFINITY));
                    });
                    inspector_row_tall(ui, "Task", |ui| {
                        ui.add(egui::TextEdit::multiline(&mut node.agent.task_prompt).desired_rows(3).desired_width(f32::INFINITY));
                    });
                } else {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Select a node on the canvas").size(12.0).color(TEXT_DIM).italics());
                    ui.add_space(6.0);
                }
            });

        ui.add_space(4.0);

        egui::CollapsingHeader::new(egui::RichText::new("Output Schema").size(12.0).color(TEXT_BRIGHT))
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut state.schema_editor_text).desired_rows(8).desired_width(f32::INFINITY));
                ui.add_space(4.0);
                if ui.add(egui::Button::new("Apply Schema").fill(SURFACE_3)).clicked() {
                    *apply_schema = true;
                }
            });

        ui.add_space(4.0);

        egui::CollapsingHeader::new(egui::RichText::new("Entrypoint").size(12.0).color(TEXT_BRIGHT))
            .default_open(true)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut state.entrypoint_text)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .hint_text("Describe what the first agent should do…"),
                );
            });
    });
}

fn show_settings_panel(ui: &mut egui::Ui, state: &mut AppState) {
    egui::Frame::new()
        .fill(SURFACE_2)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("SETTINGS").size(10.0).color(TEXT_DIM).monospace());
        });

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add_space(8.0);

        egui::CollapsingHeader::new(egui::RichText::new("API").size(12.0).color(TEXT_BRIGHT))
            .default_open(true)
            .show(ui, |ui| {
                inspector_row(ui, "OpenAI key", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut state.openai_api_key_input)
                            .password(true)
                            .hint_text("sk-…")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(SURFACE_0)
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("You can also set OPENAI_API_KEY in your environment.")
                                .size(11.0)
                                .color(TEXT_DIM),
                        );
                    });
            });

        ui.add_space(4.0);

        egui::CollapsingHeader::new(egui::RichText::new("Workflow").size(12.0).color(TEXT_BRIGHT))
            .default_open(true)
            .show(ui, |ui| {
                inspector_row(ui, "Name", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut state.workflow.name).desired_width(f32::INFINITY));
                });
            });
    });
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

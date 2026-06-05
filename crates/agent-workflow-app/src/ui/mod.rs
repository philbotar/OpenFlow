mod canvas;
mod inspector;
mod nav;
mod settings;
mod theme;
mod widgets;

use crate::execution::{spawn_interactive_workflow_run, ExecutionAction, ExecutionEvent};
use crate::provider_config::{resolve_provider_config, ProviderEnv};
use crate::settings_store::{AiProviderKind, AppSettings, FileSettingsStore};
use crate::state::{AgentStatus, AppState, RunTraceEntry, TraceStatus};
use crate::storage::FileWorkflowStore;
use eframe::egui;
use egui_phosphor::regular as ph;
use openai_client::{OpenAiClient, OpenAiClientConfig};
use std::env;
#[allow(clippy::wildcard_imports)]
use theme::*;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use workflow_core::{ChatRole, Workflow};

const COLLAPSED_NAV_ICON_SIZE: f32 = 34.0;
const COLLAPSED_NAV_ICON_RADIUS: u8 = 10;
const COLLAPSED_NAV_GLYPH_SIZE: f32 = 16.0;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn collapsed_nav_icon_bg(t: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (f32::from(SURFACE_3.r()) - f32::from(SURFACE_2.r())).mul_add(t, f32::from(SURFACE_2.r()))
            as u8,
        (f32::from(SURFACE_3.g()) - f32::from(SURFACE_2.g())).mul_add(t, f32::from(SURFACE_2.g()))
            as u8,
        (f32::from(SURFACE_3.b()) - f32::from(SURFACE_2.b())).mul_add(t, f32::from(SURFACE_2.b()))
            as u8,
    )
}

pub struct WorkflowApp {
    workflows: Vec<Workflow>,
    active: usize,
    workflow_rename_state: nav::WorkflowRenameState,
    state: AppState,
    store: FileWorkflowStore,
    settings: AppSettings,
    settings_snapshot: AppSettings,
    settings_store: FileSettingsStore,
    runtime: tokio::runtime::Runtime,
    show_settings: bool,
    show_sidebar: bool,
    bottom_panel_tab: canvas::BottomPanelTab,
    chat_dock_state: canvas::ChatDockState,
    // NEW execution fields
    execution_task: Option<tokio::task::JoinHandle<()>>,
    event_rx: Option<UnboundedReceiver<ExecutionEvent>>,
    action_tx: Option<UnboundedSender<ExecutionAction>>,
    chat_input_buffer: String,
}

impl WorkflowApp {
    /// # Panics
    /// Panics if the tokio runtime cannot be created.
    #[must_use]
    pub fn new() -> Self {
        let store = FileWorkflowStore::new(FileWorkflowStore::default_path());
        let mut workflows = store.load().unwrap_or_default();
        if workflows.is_empty() {
            workflows.push(AppState::new().workflow);
        }
        let state = AppState::from_workflow(workflows[0].clone(), String::new());
        let settings_store = FileSettingsStore::new(FileSettingsStore::default_path());
        let settings = settings_store.load().unwrap_or_default();
        let snapshot = settings.clone();
        Self {
            workflows,
            active: 0,
            workflow_rename_state: nav::WorkflowRenameState::default(),
            state,
            store,
            settings,
            settings_snapshot: snapshot,
            settings_store,
            runtime: tokio::runtime::Runtime::new().expect("tokio runtime"),
            show_settings: false,
            show_sidebar: true,
            bottom_panel_tab: canvas::BottomPanelTab::Chat,
            chat_dock_state: canvas::ChatDockState::default(),
            execution_task: None,
            event_rx: None,
            action_tx: None,
            chat_input_buffer: String::new(),
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
        let provider_api_key = self.state.provider_api_key_input.clone();
        self.state = AppState::from_workflow(workflow, provider_api_key);
    }

    fn new_workflow(&mut self) {
        self.sync_active();
        let count = self.workflows.len() + 1;
        let mut workflow = AppState::new().workflow;
        workflow.name = format!("Workflow {count}");
        self.workflows.push(workflow.clone());
        let api_key = self.state.provider_api_key_input.clone();
        self.active = self.workflows.len() - 1;
        self.state = AppState::from_workflow(workflow, api_key);
    }

    fn rename_workflow(&mut self, index: usize, name: String) {
        if index >= self.workflows.len() {
            return;
        }
        self.workflows[index].name.clone_from(&name);
        if self.active == index {
            self.state.workflow.name = name;
        }
    }

    fn start_interactive_execution(&mut self) {
        if let Err(error) = self.state.validate() {
            self.state.last_error = Some(error.to_string());
            return;
        }

        let env = ProviderEnv {
            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            compatible_api_key: env::var("OPENAI_COMPATIBLE_API_KEY").ok(),
        };
        let env_key_value = match self.settings.active_provider {
            AiProviderKind::OpenAi => env.openai_api_key.as_deref(),
            AiProviderKind::OpenAiCompatible => env.compatible_api_key.as_deref(),
        };
        let provider_config = match resolve_provider_config(
            &self.settings,
            self.state
                .resolve_provider_api_key(env_key_value)
                .as_deref(),
            &env,
        ) {
            Ok(config) => config,
            Err(error) => {
                self.state.last_error = Some(error.to_string());
                return;
            }
        };

        // Clear previous run state
        self.state.last_error = None;
        self.state.last_run = None;
        self.state.clear_run_trace();
        self.state.status_by_node.clear();
        self.state.chat_logs.clear();
        for node in &self.state.workflow.nodes {
            self.state
                .status_by_node
                .insert(node.id.clone(), AgentStatus::Idle);
            self.state.chat_logs.insert(node.id.clone(), Vec::new());
        }

        let workflow = self.state.workflow.clone();
        let entrypoint = self.state.entrypoint_text.trim().to_string();
        let entrypoint = if entrypoint.is_empty() {
            None
        } else {
            Some(entrypoint)
        };
        let ai = OpenAiClient::with_config(OpenAiClientConfig {
            api_key: provider_config.api_key,
            base_url: provider_config.base_url,
            wire_api: provider_config.wire_api,
            responses_path: provider_config.responses_path,
            chat_completions_path: provider_config.chat_completions_path,
        });
        let (handle, event_rx, action_tx) =
            spawn_interactive_workflow_run(&self.runtime, workflow, entrypoint, ai);
        self.execution_task = Some(handle);
        self.event_rx = Some(event_rx);
        self.action_tx = Some(action_tx);
    }

    fn send_chat_input(&mut self) {
        let text = self.chat_input_buffer.trim().to_string();
        if text.is_empty() {
            return;
        }
        // Add user's message to the chat log for the currently selected/awaiting node
        if let Some(ref node_id) = self.state.selected_node_id.clone() {
            if self.state.status_by_node.get(node_id) == Some(&AgentStatus::AwaitingInput) {
                self.state
                    .add_chat_message(node_id, ChatRole::User, text.clone());
                self.chat_input_buffer.clear();
                if let Some(ref tx) = self.action_tx {
                    let _ = tx.send(ExecutionAction::ProvideInput(text));
                }
            }
        }
    }

    fn save_all(&mut self) {
        self.sync_active();
        match self.store.save(&self.workflows) {
            Ok(()) => self.state.last_error = None,
            Err(error) => self.state.last_error = Some(format!("save failed: {error}")),
        }
        self.save_settings();
    }

    fn save_settings(&mut self) {
        match self.settings_store.save(&self.settings) {
            Ok(()) => self.settings_snapshot = self.settings.clone(),
            Err(error) => self.state.last_error = Some(format!("settings save failed: {error}")),
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

    fn on_exit(&mut self) {
        self.sync_active();
        let _ = self.store.save(&self.workflows);
        self.save_settings();
    }

    #[allow(deprecated, clippy::too_many_lines)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx);

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
            self.start_interactive_execution();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.new_workflow();
            self.show_settings = false;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save_all();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::B)) {
            self.show_sidebar = !self.show_sidebar;
        }

        // ── Event polling from background engine ──────────────────────────────
        if let Some(ref mut rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ExecutionEvent::NodeQueued {
                        ref node_id,
                        ref label,
                    } => {
                        self.state
                            .status_by_node
                            .insert(node_id.clone(), AgentStatus::Queued);
                        self.state.push_run_trace(RunTraceEntry {
                            node_id: node_id.clone(),
                            node_label: label.clone(),
                            status: TraceStatus::Queued,
                            message: "queued".to_string(),
                            output: None,
                        });
                    }
                    ExecutionEvent::NodeStarted {
                        ref node_id,
                        ref label,
                    } => {
                        self.state
                            .status_by_node
                            .insert(node_id.clone(), AgentStatus::Started);
                        self.state.push_run_trace(RunTraceEntry {
                            node_id: node_id.clone(),
                            node_label: label.clone(),
                            status: TraceStatus::Running,
                            message: "started OpenAI node call".to_string(),
                            output: None,
                        });
                    }
                    ExecutionEvent::ChatMessage {
                        ref node_id,
                        role,
                        ref content,
                    } => {
                        self.state.add_chat_message(node_id, role, content.clone());
                    }
                    ExecutionEvent::NodeAwaitingInput {
                        ref node_id,
                        ref label,
                        ref context,
                        ..
                    } => {
                        self.state
                            .status_by_node
                            .insert(node_id.clone(), AgentStatus::AwaitingInput);
                        self.state.push_run_trace(RunTraceEntry {
                            node_id: node_id.clone(),
                            node_label: label.clone(),
                            status: TraceStatus::Paused,
                            message: "paused for human input".to_string(),
                            output: None,
                        });
                        self.state.add_chat_message(
                            node_id,
                            ChatRole::System,
                            format!("Node '{label}' is awaiting human input."),
                        );
                        self.state.add_chat_message(
                            node_id,
                            ChatRole::Thinking,
                            format!("Context:\n{context}"),
                        );
                        self.bottom_panel_tab = canvas::BottomPanelTab::Chat;
                    }
                    ExecutionEvent::NodeCompleted {
                        ref node_id,
                        ref label,
                        ref output,
                    } => {
                        self.state
                            .status_by_node
                            .insert(node_id.clone(), AgentStatus::Completed);
                        self.state.push_run_trace(RunTraceEntry {
                            node_id: node_id.clone(),
                            node_label: label.clone(),
                            status: TraceStatus::Completed,
                            message: "completed".to_string(),
                            output: Some(output.clone()),
                        });
                        self.state.add_chat_message(
                            node_id,
                            ChatRole::Assistant,
                            output.to_string(),
                        );
                    }
                    ExecutionEvent::NodeFailed {
                        ref node_id,
                        ref label,
                        ref error,
                    } => {
                        self.state
                            .status_by_node
                            .insert(node_id.clone(), AgentStatus::Failed);
                        self.state.push_run_trace(RunTraceEntry {
                            node_id: node_id.clone(),
                            node_label: label.clone(),
                            status: TraceStatus::Failed,
                            message: error.clone(),
                            output: None,
                        });
                        self.state.add_chat_message(
                            node_id,
                            ChatRole::System,
                            format!("Failed: {error}"),
                        );
                        self.state.last_error = Some(error.clone());
                    }
                    ExecutionEvent::Finished(report) => {
                        self.state.set_run_report(report.clone());
                        self.state.last_error = None;
                    }
                    ExecutionEvent::Error(msg) => {
                        self.state.last_error = Some(msg);
                    }
                }
            }
        }

        // ── Left nav ──────────────────────────────────────────────────────────
        let was_showing_settings = self.show_settings;
        if self.show_sidebar {
            let nav = nav::show_nav_panel(
                ctx,
                &self.workflows,
                self.active,
                self.show_settings,
                &mut self.workflow_rename_state,
            );

            if let Some((index, name)) = nav.rename_workflow {
                self.rename_workflow(index, name);
            }

            if let Some(i) = nav.switch_to {
                self.switch_to(i);
                self.show_settings = false;
            }
            if nav.do_new {
                self.new_workflow();
                self.show_settings = false;
            }
            if nav.do_save {
                self.save_all();
            }
            if nav.toggle_settings {
                self.show_settings = !self.show_settings;
            }
            if nav.toggle_sidebar {
                self.show_sidebar = false;
            }
        } else {
            egui::SidePanel::left("nav_collapsed")
                .resizable(false)
                .exact_width(46.0)
                .frame(
                    egui::Frame::new()
                        .fill(SURFACE_1)
                        .stroke(egui::Stroke::NONE)
                        .inner_margin(egui::Margin::symmetric(5, 8)),
                )
                .show(ctx, |ui| {
                    ui.set_height(ui.available_height());
                    #[cfg(target_os = "macos")]
                    if !ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                        let (_, drag_response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 32.0),
                            egui::Sense::click_and_drag(),
                        );
                        if drag_response.dragged() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }
                    let (icon_rect, icon_response) = ui.allocate_exact_size(
                        egui::vec2(COLLAPSED_NAV_ICON_SIZE, COLLAPSED_NAV_ICON_SIZE),
                        egui::Sense::click(),
                    );
                    let hover_t = ctx.animate_bool(
                        egui::Id::new("collapsed_nav_show_sidebar"),
                        icon_response.hovered(),
                    );
                    let icon_bg = collapsed_nav_icon_bg(hover_t);
                    ui.painter().rect_filled(
                        icon_rect,
                        egui::CornerRadius::same(COLLAPSED_NAV_ICON_RADIUS),
                        icon_bg,
                    );

                    let glyph_rect =
                        egui::Rect::from_center_size(icon_rect.center(), egui::vec2(16.0, 14.0));
                    let glyph_color = if icon_response.hovered() {
                        TEXT_BRIGHT
                    } else {
                        TEXT_DIM
                    };
                    ui.painter().rect_stroke(
                        glyph_rect,
                        egui::CornerRadius::same(5),
                        egui::Stroke::new(1.2, glyph_color),
                        egui::StrokeKind::Inside,
                    );
                    let rail_x = glyph_rect.left() + 4.5;
                    ui.painter().line_segment(
                        [
                            egui::pos2(rail_x, glyph_rect.top() + 3.0),
                            egui::pos2(rail_x, glyph_rect.bottom() - 3.0),
                        ],
                        egui::Stroke::new(1.2, glyph_color),
                    );

                    let icon_response = icon_response.on_hover_text("Show sidebar (⌘B / Ctrl+B)");
                    if icon_response.clicked() {
                        self.show_sidebar = true;
                    }

                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        let (settings_rect, settings_response) = ui.allocate_exact_size(
                            egui::vec2(COLLAPSED_NAV_ICON_SIZE, COLLAPSED_NAV_ICON_SIZE),
                            egui::Sense::click(),
                        );
                        let settings_active = settings_response.hovered() || self.show_settings;
                        let settings_t = ctx
                            .animate_bool(egui::Id::new("collapsed_nav_settings"), settings_active);
                        ui.painter().rect_filled(
                            settings_rect,
                            egui::CornerRadius::same(COLLAPSED_NAV_ICON_RADIUS),
                            collapsed_nav_icon_bg(settings_t),
                        );
                        let settings_glyph_color = if settings_active {
                            TEXT_BRIGHT
                        } else {
                            TEXT_DIM
                        };
                        ui.painter().text(
                            settings_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            ph::GEAR_SIX,
                            egui::FontId::proportional(COLLAPSED_NAV_GLYPH_SIZE),
                            settings_glyph_color,
                        );

                        let settings_response = settings_response.on_hover_text("Settings");
                        if settings_response.clicked() {
                            self.show_settings = !self.show_settings;
                        }
                    });
                });
        }
        if was_showing_settings && !self.show_settings {
            self.save_settings();
        }

        // ── Main content ──────────────────────────────────────────────────────
        if self.show_settings {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(SURFACE_1)
                        .inner_margin(egui::Margin::same(0)),
                )
                .show(ctx, |ui| {
                    settings::show_settings_panel(ui, &mut self.state, &mut self.settings);
                });
        } else {
            let env = ProviderEnv {
                openai_api_key: env::var("OPENAI_API_KEY").ok(),
                compatible_api_key: env::var("OPENAI_COMPATIBLE_API_KEY").ok(),
            };
            let env_key_value = match self.settings.active_provider {
                AiProviderKind::OpenAi => env.openai_api_key.as_deref(),
                AiProviderKind::OpenAiCompatible => env.compatible_api_key.as_deref(),
            };
            let transient = self.state.resolve_provider_api_key(env_key_value);
            let api_key_ready =
                resolve_provider_config(&self.settings, transient.as_deref(), &env).is_ok();
            let execution_running = self
                .execution_task
                .as_ref()
                .is_some_and(|handle| !handle.is_finished());
            let bottom_out = canvas::show_bottom_panel(
                ctx,
                &mut self.state,
                canvas::BottomPanelInput {
                    settings: &mut self.settings,
                    input_buffer: &mut self.chat_input_buffer,
                    active_tab: &mut self.bottom_panel_tab,
                    dock_state: &mut self.chat_dock_state,
                    api_key_ready,
                    execution_running,
                },
            );

            if bottom_out.clear_run {
                self.state.last_run = None;
                self.state.clear_run_trace();
                self.state.refresh_statuses_from_report();
            }

            if bottom_out.send_chat {
                self.send_chat_input();
            }

            if bottom_out.retry_run || bottom_out.run_workflow {
                self.start_interactive_execution();
            }

            let canvas_rect = canvas::show_canvas_panel(ctx, &mut self.state);

            let pill_out = inspector::show_graph_pill(ctx, canvas_rect);
            if pill_out.add_agent {
                self.state.add_agent_node();
            }

            let inspector_out =
                inspector::show_floating_inspector(ctx, &mut self.state, &self.settings);

            if inspector_out.begin_link {
                self.state.begin_link_from_selected();
            }
            if inspector_out.delete_node {
                self.state.remove_selected_node();
            }
            if inspector_out.apply_schema {
                self.state.apply_schema_editor();
            }
            if inspector_out.run_workflow {
                self.start_interactive_execution();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[allow(clippy::float_cmp)]
    fn collapsed_nav_icon_tokens_match_sidebar_button() {
        assert_eq!(COLLAPSED_NAV_ICON_SIZE, 34.0);
        assert_eq!(COLLAPSED_NAV_ICON_RADIUS, 10);
        assert_eq!(COLLAPSED_NAV_GLYPH_SIZE, 16.0);
        assert_eq!(collapsed_nav_icon_bg(0.0), SURFACE_2);
        assert_eq!(collapsed_nav_icon_bg(1.0), SURFACE_3);
    }

    #[test]
    fn save_settings_propagates_error_to_state_last_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = WorkflowApp::new();
        app.settings_store = FileSettingsStore::new(dir.path().to_path_buf());
        app.save_settings();
        assert!(app.state.last_error.is_some());
        assert!(app
            .state
            .last_error
            .as_ref()
            .unwrap()
            .contains("settings save failed"));
        fs::remove_dir_all(dir.path()).ok();
    }

    #[test]
    fn save_settings_updates_snapshot_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let store_path = dir.path().join("settings.json");
        let settings = AppSettings {
            active_provider: crate::settings_store::AiProviderKind::OpenAiCompatible,
            ..AppSettings::default()
        };
        let mut app = WorkflowApp::new();
        app.settings = settings.clone();
        app.settings_snapshot = AppSettings::default();
        app.settings_store = FileSettingsStore::new(store_path);
        app.save_settings();
        assert_eq!(app.settings_snapshot, app.settings);
        assert!(app.state.last_error.is_none());
        fs::remove_dir_all(dir.path()).ok();
    }
}

#![allow(clippy::too_many_lines)]

#[allow(clippy::wildcard_imports)]
use super::theme::*;
use super::widgets::inspector_field;
use crate::settings_store::{AppSettings, ProviderTransport};
use crate::state::AppState;
use ai::{provider_spec, ProviderKind};
use eframe::egui;

pub(super) fn show_settings_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    settings: &mut AppSettings,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let avail = ui.available_width();
        let content_width = avail.min(640.0);
        let x_pad = ((avail - content_width) / 2.0).max(0.0);

        ui.add_space(24.0);

        let cursor = ui.cursor();
        let content_rect = egui::Rect::from_min_size(
            egui::pos2(cursor.min.x + x_pad, cursor.min.y),
            egui::vec2(content_width, ui.max_rect().max.y - cursor.min.y),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                ui.label(
                    egui::RichText::new("Settings")
                        .size(18.0)
                        .color(TEXT_BRIGHT),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Configure BYOK provider keys and application preferences.")
                        .size(TS_LABEL)
                        .color(TEXT_DIM),
                );
                ui.add_space(20.0);

                // -- Provider ---------------------------------------------------------------
                ui.label(
                    egui::RichText::new("PROVIDER")
                        .size(TS_SECTION)
                        .color(TEXT_DIM)
                        .monospace(),
                );
                ui.add_space(8.0);

                egui::Frame::new()
                    .fill(SURFACE_2)
                    .corner_radius(egui::CornerRadius::same(6))
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        let provider_ids = settings.provider_display_order();
                        inspector_field(ui, "Provider", |ui| {
                            egui::ComboBox::from_id_salt("provider_kind_combo")
                                .selected_text(settings.active_profile().display_name.as_str())
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    for provider_id in provider_ids {
                                        let label = settings
                                            .providers
                                            .get(&provider_id)
                                            .map_or_else(|| provider_id.to_string(), |profile| profile.display_name.clone());
                                        ui.selectable_value(
                                            &mut settings.active_provider,
                                            provider_id,
                                            label,
                                        );
                                    }
                                });
                        });

                        let active_provider = settings.active_provider.clone();
                        let active_spec = provider_spec(&active_provider);
                        let active_env_var = active_spec.and_then(|spec| spec.auth.env_var());
                        let active_kind = active_spec.map(|spec| spec.kind);
                        let profile = settings.active_profile_mut();

                        match active_kind {
                            Some(ProviderKind::OpenAiCompatible(_)) if profile.editable => {
                                inspector_field(ui, "Base URL", |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut profile.base_url)
                                            .hint_text("https://api.provider.example/v1")
                                            .desired_width(f32::INFINITY),
                                    );
                                });

                                inspector_field(ui, "Wire API", |ui| {
                                    egui::ComboBox::from_id_salt("provider_transport_combo")
                                        .selected_text(profile.transport.label())
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut profile.transport,
                                                ProviderTransport::Responses,
                                                ProviderTransport::Responses.label(),
                                            );
                                            ui.selectable_value(
                                                &mut profile.transport,
                                                ProviderTransport::ChatCompletions,
                                                ProviderTransport::ChatCompletions.label(),
                                            );
                                        });
                                });

                                inspector_field(ui, "Responses Path", |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut profile.responses_path)
                                            .hint_text("v1/responses")
                                            .desired_width(f32::INFINITY),
                                    );
                                });

                                inspector_field(ui, "Chat Completions Path", |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut profile.chat_completions_path)
                                            .hint_text("v1/chat/completions")
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            }
                            Some(ProviderKind::OpenAiCompatible(_)) => {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Uses {} with the {}.",
                                        profile.base_url,
                                        profile.transport.label()
                                    ))
                                    .size(TS_LABEL)
                                    .color(TEXT_DIM),
                                );
                                ui.add_space(6.0);
                            }
                            Some(ProviderKind::Anthropic(_)) => {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Uses {} with the Anthropic Messages API.",
                                        profile.base_url
                                    ))
                                    .size(TS_LABEL)
                                    .color(TEXT_DIM),
                                );
                                ui.add_space(6.0);
                            }
                            None => {
                                ui.label(
                                    egui::RichText::new("This provider is not implemented.")
                                        .size(TS_LABEL)
                                        .color(TEXT_DIM),
                                );
                                ui.add_space(6.0);
                            }
                        }

                        inspector_field(ui, "Provider API Key", |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut state.provider_api_key_input)
                                    .password(true)
                                    .hint_text(active_env_var.unwrap_or("optional local provider key"))
                                    .desired_width(f32::INFINITY),
                            );
                        });

                        egui::Frame::new()
                            .fill(SURFACE_0)
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                let message = active_env_var.map_or_else(
                                    || "Local providers can run without a key. Non-empty keys are saved to the OS credential store when settings are saved.".to_string(),
                                    |env_var| format!(
                                        "Non-empty keys are saved to the OS credential store when settings are saved. You can also set {env_var} in your environment."
                                    ),
                                );
                                ui.label(
                                    egui::RichText::new(message)
                                    .size(TS_LABEL)
                                    .color(TEXT_DIM),
                                );
                            });
                    });

                ui.add_space(24.0);

                // ── Models ────────────────────────────────────────────────────
                ui.label(
                    egui::RichText::new("MODELS")
                        .size(TS_SECTION)
                        .color(TEXT_DIM)
                        .monospace(),
                );
                ui.add_space(8.0);

                egui::Frame::new()
                    .fill(SURFACE_2)
                    .corner_radius(egui::CornerRadius::same(6))
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        let profile = settings.active_profile_mut();

                        ui.label(
                            egui::RichText::new(format!(
                                "Models shown in the node inspector for {}.",
                                profile.display_name
                            ))
                            .size(TS_LABEL)
                            .color(TEXT_DIM),
                        );
                        ui.add_space(8.0);

                        let mut to_remove: Option<usize> = None;
                        for (i, model) in profile.known_models.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(model).size(TS_BODY).color(TEXT_BRIGHT));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new("x")
                                                .fill(egui::Color32::TRANSPARENT)
                                                .small(),
                                        )
                                        .on_hover_text("Remove model")
                                        .clicked()
                                    {
                                        to_remove = Some(i);
                                    }
                                });
                            });
                        }
                        if let Some(i) = to_remove {
                            profile.known_models.remove(i);
                        }

                        ui.add_space(8.0);
                        ui.add(egui::Separator::default().horizontal());
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            let add_clicked = ui
                                .add(
                                    egui::Button::new("Add")
                                        .fill(SURFACE_3)
                                        .corner_radius(egui::CornerRadius::same(5)),
                                )
                                .clicked();
                            ui.add(
                                egui::TextEdit::singleline(&mut profile.new_model_input)
                                    .hint_text("model name...")
                                    .desired_width(f32::INFINITY),
                            );
                            if add_clicked {
                                let name = profile.new_model_input.trim().to_string();
                                if !name.is_empty() && !profile.known_models.contains(&name) {
                                    profile.known_models.push(name);
                                    profile.new_model_input.clear();
                                }
                            }
                        });
                    });

                ui.add_space(24.0);
            });
    });
}

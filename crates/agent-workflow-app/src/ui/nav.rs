#![allow(deprecated)]

#[allow(clippy::wildcard_imports)]
use super::theme::*;
use eframe::egui;
use egui_phosphor::regular as ph;
use workflow_core::Workflow;

const NAV_TOP_PADDING: f32 = 8.0;
const NAV_ROW_HEIGHT: f32 = 34.0;
const NAV_BOTTOM_AREA_HEIGHT: f32 = NAV_TOP_PADDING + NAV_ROW_HEIGHT;
const NAV_PILL_INSET_X: f32 = 6.0;
const NAV_PILL_INSET_Y: f32 = 2.0;
const NAV_PILL_TEXT_X: f32 = 12.0;
// Width reserved for an icon glyph + gap before the label text
const NAV_ICON_COL_W: f32 = 18.0;
const NAV_RENAME_BUTTON_SIZE: f32 = 28.0;
const NAV_RENAME_BUTTON_RADIUS: u8 = 8;
const NAV_RENAME_EDIT_INSET_Y: f32 = 4.0;
const NAV_RENAME_EDIT_RIGHT_GAP: f32 = 6.0;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lerp_color(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (f32::from(to.r()) - f32::from(from.r())).mul_add(t, f32::from(from.r())) as u8,
        (f32::from(to.g()) - f32::from(from.g())).mul_add(t, f32::from(from.g())) as u8,
        (f32::from(to.b()) - f32::from(from.b())).mul_add(t, f32::from(from.b())) as u8,
    )
}

#[allow(clippy::struct_excessive_bools)]
pub(super) struct NavOutput {
    pub switch_to: Option<usize>,
    pub rename_workflow: Option<(usize, String)>,
    pub do_new: bool,
    pub do_save: bool,
    pub toggle_sidebar: bool,
    pub toggle_settings: bool,
}

#[derive(Default)]
pub(super) struct WorkflowRenameState {
    editing_index: Option<usize>,
    draft_name: String,
    request_focus: bool,
}

const fn rename_pencil_has_chrome() -> bool {
    false
}

fn draw_nav_icon_button(
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
    icon: &str,
    icon_color: egui::Color32,
    fill: egui::Color32,
    show_chrome: bool,
) {
    let visuals = ui.style().interact(response);
    if show_chrome {
        let bg = if response.hovered() {
            visuals.weak_bg_fill
        } else {
            fill
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(NAV_RENAME_BUTTON_RADIUS), bg);
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(NAV_RENAME_BUTTON_RADIUS),
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(TS_TITLE),
        if response.hovered() {
            TEXT_BRIGHT
        } else {
            icon_color
        },
    );
}

#[allow(clippy::too_many_lines)]
pub(super) fn show_nav_panel(
    ctx: &egui::Context,
    workflows: &[Workflow],
    active: usize,
    show_settings: bool,
    rename_state: &mut WorkflowRenameState,
) -> NavOutput {
    if rename_state
        .editing_index
        .is_some_and(|editing_index| editing_index >= workflows.len())
    {
        *rename_state = WorkflowRenameState::default();
    }

    let mut out = NavOutput {
        switch_to: None,
        rename_workflow: None,
        do_new: false,
        do_save: false,
        toggle_sidebar: false,
        toggle_settings: false,
    };

    egui::SidePanel::left("nav")
        .resizable(false)
        .exact_width(220.0)
        .frame(
            egui::Frame::new()
                .fill(SURFACE_1)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            ui.set_height(ui.available_height());
            ui.add_space(NAV_TOP_PADDING);
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

            // Sidebar collapse button — padded to align with pill content
            ui.horizontal(|ui| {
                ui.add_space(NAV_PILL_INSET_X);
                if ui
                    .add_sized(
                        [28.0, 28.0],
                        egui::Button::new(
                            egui::RichText::new(ph::CARET_LEFT)
                                .size(TS_SECTION)
                                .color(TEXT_BRIGHT),
                        )
                        .fill(SURFACE_2)
                        .corner_radius(egui::CornerRadius::same(8)),
                    )
                    .on_hover_text("Hide sidebar (⌘B / Ctrl+B)")
                    .clicked()
                {
                    out.toggle_sidebar = true;
                }
            });
            ui.add_space(6.0);

            let new_width = ui.available_width();
            let (new_rect, new_response) =
                ui.allocate_exact_size(egui::vec2(new_width, NAV_ROW_HEIGHT), egui::Sense::click());
            let new_pill_rect = new_rect.shrink2(egui::vec2(NAV_PILL_INSET_X, NAV_PILL_INSET_Y));
            let new_hover_t = ctx.animate_bool(
                egui::Id::new("nav_new_workflow_hover"),
                new_response.hovered(),
            );
            let new_bg = lerp_color(SURFACE_2, SURFACE_3, new_hover_t);
            ui.painter()
                .rect_filled(new_pill_rect, egui::CornerRadius::same(9), new_bg);
            // Icon and label drawn separately for consistent, font-independent gap
            ui.painter().text(
                egui::pos2(
                    new_pill_rect.left() + NAV_PILL_TEXT_X,
                    new_pill_rect.center().y,
                ),
                egui::Align2::LEFT_CENTER,
                ph::PLUS,
                egui::FontId::proportional(TS_SECTION),
                TEXT_BRIGHT,
            );
            ui.painter().text(
                egui::pos2(
                    new_pill_rect.left() + NAV_PILL_TEXT_X + NAV_ICON_COL_W,
                    new_pill_rect.center().y,
                ),
                egui::Align2::LEFT_CENTER,
                "New workflow",
                egui::FontId::proportional(TS_SECTION),
                TEXT_BRIGHT,
            );
            let new_response = new_response.on_hover_text("New workflow (⌘N / Ctrl+N)");
            if new_response.clicked() {
                out.do_new = true;
            }

            egui::ScrollArea::vertical()
                .id_salt("nav_scroll")
                .max_height(ui.available_height() - NAV_BOTTOM_AREA_HEIGHT)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    if workflows.is_empty() {
                        let empty_width = ui.available_width();
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(empty_width, NAV_ROW_HEIGHT),
                            egui::Sense::click(),
                        );
                        let pill_rect =
                            rect.shrink2(egui::vec2(NAV_PILL_INSET_X, NAV_PILL_INSET_Y));
                        let hover_t = ctx.animate_bool(
                            egui::Id::new("nav_empty_new_workflow_hover"),
                            response.hovered(),
                        );
                        let bg = lerp_color(SURFACE_2, SURFACE_3, hover_t);
                        ui.painter()
                            .rect_filled(pill_rect, egui::CornerRadius::same(9), bg);
                        ui.painter().text(
                            pill_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Create your first workflow",
                            egui::FontId::proportional(TS_SECTION),
                            TEXT_BRIGHT,
                        );
                        if response.clicked() {
                            out.do_new = true;
                        }
                    }

                    for (i, workflow) in workflows.iter().enumerate() {
                        let is_active = i == active && !show_settings;
                        let available_width = ui.available_width();
                        let is_editing = rename_state.editing_index == Some(i);

                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(available_width, NAV_ROW_HEIGHT),
                            egui::Sense::click(),
                        );
                        let pill_rect =
                            rect.shrink2(egui::vec2(NAV_PILL_INSET_X, NAV_PILL_INSET_Y));

                        let hover_t = ctx.animate_bool(
                            egui::Id::new(("nav_hover", i)),
                            response.hovered() || is_active,
                        );
                        let bg = if is_active {
                            SURFACE_3
                        } else {
                            {
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                egui::Color32::from_rgba_premultiplied(
                                    (f32::from(SURFACE_3.r()) * hover_t) as u8,
                                    (f32::from(SURFACE_3.g()) * hover_t) as u8,
                                    (f32::from(SURFACE_3.b()) * hover_t) as u8,
                                    (180.0_f32 * hover_t) as u8,
                                )
                            }
                        };
                        ui.painter()
                            .rect_filled(pill_rect, egui::CornerRadius::same(9), bg);

                        let mut row_click_consumed = false;
                        let rename_button_center =
                            egui::pos2(pill_rect.right() - NAV_PILL_TEXT_X, pill_rect.center().y);
                        let rename_button_rect = egui::Rect::from_center_size(
                            rename_button_center,
                            egui::vec2(NAV_RENAME_BUTTON_SIZE, NAV_RENAME_BUTTON_SIZE),
                        );

                        if is_editing {
                            let edit_rect = egui::Rect::from_min_max(
                                egui::pos2(
                                    pill_rect.left() + NAV_PILL_TEXT_X,
                                    pill_rect.top() + NAV_RENAME_EDIT_INSET_Y,
                                ),
                                egui::pos2(
                                    rename_button_rect.left() - NAV_RENAME_EDIT_RIGHT_GAP,
                                    pill_rect.bottom() - NAV_RENAME_EDIT_INSET_Y,
                                ),
                            );
                            let edit_response = ui.put(
                                edit_rect,
                                egui::TextEdit::singleline(&mut rename_state.draft_name)
                                    .font(egui::FontId::proportional(TS_SECTION))
                                    .desired_width(f32::INFINITY),
                            );
                            if rename_state.request_focus {
                                edit_response.request_focus();
                                rename_state.request_focus = false;
                            }

                            let confirm_response = ui
                                .interact(
                                    rename_button_rect,
                                    egui::Id::new(("nav_confirm_workflow_rename", i)),
                                    egui::Sense::click(),
                                )
                                .on_hover_text("Save workflow name");
                            draw_nav_icon_button(
                                ui,
                                rename_button_rect,
                                &confirm_response,
                                ph::CHECK,
                                TEXT_BRIGHT,
                                SURFACE_3,
                                true,
                            );
                            let commit_with_enter = edit_response.has_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter));
                            let commit_with_button = confirm_response.clicked();
                            let cancel_with_escape = edit_response.has_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Escape));
                            let commit_from_blur = edit_response.lost_focus()
                                && !cancel_with_escape
                                && !commit_with_button;

                            if commit_with_enter || commit_with_button || commit_from_blur {
                                let name = rename_state.draft_name.trim();
                                if !name.is_empty() {
                                    out.rename_workflow = Some((i, name.to_string()));
                                }
                                *rename_state = WorkflowRenameState::default();
                            } else if cancel_with_escape {
                                *rename_state = WorkflowRenameState::default();
                            }

                            row_click_consumed =
                                edit_response.hovered() || confirm_response.hovered();
                        } else {
                            ui.painter().text(
                                egui::pos2(
                                    pill_rect.left() + NAV_PILL_TEXT_X,
                                    pill_rect.center().y,
                                ),
                                egui::Align2::LEFT_CENTER,
                                &workflow.name,
                                egui::FontId::proportional(if is_active {
                                    TS_BODY
                                } else {
                                    TS_SECTION
                                }),
                                if is_active { TEXT_BRIGHT } else { TEXT_DIM },
                            );

                            let rename_response = ui
                                .interact(
                                    rename_button_rect,
                                    egui::Id::new(("nav_rename_workflow", i)),
                                    egui::Sense::click(),
                                )
                                .on_hover_text("Rename workflow");
                            if response.hovered() || rename_response.hovered() {
                                draw_nav_icon_button(
                                    ui,
                                    rename_button_rect,
                                    &rename_response,
                                    ph::PENCIL_SIMPLE,
                                    TEXT_DIM,
                                    SURFACE_2,
                                    rename_pencil_has_chrome(),
                                );
                            }
                            if rename_response.clicked() {
                                rename_state.editing_index = Some(i);
                                rename_state.draft_name.clone_from(&workflow.name);
                                rename_state.request_focus = true;
                                row_click_consumed = true;
                            }
                            row_click_consumed |= rename_response.hovered();
                        }

                        if response.clicked()
                            && (!is_active || show_settings)
                            && !row_click_consumed
                        {
                            out.switch_to = Some(i);
                        }
                    }
                });

            let bottom_rect = ui.available_rect_before_wrap();
            ui.allocate_ui_at_rect(
                egui::Rect::from_min_max(
                    egui::pos2(
                        bottom_rect.left(),
                        bottom_rect.bottom() - NAV_BOTTOM_AREA_HEIGHT,
                    ),
                    bottom_rect.right_bottom(),
                ),
                |ui| {
                    let settings_width = ui.available_width();
                    let (settings_rect, settings_response) = ui.allocate_exact_size(
                        egui::vec2(settings_width, NAV_ROW_HEIGHT),
                        egui::Sense::click(),
                    );
                    let settings_pill_rect =
                        settings_rect.shrink2(egui::vec2(NAV_PILL_INSET_X, NAV_PILL_INSET_Y));
                    let settings_hover_t = ctx.animate_bool(
                        egui::Id::new("nav_settings_hover"),
                        settings_response.hovered() || show_settings,
                    );
                    let settings_bg = if show_settings {
                        SURFACE_3
                    } else {
                        lerp_color(SURFACE_2, SURFACE_3, settings_hover_t)
                    };
                    ui.painter().rect_filled(
                        settings_pill_rect,
                        egui::CornerRadius::same(9),
                        settings_bg,
                    );
                    // Icon and label drawn separately for consistent, font-independent gap
                    ui.painter().text(
                        egui::pos2(
                            settings_pill_rect.left() + NAV_PILL_TEXT_X,
                            settings_pill_rect.center().y,
                        ),
                        egui::Align2::LEFT_CENTER,
                        ph::GEAR_SIX,
                        egui::FontId::proportional(TS_SECTION),
                        TEXT_BRIGHT,
                    );
                    ui.painter().text(
                        egui::pos2(
                            settings_pill_rect.left() + NAV_PILL_TEXT_X + NAV_ICON_COL_W,
                            settings_pill_rect.center().y,
                        ),
                        egui::Align2::LEFT_CENTER,
                        "Settings",
                        egui::FontId::proportional(TS_SECTION),
                        TEXT_BRIGHT,
                    );

                    let settings_response = settings_response.on_hover_text("Settings");
                    if settings_response.clicked() {
                        out.toggle_settings = true;
                    }
                },
            );
        });

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn workflow_rename_button_matches_nav_icon_hit_target() {
        assert_eq!(NAV_RENAME_BUTTON_SIZE, 28.0);
        assert_eq!(NAV_RENAME_BUTTON_RADIUS, 8);
    }

    #[test]
    fn workflow_rename_pencil_has_no_background_chrome() {
        assert!(!rename_pencil_has_chrome());
    }
}

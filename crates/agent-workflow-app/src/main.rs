#![allow(clippy::cargo, clippy::nursery, clippy::pedantic)]

use agent_workflow_app::WorkflowApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 820.0])
        .with_min_inner_size([960.0, 640.0])
        .with_title("Step-through Agentic Workflow");

    #[cfg(target_os = "macos")]
    {
        viewport = viewport
            .with_titlebar_shown(false)
            .with_fullsize_content_view(true)
            .with_title_shown(false);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Step-through Agentic Workflow",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Nunito".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(
                    include_bytes!("../assets/Nunito-Regular.ttf").to_vec(),
                )),
            );
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "Nunito".to_owned());
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(WorkflowApp::new()))
        }),
    )
}

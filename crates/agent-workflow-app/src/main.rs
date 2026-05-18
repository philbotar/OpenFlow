use agent_workflow_app::WorkflowApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title("Step-through Agentic Workflow"),
        ..Default::default()
    };

    eframe::run_native(
        "Step-through Agentic Workflow",
        options,
        Box::new(|_cc| Ok(Box::new(WorkflowApp::new()))),
    )
}

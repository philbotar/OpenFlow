use engine::{NodeId, RunTelemetry};
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

pub fn emit_phase_timed(
    event_tx: &UnboundedSender<RunTelemetry>,
    phase: &str,
    label: &str,
    node_id: Option<NodeId>,
    started: Instant,
) {
    let duration_ms = started.elapsed().as_millis() as u64;
    log::info!("[perf] {phase} · {label}: {duration_ms}ms");
    let _ = event_tx.send(RunTelemetry::PhaseTimed {
        phase: phase.to_string(),
        label: label.to_string(),
        node_id,
        duration_ms,
    });
}

mod from_event;
#[cfg(test)]
mod from_event_tests;
mod model;
#[cfg(test)]
mod model_tests;
pub mod ports;
mod recorder;
#[cfg(test)]
mod recorder_tests;

pub use from_event::incident_from_execution_event;
pub use model::{
    IncidentCategory, IncidentContext, IncidentRecord, IncidentScope, IncidentSeverity,
};
pub use ports::{IncidentListOptions, IncidentStore};
pub use recorder::{incident_from_tool_error, IncidentRecorder};

mod model;
#[cfg(test)]
mod model_tests;
pub mod ports;

pub use model::{
    IncidentCategory, IncidentContext, IncidentRecord, IncidentScope, IncidentSeverity,
};
pub use ports::{IncidentListOptions, IncidentStore};

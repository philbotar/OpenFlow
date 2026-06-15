use super::model::IncidentRecord;
use std::io;

#[derive(Debug, Clone, Default)]
pub struct IncidentListOptions {
    pub include_resolved: bool,
    pub limit: Option<usize>,
}

pub trait IncidentStore: Send + Sync {
    fn append(&self, record: &IncidentRecord) -> io::Result<()>;
    fn list(&self, options: Option<IncidentListOptions>) -> io::Result<Vec<IncidentRecord>>;
    fn dismiss(&self, id: &str) -> io::Result<()>;
    fn clear_resolved(&self) -> io::Result<usize>;
}

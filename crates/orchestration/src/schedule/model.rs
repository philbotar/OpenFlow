use chrono::{DateTime, Utc};
use engine::WorkflowSchedule;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleStatus {
    pub workflow_id: String,
    pub workflow_name: String,
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_skipped_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl ScheduleStatus {
    #[must_use]
    pub fn disabled(workflow_id: String, workflow_name: String) -> Self {
        Self {
            workflow_id,
            workflow_name,
            enabled: false,
            cron: String::new(),
            timezone: String::new(),
            next_run_at: None,
            last_run_at: None,
            last_skipped_at: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledRunCandidate {
    pub workflow_id: String,
    pub workflow_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleEntry {
    pub workflow_id: String,
    pub workflow_name: String,
    pub schedule: WorkflowSchedule,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_skipped_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScheduleConfigError {
    #[error("cron expression is empty")]
    EmptyCron,
    #[error("cron expression must have 5, 6, or 7 fields; got {0}")]
    WrongFieldCount(usize),
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),
    #[error("cron expression has no future run")]
    NoFutureRun,
}

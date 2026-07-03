use chrono::{DateTime, Utc};
use engine::WorkflowSchedule;

use super::{AppBackend, BackendError, ScheduleDraft, ScheduleStatus};

impl AppBackend {
    #[must_use]
    pub fn build_schedule_from_draft(&self, draft: ScheduleDraft) -> WorkflowSchedule {
        let mut schedule = crate::schedule::schedule_from_preset(&draft);
        schedule.timezone = "UTC".to_string();
        schedule
    }

    #[must_use]
    pub fn describe_schedule(&self, schedule: &WorkflowSchedule) -> String {
        crate::schedule::describe_workflow_schedule(schedule)
    }

    #[must_use]
    pub fn schedule_draft_from_schedule(&self, schedule: &WorkflowSchedule) -> ScheduleDraft {
        crate::schedule::schedule_preset_from_cron(&schedule.cron, schedule.enabled)
    }

    pub fn refresh_schedules(&self) -> Result<Vec<ScheduleStatus>, BackendError> {
        self.refresh_schedules_at(Utc::now())
    }

    pub fn refresh_schedules_at(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ScheduleStatus>, BackendError> {
        let workflows = self.workflows.load_all(&self.projects)?;
        self.schedule
            .refresh(&workflows, now)
            .map_err(BackendError::Schedule)?;
        Ok(self.schedule.statuses())
    }

    pub fn tick_schedules_at(&self, now: DateTime<Utc>) {
        self.schedule.tick_at(now);
    }

    pub fn tick_schedules(&self) {
        self.tick_schedules_at(Utc::now());
    }

    #[must_use]
    pub fn list_schedule_statuses(&self) -> Vec<ScheduleStatus> {
        self.schedule.statuses()
    }
}

use chrono::{DateTime, Utc};
use engine::Workflow;
use parking_lot::Mutex;
use std::collections::BTreeMap;

use super::evaluator::next_run_after;
use super::model::{ScheduleEntry, ScheduleStatus, ScheduledRunCandidate};

#[derive(Debug, Default)]
pub struct ScheduleService {
    entries: Mutex<BTreeMap<String, ScheduleEntry>>,
}

impl ScheduleService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn refresh(&self, workflows: &[Workflow], now: DateTime<Utc>) -> Result<(), String> {
        let mut next_entries = BTreeMap::new();
        let previous = self.entries.lock();
        for workflow in workflows {
            let Some(schedule) = workflow.settings.schedule.clone() else {
                continue;
            };

            let workflow_id = workflow.id.to_string();
            let prior = previous.get(&workflow_id);
            let (next_run_at, last_error) = if schedule.enabled {
                match next_run_after(&schedule, now) {
                    Ok(next) => (Some(next), prior.and_then(|entry| entry.last_error.clone())),
                    Err(error) => (None, Some(error.to_string())),
                }
            } else {
                (None, prior.and_then(|entry| entry.last_error.clone()))
            };

            next_entries.insert(
                workflow_id.clone(),
                ScheduleEntry {
                    workflow_id,
                    workflow_name: workflow.name.clone(),
                    schedule,
                    next_run_at,
                    last_run_at: prior.and_then(|entry| entry.last_run_at),
                    last_skipped_at: prior.and_then(|entry| entry.last_skipped_at),
                    last_error,
                },
            );
        }
        drop(previous);
        *self.entries.lock() = next_entries;
        Ok(())
    }

    #[must_use]
    pub fn statuses(&self) -> Vec<ScheduleStatus> {
        self.entries
            .lock()
            .values()
            .map(|entry| ScheduleStatus {
                workflow_id: entry.workflow_id.clone(),
                workflow_name: entry.workflow_name.clone(),
                enabled: entry.schedule.enabled,
                cron: entry.schedule.cron.clone(),
                timezone: entry.schedule.timezone.clone(),
                next_run_at: entry.next_run_at,
                last_run_at: entry.last_run_at,
                last_skipped_at: entry.last_skipped_at,
                last_error: entry.last_error.clone(),
            })
            .collect()
    }

    pub fn claim_due_run(
        &self,
        now: DateTime<Utc>,
        run_active: bool,
    ) -> Option<ScheduledRunCandidate> {
        let mut entries = self.entries.lock();
        for entry in entries.values_mut() {
            if !entry.schedule.enabled {
                continue;
            }
            let Some(next_run_at) = entry.next_run_at else {
                continue;
            };
            if next_run_at > now {
                continue;
            }

            if run_active {
                entry.last_skipped_at = Some(now);
                entry.last_error =
                    Some("Skipped because another workflow run was active".to_string());
                entry.next_run_at = next_run_after(&entry.schedule, now).ok();
                return None;
            }

            entry.last_run_at = Some(now);
            entry.last_error = None;
            entry.next_run_at = next_run_after(&entry.schedule, now).ok();
            return Some(ScheduledRunCandidate {
                workflow_id: entry.workflow_id.clone(),
                workflow_name: entry.workflow_name.clone(),
            });
        }
        None
    }

    pub fn record_start_error(&self, workflow_id: &str, message: String) {
        if let Some(entry) = self.entries.lock().get_mut(workflow_id) {
            entry.last_error = Some(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::WorkflowSchedule;

    fn utc(raw: &str) -> DateTime<Utc> {
        raw.parse::<DateTime<Utc>>().expect("timestamp")
    }

    fn workflow_with_schedule(id: &str, cron: &str, enabled: bool) -> Workflow {
        let mut workflow = Workflow::new(format!("Workflow {id}"));
        workflow.id = engine::WorkflowId(id.to_string());
        workflow.settings.schedule = Some(WorkflowSchedule {
            cron: cron.to_string(),
            enabled,
            timezone: "UTC".to_string(),
        });
        workflow
    }

    #[test]
    fn refresh_tracks_enabled_workflows_and_next_run() {
        let service = ScheduleService::new();
        let workflow = workflow_with_schedule("wf-1", "*/15 * * * *", true);

        service
            .refresh(&[workflow], utc("2026-06-16T00:01:00Z"))
            .expect("refresh schedules");

        let statuses = service.statuses();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].workflow_id, "wf-1");
        assert_eq!(
            statuses[0].next_run_at.expect("next").to_rfc3339(),
            "2026-06-16T00:15:00+00:00"
        );
    }

    #[test]
    fn disabled_schedule_has_no_next_run() {
        let service = ScheduleService::new();
        let workflow = workflow_with_schedule("wf-1", "*/15 * * * *", false);

        service
            .refresh(&[workflow], utc("2026-06-16T00:01:00Z"))
            .expect("refresh schedules");

        let statuses = service.statuses();
        assert!(!statuses[0].enabled);
        assert_eq!(statuses[0].next_run_at, None);
    }

    #[test]
    fn claim_due_run_advances_next_run_and_records_last_run() {
        let service = ScheduleService::new();
        let workflow = workflow_with_schedule("wf-1", "*/15 * * * *", true);
        service
            .refresh(&[workflow], utc("2026-06-16T00:01:00Z"))
            .expect("refresh schedules");

        let candidate = service
            .claim_due_run(utc("2026-06-16T00:15:00Z"), false)
            .expect("candidate");

        assert_eq!(candidate.workflow_id, "wf-1");
        let status = service.statuses().remove(0);
        assert_eq!(
            status.last_run_at.expect("last run").to_rfc3339(),
            "2026-06-16T00:15:00+00:00"
        );
        assert_eq!(
            status.next_run_at.expect("next").to_rfc3339(),
            "2026-06-16T00:30:00+00:00"
        );
    }

    #[test]
    fn active_manual_run_skips_due_occurrence() {
        let service = ScheduleService::new();
        let workflow = workflow_with_schedule("wf-1", "*/15 * * * *", true);
        service
            .refresh(&[workflow], utc("2026-06-16T00:01:00Z"))
            .expect("refresh schedules");

        let candidate = service.claim_due_run(utc("2026-06-16T00:15:00Z"), true);

        assert!(candidate.is_none());
        let status = service.statuses().remove(0);
        assert_eq!(
            status.last_skipped_at.expect("skipped").to_rfc3339(),
            "2026-06-16T00:15:00+00:00"
        );
        assert_eq!(
            status.last_error.as_deref(),
            Some("Skipped because another workflow run was active")
        );
    }
}

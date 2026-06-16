use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use engine::WorkflowSchedule;
use std::str::FromStr;

use super::model::ScheduleConfigError;

pub fn normalize_cron_expression(expression: &str) -> Result<String, ScheduleConfigError> {
    let parts: Vec<&str> = expression.split_whitespace().collect();
    match parts.len() {
        5 => Ok(format!("0 {}", parts.join(" "))),
        6 | 7 => Ok(parts.join(" ")),
        0 => Err(ScheduleConfigError::EmptyCron),
        count => Err(ScheduleConfigError::WrongFieldCount(count)),
    }
}

pub fn parse_timezone(raw: &str) -> Result<Tz, ScheduleConfigError> {
    let name = if raw.trim().is_empty() {
        "UTC"
    } else {
        raw.trim()
    };
    name.parse::<Tz>()
        .map_err(|_| ScheduleConfigError::InvalidTimezone(name.to_string()))
}

pub fn next_run_after(
    schedule: &WorkflowSchedule,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, ScheduleConfigError> {
    let expression = normalize_cron_expression(&schedule.cron)?;
    let timezone = parse_timezone(&schedule.timezone)?;
    let parsed = Schedule::from_str(&expression)
        .map_err(|error| ScheduleConfigError::InvalidCron(error.to_string()))?;
    let local_after = after.with_timezone(&timezone);
    parsed
        .after(&local_after)
        .next()
        .map(|next| next.with_timezone(&Utc))
        .ok_or(ScheduleConfigError::NoFutureRun)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_field_cron_gets_zero_second_prefix() {
        assert_eq!(
            normalize_cron_expression("*/15 * * * *").expect("valid cron"),
            "0 */15 * * * *"
        );
    }

    #[test]
    fn six_field_cron_passes_through() {
        assert_eq!(
            normalize_cron_expression("30 */15 * * * *").expect("valid cron"),
            "30 */15 * * * *"
        );
    }

    #[test]
    fn blank_timezone_defaults_to_utc() {
        assert_eq!(parse_timezone("").expect("timezone"), chrono_tz::UTC);
    }

    #[test]
    fn invalid_timezone_is_reported_with_original_value() {
        let error = parse_timezone("Mars/Base").expect_err("invalid timezone");
        assert_eq!(
            error,
            ScheduleConfigError::InvalidTimezone("Mars/Base".to_string())
        );
    }

    #[test]
    fn computes_next_run_in_configured_timezone() {
        let schedule = WorkflowSchedule {
            cron: "30 9 * * *".to_string(),
            enabled: true,
            timezone: "Australia/Perth".to_string(),
        };
        let after = "2026-06-16T00:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("timestamp");

        let next = next_run_after(&schedule, after).expect("next run");

        assert_eq!(next.to_rfc3339(), "2026-06-16T01:30:00+00:00");
    }
}

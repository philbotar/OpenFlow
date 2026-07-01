use crate::api::{ScheduleDraft, ScheduleIntervalUnit, SchedulePreset};
use engine::WorkflowSchedule;

const DEFAULT_TIME: &str = "09:00";
const DEFAULT_INTERVAL_VALUE: &str = "30";
const DEFAULT_CRON: &str = "0 9 * * *";
const UTC_TIMEZONE: &str = "UTC";
const ALL_WEEKDAYS: [&str; 7] = ["0", "1", "2", "3", "4", "5", "6"];
const WEEKDAY_PRESET: [&str; 5] = ["1", "2", "3", "4", "5"];

#[must_use]
pub fn schedule_from_preset(draft: &ScheduleDraft) -> WorkflowSchedule {
    match draft.preset {
        SchedulePreset::Interval => {
            let interval_unit = draft.interval_unit;
            let value = parse_interval_value(&draft.interval_value, interval_unit);
            with_schedule(
                interval_cron_from_draft(value, interval_unit, &draft.time),
                draft.enabled,
            )
        }
        SchedulePreset::Custom => {
            let cron = draft
                .custom_cron
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_CRON)
                .to_string();
            with_schedule(cron, draft.enabled)
        }
        SchedulePreset::Timed => {
            let (hour, minute) = time_parts(&draft.time);
            with_schedule(
                format!("{minute} {hour} * * {}", cron_day_of_week(&draft.weekdays)),
                draft.enabled,
            )
        }
    }
}

#[must_use]
pub fn schedule_preset_from_cron(cron: &str, enabled: bool) -> ScheduleDraft {
    let parts: Vec<&str> = cron.split_whitespace().collect();
    let base = base_schedule_draft(enabled);

    if parts.len() == 5 {
        let minute = parts[0];
        let hour = parts[1];
        let day_of_month = parts[2];
        let month = parts[3];
        let day_of_week = parts[4];
        let fixed_time = if is_one_or_two_digits(minute) && is_one_or_two_digits(hour) {
            time_from_cron_parts(hour, minute)
        } else {
            DEFAULT_TIME.to_string()
        };

        if day_of_month == "*" && month == "*" {
            if let Some(weekdays) = weekdays_from_cron_day_of_week(day_of_week) {
                if is_one_or_two_digits(minute) && is_one_or_two_digits(hour) {
                    return ScheduleDraft {
                        preset: SchedulePreset::Timed,
                        time: fixed_time,
                        weekdays,
                        ..base
                    };
                }
            }
        }

        if let Some(interval) = interval_from_cron_parts(&parts) {
            let interval_unit = interval.unit;
            return ScheduleDraft {
                preset: SchedulePreset::Interval,
                time: interval.time.unwrap_or_else(|| base.time.clone()),
                interval_value: parse_interval_value(&interval.value, interval_unit).to_string(),
                interval_unit,
                ..base
            };
        }
    }

    ScheduleDraft {
        preset: SchedulePreset::Custom,
        custom_cron: Some(cron.to_string()),
        ..base
    }
}

#[must_use]
pub fn describe_workflow_schedule(schedule: &WorkflowSchedule) -> String {
    let draft = schedule_preset_from_cron(&schedule.cron, schedule.enabled);
    match draft.preset {
        SchedulePreset::Timed => describe_timed_schedule(&draft.time, &draft.weekdays),
        SchedulePreset::Interval => {
            let value = parse_interval_value(&draft.interval_value, draft.interval_unit);
            let label = match (draft.interval_unit, value == 1) {
                (ScheduleIntervalUnit::Minutes, true) => "minute",
                (ScheduleIntervalUnit::Minutes, false) => "minutes",
                (ScheduleIntervalUnit::Hours, true) => "hour",
                (ScheduleIntervalUnit::Hours, false) => "hours",
                (ScheduleIntervalUnit::Days, true) => "day",
                (ScheduleIntervalUnit::Days, false) => "days",
            };
            if draft.interval_unit == ScheduleIntervalUnit::Days {
                format!("Every {value} {label} at {}", draft.time)
            } else {
                format!("Every {value} {label}")
            }
        }
        SchedulePreset::Custom => "Custom schedule".to_string(),
    }
}

fn base_schedule_draft(enabled: bool) -> ScheduleDraft {
    ScheduleDraft {
        preset: SchedulePreset::Timed,
        time: DEFAULT_TIME.to_string(),
        weekdays: all_weekdays(),
        interval_value: DEFAULT_INTERVAL_VALUE.to_string(),
        interval_unit: ScheduleIntervalUnit::Minutes,
        enabled,
        custom_cron: None,
    }
}

fn all_weekdays() -> Vec<String> {
    ALL_WEEKDAYS
        .iter()
        .map(|day| (*day).to_string())
        .collect::<Vec<_>>()
}

fn parse_interval_value(raw: &str, unit: ScheduleIntervalUnit) -> u32 {
    let parsed = raw.trim().parse::<u32>().ok().unwrap_or(0);
    if parsed < 1 {
        return interval_value_default(unit);
    }
    parsed.min(interval_value_limit(unit))
}

const fn interval_value_default(unit: ScheduleIntervalUnit) -> u32 {
    match unit {
        ScheduleIntervalUnit::Minutes => 30,
        ScheduleIntervalUnit::Hours => 1,
        ScheduleIntervalUnit::Days => 1,
    }
}

const fn interval_value_limit(unit: ScheduleIntervalUnit) -> u32 {
    match unit {
        ScheduleIntervalUnit::Minutes => 59,
        ScheduleIntervalUnit::Hours => 23,
        ScheduleIntervalUnit::Days => 31,
    }
}

fn normalize_weekdays(weekdays: &[String]) -> Vec<String> {
    let mut values = weekdays
        .iter()
        .filter_map(|day| parse_weekday(day))
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values.into_iter().map(|day| day.to_string()).collect()
}

fn parse_weekday(raw: &str) -> Option<u8> {
    let bytes = raw.as_bytes();
    if bytes.len() == 1 && (b'0'..=b'6').contains(&bytes[0]) {
        Some(bytes[0] - b'0')
    } else {
        None
    }
}

fn cron_day_of_week(weekdays: &[String]) -> String {
    let normalized = normalize_weekdays(weekdays);
    if normalized.is_empty() || normalized.len() == ALL_WEEKDAYS.len() {
        return "*".to_string();
    }
    if normalized.len() == WEEKDAY_PRESET.len()
        && normalized
            .iter()
            .zip(WEEKDAY_PRESET.iter())
            .all(|(left, right)| left == right)
    {
        return "1-5".to_string();
    }
    if normalized.len() == 1 {
        return normalized[0].clone();
    }
    normalized.join(",")
}

fn weekdays_from_cron_day_of_week(day_of_week: &str) -> Option<Vec<String>> {
    if day_of_week == "*" {
        return Some(all_weekdays());
    }

    let mut days = Vec::<String>::new();
    for part in day_of_week.split(',') {
        let trimmed = part.trim();
        if let Some((start_raw, end_raw)) = trimmed.split_once('-') {
            let start = parse_weekday(start_raw)?;
            let end = parse_weekday(end_raw)?;
            if start > end {
                return None;
            }
            for day in start..=end {
                days.push(day.to_string());
            }
            continue;
        }
        let single = parse_weekday(trimmed)?;
        days.push(single.to_string());
    }

    Some(normalize_weekdays(&days))
}

fn interval_day_step(value: u32) -> u32 {
    if value < 1 {
        1
    } else {
        value.min(interval_value_limit(ScheduleIntervalUnit::Days))
    }
}

fn interval_cron_from_draft(value: u32, unit: ScheduleIntervalUnit, time: &str) -> String {
    let (hour, minute) = time_parts(time);
    match unit {
        ScheduleIntervalUnit::Minutes => format!("*/{value} * * * *"),
        ScheduleIntervalUnit::Hours => {
            if value == 1 {
                "0 * * * *".to_string()
            } else {
                format!("0 */{value} * * *")
            }
        }
        ScheduleIntervalUnit::Days => {
            format!("{minute} {hour} */{} * *", interval_day_step(value))
        }
    }
}

struct ParsedIntervalCron {
    value: String,
    unit: ScheduleIntervalUnit,
    time: Option<String>,
}

fn interval_from_cron_parts(parts: &[&str]) -> Option<ParsedIntervalCron> {
    let [minute, hour, day_of_month, month, day_of_week] = parts else {
        return None;
    };

    if let Some(step) = step_value(minute) {
        if *hour == "*" && *day_of_month == "*" && *month == "*" && *day_of_week == "*" {
            return Some(ParsedIntervalCron {
                value: step.to_string(),
                unit: ScheduleIntervalUnit::Minutes,
                time: None,
            });
        }
    }

    if *minute == "0" && *day_of_month == "*" && *month == "*" && *day_of_week == "*" {
        if *hour == "*" {
            return Some(ParsedIntervalCron {
                value: "1".to_string(),
                unit: ScheduleIntervalUnit::Hours,
                time: None,
            });
        }
        if let Some(step) = step_value(hour) {
            return Some(ParsedIntervalCron {
                value: step.to_string(),
                unit: ScheduleIntervalUnit::Hours,
                time: None,
            });
        }
    }

    if is_one_or_two_digits(minute)
        && is_one_or_two_digits(hour)
        && *month == "*"
        && *day_of_week == "*"
        && step_value(day_of_month).is_some()
    {
        let step = step_value(day_of_month)?.parse::<u32>().ok()?;
        if step < 1 {
            return None;
        }
        let clamped = step.min(interval_value_limit(ScheduleIntervalUnit::Days));
        return Some(ParsedIntervalCron {
            value: parse_interval_value(&clamped.to_string(), ScheduleIntervalUnit::Days)
                .to_string(),
            unit: ScheduleIntervalUnit::Days,
            time: Some(time_from_cron_parts(hour, minute)),
        });
    }

    None
}

fn step_value(raw: &str) -> Option<&str> {
    let value = raw.strip_prefix("*/")?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(value)
}

fn time_from_cron_parts(hour: &str, minute: &str) -> String {
    format!("{}:{}", pad_time_part(hour), pad_time_part(minute))
}

fn pad_time_part(raw: &str) -> String {
    if raw.len() >= 2 {
        raw.to_string()
    } else {
        format!("0{raw}")
    }
}

fn time_parts(time: &str) -> (u32, u32) {
    let Some((hour_raw, minute_raw)) = time.split_once(':') else {
        return (9, 0);
    };
    if !is_one_or_two_digits(hour_raw)
        || minute_raw.len() != 2
        || !minute_raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return (9, 0);
    }
    let hour = hour_raw.parse::<u32>().ok().unwrap_or(9).clamp(0, 23);
    let minute = minute_raw.parse::<u32>().ok().unwrap_or(0).clamp(0, 59);
    (hour, minute)
}

fn is_one_or_two_digits(raw: &str) -> bool {
    (1..=2).contains(&raw.len()) && raw.bytes().all(|byte| byte.is_ascii_digit())
}

fn with_schedule(cron: String, enabled: bool) -> WorkflowSchedule {
    WorkflowSchedule {
        cron,
        enabled,
        timezone: UTC_TIMEZONE.to_string(),
    }
}

fn describe_timed_schedule(time: &str, weekdays: &[String]) -> String {
    let normalized = normalize_weekdays(weekdays);
    if normalized.is_empty() || normalized.len() == ALL_WEEKDAYS.len() {
        return format!("Daily at {time}");
    }
    if normalized.len() == WEEKDAY_PRESET.len()
        && normalized
            .iter()
            .zip(WEEKDAY_PRESET.iter())
            .all(|(left, right)| left == right)
    {
        return format!("Weekdays at {time}");
    }
    if normalized.len() == 1 {
        let weekday = long_weekday_label(&normalized[0]).unwrap_or("Weekly");
        return format!("{weekday}s at {time}");
    }
    let labels = normalized
        .iter()
        .map(|day| short_weekday_label(day).unwrap_or_else(|| day.clone()))
        .collect::<Vec<_>>();
    format!("{} at {time}", labels.join(", "))
}

fn short_weekday_label(day: &str) -> Option<String> {
    match day {
        "0" => Some("Sun".to_string()),
        "1" => Some("Mon".to_string()),
        "2" => Some("Tue".to_string()),
        "3" => Some("Wed".to_string()),
        "4" => Some("Thu".to_string()),
        "5" => Some("Fri".to_string()),
        "6" => Some("Sat".to_string()),
        _ => None,
    }
}

fn long_weekday_label(day: &str) -> Option<&'static str> {
    match day {
        "0" => Some("Sunday"),
        "1" => Some("Monday"),
        "2" => Some("Tuesday"),
        "3" => Some("Wednesday"),
        "4" => Some("Thursday"),
        "5" => Some("Friday"),
        "6" => Some("Saturday"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval_draft(value: &str, unit: ScheduleIntervalUnit, time: &str) -> ScheduleDraft {
        ScheduleDraft {
            preset: SchedulePreset::Interval,
            time: time.to_string(),
            weekdays: all_weekdays(),
            interval_value: value.to_string(),
            interval_unit: unit,
            enabled: true,
            custom_cron: None,
        }
    }

    #[test]
    fn timed_presets_serialize_selected_weekdays_to_cron() {
        let weekdays = WEEKDAY_PRESET
            .iter()
            .map(|day| (*day).to_string())
            .collect::<Vec<_>>();
        let schedule = schedule_from_preset(&ScheduleDraft {
            preset: SchedulePreset::Timed,
            time: "08:30".to_string(),
            weekdays,
            interval_value: DEFAULT_INTERVAL_VALUE.to_string(),
            interval_unit: ScheduleIntervalUnit::Minutes,
            enabled: true,
            custom_cron: None,
        });
        assert_eq!(
            schedule,
            WorkflowSchedule {
                cron: "30 8 * * 1-5".to_string(),
                enabled: true,
                timezone: UTC_TIMEZONE.to_string(),
            }
        );

        let schedule = schedule_from_preset(&ScheduleDraft {
            preset: SchedulePreset::Timed,
            time: "08:30".to_string(),
            weekdays: vec!["1".to_string(), "3".to_string(), "5".to_string()],
            interval_value: DEFAULT_INTERVAL_VALUE.to_string(),
            interval_unit: ScheduleIntervalUnit::Minutes,
            enabled: true,
            custom_cron: None,
        });
        assert_eq!(
            schedule,
            WorkflowSchedule {
                cron: "30 8 * * 1,3,5".to_string(),
                enabled: true,
                timezone: UTC_TIMEZONE.to_string(),
            }
        );
    }

    #[test]
    fn interval_presets_serialize_minute_hour_and_day_cron() {
        assert_eq!(
            schedule_from_preset(&interval_draft(
                "45",
                ScheduleIntervalUnit::Minutes,
                "08:30"
            )),
            WorkflowSchedule {
                cron: "*/45 * * * *".to_string(),
                enabled: true,
                timezone: UTC_TIMEZONE.to_string(),
            }
        );
        assert_eq!(
            schedule_from_preset(&interval_draft("2", ScheduleIntervalUnit::Hours, "08:30")),
            WorkflowSchedule {
                cron: "0 */2 * * *".to_string(),
                enabled: true,
                timezone: UTC_TIMEZONE.to_string(),
            }
        );
        assert_eq!(
            schedule_from_preset(&interval_draft("31", ScheduleIntervalUnit::Days, "09:00")),
            WorkflowSchedule {
                cron: "0 9 */31 * *".to_string(),
                enabled: true,
                timezone: UTC_TIMEZONE.to_string(),
            }
        );
    }

    #[test]
    fn invalid_day_steps_load_as_clamped_day_intervals() {
        let draft = schedule_preset_from_cron("0 9 */210 * *", true);
        assert_eq!(draft.preset, SchedulePreset::Interval);
        assert_eq!(draft.interval_value, "31");
        assert_eq!(draft.interval_unit, ScheduleIntervalUnit::Days);
        assert_eq!(draft.time, "09:00");
        assert_eq!(
            describe_workflow_schedule(&schedule_from_preset(&draft)),
            "Every 31 days at 09:00"
        );
    }

    #[test]
    fn describe_workflow_schedule_creates_compact_row_summaries() {
        assert_eq!(
            describe_workflow_schedule(&WorkflowSchedule {
                cron: "30 8 * * 1-5".to_string(),
                enabled: true,
                timezone: "Australia/Perth".to_string(),
            }),
            "Weekdays at 08:30"
        );
        assert_eq!(
            describe_workflow_schedule(&WorkflowSchedule {
                cron: "30 8 * * 1,3,5".to_string(),
                enabled: true,
                timezone: "Australia/Perth".to_string(),
            }),
            "Mon, Wed, Fri at 08:30"
        );
        assert_eq!(
            describe_workflow_schedule(&WorkflowSchedule {
                cron: "*/30 * * * *".to_string(),
                enabled: true,
                timezone: "Australia/Perth".to_string(),
            }),
            "Every 30 minutes"
        );
        assert_eq!(
            describe_workflow_schedule(&WorkflowSchedule {
                cron: "0 */2 * * *".to_string(),
                enabled: true,
                timezone: "Australia/Perth".to_string(),
            }),
            "Every 2 hours"
        );
        assert_eq!(
            describe_workflow_schedule(&WorkflowSchedule {
                cron: "0 9 */2 * *".to_string(),
                enabled: true,
                timezone: "Australia/Perth".to_string(),
            }),
            "Every 2 days at 09:00"
        );
    }

    #[test]
    fn schedule_preset_from_cron_recognizes_common_persisted_schedules() {
        assert_eq!(
            schedule_preset_from_cron("30 8 * * 1-5", true),
            ScheduleDraft {
                preset: SchedulePreset::Timed,
                time: "08:30".to_string(),
                weekdays: vec![
                    "1".to_string(),
                    "2".to_string(),
                    "3".to_string(),
                    "4".to_string(),
                    "5".to_string()
                ],
                interval_value: DEFAULT_INTERVAL_VALUE.to_string(),
                interval_unit: ScheduleIntervalUnit::Minutes,
                enabled: true,
                custom_cron: None,
            }
        );
        assert_eq!(
            schedule_preset_from_cron("*/15 * * * *", true).preset,
            SchedulePreset::Interval
        );
        assert_eq!(
            schedule_preset_from_cron("0 * * * *", true).interval_unit,
            ScheduleIntervalUnit::Hours
        );
        assert_eq!(
            schedule_preset_from_cron("0 9 */14 * *", true),
            ScheduleDraft {
                preset: SchedulePreset::Interval,
                time: "09:00".to_string(),
                weekdays: all_weekdays(),
                interval_value: "14".to_string(),
                interval_unit: ScheduleIntervalUnit::Days,
                enabled: true,
                custom_cron: None,
            }
        );
    }

    #[test]
    fn unknown_cron_falls_back_to_custom_preset() {
        let draft = schedule_preset_from_cron("foo bar", false);
        assert_eq!(draft.preset, SchedulePreset::Custom);
        assert_eq!(draft.custom_cron.as_deref(), Some("foo bar"));
        assert!(!draft.enabled);
    }
}

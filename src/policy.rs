use std::str::FromStr;
use chrono::Duration;
use cron::Schedule;
use duration_string::DurationString;
use log::warn;
use regex::Regex;

pub const DEFAULT_REVISIONS: usize = 15;
/// Per default the schedule is set to daily at midnight
pub const DEFAULT_SCHEDULE: &str = "0 0 * * *";

/// Parse a revisions label. Should the label value not be a valid revisions count
/// the provided or the global default (`DEFAULT_REVISIONS`) is returned as fallback
pub fn parse_revisions(revisions_str: String, default: Option<usize>) -> usize {
    if let Ok(revisions) = revisions_str.parse::<usize>(){
        return revisions
    } else {
        warn!("Received invalid revisions value '{revisions_str}'. Expected positive integer. Using default ({}) instead", default.unwrap_or(DEFAULT_REVISIONS))
    }
    default.unwrap_or(DEFAULT_REVISIONS)
}

/// Parse a duration label. Should the label value not be a valid duration the provided default
/// or `None` is returned as fallback <br>
/// **Important**: Allowed duration values have to match the following regex `[0-9]+(ns|us|ms|[smhdwy])`
pub fn parse_duration(duration_str: String, default: Option<Duration>) -> Option<Duration> {
    match DurationString::from_string(duration_str.clone()) {
        Ok(duration_str) => {
            if let Ok(duration) = Duration::from_std(duration_str.into()) {
                return Some(duration)
            } else if let Some(default) = default {
                warn!("Received out of range duration '{duration_str}'. Using default ({}d) instead", default.num_days())
            } else {
                warn!("Received out of range duration '{duration_str}'. Using none instead")
            }
        },
        Err(_) => {
            if let Some(default) = default {
                warn!("Received out of range duration '{duration_str}'. Using default ({}d) instead", default.num_days())
            } else {
                warn!("Received out of range duration '{duration_str}'. Using none instead")
            }
        }
    }
    default
}

/// Parse a pattern label. Should the label value not be a valid rust regex an empty regex
/// which doesn't match anything is returned as fallback
pub fn parse_pattern(pattern_str: &str) -> Regex {
    Regex::new(pattern_str).unwrap_or_else(|_| {
        warn!("Received invalid pattern '{pattern_str}'. Using empty pattern instead");
        Regex::new("").expect("Empty regex should be valid")
    })
}

/// Parse a cron schedule label. Should the label value not be valid cron string the provided or
/// global default (`DEFAULT_SCHEDULE`) is returned as fallback
///
/// # Example
/// ```
/// // cron format: <sec> <min> <hour> <day of month> <month> <day of week> <year>
/// let daily_at_midnight = "0 0 * * * * *";
pub fn parse_schedule(schedule_str: &str, default: Option<String>) -> String {
    if let Ok(_) = Schedule::from_str(schedule_str) {
        schedule_str.to_string()
    } else {
        let default = default.unwrap_or(DEFAULT_SCHEDULE.to_string());
        warn!("Received invalid schedule '{schedule_str}'. Using default ({default}) instead");
        default
    }
}
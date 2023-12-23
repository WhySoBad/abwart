use std::fmt::Debug;
use std::str::FromStr;
use chrono::Duration;
use cron::Schedule;
use duration_string::DurationString;
use log::warn;

pub mod age_max;
pub mod age_min;
pub mod pattern;
pub mod revision;

#[derive(Eq, PartialEq)]
pub enum AffectionType {
    /// `Requirement` affections are matched after matching all [`AffectionType::Target`] affections. This is to ensure all
    /// targeted repositories/tags fulfil the policy and to prevent targeting all repositories/tags which fulfil
    /// the policy in a first place
    Requirement,
    /// `Target` affections are all matched before matching any [`AffectionType::Requirement`] affections. Any repository/tag which
    /// fulfils the policy should be targeted for further usage
    Target
}

pub trait Policy<T>: Debug + Send + Sync {
    /// All repositories/tags which are affected by this policy <br>
    /// **Important:** When the policy is of [`AffectionType::Requirement`] the inverse is returned. Means
    /// instead of returning which repositories/tags should be targeted it returns which should be un-targeted
    fn affects(&self, elements: Vec<T>) -> Vec<T>;

    /// Affection type of the policy
    fn affection_type(&self) -> AffectionType;

    /// Identifier of the policy used for internal identification. Same as the constant value
    /// `<POLICY_NAME>_LABEL`
    fn id(&self) -> &'static str;
}

pub fn parse_integer(value: String) -> Option<u32> {
    value.parse::<u32>().ok()
}

/// Per default the schedule is set to daily at midnight
pub const DEFAULT_SCHEDULE: &str = "0 0 * * * * *";

/// Parse a duration <br>
/// **Important**: Allowed duration values have to match the following regex `[0-9]+(ns|us|ms|[smhdwy])`
pub fn parse_duration(duration_str: String) -> Option<Duration> {
    match DurationString::from_string(duration_str.clone()) {
        Ok(duration_str) => Duration::from_std(duration_str.into()).ok(),
        Err(_) => None
    }
}

/// Parse a cron schedule label. Should the label value not be valid cron string the provided or
/// global default (`DEFAULT_SCHEDULE`) is returned as fallback
///
/// # Example
/// ```
/// // cron format: <sec> <min> <hour> <day of month> <month> <day of week> <year>
/// let daily_at_midnight = "0 0 * * * * *";
pub fn parse_schedule(schedule_str: &str) -> Option<String> {
    if Schedule::from_str(schedule_str).is_ok() {
        Some(schedule_str.to_string())
    } else {
        warn!("Received invalid schedule '{schedule_str}'");
        None
    }
}
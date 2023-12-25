use std::collections::HashMap;
use std::fmt::Debug;
use chrono::Duration;
use duration_string::DurationString;
use dyn_clone::DynClone;
use crate::api::repository::Repository;
use crate::api::tag::Tag;

pub mod age_max;
pub mod age_min;
pub mod pattern;
pub mod revision;

pub type PolicyMap<T> = HashMap<&'static str, Box<dyn Policy<T>>>;

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

dyn_clone::clone_trait_object!(Policy<Repository>);
dyn_clone::clone_trait_object!(Policy<Tag>);

pub trait Policy<T>: Debug + Send + Sync + DynClone {
    /// All repositories/tags which are affected by this policy <br>
    /// **Important:** When the policy is of [`AffectionType::Requirement`] the inverse is returned. Means
    /// instead of returning which repositories/tags should be targeted it returns which should be un-targeted
    fn affects(&self, elements: Vec<T>) -> Vec<T>;

    /// Affection type of the policy
    fn affection_type(&self) -> AffectionType;

    /// Identifier of the policy used for internal identification. Same as the constant value
    /// `<POLICY_NAME>_LABEL`
    fn id(&self) -> &'static str;

    fn enabled(&self) -> bool;
}

pub fn parse_integer(value: String) -> Option<u32> {
    value.parse::<u32>().ok()
}

/// Parse a duration <br>
/// **Important**: Allowed duration values have to match the following regex `[0-9]+(ns|us|ms|[smhdwy])`
pub fn parse_duration(duration_str: String) -> Option<Duration> {
    match DurationString::from_string(duration_str.clone()) {
        Ok(duration_str) => Duration::from_std(duration_str.into()).ok(),
        Err(_) => None
    }
}
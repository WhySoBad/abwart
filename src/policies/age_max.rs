use chrono::{Duration, Utc};
use log::info;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, parse_duration, Policy};

pub const AGE_MAX_LABEL: &str = "age.max";

/// Policy to match all tags older than a given duration
/// # Example
/// ```
/// let policy = AgeMaxPolicy::new(String::from("30d"));
///
/// // returns all tags which are older than 30 days
/// let affected = policy.affects(&tags);
/// ```
#[derive(Debug, Clone, Default)]
pub struct AgeMaxPolicy {
    age: Option<Duration>
}

impl AgeMaxPolicy {
    pub fn new(value: String) -> Self {
        if value.is_empty() {
            Self { age: None }
        } else {
            let age = parse_duration(value.clone());
            if age.is_none() {
                info!("Received invalid max age duration '{value}'")
            }
            Self { age }
        }
    }
}

impl Policy<Tag> for AgeMaxPolicy {
    fn affects(&self, tags: Vec<Tag>) -> Vec<Tag> {
        if let Some(age) = self.age {
            let now = Utc::now();
            tags.into_iter().filter(|tag| (tag.created + age) <= now).collect()
        } else {
            vec![]
        }
    }

    fn affection_type(&self) -> AffectionType {
        AffectionType::Target
    }

    fn id(&self) -> &'static str {
        AGE_MAX_LABEL
    }

    fn enabled(&self) -> bool {
        self.age.is_some()
    }
}

#[cfg(test)]
mod test {
    use chrono::Duration;
    use crate::api::tag::Tag;
    use crate::policies::age_max::AgeMaxPolicy;
    use crate::policies::Policy;
    use crate::test::get_tags;

    fn get_current_tags() -> Vec<Tag> {
        get_tags(vec![
            ("first", Duration::hours(-5), 1_000_000),
            ("second", Duration::minutes(-5), 1_000_000),
            ("third", Duration::minutes(-30), 1_000_000),
            ("fourth", Duration::minutes(-10), 1_000_000),
            ("fifth", Duration::seconds(-15), 1_000_000),
            ("sixth", Duration::minutes(-50), 1_000_000)
        ])
    }

    #[test]
    pub fn test_keeping() {
        let tags = get_current_tags();
        let policy = AgeMaxPolicy { age: Some(Duration::minutes(10)) };
        assert!(policy.age.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[0].clone(), tags[2].clone(), tags[3].clone(), tags[5].clone()])
    }

    #[test]
    pub fn test_in_future() {
        let tags = get_current_tags();
        let policy = AgeMaxPolicy { age: Some(Duration::days(10)) };
        assert!(policy.age.is_some());
        assert_eq!(policy.affects(tags), vec![])
    }

    #[test]
    pub fn test_invalid_duration() {
        let tags = get_current_tags();
        let policy = AgeMaxPolicy::new(String::from("asdf"));
        assert!(policy.age.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }
}
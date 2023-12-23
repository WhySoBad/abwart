use chrono::{Duration, Utc};
use log::info;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, parse_duration, Policy};

pub const AGE_MIN_LABEL: &str = "age.min";
pub const DEFAULT_AGE_MIN: Option<Duration> = None;

/// Policy to match all tags which have at least a given age
/// # Example
/// ```
/// let policy = AgeMinPolicy::new(String::from("5m"));
///
/// // returns only these tags which are at least 5 minutes old
/// let affected = policy.affects(&tags);
/// ```
#[derive(Debug, Clone)]
pub struct AgeMinPolicy {
    age: Option<Duration>
}

impl AgeMinPolicy {
    pub fn new(value: String) -> Self {
        let age = parse_duration(value);
        if age.is_none() {
            info!("Received invalid min age duration '{value}'")
        }
        Self { age }
    }

    pub fn from(age: Option<Duration>) -> Self {
        Self { age }
    }
}

impl Policy<Tag> for AgeMinPolicy {
    fn affects(&self, tags: Vec<Tag>) -> Vec<Tag> {
        if let Some(age) = self.age {
            let now = Utc::now();
            tags.into_iter().filter(|tag| (tag.created + age) > now).collect()
        } else {
            vec![]
        }
    }

    fn affection_type(&self) -> AffectionType {
        AffectionType::Requirement
    }

    fn id(&self) -> &'static str {
        AGE_MIN_LABEL
    }
}

#[cfg(test)]
mod test {
    use chrono::Duration;
    use crate::api::tag::Tag;
    use crate::policies::age_min::AgeMinPolicy;
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
        let policy = AgeMinPolicy::from(Some(Duration::minutes(10)));
        assert!(policy.age.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[1].clone(), tags[4].clone()])
    }

    #[test]
    pub fn test_in_future() {
        let tags = get_current_tags();
        let policy = AgeMinPolicy::from(Some(Duration::days(10)));
        assert!(policy.age.is_some());
        assert_eq!(policy.affects(tags.clone()), tags)
    }

    #[test]
    pub fn test_invalid_duration() {
        let tags = get_current_tags();
        let policy = AgeMinPolicy::new(String::from("asdf"));
        assert!(policy.age.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }
}

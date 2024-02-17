use log::info;
use regex::Regex;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, Policy};

pub const TAG_PATTERN_LABEL: &str = "tag.pattern";

/// Policy to match all tags whose name matches the provided
/// regex pattern
/// # Example
/// ```
/// let policy = TagPatternPolicy::new("test-\\w+");
///
/// // returns all tags whose name contains `test-<chars>` whereby
/// // `<chars>` is any alphanumeric character sequence of length >= 1
/// let affected = policy.affects(&tags);
/// ```
#[derive(Debug, Clone)]
pub struct TagPatternPolicy {
    pattern: Option<Regex>
}

impl TagPatternPolicy {
    pub fn new(value: &str) -> Self {
        if value.trim() == "" {
            return Self { pattern: None }
        }
        match Regex::new(value) {
            Ok(regex) => Self { pattern: Some(regex) },
            Err(err) => {
                info!("Received invalid pattern '{value}'. Reason: {err}");
                Self { pattern: None }
            }
        }
    }
}

impl Default for TagPatternPolicy {
    fn default() -> Self {
        Self { pattern: Some(Regex::new(".*").expect("Default regex should compile")) }
    }
}

impl Policy<Tag> for TagPatternPolicy {
    fn affects(&self, elements: Vec<Tag>) -> Vec<Tag> {
        if let Some(pattern) = &self.pattern {
            elements.into_iter().filter(|tag| pattern.is_match(&tag.name)).collect()
        } else {
            vec![]
        }
    }

    fn affection_type(&self) -> AffectionType {
        AffectionType::Target
    }

    fn id(&self) -> &'static str {
        TAG_PATTERN_LABEL
    }

    fn enabled(&self) -> bool {
        self.pattern.is_some()
    }
}


#[cfg(test)]
mod test {
    use chrono::Duration;
    use crate::policies::Policy;
    use crate::policies::tag_pattern::TagPatternPolicy;
    use crate::test::get_tags_by_name;

    #[test]
    pub fn test_matching() {
        let tags = get_tags_by_name(vec!["test-matching", "not-matching"], Duration::seconds(1), 1);
        let policy = TagPatternPolicy::new("test-.*");
        assert!(policy.pattern.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[0].clone()]);
    }

    #[test]
    pub fn test_empty() {
        let tags = get_tags_by_name(vec!["test-matching", "not-matching"], Duration::seconds(1), 1);
        let policy = TagPatternPolicy::new("");
        assert!(policy.pattern.is_none());
        assert_eq!(policy.affects(tags), vec![]);
    }

    #[test]
    pub fn test_default() {
        let tags = get_tags_by_name(vec!["test-matching", "not-matching"], Duration::seconds(1), 1);
        let policy = TagPatternPolicy::default();
        assert!(policy.pattern.is_some());
        assert_eq!(policy.affects(tags.clone()), tags)
    }

    #[test]
    pub fn test_invalid_regex() {
        let tags = get_tags_by_name(vec!["test-matching", "not-matching"], Duration::seconds(1), 1);
        let policy = TagPatternPolicy::new("([a-zA-Z]+"); // the regex is invalid
        assert!(policy.pattern.is_none());
        assert_eq!(policy.affects(tags), vec![]);
    }
}
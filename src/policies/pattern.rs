use log::info;
use regex::Regex;
use crate::api::repository::Repository;
use crate::policies::{AffectionType, Policy};

pub const PATTERN_LABEL: &str = "pattern";

/// Policy to match all repositories whose name matches the provided
/// regex pattern
/// # Example
/// ```
/// let policy = PatternPolicy::new("test-\\w+");
///
/// // returns all repositories whose name contains `test-<chars>` whereby
/// // `<chars>` is any alphanumeric character sequence of length >= 1
/// let affected = policy.affects(&repositories);
/// ```
#[derive(Debug, Clone)]
pub struct PatternPolicy {
    pattern: Option<Regex>
}

impl PatternPolicy {
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

impl Default for PatternPolicy {
    fn default() -> Self {
        Self { pattern: Some(Regex::new(".*").expect("Default regex should compile")) }
    }
}

impl Policy<Repository> for PatternPolicy {
    fn affects(&self, elements: Vec<Repository>) -> Vec<Repository> {
        if let Some(pattern) = &self.pattern {
            elements.into_iter().filter(|repo| pattern.is_match(&repo.name)).collect()
        } else {
            vec![]
        }
    }

    fn affection_type(&self) -> AffectionType {
        AffectionType::Target
    }

    fn id(&self) -> &'static str {
        PATTERN_LABEL
    }

    fn enabled(&self) -> bool {
        self.pattern.is_some()
    }
}


#[cfg(test)]
mod test {
    use crate::policies::pattern::PatternPolicy;
    use crate::policies::Policy;
    use crate::test::get_repositories;

    #[test]
    pub fn test_matching() {
        let repositories = get_repositories(vec!["test-matching", "not-matching"]);
        let policy = PatternPolicy::new("test-.*");
        assert!(policy.pattern.is_some());
        assert_eq!(policy.affects(repositories.clone()), vec![repositories[0].clone()]);
    }

    #[test]
    pub fn test_empty() {
        let repositories = get_repositories(vec!["test-matching", "not-matching"]);
        let policy = PatternPolicy::new("");
        assert!(policy.pattern.is_none());
        assert_eq!(policy.affects(repositories), vec![]);
    }

    #[test]
    pub fn test_default() {
        let repositories = get_repositories(vec!["test-matching", "not-matching"]);
        let policy = PatternPolicy::default();
        assert!(policy.pattern.is_some());
        assert_eq!(policy.affects(repositories.clone()), repositories)
    }

    #[test]
    pub fn test_invalid_regex() {
        let repositories = get_repositories(vec!["test-matching", "not-matching"]);
        let policy = PatternPolicy::new("([a-zA-Z]+"); // the regex is invalid
        assert!(policy.pattern.is_none());
        assert_eq!(policy.affects(repositories), vec![]);
    }
}
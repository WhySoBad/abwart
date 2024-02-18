use log::info;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, parse_size, Policy};

pub const SIZE_LABEL: &str = "size";

/// Policy to match all tags which exceed a given blob size
/// # Example
/// ```
/// let policy = SizePolicy::new(String::from("0.2 GiB"));
///
/// // returns all tags which are bigger than 0.2 GiB
/// let affected = policy.affects(&tags);

#[derive(Debug, Clone, Default)]
pub struct SizePolicy {
    size: Option<u64>
}

impl SizePolicy {
    pub fn new(value: &str) -> Self {
        if value.is_empty() {
            Self { size: None }
        } else {
            let size = parse_size(value);
            if size.is_none() {
                info!("Received invalid size '{value}'")
            }
            Self { size }
        }
    }
}

impl Policy<Tag> for SizePolicy {
    fn affects(&self, tags: Vec<Tag>) -> Vec<Tag> {
        if let Some(size) = self.size {
            tags.into_iter().filter(|tag| tag.size >= size).collect()
        } else {
            vec![]
        }
    }

    fn affection_type(&self) -> AffectionType {
        AffectionType::Target
    }

    fn id(&self) -> &'static str {
        SIZE_LABEL
    }

    fn enabled(&self) -> bool {
        self.size.is_some()
    }
}

#[cfg(test)]
mod test {
    use chrono::Duration;
    use crate::api::tag::Tag;
    use crate::policies::Policy;
    use crate::policies::size::SizePolicy;
    use crate::test::get_tags;

    fn get_current_tags() -> Vec<Tag> {
        get_tags(vec![
            ("first", Duration::hours(-5), 1_200_000),
            ("second", Duration::minutes(-5), 1_000),
            ("third", Duration::minutes(-30), 100_000_000),
            ("fourth", Duration::minutes(-10), 100_000),
            ("fifth", Duration::seconds(-15), 1_300_000),
            ("sixth", Duration::minutes(-50), 1_100_000)
        ])
    }

    #[test]
    pub fn test_matching() {
        let tags = get_current_tags();
        let policy = SizePolicy::new("1 MiB");
        assert!(policy.size.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[0].clone(), tags[2].clone(), tags[4].clone(), tags[5].clone()])
    }

    #[test]
    pub fn test_empty() {
        let tags = get_current_tags();
        let policy = SizePolicy::new("");
        assert!(policy.size.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }

    #[test]
    pub fn test_default() {
        let tags = get_current_tags();
        let policy = SizePolicy::default();
        assert!(policy.size.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }

    #[test]
    pub fn test_invalid_size() {
        let tags = get_current_tags();
        let policy = SizePolicy::new("120 asdf");
        assert!(policy.size.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }

    #[test]
    pub fn test_negative_size() {
        let tags = get_current_tags();
        let policy = SizePolicy::new("-1 MiB");
        assert!(policy.size.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }

    #[test]
    pub fn test_without_unit() {
        let tags = get_current_tags();
        let policy = SizePolicy::new("1_048_576");
        assert!(policy.size.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[0].clone(), tags[2].clone(), tags[4].clone(), tags[5].clone()])
    }
}
use log::info;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, Policy, parse_integer};

pub const REVISION_LABEL: &str = "revisions";

#[derive(Debug, Clone)]
pub struct RevisionPolicy {
    revisions: Option<usize>
}

impl RevisionPolicy {
    pub fn new(value: String) -> Self {
        match parse_integer(value.clone()) {
            Some(revisions) => {
                if revisions == 0 {
                    info!("Received invalid revisions value '{revisions}'. Expected non-zero positive integer");
                    Self { revisions: None }
                } else {
                    Self { revisions: Some(revisions as usize) }
                }
            },
            None => {
                info!("Received invalid revisions value '{value}'. Expected non-zero positive integer");
                Self { revisions: None }
            }
        }
    }
}

impl Policy<Tag> for RevisionPolicy {
    fn affects(&self, mut elements: Vec<Tag>) -> Vec<Tag> {
        elements.sort_by(|t1, t2| t1.created.cmp(&t2.created));
        if let Some(revisions) = self.revisions {
            if elements.len() > revisions {
                let length = elements.len();
                elements.into_iter().take(length - revisions).collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        }

    }

    fn affection_type(&self) -> AffectionType {
        AffectionType::Target
    }

    fn id(&self) -> &'static str {
        REVISION_LABEL
    }

    fn enabled(&self) -> bool {
        self.revisions.is_some()
    }
}

impl Default for RevisionPolicy {
    fn default() -> Self {
        Self { revisions: Some(15) }
    }
}

#[cfg(test)]
mod test {
    use chrono::Duration;
    use crate::api::tag::Tag;
    use crate::policies::Policy;
    use crate::policies::revision::RevisionPolicy;
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
    pub fn test_keeping_three() {
        let tags = get_current_tags();
        let policy = RevisionPolicy { revisions: Some(3) };
        assert!(policy.revisions.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[0].clone(), tags[5].clone(), tags[2].clone()])
    }

    #[test]
    pub fn test_keeping_one() {
        let tags = get_current_tags();
        let policy = RevisionPolicy { revisions: Some(1) };
        assert!(policy.revisions.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[0].clone(), tags[5].clone(), tags[2].clone(), tags[3].clone(), tags[1].clone()])
    }

    #[test]
    pub fn test_keeping_more() {
        let tags = get_current_tags();
        let policy = RevisionPolicy { revisions: Some(10) };
        assert!(policy.revisions.is_some());
        assert_eq!(policy.affects(tags), vec![])
    }

    #[test]
    pub fn test_invalid_integer() {
        let tags = get_current_tags();
        let policy = RevisionPolicy::new(String::from("asdf"));
        assert!(policy.revisions.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }
}
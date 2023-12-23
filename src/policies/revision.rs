use log::info;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, Policy, parse_integer};

pub const REVISION_LABEL: &str = "revisions";
pub const DEFAULT_REVISIONS: Option<usize> = Some(15);

#[derive(Debug)]
pub struct RevisionPolicy {
    revisions: Option<usize>
}

impl RevisionPolicy {
    pub fn new(value: String) -> Self {
        match parse_integer(value) {
            Some(revisions) => {
                if revisions <= 0 {
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

    pub fn from(revisions: usize) -> Self {
        Self { revisions: Some(revisions) }
    }
}

impl Policy<Tag> for RevisionPolicy {
    fn affects(&self, mut elements: Vec<Tag>) -> Vec<Tag> {
        elements.sort_by(|t1, t2| t1.created.cmp(&t2.created).reverse());
        if let Some(revisions) = self.revisions {
            if elements.len() > revisions {
                elements.into_iter().take(revisions).collect()
            } else {
                elements
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
    pub fn test_keeping() {
        let tags = get_current_tags();
        let policy = RevisionPolicy::from(3);
        assert!(policy.revisions.is_some());
        assert_eq!(policy.affects(tags.clone()), vec![tags[4].clone(), tags[1].clone(), tags[3].clone()])
    }

    #[test]
    pub fn test_invalid_integer() {
        let tags = get_current_tags();
        let policy = RevisionPolicy::new(String::from("asdf"));
        assert!(policy.revisions.is_none());
        assert_eq!(policy.affects(tags), vec![])
    }
}
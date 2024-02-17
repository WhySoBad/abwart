use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use cron::Schedule;
use log::{debug, info, warn};
use crate::api::repository::Repository;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, PolicyMap};
use crate::policies::age_min::{AGE_MIN_LABEL, AgeMinPolicy};
use crate::policies::age_max::{AGE_MAX_LABEL, AgeMaxPolicy};
use crate::policies::image_pattern::{IMAGE_PATTERN_LABEL, ImagePatternPolicy};
use crate::policies::revision::{REVISION_LABEL, RevisionPolicy};
use crate::policies::tag_pattern::{TAG_PATTERN_LABEL, TagPatternPolicy};

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    pub repository_policies: PolicyMap<Repository>,
    pub tag_policies: PolicyMap<Tag>,
    pub schedule: String,
}

impl Rule{
    pub fn new(name: String) -> Self {
        Self { name, repository_policies: HashMap::new(), tag_policies: HashMap::new(), schedule: String::new() }
    }

    /// Get all repositories which are affected by the current rule
    pub fn affected_repositories(&self, repositories: Vec<Repository>) -> Vec<Repository> {
        let mut requirements = Vec::new();
        let mut affected = HashSet::new();
        for policy in self.repository_policies.values() {
            if policy.affection_type() == AffectionType::Requirement {
                requirements.push(policy);
                continue
            }
            let affects = policy.affects(repositories.clone());
            debug!("Policy '{}' affected {} repositories", policy.id(), affects.len());
            affected.extend(affects)
        }

        if requirements.len() == self.repository_policies.len() && !requirements.is_empty() {
            // there are no target policies and therefore every repository should be affected
            affected.extend(repositories)
        }

        let mut affected = affected.into_iter().collect::<Vec<_>>();

        for requirement in requirements {
            let not_matching = requirement.affects(affected.clone());
            affected.retain(|repo| !not_matching.contains(repo))
        }

        affected
    }

    /// Get all tags which are affected by the current rule
    pub fn affected_tags(&self, tags: Vec<Tag>) -> Vec<Tag> {
        let mut requirements = Vec::new();
        let mut affected = HashSet::new();
        for policy in self.tag_policies.values() {
            if policy.affection_type() == AffectionType::Requirement {
                requirements.push(policy);
                continue
            }
            let affects = policy.affects(tags.clone());
            debug!("Policy '{}' affected {} tags", policy.id(), affects.len());
            affected.extend(affects)
        }

        if requirements.len() == self.tag_policies.len() && !requirements.is_empty() {
            // there are no target policies and therefore every tag should be affected
            affected.extend(tags)
        }

        let mut affected = affected.into_iter().collect::<Vec<_>>();

        for requirement in requirements {
            let not_matching = requirement.affects(affected.clone());
            affected.retain(|tag| !not_matching.contains(tag))
        }

        affected
    }
}

/// Parse a rule by all it's associated labels. Returns `None` should the parsed rule neither contain
/// any tag policies nor any repository policies
pub fn parse_rule(name: String, policies: Vec<(String, &str)>) -> Option<Rule> {
    let mut rule = Rule::new(name.clone());
    policies.into_iter().for_each(|(policy_name, value)| {
        match policy_name.as_str() {
            "schedule" => {
                rule.schedule = parse_schedule(value).unwrap_or_default()
            },
            AGE_MAX_LABEL => {
                rule.tag_policies.insert(AGE_MAX_LABEL, Box::new(AgeMaxPolicy::new(value.to_string())));
            },
            AGE_MIN_LABEL => {
                rule.tag_policies.insert(AGE_MIN_LABEL, Box::new(AgeMinPolicy::new(value.to_string())));
            },
            IMAGE_PATTERN_LABEL => {
                rule.repository_policies.insert(IMAGE_PATTERN_LABEL, Box::new(ImagePatternPolicy::new(value)));
            },
            TAG_PATTERN_LABEL => {
                rule.tag_policies.insert(TAG_PATTERN_LABEL, Box::new(TagPatternPolicy::new(value)));
            }
            REVISION_LABEL => {
                rule.tag_policies.insert(REVISION_LABEL, Box::new(RevisionPolicy::new(value.to_string())));
            },
            other => {
                warn!("Found unknown policy '{other}' for rule '{name}'. Ignoring policy")
            }
        };
    });

    if rule.tag_policies.is_empty() && rule.repository_policies.is_empty() && rule.schedule.is_empty() {
        info!("Rule {name} doesn't contain any policies. Ignoring rule");
        None
    } else {
        Some(rule)
    }
}

/// Parse a cron schedule string
/// # Example
/// ```
/// // cron format: <sec> <min> <hour> <day of month> <month> <day of week> <year>
/// let daily_at_midnight = "0 0 * * * * *";
pub fn parse_schedule(schedule_str: &str) -> Option<String> {
    if Schedule::from_str(schedule_str).is_ok() {
        Some(schedule_str.to_string())
    } else {
        if !schedule_str.is_empty() {
            warn!("Received invalid schedule '{schedule_str}'");
        }
        None
    }
}

#[cfg(test)]
mod test {
    use chrono::Duration;
    use crate::policies::age_max::AGE_MAX_LABEL;
    use crate::policies::age_min::AGE_MIN_LABEL;
    use crate::policies::image_pattern::IMAGE_PATTERN_LABEL;
    use crate::policies::revision::REVISION_LABEL;
    use crate::policies::tag_pattern::TAG_PATTERN_LABEL;
    use crate::rule::{parse_rule, parse_schedule};
    use crate::test::{get_repositories, get_tags, get_tags_by_name};

    fn get_labels<'a>(raw: Vec<(&'a str, &'a str)>) -> Vec<(String, &'a str)> {
        let mut labels: Vec<(String, &'a str)> = Vec::new();
        raw.iter()
            .map(|(key, value)| (key.to_string(), value))
            .for_each(|(key, value)| labels.push((key, *value)));

        labels
    }

    #[test]
    fn test_invalid_schedule_1() {
        let schedule_str = "* * * *";
        assert_eq!(parse_schedule(schedule_str), None)
    }

    #[test]
    fn test_invalid_schedule_2() {
        let schedule_str = "asdf";
        assert_eq!(parse_schedule(schedule_str), None)
    }

    #[test]
    fn test_valid_schedule() {
        let schedule_str = "* * * * * *";
        assert_eq!(parse_schedule(schedule_str), Some(String::from(schedule_str)))
    }

    #[test]
    fn test_rule_without_labels() {
        assert!(parse_rule(String::from("test-rule"), vec![]).is_none())
    }

    #[test]
    fn test_rule_with_only_schedule() {
        let labels = get_labels(vec![
            ("schedule", "* * * * 5 *")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_some());
        let parsed = rule.unwrap();
        assert_eq!(parsed.name, String::from("test-rule"));
        assert_eq!(parsed.schedule, String::from("* * * * 5 *"));
        assert_eq!(parsed.tag_policies.len(), 0);
        assert_eq!(parsed.repository_policies.len(), 0);
    }

    #[test]
    fn test_easy_rule() {
        let labels = get_labels(vec![
            ("age.max", "10s"),
            ("age.min", "20m"),
            ("schedule", "* * * * 5 *")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_some());
        let parsed = rule.unwrap();
        assert_eq!(parsed.name, String::from("test-rule"));
        assert_eq!(parsed.schedule, String::from("* * * * 5 *"));
        assert_eq!(parsed.tag_policies.len(), 2);
        assert_eq!(parsed.repository_policies.len(), 0);
        assert!(parsed.tag_policies.get(AGE_MAX_LABEL).is_some());
        assert!(parsed.tag_policies.get(AGE_MIN_LABEL).is_some());
    }

    #[test]
    fn test_with_unknown_policies() {
        let labels = get_labels(vec![
            ("age.max", "10s"),
            ("age.min", "20m"),
            ("schedule", "* * * * 5 *"),
            ("test", "10s")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_some());
        let parsed = rule.unwrap();
        assert_eq!(parsed.name, String::from("test-rule"));
        assert_eq!(parsed.schedule, String::from("* * * * 5 *"));
        assert_eq!(parsed.tag_policies.len(), 2);
        assert_eq!(parsed.repository_policies.len(), 0);
        assert!(parsed.tag_policies.get(AGE_MAX_LABEL).is_some());
        assert!(parsed.tag_policies.get(AGE_MIN_LABEL).is_some());
    }

    #[test]
    fn test_with_all_policies() {
        let labels = get_labels(vec![
            ("age.max", "10s"),
            ("age.min", "20m"),
            ("schedule", "* * * * 5 *"),
            ("image.pattern", "test-.+"),
            ("tag.pattern", "test-.+"),
            ("test", "10s"),
            ("revisions", "10")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_some());
        let parsed = rule.unwrap();
        assert_eq!(parsed.name, String::from("test-rule"));
        assert_eq!(parsed.schedule, String::from("* * * * 5 *"));
        assert_eq!(parsed.tag_policies.len(), 4);
        assert_eq!(parsed.repository_policies.len(), 1);
        assert!(parsed.tag_policies.get(AGE_MAX_LABEL).is_some());
        assert!(parsed.tag_policies.get(AGE_MIN_LABEL).is_some());
        assert!(parsed.tag_policies.get(REVISION_LABEL).is_some());
        assert!(parsed.tag_policies.get(TAG_PATTERN_LABEL).is_some());
        assert!(parsed.repository_policies.get(IMAGE_PATTERN_LABEL).is_some())
    }

    #[test]
    fn test_with_only_unknown_policies() {
        let labels = get_labels(vec![
            ("policy", "* * * * 5 *"),
            ("asdf", "test-.+"),
            ("test", "10s")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_none());
    }

    #[test]
    fn test_without_tag_policies() {
        let labels = get_labels(vec![
            ("image.pattern", "test-.+"),
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_some());
        let parsed = rule.unwrap();

        let tags = get_tags_by_name(vec!["test", "other", "amogus"], Duration::seconds(1), 1);
        assert_eq!(parsed.affected_tags(tags), vec![])
    }

    #[test]
    fn test_only_target_tag_policies() {
        todo!()
    }

    #[test]
    fn test_only_requirement_tag_policies() {
        todo!()
    }

    #[test]
    fn test_without_repository_policies() {
        let labels = get_labels(vec![
            ("age.max", "10s"),
            ("age.min", "20m"),
            ("revisions", "10")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels);
        assert!(rule.is_some());
        let parsed = rule.unwrap();

        let repositories = get_repositories(vec!["red", "is", "sus"]);
        assert_eq!(parsed.affected_repositories(repositories), vec![]);
    }

    #[test]
    fn test_only_target_repository_policies() {
        todo!()
    }

    #[test]
    fn test_only_requirement_repository_policies() {
        todo!()
    }

    #[test]
    fn test_rule_affections_1() {
        let labels = get_labels(vec![
            ("age.min", "5m"),
            ("age.max", "30m"),
            ("schedule", "* * * * 5 *"),
            ("image.pattern", "test-.+"),
            ("test", "10s")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels).unwrap();

        let tags = get_tags(vec![
            ("first", Duration::hours(-5), 1_000_000),
            ("second", Duration::minutes(-5), 1_000_000),
            ("third", Duration::minutes(-30), 1_000_000),
            ("fourth", Duration::minutes(-10), 1_000_000),
            ("fifth", Duration::seconds(-15), 1_000_000),
            ("sixth", Duration::minutes(-50), 1_000_000)
        ]);

        let repositories = get_repositories(vec!["test-asdf", "test-", "test-test"]);
        let mut affected = rule.affected_repositories(repositories.clone());
        affected.sort_by(|r1, r2| r1.name.cmp(&r2.name));
        assert_eq!(affected, vec![repositories[0].clone(), repositories[2].clone()]);

        let mut affected = rule.affected_tags(tags.clone());
        affected.sort_by(|t1, t2| t1.created.cmp(&t2.created).reverse());
        assert_eq!(affected, vec![tags[2].clone(), tags[5].clone(), tags[0].clone()]);
    }

    #[test]
    fn test_rule_affections_2() {
        let labels = get_labels(vec![
            ("age.min", "5m"),
            ("age.max", "50m"),
            ("schedule", "* * * * 5 *"),
            ("revisions", "3"),
            ("test", "10s")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels).unwrap();

        let tags = get_tags(vec![
            ("first", Duration::hours(-5), 1_000_000),
            ("second", Duration::minutes(-5), 1_000_000),
            ("third", Duration::minutes(-30), 1_000_000),
            ("fourth", Duration::minutes(-10), 1_000_000),
            ("fifth", Duration::seconds(-15), 1_000_000),
            ("sixth", Duration::minutes(-50), 1_000_000)
        ]);

        let repositories = get_repositories(vec!["test-asdf", "test-", "test-test"]);
        assert_eq!(rule.affected_repositories(repositories), vec![]);

        let mut affected = rule.affected_tags(tags.clone());
        affected.sort_by(|t1, t2| t1.created.cmp(&t2.created).reverse());
        assert_eq!(affected, vec![tags[2].clone(), tags[5].clone(), tags[0].clone()]);
    }

    #[test]
    fn test_rule_affections_3() {
        let labels = get_labels(vec![
            ("age.min", "5m"),
            ("age.max", "30m"),
            ("schedule", "* * * * 5 *"),
            ("image.pattern", "test-.+"),
            ("tag.pattern", ".*th"),
            ("test", "10s")
        ]);
        let rule = parse_rule(String::from("test-rule"), labels).unwrap();

        let tags = get_tags(vec![
            ("first", Duration::hours(-5), 1_000_000),
            ("second", Duration::minutes(-5), 1_000_000),
            ("third", Duration::minutes(-30), 1_000_000),
            ("fourth", Duration::minutes(-10), 1_000_000),
            ("fifth", Duration::seconds(-15), 1_000_000),
            ("sixth", Duration::minutes(-50), 1_000_000)
        ]);

        let repositories = get_repositories(vec!["test-asdf", "test-", "test-test"]);
        let mut affected = rule.affected_repositories(repositories.clone());
        affected.sort_by(|r1, r2| r1.name.cmp(&r2.name));
        assert_eq!(affected, vec![repositories[0].clone(), repositories[2].clone()]);

        let mut affected = rule.affected_tags(tags.clone());
        affected.sort_by(|t1, t2| t1.created.cmp(&t2.created).reverse());
        assert_eq!(affected, vec![tags[3].clone(), tags[2].clone(), tags[5].clone(), tags[0].clone()]);
    }
}
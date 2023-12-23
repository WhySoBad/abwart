use std::collections::{HashMap, HashSet};
use log::{info, warn};
use regex::Regex;
use crate::api::repository::Repository;
use crate::api::tag::Tag;
use crate::NAME;
use crate::policies::{AffectionType, DEFAULT_SCHEDULE, parse_schedule, Policy};
use crate::policies::age_min::{AGE_MIN_LABEL, AgeMinPolicy};
use crate::policies::age_max::{AGE_MAX_LABEL, AgeMaxPolicy};
use crate::policies::pattern::{PATTERN_LABEL, PatternPolicy};
use crate::policies::revision::{REVISION_LABEL, RevisionPolicy};

const RULE_REGEX: &str = "rule.(?<name>[a-z]+).(?<policy>[a-z\\.]+)";

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    pub repository_policies: Vec<Box<dyn Policy<Repository>>>,
    pub tag_policies: Vec<Box<dyn Policy<Tag>>>,
    pub schedule: String,
}

impl Rule{
    pub fn new(name: String) -> Self {
        Self { name, repository_policies: vec![], tag_policies: vec![], schedule: DEFAULT_SCHEDULE.to_string() }
    }

    /// Get all repositories which are affected by the current rule
    pub fn affected_repositories(&self, repositories: Vec<Repository>) -> Vec<Repository> {
        let mut requirements = Vec::new();
        let mut affected = HashSet::new();
        for repository_policy in &self.repository_policies {
            if repository_policy.affection_type() == AffectionType::Requirement {
                requirements.push(repository_policy);
                continue
            }
            affected.extend(repository_policy.affects(repositories.clone()))
        }

        let mut affected = affected.into_iter().collect::<Vec<_>>();

        for requirement in requirements {
            let not_matching = requirement.affects(affected.clone());
            affected.retain(|repo| !not_matching.contains(repo))
        }

        affected
    }

    /// Get all tags which are affected by the current rule
    pub fn affected_tags(&self, mut tags: Vec<Tag>) -> Vec<Tag> {
        let mut requirements = Vec::new();
        let mut affected = HashSet::new();
        tags.sort_by(|t1, t2| t1.created.cmp(&t2.created).reverse());
        for tag_policy in &self.tag_policies {
            if tag_policy.affection_type() == AffectionType::Requirement {
                requirements.push(tag_policy);
                continue
            }
            affected.extend(tag_policy.affects(tags.clone()))
        }

        let mut affected = affected.into_iter().collect::<Vec<_>>();

        for requirement in requirements {
            let not_matching = requirement.affects(affected.clone());
            affected.retain(|tag| !not_matching.contains(tag))
        }

        affected
    }
}

/// Parse a list of LABEL key-value-paris into rules. All rule labels have to match `{NAME}.{RULE_REGEX}`. <br>
/// The method accepts some defaults for when being called within an `Instance` constructor.
pub fn parse_rules(labels: HashMap<String, String>, default_schedule: String) -> HashMap<String, Rule> {
    let target_pattern = Regex::new(format!("{NAME}.{RULE_REGEX}").as_str()).expect("Rule pattern should be valid");
    let mut rules = HashMap::new();

    for (key, value) in labels {
        let Some(captures) = target_pattern.captures(key.as_str()) else {
            continue
        };
        let name = captures["name"].to_string();
        let entry = rules.entry(name.clone()).or_insert(Rule::new(name.clone()));
        let policy = &captures["policy"];
        if policy == "schedule" {
            entry.schedule = parse_schedule(value.as_str()).unwrap_or_else(|| {
                println!("Using default schedule '{default_schedule}'");
                default_schedule.clone()
            });
            continue
        }
        match policy {
            AGE_MAX_LABEL => {
                entry.tag_policies.push(Box::new(AgeMaxPolicy::new(value)))
            },
            AGE_MIN_LABEL => {
                entry.tag_policies.push(Box::new(AgeMinPolicy::new(value)))
            },
            PATTERN_LABEL => {
                entry.repository_policies.push(Box::new(PatternPolicy::new(value.as_str())))
            },
            REVISION_LABEL => {
                entry.tag_policies.push(Box::new(RevisionPolicy::new(value)))
            },
            other => {
                warn!("Found unknown policy '{other}' for rule '{name}'. Ignoring policy")
            }
        }
    }

    rules.retain(|name, value| {
       if value.tag_policies.is_empty() && value.repository_policies.is_empty() {
           info!("Rule {name} doesn't contain any rules. Ignoring rule");
           false
       } else {
           true
       }
    });

    rules
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    fn get_labels() -> HashMap<String, String> {
        let labels = HashMap::new();
        labels
    }
}
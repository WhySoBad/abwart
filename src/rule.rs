use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use cron::Schedule;
use log::{debug, info, warn};
use crate::api::repository::Repository;
use crate::api::tag::Tag;
use crate::policies::{AffectionType, PolicyMap};
use crate::policies::age_min::{AGE_MIN_LABEL, AgeMinPolicy};
use crate::policies::age_max::{AGE_MAX_LABEL, AgeMaxPolicy};
use crate::policies::pattern::{PATTERN_LABEL, PatternPolicy};
use crate::policies::revision::{REVISION_LABEL, RevisionPolicy};

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
        for policy in self.tag_policies.values() {
            if policy.affection_type() == AffectionType::Requirement {
                requirements.push(policy);
                continue
            }
            let affects = policy.affects(tags.clone());
            debug!("Policy '{}' affected {} tags", policy.id(), affects.len());
            affected.extend(affects)
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
pub fn parse_rule(name: String, policies: Vec<(String, &String)>) -> Option<Rule> {
    let mut rule = Rule::new(name.clone());
    policies.into_iter().for_each(|(policy_name, value)| {
        match policy_name.as_str() {
            "schedule" => {
                rule.schedule = parse_schedule(value.as_str()).unwrap_or_default()
            },
            AGE_MAX_LABEL => {
                rule.tag_policies.insert(AGE_MAX_LABEL, Box::new(AgeMaxPolicy::new(value.clone())));
            },
            AGE_MIN_LABEL => {
                rule.tag_policies.insert(AGE_MIN_LABEL, Box::new(AgeMinPolicy::new(value.clone())));
            },
            PATTERN_LABEL => {
                rule.repository_policies.insert(PATTERN_LABEL, Box::new(PatternPolicy::new(value)));
            },
            REVISION_LABEL => {
                rule.tag_policies.insert(REVISION_LABEL, Box::new(RevisionPolicy::new(value.clone())));
            },
            other => {
                warn!("Found unknown policy '{other}' for rule '{name}'. Ignoring policy")
            }
        };
    });

    if rule.tag_policies.is_empty() && rule.repository_policies.is_empty() {
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
        warn!("Received invalid schedule '{schedule_str}'");
        None
    }
}

#[cfg(test)]
mod test {
}
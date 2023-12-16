use std::collections::{HashMap, HashSet};
use chrono::{Duration, Utc};
use log::warn;
use regex::Regex;
use crate::api::repository::Repository;
use crate::api::tag::Tag;
use crate::NAME;
use crate::policy::{DEFAULT_REVISIONS, DEFAULT_SCHEDULE, parse_duration, parse_revisions, parse_pattern, parse_schedule};

const RULE_REGEX: &str = "rule.(?<name>[a-z]+).(?<policy>[a-z\\.]+)";

#[derive(Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub age_max: Option<Duration>,
    pub age_min: Option<Duration>,
    pub pattern: Regex,
    pub revisions: usize,
    pub schedule: String,
}

impl Rule {
    pub fn new(name: String) -> Self {
        Self { name, age_max: None, age_min: None, revisions: DEFAULT_REVISIONS, pattern: Regex::new("").expect("Empty regex should be valid"), schedule: DEFAULT_SCHEDULE.to_string() }
    }

    /// Get all repositories which are affected by the current rule
    pub fn affected_repositories<'a>(&'a self, repositories: &'a [Repository]) -> Vec<&Repository> {
        repositories.iter().filter(|repo| self.pattern.is_match(&repo.name)).collect()
    }

    /// Get all tags which are affected by the current rule <br>
    /// **Important**: The vector which is given as parameter will get sorted by the creation
    /// date of the tags in descending order
    pub fn affected_tags<'a>(&'a self, tags: &'a mut Vec<Tag>) -> Vec<&Tag> {
        let mut affected = HashSet::new();
        tags.sort_by(|t1, t2| t1.created.cmp(&t2.created).reverse());

        // add all tags which are over the maximum allowed revision count to the affected vector
        if tags.len() > self.revisions {
            affected.extend(tags[self.revisions..].iter());
        }
        // add all tags which are older than `age_max` to the affected vector
        if let Some(age_max) = self.age_max {
            let now = Utc::now();
            affected.extend(tags.iter().filter(|t| (t.created + age_max) < now));
        }
        // check for all affected tags whether the min age policy is fulfilled
        // otherwise un-affect them
        if let Some(age_min) = self.age_min {
            let now = Utc::now();
            return affected.into_iter().filter(|t| t.created + age_min < now).collect::<Vec<&Tag>>();
        }

        affected.into_iter().collect()
    }
}

/// Parse a list of label key-value-paris into rules. All rule labels have to match `{NAME}.{RULE_REGEX}`. <br>
/// The method accepts some defaults for when being called within an `Instance` constructor.
pub fn parse_rules(labels: HashMap<String, String>, default_age_max: Option<Duration>, default_age_min: Option<Duration>, default_revisions: usize, default_schedule: String) -> HashMap<String, Rule> {
    let target_pattern = Regex::new(format!("{NAME}.{RULE_REGEX}").as_str()).expect("Rule pattern should be valid");
    let mut rules = HashMap::new();

    for (key, value) in labels {
        let Some(captures) = target_pattern.captures(key.as_str()) else {
            continue
        };
        let name = captures["name"].to_string();
        let entry = rules.entry(name.clone()).or_insert(Rule::new(name.clone()));
        let policy = &captures["policy"];
        match policy {
            "revisions" => {
                entry.revisions = parse_revisions(value, Some(default_revisions));
            },
            "pattern" => {
                entry.pattern = parse_pattern(value.as_str());
            },
            "age.max" => {
                entry.age_max = parse_duration(value, default_age_max);
            },
            "age.min" => {
                entry.age_min = parse_duration(value, default_age_min);
            },
            "schedule" => {
                entry.schedule = parse_schedule(value.as_str(), Some(default_schedule.clone()))
            }
            other => {
                warn!("Found unknown policy '{other}' for rule '{name}'. Ignoring policy")
            }
        }
    }

    rules
}

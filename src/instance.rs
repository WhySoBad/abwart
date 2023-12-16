use std::collections::HashMap;
use std::sync::Arc;
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::models::{ContainerSummary, EventActor};
use bollard::secret::EndpointSettings;
use chrono::Duration;
use log::{error, info, warn};
use regex::Regex;
use crate::api::distribution::Distribution;
use crate::api::DistributionConfig;
use crate::error::Error;
use crate::label;
use crate::policy::{DEFAULT_REVISIONS, DEFAULT_SCHEDULE, parse_duration, parse_revisions, parse_schedule};
use crate::rule::{parse_rules, Rule};

#[derive(Debug, Clone)]
pub struct Instance {
    pub name: String,
    pub id: String,
    pub distribution: DistributionConfig,
    pub default_revisions: usize,
    pub default_age_max: Option<Duration>,
    pub default_age_min: Option<Duration>,
    pub default_schedule: String,
    pub rules: HashMap<String, Rule>,
    client: Arc<Docker>
}

impl Instance {
    pub fn new(id: String, mut name: String, labels: HashMap<String, String>, networks: HashMap<String, EndpointSettings>, client: Arc<Docker>) -> Result<Self, Error> {
        // TODO: Check whether for actors outside scope "LOCAL" secure would make sense
        let mut default_age_max = None;
        let mut default_age_min = None;
        let mut default_revisions = DEFAULT_REVISIONS;
        let mut default_schedule = DEFAULT_SCHEDULE.to_string();
        let mut network = None;
        let mut rules = HashMap::new();

        if networks.is_empty() {
            return Err(Error::NoNetwork(name))
        }

        if !labels.is_empty() {
            if let Some(interval) = labels.get(&label("default.age.max")) {
                default_age_max = parse_duration(interval.clone(), None)
            }
            if let Some(interval) = labels.get(&label("default.age.min")) {
                default_age_min = parse_duration(interval.clone(), None)
            }
            if let Some(revisions) = labels.get(&label("default.revisions")) {
                default_revisions = parse_revisions(revisions.clone(), None)
            }
            if let Some(schedule) = labels.get(&label("default.schedule")) {
                default_schedule = parse_schedule(schedule.as_str(), None);
            }
            if let Some(nw) = labels.get(&label("network")) {
                if networks.contains_key(nw) {
                    network = Some(nw.clone())
                } else {
                    warn!("Received network '{nw}' which doesn't exist on container. Using default instead")
                }
            }
            rules = parse_rules(labels, default_age_max, default_age_min, default_revisions, default_schedule.clone());
        } else {
            info!("Using default instance attributes");
        }

        if rules.is_empty() {
            let mut default_rule = Rule::new(format!("{}-{}-default", name, &id[0..8]));
            default_rule.revisions = default_revisions;
            default_rule.age_min = default_age_min;
            default_rule.age_max = default_age_max;
            default_rule.schedule = default_schedule.clone();
            default_rule.pattern = Regex::new("\\w+").expect("All word regex should exist");
            rules.insert(default_rule.name.clone(), default_rule);
        }

        let ip;
        if let Some(network) = network {
            ip = networks.get(network.as_str()).expect("Network should exist").ip_address.clone();
        } else {
            ip = networks.values().next().expect("There should be at least one network").ip_address.clone()
        }
        let mut address = ip.unwrap_or(String::from("127.0.0.1:5000"));
        if address.is_empty() {
            address = String::from("127.0.0.1:5000")
        }

        let distribution = DistributionConfig::new(address, None, None, true);

        if name.starts_with('/') {
            name = name[1..name.len()].to_string()
        }

        Ok(Self{ id, name, rules, default_revisions, default_age_min, default_age_max, distribution, default_schedule, client })
    }

    pub async fn from_actor(actor: EventActor, client: Arc<Docker>) -> Result<Instance, Error> {
        let id = actor.id.ok_or(Error::MissingId)?;
        let container = client.inspect_container(id.as_str(), None).await.map_err(|_| Error::InexistentContainer(id.clone()))?;
        let name = container.name.unwrap_or(id.clone());
        Self::new(id, name, actor.attributes.unwrap_or_default(), container.network_settings.ok_or(Error::MissingNetworks)?.networks.unwrap_or_default(), client)
    }

    pub fn from_container(container: ContainerSummary, client: Arc<Docker>) -> Result<Instance, Error> {
        let id = container.id.ok_or(Error::MissingId)?;
        let name = container.names.unwrap_or(Vec::new()).get(0).unwrap_or(&id).clone();
        Self::new(id, name, container.labels.unwrap_or_default(), container.network_settings.ok_or(Error::MissingNetworks)?.networks.unwrap_or_default(), client)
    }

    /// Get all rules of the instance in a bundled format where the keys are the cron schedules and the values
    /// are the associated rules which should run in the given schedule
    pub fn get_bundled_rules(&self) -> HashMap<String, Vec<String>> {
        let mut bundles = HashMap::<String, Vec<String>>::new();
        self.rules.iter().for_each(|(_, rule)| {
            if let Some(rules) = bundles.get_mut(&rule.schedule) {
                rules.push(rule.name.clone())
            } else {
                bundles.insert(rule.schedule.clone(), vec![rule.name.clone()]);
            }
        });

        bundles
    }

    /// Apply a given set of rules defined on the instance onto the associated registry. The
    /// rules are referenced by their name <br>
    /// All tags (on repositories) which match at least one of the rules will be deleted and
    /// additionally the garbage collector inside the registry will be run automatically
    pub async fn apply_rules(&self, rules: Vec<String>) -> Result<(), Error> {
        let distribution = Distribution::new(&self.distribution);
        let repositories = distribution.get_repositories().await?;

        if repositories.is_empty() {
            info!("The registry '{}' doesn't contain any repositories. Skipping it", self.name);
            return Ok(())
        }

        let rules = self.rules.iter()
            .filter(|(name, _)| rules.contains(name))
            .map(|(_, rule)| rule)
            .collect::<Vec<&Rule>>();

        for rule in rules {
            let repositories = rule.affected_repositories(&repositories);
            for repository in repositories {
                let mut tags = repository.get_tags_with_data().await?;
                let affected_tags = rule.affected_tags(&mut tags);
                for tag in affected_tags {
                    info!("Deleting tag '{}' from repository '{}' in registry '{}'", tag.name, repository.name, self.name);
                    repository.delete_manifest(&tag.digest).await?;
                }
            }
        }

        let exec = self.client.create_exec(self.id.as_str(), CreateExecOptions::<&str>{
            cmd: Some(vec!["/bin/registry", "garbage-collect", "--delete-untagged", "/etc/docker/registry/config.yml"]),
            user: Some("root"),
            ..CreateExecOptions::default()
        }).await;

        match exec {
            Ok(exec) => {
                match self.client.start_exec(exec.id.as_str(), None::<StartExecOptions>).await {
                    Ok(_) => info!("Successfully ran garbage collector in registry '{}'", self.name),
                    Err(err) => error!("Unable to run garbage collector in registry '{}'. Reason: {err}", self.name)
                }
            },
            Err(err) => error!("Unable to create new exec in registry '{}'. Reason: {err}", self.name)
        }

        Ok(())
    }
}


use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::models::{ContainerSummary, EventActor};
use bollard::secret::EndpointSettings;
use log::{debug, error, info, warn};
use regex::Regex;
use crate::api::distribution::Distribution;
use crate::api::DistributionConfig;
use crate::error::Error;
use crate::{label, NAME};
use crate::config::Config;
use crate::policies::age_max::{AGE_MAX_LABEL, AgeMaxPolicy};
use crate::policies::age_min::{AGE_MIN_LABEL, AgeMinPolicy};
use crate::policies::pattern::{PATTERN_LABEL, PatternPolicy};
use crate::policies::revision::{REVISION_LABEL, RevisionPolicy};
use crate::rule::{parse_rule, parse_schedule, Rule};

#[derive(Debug)]
pub struct Instance {
    pub name: String,
    pub id: String,
    pub distribution: DistributionConfig,
    pub default_rule: Rule,
    pub rules: HashMap<String, Rule>,
    pub port: u16,
    client: Arc<Docker>
}

const RULE_REGEX: &str = "rule\\.(?<name>[a-z]+)";
const DEFAULT_RULE_REGEX: &str = "default";
const POLICY_NAME_REGEX: &str = "(?<policy>[a-z\\.]+)";
/// Per default the schedule is set to daily at midnight
const DEFAULT_SCHEDULE: &str = "0 0 0 * * * *";

impl Instance {
    pub fn new(id: String, mut name: String, labels: HashMap<String, String>, networks: HashMap<String, EndpointSettings>, client: Arc<Docker>) -> Result<Self, Error> {
        let mut network = None;
        let mut port = 5000u16;
        // TODO: Check whether for actors outside scope "LOCAL" secure would make sense
        let mut distribution = DistributionConfig::new(String::new(), None, None, true);

        if networks.is_empty() {
            return Err(Error::NoNetwork(name))
        }

        let (default_rule, rules) = Instance::parse_rules(&id, &labels);

        if !labels.is_empty() {
            if let Some(custom_network) = labels.get(&label("network")) {
                if networks.contains_key(custom_network) {
                    network = Some(custom_network.clone())
                } else {
                    warn!("Received network '{custom_network}' which doesn't exist on container. Using default instead")
                }
            }
            if let Some(custom_port) = labels.get(&label("port")) {
                if let Ok(custom_port) = custom_port.parse::<u16>(){
                    port = custom_port
                } else {
                    warn!("Received invalid custom port value '{custom_port}'. Expected positive 16-bit integer. Using default ({port}) instead")
                }
            }
            distribution.username = labels.get(&label("username")).cloned();
            distribution.password = labels.get(&label("password")).cloned();
        } else {
            info!("Using default instance attributes");
        }

        let mut address = match &network {
            Some(network) => networks.get(network.as_str()).expect("Network should exist").ip_address.clone(),
            None => networks.values().next().expect("There should be at least one network").ip_address.clone()
        }.unwrap_or(String::from("127.0.0.1"));
        if address.is_empty() {
            address = String::from("127.0.0.1")
        }

        distribution.host = format!("{address}:{port}");

        if name.starts_with('/') {
            // the `/` in the container name can be removed for aesthetic reasons
            name = name[1..name.len()].to_string()
        }

        debug!("Registered new registry '{name}' with: {address}:{port} ({network:?}) {rules:?} {default_rule:?}");

        let mut instance = Self{ id, port, name, rules, default_rule, distribution, client };
        instance.apply_defaults();
        Ok(instance)
    }

    pub async fn from_actor(actor: EventActor, client: Arc<Docker>, config: Arc<Mutex<Config>>) -> Result<Instance, Error> {
        let id = actor.id.ok_or(Error::MissingId)?;
        let container = client.inspect_container(id.as_str(), None).await.map_err(|_| Error::InexistentContainer(id.clone()))?;
        let name = container.name.unwrap_or(id.clone())[1..].to_string();
        let registry_config = config.lock().map_err(|_| Error::ConfigLockError())?.get_registry(&name).unwrap_or_default();
        let mut labels = actor.attributes.unwrap_or_default();
        labels.extend(registry_config);
        Self::new(id, name, labels, container.network_settings.ok_or(Error::MissingNetworks)?.networks.unwrap_or_default(), client)
    }

    pub fn from_container(container: ContainerSummary, client: Arc<Docker>, config: Arc<Mutex<Config>>) -> Result<Instance, Error> {
        let id = container.id.ok_or(Error::MissingId)?;
        let name = container.names.unwrap_or(Vec::new()).get(0).unwrap_or(&id).clone()[1..].to_string();
        let registry_config = config.lock().map_err(|_| Error::ConfigLockError())?.get_registry(&name).unwrap_or_default();
        let mut labels = container.labels.unwrap_or_default();
        labels.extend(registry_config);
        Self::new(id, name, labels, container.network_settings.ok_or(Error::MissingNetworks)?.networks.unwrap_or_default(), client)
    }

    /// Apply the `default_tag_policies`, `default_repository_policies` and `default_schedule` to the rules in the current instance
    fn apply_defaults(&mut self) {
        self.rules.iter_mut().for_each(|(_, rule)| {
            self.default_rule.tag_policies.iter().for_each(|(name, policy)| {
                if !rule.tag_policies.contains_key(name) {
                    rule.tag_policies.insert(name, policy.clone());
                }
            });
            self.default_rule.repository_policies.iter().for_each(|(name, policy)| {
                if !rule.repository_policies.contains_key(name) {
                    rule.repository_policies.insert(name, policy.clone());
                }
            });
            if rule.schedule.is_empty() {
                rule.schedule = self.default_rule.schedule.clone()
            }
        });
    }

    fn get_default_rule_pattern() -> Regex {
        Regex::new(format!("{NAME}\\.{DEFAULT_RULE_REGEX}\\.{POLICY_NAME_REGEX}").as_str()).expect("Default rule pattern should be valid")
    }

    fn get_rule_pattern() -> Regex {
        Regex::new(format!("{NAME}\\.{RULE_REGEX}\\.{POLICY_NAME_REGEX}").as_str()).expect("Rule pattern should be valid")
    }

    /// Parse all rules including the default rule from the instance configuration
    fn parse_rules(id: &str, labels: &HashMap<String, String>) -> (Rule, HashMap<String, Rule>) {
        let mut rule_labels = HashMap::new();
        let mut rules = HashMap::new();
        let default_schedule = parse_schedule(DEFAULT_SCHEDULE).expect("Default schedule should be valid cron schedule");

        let rule_pattern = Instance::get_rule_pattern();
        let default_rule_pattern = Instance::get_default_rule_pattern();
        let default_rule_name = id.to_string();
        let mut default_rule = Rule::new(default_rule_name.clone());
        default_rule.repository_policies.insert(PATTERN_LABEL, Box::<PatternPolicy>::default());
        default_rule.tag_policies.insert(AGE_MAX_LABEL, Box::<AgeMaxPolicy>::default());
        default_rule.tag_policies.insert(AGE_MIN_LABEL, Box::<AgeMinPolicy>::default());
        default_rule.tag_policies.insert(REVISION_LABEL, Box::<RevisionPolicy>::default());

        // parse default rules
        labels.iter()
            .filter_map(|(key, value)| rule_pattern.captures(key).map(|captures| (captures["name"].to_string(), captures["policy"].to_string(), value)))
            .for_each(|(name, key, value)| {
                let entry = rule_labels.entry(name).or_insert(vec![]);
                entry.push((key, value.as_str()));
            });

        // parse default policies
        labels.iter()
            .filter_map(|(key, value)| default_rule_pattern.captures(key).map(|captures| (captures["policy"].to_string(), value)))
            .for_each(|(key, value)| {
                let entry = rule_labels.entry(default_rule_name.clone()).or_insert(vec![]);
                entry.push((key, value.as_str()))
            });

        debug!("Rule labels {rule_labels:?}");

        for (name, labels) in rule_labels {
            let is_default = default_rule_name.eq(&name);
            let rule = parse_rule(name.clone(), labels);
            debug!("Parsed rule {rule:?} default? {is_default}");
            if let Some(rule) = rule {
                if is_default {
                    default_rule.tag_policies.extend(rule.tag_policies);
                    default_rule.repository_policies.extend(rule.repository_policies);
                    if rule.schedule.is_empty() {
                        default_rule.schedule = default_schedule.clone();
                    } else {
                        default_rule.schedule = rule.schedule;
                    }
                } else {
                    rules.insert(name, rule);
                }
            }
        }

        debug!("Default rule {default_rule:?}");

        (default_rule, rules)
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
        debug!("Applying rules to registry '{}'", self.name);
        let distribution = Distribution::new(Arc::new(self.distribution.clone()));
        let repositories = distribution.get_repositories().await?;

        if repositories.is_empty() {
            info!("The registry '{}' doesn't contain any repositories. Skipping it", self.name);
            return Ok(())
        }

        let rules = self.rules.iter()
            .filter(|(name, _)| rules.contains(name))
            .map(|(_, rule)| rule)
            .collect::<Vec<&Rule>>();

        let mut tag_cache = HashMap::new();

        let mut affected_repositories = HashSet::new();
        let mut deleted_tags = 0;
        for rule in rules {
            let repositories = rule.affected_repositories(repositories.clone());
            affected_repositories.extend(repositories.iter().map(|r| r.name.clone()));
            for repository in repositories {
                let tags = tag_cache.entry(repository.name.clone()).or_insert(repository.get_tags_with_data().await?);
                let affected_tags = rule.affected_tags(tags.clone());
                for tag in &affected_tags {
                    info!("Deleting tag '{}' from repository '{}' in registry '{}'", tag.name, repository.name, self.name);
                    repository.delete_manifest(&tag.digest).await?;
                    deleted_tags += 1;
                }
                if !affected_tags.is_empty() {
                    tags.retain(|tag| !affected_tags.contains(tag))
                }
            }
        }

        if deleted_tags == 0 {
            info!("Left all repositories in registry '{}' unmodified", self.name)
        } else {
            info!("Deleted {deleted_tags} tags from {} repositories in registry '{}'", affected_repositories.len(), self.name);
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

#[cfg(test)]
mod test {
    use crate::instance::Instance;

    #[test]
    fn test_rule_pattern() {
        Instance::get_rule_pattern();
    }

    #[test]
    fn test_default_rule_pattern() {
        Instance::get_default_rule_pattern();
    }
}



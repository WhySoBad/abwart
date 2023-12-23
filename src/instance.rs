use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::models::{ContainerSummary, EventActor};
use bollard::secret::EndpointSettings;
use chrono::Duration;
use log::{debug, error, info, warn};
use crate::api::distribution::Distribution;
use crate::api::DistributionConfig;
use crate::error::Error;
use crate::label;
use crate::policies::age_max::AgeMaxPolicy;
use crate::policies::age_min::AgeMinPolicy;
use crate::policies::pattern::PatternPolicy;
use crate::policies::revision::RevisionPolicy;
use crate::rule::{parse_rules, Rule};

#[derive(Debug)]
pub struct Instance {
    pub name: String,
    pub id: String,
    pub distribution: DistributionConfig,
    pub default_revisions: usize,
    pub default_age_max: Option<Duration>,
    pub default_age_min: Option<Duration>,
    pub default_schedule: String,
    pub rules: HashMap<String, Rule>,
    pub port: u16,
    client: Arc<Docker>
}

impl Instance {
    pub fn new(id: String, mut name: String, labels: HashMap<String, String>, networks: HashMap<String, EndpointSettings>, client: Arc<Docker>) -> Result<Self, Error> {
        // TODO: Check whether for actors outside scope "LOCAL" secure would make sense
        let mut network = None;
        let mut rules = HashMap::new();
        let mut port = 5000u16;
        // FIXME: Implement the default behavior handling
        let mut default_repository_policies = HashMap::new();
        let mut default_tag_policies = HashMap::new();

        if networks.is_empty() {
            return Err(Error::NoNetwork(name))
        }

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

            // TODO: Do rule parsing in a better place
            rules = parse_rules(labels, default_schedule.clone());
        } else {
            info!("Using default instance attributes");
        }

        // FIXME: This should be possible without having the always-there rule
        if rules.is_empty() {
            let mut default_rule = Rule::new(format!("{}-{}-default", name, &id[0..8]));
            default_rule.tag_policies.push(Box::new(RevisionPolicy::from(default_revisions)));
            default_rule.tag_policies.push(Box::new(AgeMinPolicy::from(default_age_min)));
            default_rule.tag_policies.push(Box::new(AgeMaxPolicy::from(default_age_max)));
            default_rule.repository_policies.push(Box::<PatternPolicy>::default());
            default_rule.schedule = default_schedule.clone();
            rules.insert(default_rule.name.clone(), default_rule);
        }

        let ip;
        if let Some(network) = &network {
            ip = networks.get(network.as_str()).expect("Network should exist").ip_address.clone();
        } else {
            ip = networks.values().next().expect("There should be at least one network").ip_address.clone()
        }
        let mut address = ip.unwrap_or(String::from("127.0.0.1"));
        if address.is_empty() {
            address = String::from("127.0.0.1")
        }

        let distribution = DistributionConfig::new(format!("{address}:{port}"), None, None, true);

        // containers started in a docker compose deployment start with a `/` which can be removed for aesthetic reasons
        if name.starts_with('/') {
            name = name[1..name.len()].to_string()
        }

        debug!("Registered new registry '{name}' with: {address}:{port} ({network:?}) {rules:?} {default_revisions} {default_age_min:?} {default_age_max:?} {default_schedule}");

        Ok(Self{ id, port, name, rules, default_revisions, default_age_min, default_age_max, distribution, default_schedule, client })
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
                println!("{tags:?}, {}", repository.name);
                println!("Affected: {affected_tags:?}");
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

        info!("Deleted {deleted_tags} tags from {} repositories in registry '{}'", affected_repositories.len(), self.name);

        // TODO: Add some kind of config option whether the garbage collector should run when no tags were deleted
        // TODO: for some scenarios it would still be beneficial e.g. when one always only updates the :latest tag which produces
        // TODO: untagged blobs which only would get cleaned up when this config is set to true
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


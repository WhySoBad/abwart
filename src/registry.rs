use std::{collections::HashMap, sync::Arc};
use std::collections::{HashSet, LinkedList};
use crate::{error::Error, label, rule::{parse_schedule, Rule}, NAME};
use bollard::{secret::{ContainerSummary, EndpointSettings, EventActor}, Docker};
use chrono::{DateTime, Utc};
use dkregistry::v2::Client;
use dkregistry::v2::manifest::Manifest;
use futures::{StreamExt, TryStreamExt};
use lazy_static::lazy_static;
use oci_registry_client::DockerRegistryClientV2;
use regex::Regex;
use serde::Deserialize;
use crate::error::ErrorContext;
use crate::policies::{Repository, Tag};

lazy_static! {
    /// Regex matching policies of any rule
    static ref RULE_REGEX: Regex = Regex::new("{NAME}\\.rule\\.(?<name>[a-z]+)\\.(?<policy>[a-z\\.]+)").expect("should be valid regex");
    /// Regex matching policies of the default rule
    static ref DEFAULT_RULE_REGEX: Regex = Regex::new("{NAME}\\.default\\.(?<policy>[a-z\\.]+)").expect("should be valid regex");
    static ref DEFAULT_SCHEDULE: String = String::from("0 0 0 * * * *");
}

#[derive(Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(default)]
pub struct RegistryConfig {
    /// Default policies of the registry
    pub default: HashMap<String, String>,
    #[serde(rename = "rule")]
    /// Rules configured on the registry
    pub rules: HashMap<String, HashMap<String, String>>,
    /// Docker network under which the registry is accessible
    pub network: String,
    /// Port on which the registry is running in the container
    pub port: u16,
    /// Cleanup cron schedule
    pub cleanup: Option<String>,
    #[serde(flatten)]
    /// Credentials used for basic auth authentication to the registry
    pub credentials: Option<RegistryCredentials>,
    /// Boolean whether the registry is secured over tls
    pub secure: Option<bool>,
}

#[derive(Deserialize, Clone, Debug, Eq, PartialEq)]
struct RegistryCredentials {
    pub username: String,
    pub password: String
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            default: HashMap::new(),
            rules: HashMap::new(),
            network: NAME.to_string(),
            port: 5000u16,
            cleanup: None,
            credentials: None,
            secure: None
        }
    }
}

impl RegistryConfig {
    /// Extend the registry config with another registry config
    ///
    /// This is useful when we have a registry configured with the configuration file
    /// and the container has some labels which are relevant as well
    pub fn extend(&mut self, other: RegistryConfig) {
        self.default.extend(other.default);
        other.rules.into_iter().for_each(|(name, other_rule)| {
            if let Some(rule) = self.rules.get_mut(&name) {
                rule.extend(other_rule);
            } else {
                self.rules.insert(name.clone(), other_rule);
            }
        });
        // TODO: Extend the remaining fields
    }

    fn from_container_data(labels: HashMap<String, String>, networks: HashMap<String, EndpointSettings>) -> Result<Self, Error> {
        let mut default = HashMap::new();
        let mut rules = HashMap::new();

        labels.iter()
            .filter_map(|(key, value)| {
                RULE_REGEX.captures(key)
                    .map(|captures| (captures["name"].to_string(), captures["policy"].to_string(), value))
            })
            .for_each(|(name, key, value)| {
                let entry = rules.entry(name).or_insert(HashMap::new());
                entry.insert(key, value.clone());
            });

        labels.iter()
            .filter_map(|(key, value)| {
                DEFAULT_RULE_REGEX.captures(key)
                    .map(|captures| (captures["policy"].to_string(), value))
            })
            .for_each(|(key, value)| {
                default.insert(key, value.clone());
            });

        let default_config = RegistryConfig::default();

        let network = labels.get(&label("network")).and_then(|network| {
            networks.get(network).map(|_| network.clone())
        }).unwrap_or(default_config.network.clone());

        let port = labels.get(&label("port")).and_then(|port| {
            port.parse::<u16>().ok()
        }).unwrap_or(default_config.port);

        let cleanup = labels.get(&label("cleanup")).and_then(|schedule| {
            parse_schedule(schedule)
        });

        let credentials = labels.get(&label("username")).zip(labels.get(&label("password")))
            .map(|(usr, pwd)| RegistryCredentials { username: usr.clone(), password: pwd.clone() });

        let secure = labels.get(&label("secure")).and_then(|secure| {
            Some(secure.parse::<bool>().ok())
        }).unwrap_or(default_config.secure);

        Ok(Self { default, rules, network, port, cleanup, credentials, secure })
    }

    /// Create a new registry config from a running container
    pub fn from_container(container: &ContainerSummary) -> Result<Self, Error> {
        let labels = container.labels.clone().unwrap_or_default();
        let networks = container.network_settings.clone().ok_or(Error::MissingNetworks)?.networks.unwrap_or_default();
        RegistryConfig::from_container_data(labels, networks)
    }

    /// Create a new registry config from an event actor which should be a container
    pub async fn from_actor(actor: EventActor, client: Arc<Docker>) -> Result<Self, Error> {
        let id = actor.id.ok_or(Error::MissingId)?;
        let container = client.inspect_container(id.as_str(), None).await
            .map_err(|_| Error::InexistentContainer(id.clone()))?;
        let labels = container.config.unwrap_or_default().labels.unwrap_or_default();
        let networks = container.network_settings.ok_or(Error::MissingNetworks)?.networks.unwrap_or_default();
        RegistryConfig::from_container_data(labels, networks)
    }
}

pub struct Registry {
    /// Name of the registry
    pub name: String,
    /// Id of the registry container
    pub container_id: String,
    /// Rules which shall be applied to the registry
    pub rules: HashMap<String, Rule>,
    /// Optional cron schedule when the automatic cleanup of the registry should happen
    pub cleanup: Option<String>,
    /// Api client to communicate with the registry
    client: Client,
}

impl Registry {
    pub async fn from_container(container: ContainerSummary, config: RegistryConfig) -> Result<Self, Error> {
        let id = container.id.ok_or(Error::MissingId)?;
        let name = container.names.unwrap_or_default().first().ok_or(Error::MissingNames)?;
        Registry::from_container_data(id, name.clone(), config, container.network_settings.unwrap_or_default().networks.unwrap_or_default()).await
    }

    pub async fn from_actor(actor: EventActor, config: RegistryConfig, client: Arc<Docker>) -> Result<Self, Error> {
        let id = actor.id.ok_or(Error::MissingId)?;
        let container = client.inspect_container(id.as_str(), None)
            .await.map_err(|_| Error::InexistentContainer(id.clone()))?;
        let name = container.name.ok_or(Error::MissingNames)?;
        Registry::from_container_data(id, name, config, container.network_settings.unwrap_or_default().networks.unwrap_or_default()).await
    }

    async fn from_container_data(container_id: String, name: String, config: RegistryConfig, networks: HashMap<String, EndpointSettings>) -> Result<Self, Error> {
        let name = if name.starts_with('/') {
            name[1..name.len()].to_string()
        } else {
            name
        };

        let address = networks
            .get(&config.network.clone())
            .ok_or(Error::NoNetwork(name.clone(), config.network.clone()))?
            .clone()
            .ip_address
            .ok_or(Error::NoAddress(name.clone(), config.network.clone()))?;

        let registry_address = format!("{address}:{}", config.port);

        let client = Client::configure()
            .insecure_registry(!config.secure)
            .registry(&registry_address)
            .username(config.credentials.clone().map(|cred| cred.username.clone()))
            .password(config.credentials.clone().map(|cred| cred.password.clone()))
            .build()?;

        let client = client.ensure_v2_registry().await?;

        let mut registry = Self { name, container_id, client, cleanup: config.cleanup.clone(), rules: HashMap::new() };
        registry.compute_rules(config);

        Ok(registry)
    }

    /// Get the name of the default rule on the registry
    fn get_default_rule_name(&self) -> String {
        format!("{}-default-{}", self.name, self.container_id[0..8].to_string())
    }

    /// Compute the rules on the registry using the `[RegistryConfig]`
    fn compute_rules(&mut self, config: RegistryConfig) {
        let mut default = Rule::default();
        default.set_name(self.get_default_rule_name());
        default.extend_with_labels(&config.default);

        config.rules.into_iter().for_each(|(name, policies)| {
            let mut rule = default.clone();
            rule.set_name(name.clone());
            rule.extend_with_labels(&policies);
            self.rules.insert(name, rule);
        });
    }

    /// Get all rules of the registry in a bundled format where the keys are the cron schedules and the values
    /// are the associated rules which should run in the given schedule
    pub fn get_bundled_rules(&self) -> HashMap<String, Vec<&Rule>> {
        let mut bundles = HashMap::<String, Vec<&Rule>>::new();
        self.rules.iter().for_each(|(_, rule)| {
            if let Some(rules) = bundles.get_mut(&rule.schedule) {
                rules.push(rule)
            } else {
                bundles.insert(rule.schedule.clone(), vec![rule]);
            }
        });

        bundles
    }

    /// Get a list of all tags which would currently be deleted in the registry due to being
    /// matched by a rule defined on the registry
    pub async fn get_matched_manifests(&self, rules: Vec<String>) -> Result<Vec<ManifestMatch>, Error> {
        let mut tag_cache = HashMap::new(); // cache the tags of repositories to prevent duplicate calls

        let get_tags = async move |repository: &String| -> Result<&mut Vec<Tag>, Error> {
            match tag_cache.get_mut(repository) {
                Some(value) => Ok(value),
                None => {
                    let tags = self.get_tags_for_repository(repository).await?;
                    tag_cache.insert(repository, tags)
                        .ok_or(Error::Custom(format!("failed to insert tags for image '{repository}' to cache")))?;
                    tag_cache.get_mut(repository)
                        .ok_or(Error::Custom(String::from("tags for repository should just have been inserted")))
                }
            }
        };

        let repositories = self.client.get_catalog(None).collect::<Result<Vec<String>, _>>().await
            .map_err(Error::from)
            .map(|repos| repos.into_iter().map(|repo| Repository { name: repo }).collect::<Vec<_>>())
            .context(&format!("failed to get image catalog for repository '{}'", self.name))?;

        let mut matches = Vec::new();

        for rule in self.rules.iter().filter_map(|(name, value)| rules.contains(name).then_some(value)) {
            for repository in rule.affected_repositories(repositories.clone()) {
                let tags = get_tags(&repository.name).await
                    .context(&format!("unable to get tags for repository '{}'", self.name))?;
                let affected = rule.affected_tags(tags.clone());
                let rule_matches = affected.iter()
                    .map(|tag| ManifestMatch { repository: repository.name.clone(), tag: tag.name.clone(), rule: rule.name.clone() })
                    .collect::<&mut Vec<_>>();
                matches.append(rule_matches);
                tags.retain(|tag| !affected.contains(tag))
            }
        }
        Ok(matches)
    }

    /// Get all tags in a repository in the format we need for further processing
    async fn get_tags_for_repository(&self, repository: &String) -> Result<Vec<Tag>, Error> {
        let mut tags = Vec::new();
        while let Some(tag) = self.client.get_tags(repository, None).next().await {
            let tag_str = tag?;
            let manifest = self.client.get_manifest(repository, &tag_str).await
                .map_err(Error::from)
                .context("unable to get tag manifest")?;
            let manifest = match manifest {
                Manifest::S1Signed(manifest) => {
                    Err(Error::Custom(format!("manifest for tag '{tag_str}' is of unsupported type: expected manifest list or manifest schema 2")))?
                }
                Manifest::S2(manifest) => Ok(manifest),
                Manifest::ML(list) => {
                    let manifest_obj = list.manifests.first()
                        .ok_or(Error::Custom(String::from("manifest list should not be empty")))?;
                    let manifest = self.client.get_manifest(repository, &manifest_obj.digest).await
                        .map_err(Error::from)
                        .context("unable to get first manifest list manifest")?;
                    match manifest {
                        Manifest::S2(manifest) => Ok(manifest),
                        _ => {
                            Err(Error::Custom(format!("first manifest of manifest list for tag '{tag_str}' is of unsupported type: expected manifest schema 2")))?
                        }
                    }
                }
            }?;

            let config = manifest.manifest_spec.config();
            let manifest_config = self.client.get_blob(repository, &config.digest).await
                .map_err(Error::from)
                .context("unable to get manifest config")?;
            let manifest_config: ManifestConfig = serde_json::from_slice(manifest_config.as_slice())
                .map_err(|err| Error::Custom(format!("received invalid manifest config: {err}")))?;
            let tag = crate::policies::Tag { digest: config.digest.clone(), name: tag_str, created: manifest_config.created, size: config.size };
            tags.push(tag);
        }

        Ok(tags)
    }
    
    pub async fn delete_matched_manifest(&self, manifest: &ManifestMatch) {
        // FIXME: The dkregistry crate does not have the functionality to 
    }
}

pub struct ManifestMatch {
    /// Name of the tag to which the manifest belongs
    tag: String,
    /// Name of the repository to which the manifest belongs
    repository: String,
    /// Name of the rule which was responsible for the manifest being selected
    rule: String
}

/// Custom struct for representing a `[dkregistry::v2::manifest::ConfigBlob]` since it does
/// [not cover fields to the image spec v1](https://docs.rs/dkregistry/0.5.0/dkregistry/v2/manifest/struct.ConfigBlob.html)
///
/// **Note:** <br>
/// Only added minimal amount of fields which are needed for the sake of this application to
/// the config
///
/// When more fields are needed one can simply add them but one has to make sure it can be deserialized
/// by serde:
/// https://github.com/moby/moby/blob/a30990b3c8d0d42280fa501287859e1d2393a951/image/spec/v1.md#image-json-description
#[derive(Deserialize, Debug)]
pub struct ManifestConfig {
    /// ISO-8601 formatted combined date and time at which the image was created.
    pub created: DateTime<Utc>,
}
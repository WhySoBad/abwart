use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::Path;
use std::time::Duration;
use log::{error, info, warn};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{Debouncer, new_debouncer_opt};
use serde::Deserialize;
use crate::NAME;

#[derive(Deserialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    registries: HashMap<String, InstanceConfig>
}

impl Config {
    pub fn parse() -> serde_yaml::Result<Self> {
        let path = Path::new("config.yml");
        if let Ok(content) = read_to_string(path) {
            serde_yaml::from_str(&content)
        } else {
            Ok(Self { registries: HashMap::new() })
        }
    }

    pub fn path() -> String {
        std::env::var("CONFIG_PATH").unwrap_or(String::from("config.yml"))
    }

    pub fn is_empty(&self) -> bool {
        self.registries.is_empty()
    }

    pub fn get_registries(&self) -> HashMap<String, HashMap<String, String>> {
        let mut registries = HashMap::new();
        self.registries.iter().for_each(|(name, config)| {
            let mut labels = HashMap::new();
            if let Some(network) = &config.network {
                labels.insert(String::from("network"), network.clone());
            }
            if let Some(default) = &config.default {
                default.iter().for_each(|(key, value)| { labels.insert(format!("{NAME}.default.{key}"), value.clone()); });
            }
            if let Some(rules) = &config.rules {
                rules.iter().for_each(|(rule, value)| {
                    value.iter().for_each(|(key, value)| { labels.insert(format!("{NAME}.rule.{rule}.{key}"), value.clone()); });
                });
            }
            registries.insert(name.clone(), labels);
        });
        registries
    }

    pub fn get_registry(&self, name: &str) -> Option<HashMap<String, String>> {
        self.get_registries().get(name).cloned()
    }
}

#[derive(Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct InstanceConfig {
    default: Option<HashMap<String, String>>,
    #[serde(rename = "rule")]
    rules: Option<HashMap<String, HashMap<String, String>>>,
    network: Option<String>,
}

/// Watch the static configuration file at [`Config::path()`]. Any successful changes to the config file
/// are through the channel where the updating of the instances takes place
pub fn watch_config(sender: tokio::sync::mpsc::Sender<Config>) -> Result<(), notify::Error> {
    let (tx, rx) = std::sync::mpsc::channel::<notify_debouncer_mini::DebounceEventResult>();
    let watch_config = notify_debouncer_mini::Config::default()
        .with_batch_mode(true)
        .with_timeout(Duration::from_secs(2))
        .with_notify_config(notify::Config::default());
    let mut debouncer: Debouncer<RecommendedWatcher> = new_debouncer_opt(watch_config, tx)?;

    debouncer.watcher().watch(Path::new(&Config::path()), RecursiveMode::Recursive)?;

    tokio::spawn(async move {
        // this is necessary to move the watcher and therefore prevent tx from leaving the scope
        // and closing the channel
        let _file_watcher = debouncer;
        for res in &rx {
            // FIXME: The events are currently sent multiple times for the same thing
            let mut events = vec![];
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            match res {
                Ok(_) => {
                    match Config::parse() {
                        Ok(config) => {
                            futures::executor::block_on(async {
                                sender.send(config).await.expect("Channel should be open");
                            })
                        },
                        Err(err) => error!("Error whilst parsing updated config. Reason: {err}")
                    }
                },
                Err(err) => warn!("Received error whilst watching static configuration file. Reason: {err}")
            }
        }
    });
    info!("Set up static configuration file listener at '{}'", Config::path());
    Ok(())
}
use std::{collections::HashMap, process::exit};
use std::fs::read_to_string;
use std::ops::DerefMut;
use std::path::Path;
use std::time::Duration;
use lazy_static::lazy_static;
use log::{error, info, warn};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEventKind, Debouncer, new_debouncer_opt};
use serde::Deserialize;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::RwLock;
use crate::error::Error;
use crate::registry::RegistryConfig;
use crate::NAME;

lazy_static! {
    pub static ref CONFIG: RwLock<Config> = RwLock::new(Config::new().unwrap());
}

#[derive(Deserialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    /// Static registry configurations indexed by name
    pub registries: HashMap<String, RegistryConfig>,
}

impl Config {
    fn new() -> Result<Self, Error> {
        let path_str = Config::path();
        let path = Path::new(&path_str);
        let content = read_to_string(path).map_err(|err| Error::Custom(format!("unable to read config file at '{path_str}': {err}")))?;
        let config = serde_yaml::from_str(&content).map_err(|err| Error::Custom(format!("unable to read config: {err}")))?;
        Ok(config)
    }

    /// Path from where to read the config file
    pub fn path() -> String {
        std::env::var("CONFIG_PATH").unwrap_or(String::from("config.yml"))
    }

    /// Check whether hot reload is enabled for the configuration file
    ///
    /// Hot reload is enabled when a config file exists on the filesystem
    pub fn is_hot_reload_enabled() -> bool {
        !Path::new(&Config::path()).exists()
    }
}

pub struct ConfigWatcher {
    tx: Sender<()>
}

impl ConfigWatcher {
    pub fn new() -> Result<Self, notify::Error> {
        if !Config::is_hot_reload_enabled() {
            Err(notify::Error::generic("hot reloading is disabled due to absence of static configuration file"))?
        }

        let (ctx, crx) = std::sync::mpsc::channel::<notify_debouncer_mini::DebounceEventResult>();
        let path = Config::path();
        let watch_config = notify_debouncer_mini::Config::default()
            .with_batch_mode(true)
            .with_timeout(Duration::from_secs(5))
            .with_notify_config(notify::Config::default());
        let mut debouncer: Debouncer<RecommendedWatcher> = new_debouncer_opt(watch_config, ctx)?;
        debouncer.watcher().watch(Path::new(&path), RecursiveMode::NonRecursive)?;

        let (tx, _) = tokio::sync::broadcast::channel::<()>(3);
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            // this is necessary to move the watcher and therefore prevent tx from leaving the scope
            // and closing the channel
            let _ = debouncer;
            for res in &crx {
                match res {
                    Ok(events) => {
                        if events.iter().any(|event| event.kind == DebouncedEventKind::Any) {
                            match Config::new() {
                                Ok(config) => {
                                    futures::executor::block_on(async {
                                        let mut write_guard = CONFIG.write().await;
                                        *write_guard = config;
                                        tx.send(()).expect("channel should be open");
                                    });
                                },
                                Err(err) => error!("unable to parse updated config: {err}")
                            };
                        }
                    },
                    Err(err) => warn!("received error whilst watching static configuration file: {err}")
                }
            }
        });

        info!("set up static configuration file watcher at '{path}'");

        Ok(Self { tx: tx_clone })
    }

    /// Subscribe to the config change broadcast
    pub fn subscribe(&self) -> Receiver<()> {
        self.tx.subscribe()
    }
}
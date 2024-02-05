mod instance;
mod scheduler;
mod error;
mod task;
mod rule;
mod api;
mod policies;
mod config;
#[cfg(test)]
mod test;

use bollard::container::ListContainersOptions;
use bollard::system::EventsOptions;
use bollard::{API_DEFAULT_VERSION, Docker};
use futures::StreamExt;
use std::collections::HashMap;
use std::process::exit;
use std::sync::{Arc, Mutex};
use bollard::service::EventMessage;
use log::{error, info, warn};
use tokio::select;
use crate::config::{Config, watch_config};
use crate::error::Error;
use crate::instance::Instance;
use crate::scheduler::{DescheduleReason, Scheduler, ScheduleReason};

pub const NAME: &str = "abwart";

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let docker: Arc<Docker>;
    match Docker::connect_with_unix("/var/run/docker.sock", 30, API_DEFAULT_VERSION) {
        Ok(client) => {
            match client.ping().await {
                Ok(_) => docker = Arc::new(client),
                Err(_) => {
                    error!("Ping to docker client failed");
                    exit(1)
                }
            }
        },
        Err(err) => {
            error!("Unable to connect to docker socket. Reason: {err}");
            exit(1)
        }
    }

    let config = match Config::parse() {
        Ok(config) => {
            if !config.is_empty() {
                info!("Using config from static configuration file at '{}'", Config::path())
            }
            Arc::new(Mutex::new(config))
        },
        Err(err) => {
            error!("Error whilst parsing static configuration file. Reason: {err}");
            Arc::new(Mutex::new(Config::default()))
        }
    };

    let mut filters = HashMap::new();
    filters.insert(String::from("label"), vec![format!("{}=true", label("enable"))]);

    let mut scheduler = Scheduler::new();

    let options = ListContainersOptions {
        filters,
        ..ListContainersOptions::default()
    };
    let containers = docker.list_containers(Some(options)).await
        .map_err(|err| error!("Unable to get existing running registries. Reason: {err}"))
        .unwrap_or_default();

    for container in containers {
        if !&container.image.clone().unwrap_or_default().starts_with("registry") {
            warn!("Potentially found running container which is enabled and doesn't use image 'registry'");
        }
        match Instance::from_container(container, docker.clone(), config.clone()) {
            Ok(instance) => scheduler.schedule_instance(instance, ScheduleReason::RegistryRunning).await,
            Err(err) => error!("Unable to add registry to schedule. Reason: {err}")
        }
    }

    subscribe_events(docker, config, scheduler).await;
}

async fn subscribe_events(docker: Arc<Docker>, config: Arc<Mutex<Config>>, mut scheduler: Scheduler) {
    let mut filters = HashMap::new();
    filters.insert(String::from("label"), vec![format!("{}=true", label("enable"))]);
    filters.insert(String::from("type"), vec![String::from("container")]);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Config>(1);

    let options = EventsOptions::<String> {
        filters,
        ..EventsOptions::<String>::default()
    };
    let mut events = docker.events(Some(options));
    if let Err(err) = watch_config(tx.clone()) {
        error!("Unable to watch config file at '{}'. Disabled static config hot reloading. Reason: {err}", Config::path())
    }

    loop {
        select! {
            Some(event) = events.next() => {
                let result = handle_event(&event, &mut scheduler, docker.clone(), config.clone()).await;
                if let Err(err) = result {
                    info!("{err}")
                }
            },
            Some(new_config) = rx.recv() => handle_config_update(&new_config, &mut scheduler, docker.clone(), config.clone()).await
        }
    };
}

async fn handle_event(event: &Result<EventMessage, bollard::errors::Error>, scheduler: &mut Scheduler, docker: Arc<Docker>, config: Arc<Mutex<Config>>) -> Result<(), String> {
    match event {
        Ok(message) => {
            let actor = message.actor.clone().ok_or(String::from("Event message is missing actor. Ignoring message"))?;
            let action = message.action.clone().ok_or(String::from("Event message is missing action. Ignoring message"))?;
            match action.as_str() {
                "stop" | "pause" | "kill" => {
                    match actor.id {
                        Some(id) => { scheduler.deschedule_instance(id, DescheduleReason::RegistryStop).await; },
                        None => println!("Unable to request deschedule of registry. Reason: {}", Error::MissingId)
                    }
                }
                "start" | "unpause" => {
                    match Instance::from_actor(actor, docker.clone(), config).await {
                        Ok(instance) => scheduler.schedule_instance(instance, ScheduleReason::RegistryStart).await,
                        Err(err) => error!("Unable to add new registry to schedule. Reason: {err}")
                    }
                }
                _ => {}
            }
        }
        Err(err) => warn!("Received event error: {err}")
    }
    Ok(())
}

async fn handle_config_update(new_config: &Config, scheduler: &mut Scheduler, docker: Arc<Docker>, config: Arc<Mutex<Config>>) {
    let updatable = match config.lock() {
        Ok(mut config) => {
            let new_registries = new_config.get_registries();
            let updatable = config.get_registries().iter()
                .filter(|(key, old_value)| new_registries.get(*key).map_or(true, |v| old_value.ne(&v)))
                .filter_map(|(key, _)| scheduler.get_instance(key))
                .collect::<Vec<String>>();

            *config = new_config.clone();
            updatable
        }
        Err(err) => {
            error!("Unable to lock old config. Reason: {err}");
            return
        }
    };

    if updatable.is_empty() {
        info!("Received config update affecting no running instances")
    } else {
        info!("Received config update affecting {} running instances", updatable.len());

        let mut filters = HashMap::new();
        filters.insert(String::from("id"), updatable);
        let options = ListContainersOptions {
            filters,
            ..ListContainersOptions::default()
        };

        match docker.list_containers(Some(options)).await {
            Ok(containers) => {
                for container in containers {
                    let id = container.id.clone().unwrap_or_default();
                    scheduler.deschedule_instance(id, DescheduleReason::ConfigUpdate).await;
                    match Instance::from_container(container, docker.clone(), config.clone()) {
                        Ok(instance) => scheduler.schedule_instance(instance, ScheduleReason::ConfigUpdate).await,
                        Err(err) => error!("Unable to create instance from container. Reason: {err}")
                    }
                }
            },
            Err(err) => error!("Unable to reflect config change. Cannot get containers. Reason: {err}")
        }
    }
}

/// Format a label which is associated with the program to omit repeating the name
/// # Example
/// ```
/// assert_eq!(label("rule.age.max"), "abwart.rule.age.max");
/// ```
pub fn label(suffix: &str) -> String {
    format!("{NAME}.{suffix}")
}
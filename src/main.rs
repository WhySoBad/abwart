mod instance;
mod scheduler;
mod error;
mod task;
mod rule;
mod policies;
mod config;
mod registry;
mod docker;
#[cfg(test)]
mod test;
mod daemon;

use bollard::container::ListContainersOptions;
use bollard::system::EventsOptions;
use bollard::{API_DEFAULT_VERSION, Docker};
use config::ConfigWatcher;
use docker::{get_docker_client, DockerClient};
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

    let docker = match DockerClient::new().await {
        Ok(client) => Arc::new(client),
        Err(err) => {
            error!("unable to create docker client: {err}");
            exit(1);
        }
    };
    let watcher = match ConfigWatcher::new() {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            warn!("unable to create new config watcher: {err}");
            None
        }
    };
    let mut scheduler = Scheduler::new();

    let containers = docker.get_containers().await.unwrap_or_else(|err| {
        error!("unable to get running containers: {err}");
        vec![]
    });

    for container in containers {
        let image = container.image.clone().unwrap_or_default();
        if !image.starts_with("registry") && !image.starts_with("distribution") {
            warn!("found running container which potentially does not use 'registry' or 'distribution' image: instead found '{image}'");
        }
    }

    // subscribe_events(docker, config, scheduler).await;
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
                        Some(id) => { scheduler.deschedule_registry(id, DescheduleReason::RegistryStop).await; },
                        None => println!("Unable to request deschedule of registry. Reason: {}", Error::MissingId)
                    }
                }
                "start" | "unpause" => {
                    match Instance::from_actor(actor, docker.clone(), config).await {
                        Ok(instance) => scheduler.schedule_registry(instance, ScheduleReason::RegistryStart).await,
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
                .filter_map(|(key, _)| scheduler.get_registry(key))
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
                    scheduler.deschedule_registry(id, DescheduleReason::ConfigUpdate).await;
                    match Instance::from_container(container, docker.clone(), config.clone()) {
                        Ok(instance) => scheduler.schedule_registry(instance, ScheduleReason::ConfigUpdate).await,
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
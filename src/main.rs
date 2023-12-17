mod instance;
mod scheduler;
mod policy;
mod error;
mod task;
mod rule;
mod api;

use bollard::container::ListContainersOptions;
use bollard::system::EventsOptions;
use bollard::Docker;
use futures::StreamExt;
use std::collections::HashMap;
use std::process::exit;
use std::sync::Arc;
use log::{error, warn};
use crate::instance::Instance;
use crate::scheduler::Scheduler;

pub const NAME: &str = "abwart";

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let docker: Arc<Docker>;
    match Docker::connect_with_unix_defaults() {
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

    let mut filters = HashMap::new();
    filters.insert(String::from("label"), vec![format!("{}=true", label("enable"))]);

    let mut scheduler = Scheduler::new();

    let containers = docker.list_containers(Some(ListContainersOptions {
            filters: filters.clone(),
            ..ListContainersOptions::default()
        })).await.map_err(|err| error!("Unable to get existing running registries. Reason: {err}"))
        .unwrap_or_default();

    for container in containers {
        if !&container.image.clone().unwrap_or_default().starts_with("distribution/distribution") {
            warn!("Found running container which is enabled and doesn't use image distribution/distribution. Ignoring container");
            continue;
        }
        match Instance::from_container(container, docker.clone()) {
            Ok(instance) => scheduler.schedule_instance(instance).await,
            Err(err) => error!("Unable to add registry to schedule. Reason: {err}")
        }
    }

    filters.insert(String::from("image"), vec![String::from("distribution/distribution")]);
    filters.insert(String::from("type"), vec![String::from("container")]);

    let options = EventsOptions::<String> {
        filters,
        ..EventsOptions::<String>::default()
    };
    let mut events = docker.events(Some(options));

    while let Some(event) = events.next().await {
        match event {
            Ok(message) => {
                if message.actor.is_none() {
                    warn!("Event message is missing actor. Ignoring message");
                };
                let actor = message.actor.expect("Actor should exist");
                let action = message.action.unwrap_or_default();
                match action.as_str() {
                    "stop" | "pause" | "kill" => {
                        match actor.id {
                            Some(id) => scheduler.deschedule_instance(id).await,
                            None => println!("Unable to request deschedule of registry. Reason: {}", error::Error::MissingId)
                        }
                    }
                    "start" | "unpause" => {
                        match Instance::from_actor(actor, docker.clone()).await {
                            Ok(instance) => scheduler.schedule_instance(instance).await,
                            Err(err) => error!("Unable to add new registry to schedule. Reason: {err}")
                        }
                    }
                    _ => {}
                }
            }
            Err(err) => warn!("Received event error: {err}")
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
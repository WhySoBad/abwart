use std::process::exit;
use std::sync::Arc;
use bollard::Docker;
use futures::StreamExt;
use log::{error, warn};
use tokio::select;
use tokio::sync::broadcast::Receiver;
use crate::config::{ConfigWatcher, CONFIG};
use crate::docker::DockerClient;
use crate::error::{Error, ErrorContext};
use crate::registry::{Registry, RegistryConfig};
use crate::scheduler::{DescheduleReason, ScheduleReason, Scheduler};

pub async fn start_daemon() -> Result<(), Error> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let docker = DockerClient::new().await.map(Arc::new).map_err(Error::Custom).context("unable to create docker client")?;
    let watcher = ConfigWatcher::new().map_err(|err| warn!("unable to create new config watcher: {err}")).ok();
    let mut scheduler = Arc::new(Scheduler::new());

    schedule_running_registries(&docker, &mut scheduler).await?;

    let mut events = docker.get_events().await;
    if let Some(watcher) = watcher {
        let mut rx = watcher.subscribe();
        loop {
            select! {
                Some(event) = events.next() => {}
                Some(_) = rx.recv() => handle_config_change(&docker, &mut scheduler).await
            }
        }
    } else {
        while let Some(Ok(event)) = events.next().await {

        }
    }

    Ok(())
}

/// Get all running containers which have the `enabled` label and schedule them
/// for periodic executions
async fn schedule_running_registries(docker: &Arc<DockerClient>, scheduler: &mut Arc<Scheduler>) -> Result<(), Error> {
    let containers = docker.get_containers().await.unwrap_or_else(|err| {
        error!("unable to get running containers: {err}");
        vec![]
    });

    for container in containers {
        let image = container.image.clone().unwrap_or_default();
        if !image.starts_with("registry") && !image.starts_with("distribution") {
            warn!("found running container which potentially does not use 'registry' or 'distribution' image: instead found '{image}'");
        }

        let config = RegistryConfig::from_container(&container).context("unable to create registry config from container")?;
        let registry = Registry::from_container(container, config).await?;
        scheduler.schedule_registry(registry, ScheduleReason::RegistryRunning).await;
    }

    Ok(())
}

async fn handle_config_change(docker: &Arc<DockerClient>, scheduler: &mut Arc<Scheduler>) {

    for (name, config) in CONFIG.registries.iter() {
        if let Some(container_id) = scheduler.get_registry(name) {
            let container = docker.
            let registry = scheduler.deschedule_registry(container_id, DescheduleReason::ConfigUpdate).await;
        }
    }
}
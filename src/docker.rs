use std::{collections::HashMap};
use bollard::{container::ListContainersOptions, exec::{CreateExecOptions, StartExecOptions}, secret::{ContainerSummary, EventMessage}, system::EventsOptions, Docker, API_DEFAULT_VERSION};
use futures::Stream;
use crate::error::Error;
use crate::label;

pub struct DockerClient {
    client: Docker
}

impl DockerClient {
    pub async fn new() -> Result<Self, String> {
        let client = Docker::connect_with_unix("/var/run/docker.sock", 30, API_DEFAULT_VERSION)?;
        match client.ping().await {
            Ok(_) => {},
            Err(err) => Err(format!("ping to docker socket failed: {err}"))?
        }
        Ok(Self { client })
    }

    /// Run the garbage collector inside a container
    pub async fn run_garbage_collector(&self, container_id: String) -> Result<(), Error> {
        // TODO: Add way for customizing the exec command to support a wider variety of registries
        let exec = self.client.create_exec(&container_id, CreateExecOptions::<&str>{
            cmd: Some(vec!["/bin/registry", "garbage-collect", "--delete-untagged", "/etc/docker/registry/config.yml"]),
            user: Some("root"),
            ..CreateExecOptions::default()
        }).await;

        match exec {
            Ok(exec) => {
                match self.client.start_exec(exec.id.as_str(), None::<StartExecOptions>).await {
                    Ok(_) => Ok(()),
                    Err(err) => Err(format!("unable to run garbage collector in container '{container_id}': {err}").into())
                }
            },
            Err(err) => Err(format!("unable to create new exec in container '{container_id}': {err}").into())
        }
    }

    /// Subscribe to events happening to all containers which have the `enabled` label set
    pub async fn get_events(&self) -> impl Stream<Item = Result<EventMessage, bollard::errors::Error>> {
        let mut filters = HashMap::new();
        filters.insert(String::from("label"), vec![format!("{}=true", label("enable"))]);
        filters.insert(String::from("type"), vec![String::from("container")]);
        let options = EventsOptions::<String> {
            filters,
            ..EventsOptions::<String>::default()
        };
        self.client.events(Some(options))
    }

    /// Get all running containers which have the `enabled` label set
    pub async fn get_containers(&self) -> Result<Vec<ContainerSummary>, Error> {
        let mut filters = HashMap::new();
        filters.insert(String::from("label"), vec![format!("{}=true", label("enable"))]);

        let options = ListContainersOptions {
            filters,
            ..ListContainersOptions::default()
        };
        self.client.list_containers(Some(options)).await.map_err(|err| Error::from(err))
    }
    
    pub async fn get_registry(&self, id: &str) -> Result<ContainerSummary, Error> {
        self.client.inspect_container()
    } 
}
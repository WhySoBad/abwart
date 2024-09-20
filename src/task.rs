use std::sync::Arc;
use log::{debug, error, info};
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::error::Error;
use crate::instance::Instance;
use crate::registry::Registry;

pub struct Task {
    pub registry: Arc<Registry>,
    tx: Option<tokio::sync::mpsc::Sender<()>>
}

impl Task {
    pub fn new(registry: Registry) -> Self {
        Self { registry: Arc::new(registry), tx: None }
    }

    /// Start the scheduling process for all unique cron times of the registry
    pub async fn start(&mut self) -> Result<(), Error> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        self.tx = Some(tx.clone());
        let bundles = self.registry.get_bundled_rules();
        let name = self.registry.name.clone();
        let copy_name = name.clone();
        let registry = self.registry.clone();

        let mut scheduler = JobScheduler::new().await
            .map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;

        for (cron, rules) in bundles {
            debug!("cron '{cron}' with rules '{}'", rules.join(", "));
            let registry = registry.clone();
            let copy_name = copy_name.clone();
            let job = Job::new_async(cron.as_str(), move |_uuid, mut _l| {
                let registry = registry.clone();
                let rules = rules.clone();
                let name = copy_name.clone();

                Box::pin(async move {
                    let next_tick = _l.next_tick_for_job(_uuid).await;
                    debug!("next tick for registry '{name}' is {:?}", next_tick.unwrap_or_default().unwrap_or_default());
                    info!("applying rules '{}' to registry '{name}'", rules.join(", "));
                    match registry.apply_rules(rules.clone()).await {
                        Ok(_) => info!("successfully applied rules '{}' to registry '{name}'", rules.join(", ")),
                        Err(err) => error!("unable to apply rules '{}' to registry '{name}': {err}", rules.join(", "))
                    }
                })
            }).map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;

            scheduler.add(job).await.map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;
        }

        if let Some(cleanup_schedule) = &registry.cleanup {
            debug!("Found cleanup schedule defined on registry '{name}'");
            let registry = registry.clone();
            let copy_name = copy_name.clone();
            let job = Job::new_async(cleanup_schedule, move |_uuid, mut _l| {
                let registry = registry.clone();
                let name = copy_name.clone();

                Box::pin(async move {
                    let next_tick = _l.next_tick_for_job(_uuid).await;
                    debug!("Next automated cleanup for registry '{name}' is {:?}", next_tick.unwrap_or_default().unwrap_or_default());
                    info!("Running automated cleanup in registry '{name}'");
                    registry.run_garbage_collector().await;
                })
            }).map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;
            scheduler.add(job).await.map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;
        }

        tokio::spawn(async move {
            if let Err(err) = scheduler.start().await {
                error!("Task for registry '{name}' couldn't be started. Reason: {err}");
            } else {
                 info!("Successfully started task for registry '{name}'");
            }
            rx.recv().await;
            debug!("Interrupting task for registry '{name}'");
            scheduler.shutdown().await.unwrap();
        });

        Ok(())
    }

    /// Stop the scheduling process for all unique cron times of a registry
    pub async fn stop(&mut self) -> Result<(), Error> {
        let name = self.registry.name.clone();
        if let Some(tx) = &mut self.tx {
            info!("stopping task for registry '{name}'");
            tx.send(()).await.map_err(|err| Error::TaskInterruptionFailed(name, err.to_string()))?;
            self.tx = None;

            Ok(())
        } else {
            Err(Error::TaskNotStarted(name))
        }
    }
}

use std::sync::Arc;
use log::{debug, error, info};
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::error::Error;
use crate::instance::Instance;

pub struct Task {
    pub instance: Arc<Instance>,
    tx: Option<tokio::sync::mpsc::Sender<()>>
}

impl Task {
    pub fn new(instance: Instance) -> Self {
        Self { instance: Arc::new(instance), tx: None }
    }

    /// Start the scheduling process for all unique cron times of an instance
    pub async fn start(&mut self) -> Result<(), Error> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        self.tx = Some(tx.clone());
        let bundles = self.instance.get_bundled_rules();
        let name = self.instance.name.clone();
        let copy_name = name.clone();
        let instance = self.instance.clone();

        let mut sched = JobScheduler::new().await.map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;

        for (cron, rules) in bundles {
            let instance = instance.clone();
            let copy_name = copy_name.clone();
            let job = Job::new_async(cron.as_str(), move |_uuid, _l| {
                let instance = instance.clone();
                let rules = rules.clone();
                let name = copy_name.clone();
                Box::pin(async move {
                    info!("Applying rules '{}' to registry '{name}'", rules.join(", "));
                    match instance.apply_rules(rules.clone()).await {
                        Ok(_) => info!("Successfully applied rules '{}' to registry '{name}'", rules.join(", ")),
                        Err(err) => error!("Unable to apply rules '{}' to registry '{name}'. Reason: {err}", rules.join(", "))
                    }
                })
            }).map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;
            sched.add(job).await.map_err(|err| Error::TaskCreationFailed(name.clone(), err.to_string()))?;
        }

        tokio::spawn(async move {
            if let Err(err) = sched.start().await {
                error!("Task for registry '{name}' couldn't be started. Reason: {err}");
            } else {
                 info!("Successfully started task for registry '{name}'");
            }
            rx.recv().await;
            debug!("Interrupting task for registry '{name}'");
            sched.shutdown().await.unwrap();
        });

        Ok(())
    }

    /// Stop the scheduling process for all unique cron times of an instance
    pub async fn stop(&mut self) -> Result<(), Error> {
        let name = self.instance.name.clone();
        if let Some(tx) = &mut self.tx {
            info!("Stopping task for registry '{name}'");
            tx.send(()).await.map_err(|err| Error::TaskInterruptionFailed(name, err.to_string()))?;
            self.tx = None;

            Ok(())
        } else {
            Err(Error::TaskNotStarted(name))
        }
    }
}

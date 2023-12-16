use std::collections::HashMap;
use log::{error, info, warn};
use crate::instance::Instance;
use crate::task::Task;

pub struct Scheduler {
    tasks: HashMap<String, Task>
}

impl Scheduler {
    pub fn new() -> Self {
        Self { tasks: HashMap::new() }
    }

    /// Start scheduling a given instance
    pub async fn schedule_instance(&mut self, instance: Instance) {
        if self.tasks.contains_key(instance.id.as_str()) {
            warn!("Received duplicate schedule request for registry '{}'. Ignoring request", instance.name);
            return
        }

        let id = instance.id.clone();
        let name = instance.name.clone();
        let mut task = Task::new(instance);
        match task.start().await {
            Ok(_) => {
                info!("Added registry '{name}' to scheduler");
                self.tasks.insert(id, task);
            },
            Err(err) => {
                error!("Unable add registry '{name}' to scheduler. Reason: {err}")
            }
        }
    }

    /// Remove a given instance from the scheduler
    pub async fn deschedule_instance(&mut self, id: String) {
        if let Some(task) = self.tasks.get_mut(id.as_str()) {
            let name = task.instance.name.clone();
            match task.stop().await {
                Ok(_) => {
                    info!("Removed registry '{name}' from scheduler");
                    self.tasks.remove(id.as_str());
                },
                Err(err) => {
                    error!("Unable remove registry '{name}' from scheduler. Reason: {err}")
                }
            }
        } else {
            warn!("Received deschedule request for unscheduled registry '{id}'. Ignoring request");
        }
    }
}
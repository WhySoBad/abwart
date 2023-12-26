use std::collections::HashMap;
use std::sync::Arc;
use log::{error, info, warn};
use crate::instance::Instance;
use crate::task::Task;

#[derive(Debug)]
pub enum ScheduleReason {
    RegistryStart,
    RegistryRunning,
    ConfigUpdate
}

#[derive(Debug)]
pub enum DescheduleReason {
    RegistryStop,
    ConfigUpdate
}

pub struct Scheduler {
    tasks: HashMap<String, Task>,
    names: HashMap<String, String>
}

impl Scheduler {
    pub fn new() -> Self {
        Self { tasks: HashMap::new(), names: HashMap::new() }
    }

    /// Start scheduling a given instance
    pub async fn schedule_instance(&mut self, instance: Instance, reason: ScheduleReason) {
        if self.tasks.contains_key(instance.id.as_str()) {
            warn!("Received duplicate schedule request for registry '{}' ({reason:?}). Ignoring request", instance.name);
            return
        }

        let id = instance.id.clone();
        let name = instance.name.clone();
        let mut task = Task::new(instance);
        self.names.insert(name.clone(), id.clone());
        match task.start().await {
            Ok(_) => {
                info!("Added registry '{name}' to scheduler ({reason:?})");
                self.tasks.insert(id, task);
            },
            Err(err) => {
                error!("Unable add registry '{name}' to scheduler ({reason:?}). Reason: {err}")
            }
        }
    }

    /// Remove a given instance from the scheduler <br>
    /// Returns the instance which was descheduled
    pub async fn deschedule_instance(&mut self, id: String, reason: DescheduleReason) -> Option<Arc<Instance>> {
        if let Some(task) = self.tasks.get_mut(id.as_str()) {
            let instance = task.instance.clone();
            let name = instance.name.clone();
            match task.stop().await {
                Ok(_) => {
                    info!("Removed registry '{name}' from scheduler ({reason:?})");
                    self.tasks.remove(id.as_str());
                    self.names.remove(&name);
                    Some(instance)
                },
                Err(err) => {
                    error!("Unable remove registry '{name}' from scheduler ({reason:?}). Reason: {err}");
                    None
                }
            }
        } else {
            warn!("Received deschedule request for unscheduled registry '{id}' ({reason:?}). Ignoring request");
            None
        }
    }

    pub fn get_instance(&self, name: &str) -> Option<String> {
        self.names.get(name).cloned()
    }
}
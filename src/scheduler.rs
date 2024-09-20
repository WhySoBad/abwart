use std::collections::HashMap;
use std::sync::Arc;
use log::{error, info, warn};
use crate::instance::Instance;
use crate::registry::Registry;
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

    /// Start scheduling a given registry
    pub async fn schedule_registry(&mut self, registry: Registry, reason: ScheduleReason) {
        if self.tasks.contains_key(&registry.container_id) {
            warn!("received duplicate schedule request for registry '{}' ({reason:?}): ignoring request", registry.name);
            return
        }

        let id = registry.container_id.clone();
        let name = registry.name.clone();
        let mut task = Task::new(registry);
        self.names.insert(name.clone(), id.clone());
        match task.start().await {
            Ok(_) => {
                info!("added registry '{name}' to scheduler ({reason:?})");
                self.tasks.insert(id, task);
            },
            Err(err) => {
                error!("unable add registry '{name}' to scheduler ({reason:?}): {err}")
            }
        }
    }

    /// Remove a given registry from the scheduler
    ///
    /// Returns the registry which was descheduled
    pub async fn deschedule_registry(&mut self, id: String, reason: DescheduleReason) -> Option<Arc<Registry>> {
        if let Some(task) = self.tasks.get_mut(id.as_str()) {
            let registry = task.registry.clone();
            let name = registry.name.clone();
            match task.stop().await {
                Ok(_) => {
                    info!("removed registry '{name}' from scheduler ({reason:?})");
                    self.tasks.remove(id.as_str());
                    self.names.remove(&name);
                    Some(registry)
                },
                Err(err) => {
                    error!("unable remove registry '{name}' from scheduler ({reason:?}): {err}");
                    None
                }
            }
        } else {
            warn!("received deschedule request for unscheduled registry '{id}' ({reason:?}): ignoring request");
            None
        }
    }

    pub fn get_registry(&self, name: &str) -> Option<String> {
        self.names.get(name).cloned()
    }
}
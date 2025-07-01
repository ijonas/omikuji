use crate::network::NetworkManager as NetworkProviders;
use crate::scheduled_tasks::{
    condition_checker::ConditionChecker, executor::FunctionExecutor, models::ScheduledTask,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, debug};

pub struct ScheduledTaskManager {
    tasks: Arc<RwLock<HashMap<String, ScheduledTask>>>,
    network_providers: Arc<NetworkProviders>,
    scheduler: Arc<JobScheduler>,
    handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
}

impl ScheduledTaskManager {
    pub async fn new(
        tasks: Vec<ScheduledTask>,
        network_providers: Arc<NetworkProviders>,
    ) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;
        
        let task_map = tasks
            .into_iter()
            .map(|task| (task.name.clone(), task))
            .collect();

        Ok(Self {
            tasks: Arc::new(RwLock::new(task_map)),
            network_providers,
            scheduler: Arc::new(scheduler),
            handles: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub async fn start(&self) -> Result<()> {
        let tasks = self.tasks.read().await;
        
        for (name, task) in tasks.iter() {
            self.schedule_task(name.clone(), task.clone()).await?;
        }

        self.scheduler.start().await?;
        info!("Scheduled task manager started with {} tasks", tasks.len());
        
        Ok(())
    }

    async fn schedule_task(&self, name: String, task: ScheduledTask) -> Result<()> {
        let network_providers = self.network_providers.clone();
        let task_clone = task.clone();
        
        let job = Job::new_async(task.schedule.as_str(), move |_uuid, _l| {
            let name = name.clone();
            let task = task_clone.clone();
            let providers = network_providers.clone();
            
            Box::pin(async move {
                debug!("Executing scheduled task: {}", name);
                
                if let Err(e) = execute_task(task, providers).await {
                    error!("Failed to execute scheduled task '{}': {}", name, e);
                }
            })
        })?;

        self.scheduler.add(job).await?;
        info!("Scheduled task '{}' with cron expression '{}'", task.name, task.schedule);
        
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        info!("Stopping scheduled task manager");
        
        // Note: tokio-cron-scheduler doesn't provide a mutable shutdown method
        // The scheduler will be cleaned up when dropped
        
        // Cancel all running tasks
        let mut handles = self.handles.write().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
        
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_task_status(&self) -> HashMap<String, TaskStatus> {
        let tasks = self.tasks.read().await;
        let mut status_map = HashMap::new();
        
        for (name, task) in tasks.iter() {
            status_map.insert(
                name.clone(),
                TaskStatus {
                    name: name.clone(),
                    schedule: task.schedule.clone(),
                    network: task.network.clone(),
                    // Additional status fields can be added here
                },
            );
        }
        
        status_map
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TaskStatus {
    pub name: String,
    pub schedule: String,
    pub network: String,
}

async fn execute_task(
    task: ScheduledTask,
    network_providers: Arc<NetworkProviders>,
) -> Result<()> {
    // Get the provider for the task's network
    let provider = network_providers
        .get_provider(&task.network)
        .context(format!("No provider found for network: {}", task.network))?;

    // Check condition if specified
    if let Some(condition) = &task.check_condition {
        let condition_met = ConditionChecker::check_condition(
            provider.clone(),
            condition,
        )
        .await
        .context("Failed to check condition")?;

        if !condition_met {
            debug!(
                "Condition not met for task '{}', skipping execution",
                task.name
            );
            return Ok(());
        }
    }

    // Execute the target function
    let executor = FunctionExecutor::new(provider);
    executor
        .execute_function(&task.target_function, task.gas_config.as_ref())
        .await
        .context("Failed to execute target function")?;

    info!("Successfully executed scheduled task: {}", task.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduled_tasks::models::TargetFunction;

    #[tokio::test]
    async fn test_scheduled_task_manager_creation() {
        let _tasks = vec![
            ScheduledTask {
                name: "test_task".to_string(),
                network: "ethereum-mainnet".to_string(),
                schedule: "0 0 * * * *".to_string(),
                check_condition: None,
                target_function: TargetFunction {
                    contract_address: "0x1234567890123456789012345678901234567890".to_string(),
                    function: "execute()".to_string(),
                    parameters: vec![],
                },
                gas_config: None,
            },
        ];

        // This test would need a mock NetworkProviders
        // For now, we just test that the structure compiles correctly
    }
}
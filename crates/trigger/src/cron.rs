use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info};

use crate::traits::{Trigger, TriggerError, TriggerEvent};

pub struct CronTrigger {
    schedule: String,
    workflow_name: String,
    scheduler: Arc<Mutex<Option<JobScheduler>>>,
}

impl CronTrigger {
    pub fn new(schedule: String, workflow_name: String) -> Self {
        Self {
            schedule,
            workflow_name,
            scheduler: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl Trigger for CronTrigger {
    async fn start(&self, tx: mpsc::Sender<TriggerEvent>) -> Result<(), TriggerError> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|e| TriggerError::StartError(format!("failed to create scheduler: {e}")))?;

        let workflow_name = self.workflow_name.clone();
        let job = Job::new_async(self.schedule.as_str(), move |_uuid, _lock| {
            let tx = tx.clone();
            let name = workflow_name.clone();
            Box::pin(async move {
                info!("cron trigger fired for workflow '{name}'");
                let event = TriggerEvent {
                    workflow_name: name,
                    trigger_type: "cron".to_string(),
                    triggered_at: std::time::SystemTime::now(),
                };
                if let Err(e) = tx.send(event).await {
                    error!("failed to send trigger event: {e}");
                }
            })
        })
        .map_err(|e| TriggerError::InvalidCron(format!("{e}")))?;

        scheduler
            .add(job)
            .await
            .map_err(|e| TriggerError::StartError(format!("failed to add job: {e}")))?;

        scheduler
            .start()
            .await
            .map_err(|e| TriggerError::StartError(format!("failed to start scheduler: {e}")))?;

        debug!("cron trigger started with schedule '{}'", self.schedule);
        *self.scheduler.lock().await = Some(scheduler);
        Ok(())
    }

    async fn stop(&self) -> Result<(), TriggerError> {
        if let Some(mut scheduler) = self.scheduler.lock().await.take() {
            scheduler
                .shutdown()
                .await
                .map_err(|e| TriggerError::StartError(format!("failed to shutdown: {e}")))?;
        }
        Ok(())
    }

    fn trigger_type(&self) -> &str {
        "cron"
    }
}

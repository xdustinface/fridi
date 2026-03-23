use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex, Notify};
use tracing::debug;

use crate::traits::{Trigger, TriggerError, TriggerEvent};

pub struct ManualTrigger {
    workflow_name: String,
    notify: Arc<Notify>,
    stop_flag: Arc<Mutex<bool>>,
}

impl ManualTrigger {
    pub fn new(workflow_name: String) -> Self {
        Self {
            workflow_name,
            notify: Arc::new(Notify::new()),
            stop_flag: Arc::new(Mutex::new(false)),
        }
    }

    pub fn fire(&self) {
        debug!("manual trigger fired for workflow '{}'", self.workflow_name);
        self.notify.notify_one();
    }
}

#[async_trait]
impl Trigger for ManualTrigger {
    async fn start(&self, tx: mpsc::Sender<TriggerEvent>) -> Result<(), TriggerError> {
        let notify = self.notify.clone();
        let stop_flag = self.stop_flag.clone();
        let workflow_name = self.workflow_name.clone();

        tokio::spawn(async move {
            loop {
                notify.notified().await;
                if *stop_flag.lock().await {
                    break;
                }
                let event = TriggerEvent {
                    workflow_name: workflow_name.clone(),
                    trigger_type: "manual".to_string(),
                    triggered_at: std::time::SystemTime::now(),
                };
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), TriggerError> {
        *self.stop_flag.lock().await = true;
        self.notify.notify_one();
        Ok(())
    }

    fn trigger_type(&self) -> &str {
        "manual"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manual_trigger_fire() {
        let trigger = ManualTrigger::new("test-workflow".to_string());
        let (tx, mut rx) = mpsc::channel(10);
        trigger.start(tx).await.unwrap();
        trigger.fire();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.workflow_name, "test-workflow");
        assert_eq!(event.trigger_type, "manual");
        trigger.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_manual_trigger_multiple_fires() {
        let trigger = ManualTrigger::new("test".to_string());
        let (tx, mut rx) = mpsc::channel(10);
        trigger.start(tx).await.unwrap();
        trigger.fire();
        let e1 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(e1.workflow_name, "test");
        trigger.fire();
        let e2 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(e2.workflow_name, "test");
        trigger.stop().await.unwrap();
    }
}

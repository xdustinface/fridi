use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::traits::{OverlapPolicy, Trigger, TriggerError, TriggerEvent};

pub(crate) struct WorkflowTriggerConfig {
    triggers: Vec<Arc<dyn Trigger>>,
    overlap_policy: OverlapPolicy,
}

/// Central coordinator that manages triggers for all registered workflows
/// and dispatches trigger events through a single channel.
pub struct TriggerManager {
    workflows: HashMap<String, WorkflowTriggerConfig>,
    event_tx: mpsc::Sender<TriggerEvent>,
    event_rx: Option<mpsc::Receiver<TriggerEvent>>,
}

impl TriggerManager {
    pub fn new(buffer_size: usize) -> Self {
        let (event_tx, event_rx) = mpsc::channel(buffer_size);
        Self {
            workflows: HashMap::new(),
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Registers triggers for a workflow with the given overlap policy.
    pub fn register(
        &mut self,
        workflow_name: String,
        triggers: Vec<Arc<dyn Trigger>>,
        overlap_policy: OverlapPolicy,
    ) {
        self.workflows.insert(
            workflow_name,
            WorkflowTriggerConfig {
                triggers,
                overlap_policy,
            },
        );
    }

    /// Takes the event receiver. Can only be called once, before `start`.
    /// The consumer uses this to receive trigger events.
    pub fn subscribe(&mut self) -> Option<mpsc::Receiver<TriggerEvent>> {
        self.event_rx.take()
    }

    /// Starts all registered triggers. Each trigger's events are wrapped
    /// to include the configured overlap policy before being forwarded.
    pub async fn start(&self) -> Result<(), TriggerError> {
        for (workflow_name, config) in &self.workflows {
            let overlap_policy = config.overlap_policy;
            for trigger in &config.triggers {
                let (inner_tx, mut inner_rx) = mpsc::channel::<TriggerEvent>(16);
                trigger.start(inner_tx).await?;

                let out_tx = self.event_tx.clone();
                let wf_name = workflow_name.clone();
                let trigger_type = trigger.trigger_type().to_string();
                tokio::spawn(async move {
                    while let Some(mut event) = inner_rx.recv().await {
                        event.overlap_policy = overlap_policy;
                        if out_tx.send(event).await.is_err() {
                            debug!(
                                "event channel closed for trigger '{}' on workflow '{}'",
                                trigger_type, wf_name
                            );
                            break;
                        }
                    }
                });

                info!(
                    "started {} trigger for workflow '{}'",
                    trigger.trigger_type(),
                    workflow_name
                );
            }
        }
        Ok(())
    }

    /// Stops all registered triggers.
    pub async fn stop(&self) {
        for (workflow_name, config) in &self.workflows {
            for trigger in &config.triggers {
                if let Err(e) = trigger.stop().await {
                    debug!(
                        "error stopping {} trigger for workflow '{}': {}",
                        trigger.trigger_type(),
                        workflow_name,
                        e
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;
    use crate::ManualTrigger;

    const TIMEOUT: Duration = Duration::from_secs(1);

    #[tokio::test]
    async fn test_manager_manual_trigger() {
        let mut manager = TriggerManager::new(16);
        let trigger = Arc::new(ManualTrigger::new("wf1".to_string()));
        manager.register(
            "wf1".to_string(),
            vec![trigger.clone()],
            OverlapPolicy::Skip,
        );

        let mut rx = manager.subscribe().unwrap();
        manager.start().await.unwrap();

        trigger.fire();
        let event = timeout(TIMEOUT, rx.recv()).await.unwrap().unwrap();
        assert_eq!(event.workflow_name, "wf1");
        assert_eq!(event.trigger_type, "manual");
        assert_eq!(event.overlap_policy, OverlapPolicy::Skip);

        manager.stop().await;
    }

    #[tokio::test]
    async fn test_manager_multiple_triggers() {
        let mut manager = TriggerManager::new(16);
        let t1 = Arc::new(ManualTrigger::new("wf1".to_string()));
        let t2 = Arc::new(ManualTrigger::new("wf1".to_string()));
        manager.register(
            "wf1".to_string(),
            vec![t1.clone(), t2.clone()],
            OverlapPolicy::Queue,
        );

        let mut rx = manager.subscribe().unwrap();
        manager.start().await.unwrap();

        t1.fire();
        let event = timeout(TIMEOUT, rx.recv()).await.unwrap().unwrap();
        assert_eq!(event.workflow_name, "wf1");
        assert_eq!(event.overlap_policy, OverlapPolicy::Queue);

        t2.fire();
        let event = timeout(TIMEOUT, rx.recv()).await.unwrap().unwrap();
        assert_eq!(event.workflow_name, "wf1");

        manager.stop().await;
    }

    #[tokio::test]
    async fn test_manager_multiple_workflows() {
        let mut manager = TriggerManager::new(16);
        let t1 = Arc::new(ManualTrigger::new("wf1".to_string()));
        let t2 = Arc::new(ManualTrigger::new("wf2".to_string()));
        manager.register("wf1".to_string(), vec![t1.clone()], OverlapPolicy::Skip);
        manager.register(
            "wf2".to_string(),
            vec![t2.clone()],
            OverlapPolicy::AllowParallel,
        );

        let mut rx = manager.subscribe().unwrap();
        manager.start().await.unwrap();

        t2.fire();
        let event = timeout(TIMEOUT, rx.recv()).await.unwrap().unwrap();
        assert_eq!(event.workflow_name, "wf2");
        assert_eq!(event.overlap_policy, OverlapPolicy::AllowParallel);

        t1.fire();
        let event = timeout(TIMEOUT, rx.recv()).await.unwrap().unwrap();
        assert_eq!(event.workflow_name, "wf1");
        assert_eq!(event.overlap_policy, OverlapPolicy::Skip);

        manager.stop().await;
    }

    #[tokio::test]
    async fn test_manager_stop() {
        let mut manager = TriggerManager::new(16);
        let trigger = Arc::new(ManualTrigger::new("wf1".to_string()));
        manager.register(
            "wf1".to_string(),
            vec![trigger.clone()],
            OverlapPolicy::Skip,
        );

        let mut rx = manager.subscribe().unwrap();
        manager.start().await.unwrap();
        manager.stop().await;

        // After stop, firing should not produce events
        trigger.fire();
        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_err(), "should not receive events after stop");
    }

    #[tokio::test]
    async fn test_manager_overlap_policy_in_event() {
        let mut manager = TriggerManager::new(16);
        let trigger = Arc::new(ManualTrigger::new("wf1".to_string()));
        manager.register(
            "wf1".to_string(),
            vec![trigger.clone()],
            OverlapPolicy::AllowParallel,
        );

        let mut rx = manager.subscribe().unwrap();
        manager.start().await.unwrap();

        trigger.fire();
        let event = timeout(TIMEOUT, rx.recv()).await.unwrap().unwrap();
        assert_eq!(event.overlap_policy, OverlapPolicy::AllowParallel);

        manager.stop().await;
    }

    #[tokio::test]
    async fn test_manager_subscribe_once() {
        let mut manager = TriggerManager::new(16);
        let first = manager.subscribe();
        assert!(first.is_some());
        let second = manager.subscribe();
        assert!(second.is_none());
    }
}

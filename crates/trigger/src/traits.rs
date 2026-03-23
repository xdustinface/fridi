use std::time::SystemTime;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum TriggerError {
    #[error("failed to start trigger: {0}")]
    StartError(String),
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("trigger stopped")]
    Stopped,
}

#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub workflow_name: String,
    pub trigger_type: String,
    pub triggered_at: SystemTime,
    pub overlap_policy: OverlapPolicy,
}

#[async_trait]
pub trait Trigger: Send + Sync {
    async fn start(&self, tx: mpsc::Sender<TriggerEvent>) -> Result<(), TriggerError>;
    async fn stop(&self) -> Result<(), TriggerError>;
    fn trigger_type(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlapPolicy {
    #[default]
    Skip,
    Queue,
    AllowParallel,
}

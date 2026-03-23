use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("failed to spawn agent: {0}")]
    SpawnError(String),
    #[error("agent execution failed: {0}")]
    ExecutionError(String),
    #[error("agent was killed")]
    Killed,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub enum AgentOutput {
    Stdout(Vec<u8>),
    Exited(i32),
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub agent_type: String,
    pub skill: Option<String>,
    pub args: Option<String>,
    pub prompt: Option<String>,
    pub working_dir: Option<String>,
    pub env: HashMap<String, String>,
    pub context: HashMap<String, JsonValue>,
}

#[async_trait]
pub trait AgentHandle: Send + Sync {
    fn subscribe(&self) -> broadcast::Receiver<AgentOutput>;
    async fn write_stdin(&self, data: &[u8]) -> Result<(), AgentError>;
    async fn wait(&mut self) -> Result<i32, AgentError>;
    async fn kill(&mut self) -> Result<(), AgentError>;
    fn is_running(&self) -> bool;
    fn collected_output(&self) -> String;
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> &str;
    async fn spawn(&self, config: AgentConfig) -> Result<Box<dyn AgentHandle>, AgentError>;
}

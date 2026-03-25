use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use thiserror::Error;
use tokio::sync::broadcast;

use crate::pty::PtyResizer;

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

#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    pub agent_type: String,
    pub skill: Option<String>,
    pub args: Option<String>,
    pub prompt: Option<String>,
    pub working_dir: Option<String>,
    pub env: HashMap<String, String>,
    pub context: HashMap<String, JsonValue>,
    /// Claude session UUID; generated if not provided
    pub session_id: Option<String>,
    /// Whether to resume an existing session instead of starting fresh
    pub resume: bool,
    /// Human-readable name for the session
    pub session_name: Option<String>,
    /// Path to an MCP config JSON file to pass via `--mcp-config`
    pub mcp_config: Option<String>,
}

#[async_trait]
pub trait AgentHandle: Send + Sync {
    fn subscribe(&self) -> broadcast::Receiver<AgentOutput>;
    /// Returns the pre-subscribed receiver created before the reader thread
    /// started. Guarantees no output is lost between spawn and subscribe.
    fn take_initial_receiver(&mut self) -> Option<broadcast::Receiver<AgentOutput>>;
    async fn write_stdin(&self, data: &[u8]) -> Result<(), AgentError>;
    async fn wait(&mut self) -> Result<i32, AgentError>;
    async fn kill(&mut self) -> Result<(), AgentError>;
    fn is_running(&self) -> bool;
    fn collected_output(&self) -> String;
    fn session_id(&self) -> Option<&str>;
    /// Returns a handle for resizing the underlying PTY, if applicable.
    fn resizer(&self) -> Option<PtyResizer> { None }
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> &str;
    async fn spawn(&self, config: AgentConfig) -> Result<Box<dyn AgentHandle>, AgentError>;
}

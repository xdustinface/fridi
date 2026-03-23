use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Message sent between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: String,
    pub to: String,
    pub content: JsonValue,
    pub timestamp: SystemTime,
}

/// Agent status update broadcast to subscribers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub agent_id: String,
    pub status: String,
    pub detail: Option<String>,
    pub timestamp: SystemTime,
}

/// Result reported by an agent upon completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: String,
    pub result: JsonValue,
    pub timestamp: SystemTime,
}

/// Request to spawn a new agent, forwarded to the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub role: String,
    pub input: JsonValue,
    pub requested_by: String,
}

/// Snapshot of a running agent's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub role: String,
    pub status: String,
    pub parent: Option<String>,
    pub spawned_at: SystemTime,
}

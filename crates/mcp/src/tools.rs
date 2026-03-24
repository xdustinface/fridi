use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::types::{AgentInfo, AgentMessage};

/// Tool calls that agents can make via the MCP interface
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "params", rename_all = "snake_case")]
pub enum McpToolCall {
    SendMessage {
        to: String,
        content: JsonValue,
    },
    ReadMessages,
    UpdateStatus {
        status: String,
        detail: Option<String>,
    },
    ReportResult {
        result: JsonValue,
    },
    SpawnAgent {
        role: String,
        input: JsonValue,
    },
    ListAgents,
}

/// Results returned from MCP tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "data", rename_all = "snake_case")]
pub enum McpToolResult {
    SendMessage { success: bool },
    ReadMessages { messages: Vec<AgentMessage> },
    UpdateStatus { success: bool },
    ReportResult { success: bool },
    SpawnAgent { agent_id: String },
    ListAgents { agents: Vec<AgentInfo> },
}

/// Tool names as string constants for permission checking
pub const TOOL_SEND_MESSAGE: &str = "send_message";
pub const TOOL_READ_MESSAGES: &str = "read_messages";
pub const TOOL_UPDATE_STATUS: &str = "update_status";
pub const TOOL_REPORT_RESULT: &str = "report_result";
pub const TOOL_SPAWN_AGENT: &str = "spawn_agent";
pub const TOOL_LIST_AGENTS: &str = "list_agents";

/// Tools that require coordinator-level access
pub(crate) const COORDINATOR_TOOLS: &[&str] = &[TOOL_SPAWN_AGENT, TOOL_LIST_AGENTS];

impl McpToolCall {
    /// The canonical tool name for permission checking
    pub(crate) fn tool_name(&self) -> &'static str {
        match self {
            Self::SendMessage { .. } => TOOL_SEND_MESSAGE,
            Self::ReadMessages => TOOL_READ_MESSAGES,
            Self::UpdateStatus { .. } => TOOL_UPDATE_STATUS,
            Self::ReportResult { .. } => TOOL_REPORT_RESULT,
            Self::SpawnAgent { .. } => TOOL_SPAWN_AGENT,
            Self::ListAgents => TOOL_LIST_AGENTS,
        }
    }
}

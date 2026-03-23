use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use uuid::Uuid;

use crate::broker::MessageBroker;
use crate::tools::{COORDINATOR_TOOLS, McpToolCall, McpToolResult};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("agent '{0}' is not registered")]
    UnknownAgent(String),
    #[error("agent '{agent}' is not allowed to call tool '{tool}'")]
    PermissionDenied { agent: String, tool: String },
    #[error("broker error: {0}")]
    Broker(String),
}

/// MCP server that dispatches tool calls to the broker with permission checks.
pub struct McpServer {
    broker: Arc<MessageBroker>,
    /// Maps agent_id to the set of tool names it may call.
    allowed_tools: HashMap<String, Vec<String>>,
}

impl McpServer {
    pub fn new(broker: Arc<MessageBroker>) -> Self {
        Self {
            broker,
            allowed_tools: HashMap::new(),
        }
    }

    /// Register an agent and the tools it is allowed to call.
    ///
    /// Agents with the "coordinator" role automatically gain access to
    /// coordinator-only tools (`spawn_agent`, `list_agents`).
    pub fn register_agent(&mut self, id: String, role: &str, mut tools: Vec<String>) {
        if role == "coordinator" {
            for &tool in COORDINATOR_TOOLS {
                let name = tool.to_string();
                if !tools.contains(&name) {
                    tools.push(name);
                }
            }
        } else {
            tools.retain(|t| !COORDINATOR_TOOLS.contains(&t.as_str()));
        }
        self.broker
            .register_agent(id.clone(), role.to_string(), None);
        self.allowed_tools.insert(id, tools);
    }

    /// Dispatch a tool call from `agent_id`, checking permissions first.
    pub async fn handle_tool_call(
        &self,
        agent_id: &str,
        call: McpToolCall,
    ) -> Result<McpToolResult, ServerError> {
        let tools = self
            .allowed_tools
            .get(agent_id)
            .ok_or_else(|| ServerError::UnknownAgent(agent_id.to_string()))?;

        let tool_name = call.tool_name();
        if !tools.iter().any(|t| t == tool_name) {
            return Err(ServerError::PermissionDenied {
                agent: agent_id.to_string(),
                tool: tool_name.to_string(),
            });
        }

        match call {
            McpToolCall::SendMessage { to, content } => {
                let success = self.broker.send_message(agent_id, &to, content);
                Ok(McpToolResult::SendMessage { success })
            }
            McpToolCall::ReadMessages => {
                let messages = self.broker.read_messages(agent_id);
                Ok(McpToolResult::ReadMessages { messages })
            }
            McpToolCall::UpdateStatus { status, detail } => {
                let success = self.broker.update_status(agent_id, status, detail);
                Ok(McpToolResult::UpdateStatus { success })
            }
            McpToolCall::ReportResult { result } => {
                let success = self.broker.report_result(agent_id, result);
                Ok(McpToolResult::ReportResult { success })
            }
            McpToolCall::SpawnAgent { role, input } => {
                let agent_id_new = Uuid::new_v4().to_string();
                self.broker
                    .request_spawn(role, input, agent_id.to_string())
                    .await
                    .map_err(|e| ServerError::Broker(e.to_string()))?;
                Ok(McpToolResult::SpawnAgent {
                    agent_id: agent_id_new,
                })
            }
            McpToolCall::ListAgents => {
                let agents = self.broker.list_agents();
                Ok(McpToolResult::ListAgents { agents })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tools::*;

    fn all_basic_tools() -> Vec<String> {
        vec![
            TOOL_SEND_MESSAGE.into(),
            TOOL_READ_MESSAGES.into(),
            TOOL_UPDATE_STATUS.into(),
            TOOL_REPORT_RESULT.into(),
        ]
    }

    fn all_tools() -> Vec<String> {
        let mut tools = all_basic_tools();
        tools.push(TOOL_SPAWN_AGENT.into());
        tools.push(TOOL_LIST_AGENTS.into());
        tools
    }

    #[tokio::test]
    async fn test_permission_denied_for_non_coordinator() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        server.register_agent("dev1".into(), "developer", all_basic_tools());

        let result = server
            .handle_tool_call(
                "dev1",
                McpToolCall::SpawnAgent {
                    role: "tester".into(),
                    input: json!({}),
                },
            )
            .await;

        assert!(matches!(result, Err(ServerError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn test_coordinator_can_spawn_and_list() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        server.register_agent("coord".into(), "coordinator", all_tools());

        let result = server
            .handle_tool_call(
                "coord",
                McpToolCall::SpawnAgent {
                    role: "developer".into(),
                    input: json!({"task": "build"}),
                },
            )
            .await
            .unwrap();
        assert!(matches!(result, McpToolResult::SpawnAgent { .. }));

        let result = server
            .handle_tool_call("coord", McpToolCall::ListAgents)
            .await
            .unwrap();
        if let McpToolResult::ListAgents { agents } = result {
            assert!(!agents.is_empty());
        } else {
            panic!("expected ListAgents result");
        }
    }

    #[tokio::test]
    async fn test_coordinator_auto_gets_coordinator_tools() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        // Only pass basic tools; coordinator role should auto-add spawn/list
        server.register_agent("coord".into(), "coordinator", all_basic_tools());

        let result = server
            .handle_tool_call("coord", McpToolCall::ListAgents)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_non_coordinator_cannot_sneak_coordinator_tools() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        // Pass coordinator tool names explicitly for a non-coordinator role
        server.register_agent("dev1".into(), "developer", all_tools());

        let result = server
            .handle_tool_call("dev1", McpToolCall::ListAgents)
            .await;
        assert!(matches!(result, Err(ServerError::PermissionDenied { .. })));

        let result = server
            .handle_tool_call(
                "dev1",
                McpToolCall::SpawnAgent {
                    role: "tester".into(),
                    input: json!({}),
                },
            )
            .await;
        assert!(matches!(result, Err(ServerError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn test_unknown_agent() {
        let (broker, _rx) = MessageBroker::new();
        let server = McpServer::new(Arc::new(broker));

        let result = server
            .handle_tool_call("nobody", McpToolCall::ReadMessages)
            .await;
        assert!(matches!(result, Err(ServerError::UnknownAgent(_))));
    }

    #[tokio::test]
    async fn test_tool_dispatch_send_and_read() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        server.register_agent("a".into(), "dev", all_basic_tools());
        server.register_agent("b".into(), "dev", all_basic_tools());

        let result = server
            .handle_tool_call(
                "a",
                McpToolCall::SendMessage {
                    to: "b".into(),
                    content: json!("hello"),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            result,
            McpToolResult::SendMessage { success: true }
        ));

        let result = server
            .handle_tool_call("b", McpToolCall::ReadMessages)
            .await
            .unwrap();
        if let McpToolResult::ReadMessages { messages } = result {
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].content, json!("hello"));
        } else {
            panic!("expected ReadMessages result");
        }
    }

    #[tokio::test]
    async fn test_tool_dispatch_update_status() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        server.register_agent("a".into(), "dev", all_basic_tools());

        let result = server
            .handle_tool_call(
                "a",
                McpToolCall::UpdateStatus {
                    status: "busy".into(),
                    detail: Some("compiling".into()),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            result,
            McpToolResult::UpdateStatus { success: true }
        ));
    }

    #[tokio::test]
    async fn test_tool_dispatch_report_result() {
        let (broker, _rx) = MessageBroker::new();
        let mut server = McpServer::new(Arc::new(broker));
        server.register_agent("a".into(), "dev", all_basic_tools());

        let result = server
            .handle_tool_call(
                "a",
                McpToolCall::ReportResult {
                    result: json!({"ok": true}),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            result,
            McpToolResult::ReportResult { success: true }
        ));
    }
}

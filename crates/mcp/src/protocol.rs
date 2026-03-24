use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// Returns the MCP tool definitions for the `tools/list` response.
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "send_message",
            "description": "Send a message to another agent by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "The recipient agent's ID"
                    },
                    "content": {
                        "description": "The message content (any JSON value)"
                    }
                },
                "required": ["to", "content"]
            }
        }),
        json!({
            "name": "read_messages",
            "description": "Read and drain all pending messages for this agent.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "update_status",
            "description": "Update this agent's status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "The new status string"
                    },
                    "detail": {
                        "type": "string",
                        "description": "Optional detail about the status"
                    }
                },
                "required": ["status"]
            }
        }),
        json!({
            "name": "report_result",
            "description": "Report the final result for this agent's task and mark it completed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "result": {
                        "description": "The result data (any JSON value)"
                    }
                },
                "required": ["result"]
            }
        }),
        json!({
            "name": "spawn_agent",
            "description": "Spawn a new child agent (coordinator only).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "role": {
                        "type": "string",
                        "description": "Role for the new agent"
                    },
                    "input": {
                        "description": "Input data for the new agent (any JSON value)"
                    }
                },
                "required": ["role", "input"]
            }
        }),
        json!({
            "name": "list_agents",
            "description": "List all registered agents and their statuses (coordinator only).",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_returns_all_tools() {
        let schemas = tool_schemas();
        assert_eq!(schemas.len(), 6);

        let names: Vec<&str> = schemas
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"send_message"));
        assert!(names.contains(&"read_messages"));
        assert!(names.contains(&"update_status"));
        assert!(names.contains(&"report_result"));
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"list_agents"));
    }

    #[test]
    fn test_tool_schemas_have_input_schema() {
        for schema in tool_schemas() {
            assert!(
                schema.get("inputSchema").is_some(),
                "tool {} missing inputSchema",
                schema["name"]
            );
            assert_eq!(schema["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let resp = JsonRpcResponse::success(Some(json!(1)), json!({"ok": true}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let resp = JsonRpcResponse::error(Some(json!(1)), -32600, "Invalid request");
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32600);
    }

    #[test]
    fn test_jsonrpc_request_deserialize() {
        let json_str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.id, Some(json!(1)));
    }

    #[test]
    fn test_jsonrpc_request_deserialize_no_params() {
        let json_str = r#"{"jsonrpc":"2.0","id":2,"method":"initialize"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.method, "initialize");
        assert!(req.params.is_null());
    }
}

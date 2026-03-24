use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin, Stdout};
use tracing::{debug, warn};

use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

/// Stdio-based JSON-RPC transport for MCP communication.
///
/// Reads newline-delimited JSON requests from stdin and writes
/// newline-delimited JSON responses to stdout.
pub struct StdioTransport {
    reader: BufReader<Stdin>,
    writer: Stdout,
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()),
            writer: tokio::io::stdout(),
        }
    }

    /// Read one JSON-RPC request from stdin. Returns `None` on EOF.
    pub async fn read_request(&mut self) -> Result<Option<JsonRpcRequest>> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        debug!(raw = trimmed, "received request");
        let request: JsonRpcRequest = serde_json::from_str(trimmed)?;
        Ok(Some(request))
    }

    /// Write a JSON-RPC response to stdout (newline-delimited).
    pub async fn write_response(&mut self, response: &JsonRpcResponse) -> Result<()> {
        let mut data = serde_json::to_vec(response)?;
        data.push(b'\n');
        self.writer.write_all(&data).await?;
        self.writer.flush().await?;
        debug!("sent response");
        Ok(())
    }
}

/// Parse a `tools/call` request's params into an `McpToolCall`.
pub fn parse_tool_call(params: &serde_json::Value) -> Result<crate::tools::McpToolCall, String> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing 'name' in tool call params".to_string())?;

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    match name {
        "send_message" => {
            let to = arguments
                .get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing 'to' argument".to_string())?
                .to_string();
            let content = arguments
                .get("content")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Ok(crate::tools::McpToolCall::SendMessage { to, content })
        }
        "read_messages" => Ok(crate::tools::McpToolCall::ReadMessages),
        "update_status" => {
            let status = arguments
                .get("status")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing 'status' argument".to_string())?
                .to_string();
            let detail = arguments
                .get("detail")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Ok(crate::tools::McpToolCall::UpdateStatus { status, detail })
        }
        "report_result" => {
            let result = arguments
                .get("result")
                .cloned()
                .ok_or_else(|| "missing 'result' argument".to_string())?;
            Ok(crate::tools::McpToolCall::ReportResult { result })
        }
        "spawn_agent" => {
            let role = arguments
                .get("role")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing 'role' argument".to_string())?
                .to_string();
            let input = arguments
                .get("input")
                .cloned()
                .ok_or_else(|| "missing 'input' argument".to_string())?;
            Ok(crate::tools::McpToolCall::SpawnAgent { role, input })
        }
        "list_agents" => Ok(crate::tools::McpToolCall::ListAgents),
        other => {
            warn!(tool = other, "unknown tool");
            Err(format!("unknown tool: {other}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_parse_tool_call_send_message() {
        let params =
            json!({"name": "send_message", "arguments": {"to": "agent-2", "content": "hello"}});
        let call = parse_tool_call(&params).unwrap();
        assert!(matches!(
            call,
            crate::tools::McpToolCall::SendMessage { .. }
        ));
    }

    #[test]
    fn test_parse_tool_call_read_messages() {
        let params = json!({"name": "read_messages", "arguments": {}});
        let call = parse_tool_call(&params).unwrap();
        assert!(matches!(call, crate::tools::McpToolCall::ReadMessages));
    }

    #[test]
    fn test_parse_tool_call_update_status() {
        let params = json!({"name": "update_status", "arguments": {"status": "working", "detail": "step 1"}});
        let call = parse_tool_call(&params).unwrap();
        assert!(matches!(
            call,
            crate::tools::McpToolCall::UpdateStatus { .. }
        ));
    }

    #[test]
    fn test_parse_tool_call_report_result() {
        let params = json!({"name": "report_result", "arguments": {"result": {"done": true}}});
        let call = parse_tool_call(&params).unwrap();
        assert!(matches!(
            call,
            crate::tools::McpToolCall::ReportResult { .. }
        ));
    }

    #[test]
    fn test_parse_tool_call_spawn_agent() {
        let params = json!({"name": "spawn_agent", "arguments": {"role": "dev", "input": {}}});
        let call = parse_tool_call(&params).unwrap();
        assert!(matches!(call, crate::tools::McpToolCall::SpawnAgent { .. }));
    }

    #[test]
    fn test_parse_tool_call_list_agents() {
        let params = json!({"name": "list_agents"});
        let call = parse_tool_call(&params).unwrap();
        assert!(matches!(call, crate::tools::McpToolCall::ListAgents));
    }

    #[test]
    fn test_parse_tool_call_unknown() {
        let params = json!({"name": "unknown_tool"});
        let err = parse_tool_call(&params).unwrap_err();
        assert!(err.contains("unknown tool"));
    }

    #[test]
    fn test_parse_tool_call_missing_name() {
        let params = json!({"arguments": {}});
        let err = parse_tool_call(&params).unwrap_err();
        assert!(err.contains("missing 'name'"));
    }

    #[test]
    fn test_parse_tool_call_missing_required_arg() {
        let params = json!({"name": "send_message", "arguments": {"content": "hi"}});
        let err = parse_tool_call(&params).unwrap_err();
        assert!(err.contains("missing 'to'"));
    }
}

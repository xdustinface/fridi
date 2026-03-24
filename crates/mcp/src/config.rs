use serde_json::{Value as JsonValue, json};

/// Generate the `mcpServers` JSON block that gets passed to Claude via `--mcp-config`.
///
/// The generated config describes a single stdio-based MCP server at the given
/// socket path. Claude CLI reads this to discover available MCP tools.
pub fn generate_mcp_config(socket_path: &str, agent_id: &str) -> JsonValue {
    json!({
        "mcpServers": {
            "fridi": {
                "type": "stdio",
                "command": "fridi-mcp-server",
                "args": ["--socket", socket_path, "--agent-id", agent_id],
                "env": {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_generation() {
        let config = generate_mcp_config("/tmp/fridi.sock", "agent-1");

        assert!(config.get("mcpServers").is_some());
        let servers = &config["mcpServers"];
        assert!(servers.get("fridi").is_some());

        let fridi = &servers["fridi"];
        assert_eq!(fridi["type"], "stdio");
        assert_eq!(fridi["command"], "fridi-mcp-server");

        let args = fridi["args"].as_array().unwrap();
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "--socket");
        assert_eq!(args[1], "/tmp/fridi.sock");
        assert_eq!(args[2], "--agent-id");
        assert_eq!(args[3], "agent-1");
    }

    #[test]
    fn test_mcp_config_different_paths() {
        let config = generate_mcp_config("/var/run/mcp/agent-123.sock", "planner");
        let args = config["mcpServers"]["fridi"]["args"].as_array().unwrap();
        assert_eq!(args[1].as_str().unwrap(), "/var/run/mcp/agent-123.sock");
        assert_eq!(args[3].as_str().unwrap(), "planner");
    }
}

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use fridi_mcp::broker::MessageBroker;
use fridi_mcp::protocol::{self, JsonRpcResponse};
use fridi_mcp::server::McpServer;
use fridi_mcp::tools;
use fridi_mcp::transport::{self, StdioTransport};
use serde_json::json;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "fridi-mcp-server")]
struct Args {
    /// Unix socket path for IPC with the main fridi process (reserved for future use)
    #[arg(long)]
    socket: String,
    /// This agent's unique ID
    #[arg(long)]
    agent_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Logs go to stderr so stdout remains the pure MCP/JSON-RPC channel
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    info!(agent_id = %args.agent_id, socket = %args.socket, "starting MCP server");

    // For v1 we use a local in-process broker. Inter-agent messaging across
    // processes will be added once Unix socket IPC is implemented.
    let (broker, _spawn_rx) = MessageBroker::new();
    let broker = Arc::new(broker);
    let mut server = McpServer::new(Arc::clone(&broker));

    // Register this agent with all available tools (role-based filtering
    // happens in McpServer::register_agent)
    let all_tools: Vec<String> = vec![
        tools::TOOL_SEND_MESSAGE.into(),
        tools::TOOL_READ_MESSAGES.into(),
        tools::TOOL_UPDATE_STATUS.into(),
        tools::TOOL_REPORT_RESULT.into(),
        tools::TOOL_SPAWN_AGENT.into(),
        tools::TOOL_LIST_AGENTS.into(),
    ];
    server.register_agent(args.agent_id.clone(), "coordinator", all_tools);

    let mut stdio = StdioTransport::new();

    loop {
        let request = match stdio.read_request().await {
            Ok(Some(req)) => req,
            Ok(None) => {
                info!("stdin closed, shutting down");
                break;
            }
            Err(e) => {
                error!(error = %e, "failed to read request");
                continue;
            }
        };

        let response = match request.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                request.id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "fridi-mcp-server",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),
            "notifications/initialized" => {
                // This is a notification (no id), no response required
                continue;
            }
            "tools/list" => {
                let schemas = protocol::tool_schemas();
                JsonRpcResponse::success(request.id, json!({ "tools": schemas }))
            }
            "tools/call" => match transport::parse_tool_call(&request.params) {
                Ok(tool_call) => match server.handle_tool_call(&args.agent_id, tool_call).await {
                    Ok(result) => {
                        let content = serde_json::to_value(&result)
                            .unwrap_or_else(|e| json!({"error": e.to_string()}));
                        JsonRpcResponse::success(
                            request.id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": content.to_string()
                                }]
                            }),
                        )
                    }
                    Err(e) => JsonRpcResponse::error(request.id, -32000, e.to_string()),
                },
                Err(msg) => JsonRpcResponse::error(request.id, -32602, msg),
            },
            _ => JsonRpcResponse::error(request.id, -32601, "method not found"),
        };

        if let Err(e) = stdio.write_response(&response).await {
            error!(error = %e, "failed to write response");
        }
    }

    Ok(())
}

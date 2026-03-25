use std::path::PathBuf;
use std::sync::Arc;

use fridi_agent::claude::{ClaudeAgent, ClaudeAgentConfig};
use fridi_agent::definition::{AgentDefinition, TemplateContext, interpolate_prompt};
use fridi_agent::traits::{Agent, AgentConfig, AgentOutput};
use fridi_core::engine::{AgentSpawner, EngineEvent, StepResult, WorkflowContext};
use fridi_core::github::detect_repo_in;
use fridi_core::orchestrator::Orchestrator;
use fridi_core::schema::Step;
use fridi_mcp::config::generate_mcp_config;
use tokio::sync::{Mutex, broadcast};
use tracing::warn;

/// Bridges the engine's `AgentSpawner` trait with the orchestrator and Claude agent.
///
/// The forwarder task subscribes to the PTY broadcast channel and re-emits each
/// `AgentOutput::Stdout` chunk as an `EngineEvent::AgentOutput`.
///
/// For each step, it registers the agent with the orchestrator, writes an MCP config
/// file, and runs the Claude CLI session to completion.
pub struct OrchestratorSpawner {
    orchestrator: Arc<Mutex<Orchestrator>>,
    agent_config: ClaudeAgentConfig,
    mcp_socket_path: String,
    session_dir: PathBuf,
    agent_definitions: Vec<AgentDefinition>,
    event_tx: Option<broadcast::Sender<EngineEvent>>,
}

impl OrchestratorSpawner {
    pub fn new(
        orchestrator: Arc<Mutex<Orchestrator>>,
        agent_config: ClaudeAgentConfig,
        mcp_socket_path: String,
        session_dir: PathBuf,
        agent_definitions: Vec<AgentDefinition>,
    ) -> Self {
        Self {
            orchestrator,
            agent_config,
            mcp_socket_path,
            session_dir,
            agent_definitions,
            event_tx: None,
        }
    }

    pub fn with_event_sender(mut self, tx: broadcast::Sender<EngineEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }
}

impl AgentSpawner for OrchestratorSpawner {
    fn spawn_step(
        &self,
        step: &Step,
        context: &WorkflowContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<StepResult, String>> + Send>>
    {
        let orchestrator = self.orchestrator.clone();
        let agent_type = step.agent.clone().unwrap_or_else(|| "claude".to_string());
        let step_name = step.name.clone();
        let step_clone = step.clone();
        let ctx = context.as_agent_context();
        let agent_config = self.agent_config.clone();
        let mcp_socket_path = self.mcp_socket_path.clone();
        let definitions = self.agent_definitions.clone();
        let event_tx = self.event_tx.clone();

        let session_dir = self.session_dir.clone();

        // Resolve CWD once outside the async block for path resolution
        let cwd = std::env::current_dir().ok();

        Box::pin(async move {
            // Ensure session_dir is absolute so MCP config paths work regardless of CWD
            let session_dir = if session_dir.is_absolute() {
                session_dir
            } else if let Some(ref cwd) = cwd {
                cwd.join(&session_dir)
            } else {
                session_dir
            };
            let agent_id = {
                let mut orch = orchestrator.lock().await;
                orch.spawn_agent(&agent_type, serde_json::json!({"step": &step_name}), None)
                    .map_err(|e| e.to_string())?
            };

            let mcp_config_path = {
                let config = generate_mcp_config(&mcp_socket_path, &agent_id);
                let config_path = session_dir.join(format!("mcp-{}.json", agent_id));
                tokio::fs::create_dir_all(&session_dir)
                    .await
                    .map_err(|e| e.to_string())?;
                let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
                tokio::fs::write(&config_path, json)
                    .await
                    .map_err(|e| e.to_string())?;
                config_path
            };

            // Detect repo from git remote in the working directory instead of
            // relying on the orchestrator's stored value, which may come from
            // env vars or config that doesn't match the actual checkout.
            let repo = match cwd.as_ref() {
                Some(p) => {
                    let p = p.clone();
                    tokio::task::spawn_blocking(move || detect_repo_in(&p))
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_default()
                }
                None => String::new(),
            };

            let orch = orchestrator.lock().await;
            let session_id_str = orch.session().id.to_string();
            let session_dir_str = orch.session_dir().to_string_lossy().to_string();
            drop(orch);

            let mcp_socket_exists = std::path::Path::new(&mcp_socket_path).exists();

            let template_ctx = TemplateContext {
                repo,
                session_id: session_id_str,
                session_dir: session_dir_str,
                mcp_socket: mcp_socket_path,
            };

            let agent_def = definitions.iter().find(|d| d.name == agent_type);

            let prompt = step_clone
                .prompt
                .clone()
                .or_else(|| agent_def.map(|def| interpolate_prompt(&def.prompt, &template_ctx)));

            let args = step_clone.args.clone().or_else(|| {
                agent_def
                    .filter(|def| !def.default_args.is_empty())
                    .map(|def| def.default_args.join(" "))
            });

            // Only pass the MCP config if the socket file actually exists,
            // otherwise Claude CLI will fail trying to connect.
            let mcp_config = if mcp_socket_exists {
                Some(mcp_config_path.to_string_lossy().into())
            } else {
                None
            };

            let config = AgentConfig {
                agent_type: agent_type.clone(),
                skill: step_clone.skill.clone(),
                args,
                prompt,
                working_dir: cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
                env: Default::default(),
                context: ctx,
                session_id: None,
                resume: false,
                session_name: Some(step_name.clone()),
                mcp_config,
            };

            let agent = ClaudeAgent::new(agent_config);
            let mut handle = agent.spawn(config).await.map_err(|e| e.to_string())?;

            // Forward PTY output to engine events if a sender is available.
            // Use the pre-subscribed receiver to avoid missing output emitted
            // between spawn and subscribe.
            let forwarder = event_tx.map(|tx| {
                let mut rx = handle.take_initial_receiver().unwrap_or_else(|| {
                    warn!("initial PTY receiver already taken; falling back to subscribe (output may be lost)");
                    handle.subscribe()
                });
                let name = step_name.clone();
                tokio::spawn(async move {
                    while let Ok(output) = rx.recv().await {
                        if let AgentOutput::Stdout(data) = output {
                            let _ = tx.send(EngineEvent::AgentOutput {
                                step_name: name.clone(),
                                data,
                            });
                        }
                    }
                })
            });

            let exit_code = handle.wait().await.map_err(|e| e.to_string())?;

            // Wait for the forwarder to finish draining
            if let Some(handle) = forwarder {
                let _ = handle.await;
            }

            let output = handle.collected_output();

            let structured_output = serde_json::from_str(&output).ok();

            Ok(StepResult {
                exit_code,
                output,
                structured_output,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use fridi_agent::traits::AgentOutput;
    use fridi_core::engine::EngineEvent;
    use tokio::sync::broadcast;

    /// Tests the forwarder pattern used in `spawn_step`:
    /// PTY broadcast -> forwarder task -> EngineEvent broadcast.
    ///
    /// Creates both channels manually, spawns the forwarder, and verifies
    /// that `AgentOutput::Stdout` chunks arrive as `EngineEvent::AgentOutput`.
    /// The `take_initial_receiver` path is covered in `pty::tests`.
    #[tokio::test]
    async fn test_forwarder_relays_agent_output_to_engine_events() {
        // Simulate the PTY broadcast channel
        let (pty_tx, _) = broadcast::channel::<AgentOutput>(64);
        let mut pty_rx = pty_tx.subscribe();

        // Simulate the engine event channel
        let (engine_tx, mut engine_rx) = broadcast::channel::<EngineEvent>(64);

        let step_name = "test-step".to_string();

        // Spawn the forwarder task (mirrors the pattern in spawn_step)
        let tx = engine_tx.clone();
        let name = step_name.clone();
        let forwarder = tokio::spawn(async move {
            while let Ok(output) = pty_rx.recv().await {
                if let AgentOutput::Stdout(data) = output {
                    let _ = tx.send(EngineEvent::AgentOutput {
                        step_name: name.clone(),
                        data,
                    });
                }
            }
        });

        // Send some data through the PTY channel
        pty_tx
            .send(AgentOutput::Stdout(b"chunk 1".to_vec()))
            .unwrap();
        pty_tx
            .send(AgentOutput::Stdout(b"chunk 2".to_vec()))
            .unwrap();
        pty_tx.send(AgentOutput::Exited(0)).unwrap();

        // Drop the sender to close the channel and let the forwarder finish
        drop(pty_tx);
        let _ = forwarder.await;

        // Verify the engine received both chunks
        let mut chunks = Vec::new();
        while let Ok(event) = engine_rx.try_recv() {
            if let EngineEvent::AgentOutput { step_name, data } = event {
                assert_eq!(step_name, "test-step");
                chunks.push(data);
            }
        }

        assert_eq!(chunks.len(), 2, "expected 2 chunks, got {}", chunks.len());
        assert_eq!(chunks[0], b"chunk 1");
        assert_eq!(chunks[1], b"chunk 2");
    }

    /// Tests that the forwarder correctly filters out non-Stdout variants.
    #[tokio::test]
    async fn test_forwarder_ignores_exited_events() {
        let (pty_tx, _) = broadcast::channel::<AgentOutput>(64);
        let mut pty_rx = pty_tx.subscribe();
        let (engine_tx, mut engine_rx) = broadcast::channel::<EngineEvent>(64);

        let tx = engine_tx.clone();
        let forwarder = tokio::spawn(async move {
            while let Ok(output) = pty_rx.recv().await {
                if let AgentOutput::Stdout(data) = output {
                    let _ = tx.send(EngineEvent::AgentOutput {
                        step_name: "s".to_string(),
                        data,
                    });
                }
            }
        });

        // Only send Exited — no Stdout
        pty_tx.send(AgentOutput::Exited(0)).unwrap();
        drop(pty_tx);
        let _ = forwarder.await;

        // Should have no AgentOutput events
        let mut count = 0;
        while let Ok(event) = engine_rx.try_recv() {
            if matches!(event, EngineEvent::AgentOutput { .. }) {
                count += 1;
            }
        }
        assert_eq!(count, 0, "forwarder should not relay Exited events");
    }
}

use std::sync::Arc;

use fridi_agent::claude::ClaudeAgent;
use fridi_agent::traits::{Agent, AgentConfig};
use fridi_core::engine::{AgentSpawner, StepResult, WorkflowContext};
use fridi_core::orchestrator::Orchestrator;
use fridi_core::schema::Step;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Bridges the Engine to real Claude agent execution via the Orchestrator.
///
/// Implements `AgentSpawner` so the engine can drive workflow steps through
/// actual Claude CLI sessions while the orchestrator handles bookkeeping.
pub struct OrchestratorSpawner {
    orchestrator: Arc<Mutex<Orchestrator>>,
    claude_agent: ClaudeAgent,
    mcp_socket_path: String,
}

impl OrchestratorSpawner {
    pub fn new(
        orchestrator: Arc<Mutex<Orchestrator>>,
        claude_agent: ClaudeAgent,
        mcp_socket_path: String,
    ) -> Self {
        Self {
            orchestrator,
            claude_agent,
            mcp_socket_path,
        }
    }
}

impl AgentSpawner for OrchestratorSpawner {
    fn spawn_step(
        &self,
        step: &Step,
        context: &WorkflowContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<StepResult, String>> + Send>>
    {
        let orchestrator = Arc::clone(&self.orchestrator);
        let step = step.clone();
        let agent_context = context.as_agent_context();
        let mcp_socket_path = self.mcp_socket_path.clone();
        let claude_agent = self.claude_agent.clone();

        Box::pin(async move {
            let role = step.agent.as_deref().unwrap_or("claude");

            // Register the agent in the orchestrator for session bookkeeping
            let (agent_id, mcp_config_dir) = {
                let mut orch = orchestrator.lock().await;
                let id = orch
                    .spawn_agent(role, serde_json::json!({}), None)
                    .map_err(|e| format!("orchestrator spawn failed: {e}"))?;
                let dir = orch.session_dir().join("mcp");
                (id, dir)
            };

            // Write MCP config so the agent can communicate back
            let mcp_config = fridi_mcp::config::generate_mcp_config(&mcp_socket_path, &agent_id);
            let mcp_config_path = mcp_config_dir.join(format!("{agent_id}.json"));

            std::fs::create_dir_all(&mcp_config_dir)
                .map_err(|e| format!("failed to create MCP config dir: {e}"))?;
            std::fs::write(
                &mcp_config_path,
                serde_json::to_string_pretty(&mcp_config)
                    .map_err(|e| format!("failed to serialize MCP config: {e}"))?,
            )
            .map_err(|e| format!("failed to write MCP config: {e}"))?;

            debug!(agent_id = %agent_id, path = %mcp_config_path.display(), "wrote MCP config");

            // Build the agent config from step fields
            let config = AgentConfig {
                agent_type: "claude".into(),
                skill: step.skill.clone(),
                args: step.args.clone(),
                prompt: step.prompt.clone(),
                working_dir: None,
                env: Default::default(),
                context: agent_context,
                session_id: None,
                resume: false,
                session_name: Some(format!("{}-{}", step.name, agent_id)),
                mcp_config: Some(mcp_config_path.to_string_lossy().into_owned()),
            };

            // Spawn the Claude CLI process
            let mut handle = claude_agent
                .spawn(config)
                .await
                .map_err(|e| format!("agent spawn failed: {e}"))?;

            // Record the claude session ID in the orchestrator
            if let Some(session_id) = handle.session_id() {
                let mut orch = orchestrator.lock().await;
                if let Some(entry) = orch.session_mut().agents.get_mut(&agent_id) {
                    entry.claude_session_id = Some(session_id.to_string());
                    entry.status = "running".into();
                }
            }

            info!(agent_id = %agent_id, step = %step.name, "agent spawned, waiting for completion");

            // Wait for completion and collect output
            let exit_code = handle
                .wait()
                .await
                .map_err(|e| format!("agent wait failed: {e}"))?;

            let output = handle.collected_output();

            // Update orchestrator status
            {
                let mut orch = orchestrator.lock().await;
                if let Some(entry) = orch.session_mut().agents.get_mut(&agent_id) {
                    entry.status = if exit_code == 0 {
                        "completed".into()
                    } else {
                        format!("failed (exit {})", exit_code)
                    };
                }
            }

            // Try to parse structured JSON from the output
            let structured_output = serde_json::from_str::<serde_json::Value>(&output).ok();

            info!(
                agent_id = %agent_id,
                step = %step.name,
                exit_code,
                "agent completed"
            );

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
    use fridi_agent::claude::ClaudeAgentConfig;
    use fridi_core::orchestrator::AgentRoleConfig;
    use fridi_core::session::{Session, SessionId, SessionStore};
    use tempfile::TempDir;

    use super::*;

    fn test_role_configs() -> Vec<AgentRoleConfig> {
        vec![AgentRoleConfig {
            name: "claude".into(),
            description: "Default Claude agent".into(),
            prompt: "You are a helpful assistant".into(),
            permissions: None,
            allowed_tools: vec![],
            spawnable_roles: vec![],
            default_args: vec![],
        }]
    }

    fn test_session() -> Session {
        Session::new(
            SessionId::new("test-wf"),
            "test-wf".into(),
            "test.yaml".into(),
            Some("owner/repo".into()),
        )
    }

    fn make_spawner(tmp: &TempDir) -> OrchestratorSpawner {
        let store = SessionStore::new(tmp.path());
        let session = test_session();
        let session_dir = tmp.path().join(session.id.as_str());

        let orchestrator = Orchestrator::new(
            session,
            store,
            test_role_configs(),
            "owner/repo",
            session_dir,
        );

        let claude_agent = ClaudeAgent::new(ClaudeAgentConfig {
            binary: "echo".into(),
            default_args: vec![],
        });

        OrchestratorSpawner::new(
            Arc::new(Mutex::new(orchestrator)),
            claude_agent,
            "/tmp/test.sock".into(),
        )
    }

    #[test]
    fn test_orchestrator_spawner_new() {
        let tmp = TempDir::new().unwrap();
        let spawner = make_spawner(&tmp);
        assert_eq!(spawner.mcp_socket_path, "/tmp/test.sock");
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_spawn_step_builds_correct_config() {
        let tmp = TempDir::new().unwrap();
        let spawner = make_spawner(&tmp);

        let step = Step {
            name: "build".into(),
            agent: Some("claude".into()),
            skill: Some("coding".into()),
            args: None,
            prompt: Some("Build the project".into()),
            depends_on: vec![],
            condition: None,
            for_each: None,
            outputs: vec![],
            on_failure: None,
            retry: None,
            step_type: None,
            message: None,
        };
        let context = WorkflowContext::default();

        let result = spawner.spawn_step(&step, &context).await;
        assert!(result.is_ok(), "spawn_step failed: {:?}", result.err());

        let result = result.unwrap();
        assert_eq!(result.exit_code, 0);

        // Verify the orchestrator tracked the agent
        let orch = spawner.orchestrator.lock().await;
        assert_eq!(orch.session().agents.len(), 1);
        let entry = &orch.session().agents["claude-1"];
        assert_eq!(entry.role, "claude");
        assert_eq!(entry.status, "completed");
        assert!(entry.claude_session_id.is_some());

        // Verify MCP config was written under the session directory
        let session_id = orch.session().id.as_str();
        let mcp_path = tmp.path().join(format!("{session_id}/mcp/claude-1.json"));
        assert!(mcp_path.exists(), "MCP config file should exist");
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_spawn_step_default_role() {
        let tmp = TempDir::new().unwrap();
        let spawner = make_spawner(&tmp);

        let step = Step {
            name: "default-role".into(),
            agent: None,
            skill: None,
            args: None,
            prompt: Some("Do something".into()),
            depends_on: vec![],
            condition: None,
            for_each: None,
            outputs: vec![],
            on_failure: None,
            retry: None,
            step_type: None,
            message: None,
        };
        let context = WorkflowContext::default();

        let result = spawner.spawn_step(&step, &context).await;
        assert!(result.is_ok());

        let orch = spawner.orchestrator.lock().await;
        let entry = &orch.session().agents["claude-1"];
        assert_eq!(entry.role, "claude");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_spawn_step_unknown_role_fails() {
        let tmp = TempDir::new().unwrap();
        let spawner = make_spawner(&tmp);

        let step = Step {
            name: "bad-role".into(),
            agent: Some("nonexistent".into()),
            skill: None,
            args: None,
            prompt: None,
            depends_on: vec![],
            condition: None,
            for_each: None,
            outputs: vec![],
            on_failure: None,
            retry: None,
            step_type: None,
            message: None,
        };
        let context = WorkflowContext::default();

        let result = spawner.spawn_step(&step, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("orchestrator spawn failed"));
    }
}

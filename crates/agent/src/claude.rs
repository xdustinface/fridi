use async_trait::async_trait;
use portable_pty::CommandBuilder;
use tokio::sync::broadcast;
use tracing::info;
use uuid::Uuid;

use crate::pty::PtyProcess;
use crate::traits::{Agent, AgentConfig, AgentError, AgentHandle, AgentOutput};

/// Configuration for the Claude CLI agent
#[derive(Debug, Clone)]
pub struct ClaudeAgentConfig {
    /// Path to the claude binary (defaults to "claude")
    pub binary: String,
    /// Default arguments to pass to every invocation
    pub default_args: Vec<String>,
}

impl Default for ClaudeAgentConfig {
    fn default() -> Self {
        Self {
            binary: "claude".to_string(),
            default_args: Vec::new(),
        }
    }
}

/// Agent implementation that spawns Claude Code CLI sessions
#[derive(Clone)]
pub struct ClaudeAgent {
    config: ClaudeAgentConfig,
}

impl ClaudeAgent {
    pub fn new(config: ClaudeAgentConfig) -> Self { Self { config } }
}

impl Default for ClaudeAgent {
    fn default() -> Self { Self::new(ClaudeAgentConfig::default()) }
}

#[async_trait]
impl Agent for ClaudeAgent {
    fn agent_type(&self) -> &str { "claude" }

    async fn spawn(&self, config: AgentConfig) -> Result<Box<dyn AgentHandle>, AgentError> {
        let mut cmd = CommandBuilder::new(&self.config.binary);

        for arg in &self.config.default_args {
            cmd.arg(arg);
        }

        // Resolve the session ID: use provided or generate a new one
        let session_id = config
            .session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        if config.resume {
            cmd.arg("--resume");
            cmd.arg(&session_id);
        } else {
            cmd.arg("--session-id");
            cmd.arg(&session_id);
        }

        if let Some(name) = &config.session_name {
            cmd.arg("--name");
            cmd.arg(name);
        }

        if let Some(skill) = &config.skill {
            cmd.arg("--skill");
            cmd.arg(skill);
        }

        if let Some(args) = &config.args {
            for arg in args.split_whitespace() {
                cmd.arg(arg);
            }
        }

        if let Some(prompt) = &config.prompt {
            cmd.arg("--print");
            cmd.arg(prompt);
        }

        if let Some(mcp_config_path) = &config.mcp_config {
            cmd.arg("--mcp-config");
            cmd.arg(mcp_config_path);
        }

        if let Some(dir) = &config.working_dir {
            cmd.cwd(dir);
        }

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        if !config.context.is_empty() {
            if let Ok(ctx_json) = serde_json::to_string(&config.context) {
                cmd.env("FRIDI_CONTEXT", &ctx_json);
            }
        }

        info!("spawning Claude CLI: {:?}", cmd);

        let pty = PtyProcess::spawn(cmd)?;
        Ok(Box::new(ClaudeAgentHandle { pty, session_id }))
    }
}

/// Handle to a running Claude CLI session
struct ClaudeAgentHandle {
    pty: PtyProcess,
    session_id: String,
}

#[async_trait]
impl AgentHandle for ClaudeAgentHandle {
    fn subscribe(&self) -> broadcast::Receiver<AgentOutput> { self.pty.subscribe() }

    async fn write_stdin(&self, data: &[u8]) -> Result<(), AgentError> {
        self.pty.write_stdin(data).await
    }

    async fn wait(&mut self) -> Result<i32, AgentError> { self.pty.wait().await }

    async fn kill(&mut self) -> Result<(), AgentError> { self.pty.kill().await }

    fn is_running(&self) -> bool { self.pty.is_running() }

    fn collected_output(&self) -> String { self.pty.collected_output_sync() }

    fn session_id(&self) -> Option<&str> { Some(&self.session_id) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_agent_default_config() {
        let agent = ClaudeAgent::default();
        assert_eq!(agent.agent_type(), "claude");
        assert_eq!(agent.config.binary, "claude");
        assert!(agent.config.default_args.is_empty());
    }

    #[test]
    fn test_claude_agent_custom_config() {
        let config = ClaudeAgentConfig {
            binary: "/usr/local/bin/claude".to_string(),
            default_args: vec!["--verbose".to_string()],
        };
        let agent = ClaudeAgent::new(config);
        assert_eq!(agent.config.binary, "/usr/local/bin/claude");
        assert_eq!(agent.config.default_args.len(), 1);
    }

    /// Helper that spawns a ClaudeAgent using `echo` as the binary so we can
    /// inspect the arguments it passes and the returned handle.
    async fn spawn_echo_agent(config: AgentConfig) -> Box<dyn AgentHandle> {
        let agent = ClaudeAgent::new(ClaudeAgentConfig {
            binary: "echo".to_string(),
            default_args: Vec::new(),
        });
        agent.spawn(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_claude_agent_generates_session_id() {
        let config = AgentConfig {
            agent_type: "claude".into(),
            ..Default::default()
        };
        let handle = spawn_echo_agent(config).await;
        let sid = handle.session_id().expect("should have a session id");
        assert!(!sid.is_empty());
        // Should be a valid UUID v4
        assert!(Uuid::parse_str(sid).is_ok(), "not a valid UUID: {sid}");
    }

    #[tokio::test]
    async fn test_claude_agent_uses_provided_session_id() {
        let expected_id = "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d";
        let config = AgentConfig {
            agent_type: "claude".into(),
            session_id: Some(expected_id.to_string()),
            ..Default::default()
        };
        let handle = spawn_echo_agent(config).await;
        assert_eq!(handle.session_id(), Some(expected_id));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_claude_agent_resume_mode() {
        let sid = Uuid::new_v4().to_string();
        let config = AgentConfig {
            agent_type: "claude".into(),
            session_id: Some(sid.clone()),
            resume: true,
            ..Default::default()
        };
        let mut handle = spawn_echo_agent(config).await;
        assert_eq!(handle.session_id(), Some(sid.as_str()));

        // The echo binary prints its arguments, so we can verify --resume was passed
        let _ = handle.wait().await;
        let output = handle.collected_output();
        assert!(
            output.contains("--resume"),
            "expected --resume in output: {output}"
        );
        assert!(
            !output.contains("--session-id"),
            "should not contain --session-id in resume mode: {output}"
        );
    }

    #[tokio::test]
    async fn test_session_id_is_valid_uuid() {
        // Spawn multiple times and verify each generated ID is a valid UUID
        for _ in 0..5 {
            let config = AgentConfig {
                agent_type: "claude".into(),
                ..Default::default()
            };
            let handle = spawn_echo_agent(config).await;
            let sid = handle.session_id().unwrap();
            assert!(
                Uuid::parse_str(sid).is_ok(),
                "generated session id is not a valid UUID: {sid}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_claude_agent_mcp_config() {
        let config = AgentConfig {
            agent_type: "claude".into(),
            mcp_config: Some("/tmp/mcp-config.json".to_string()),
            ..Default::default()
        };
        let mut handle = spawn_echo_agent(config).await;
        let _ = handle.wait().await;
        let output = handle.collected_output();
        assert!(
            output.contains("--mcp-config"),
            "expected --mcp-config in output: {output}"
        );
        assert!(
            output.contains("/tmp/mcp-config.json"),
            "expected mcp config path in output: {output}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_claude_agent_session_name() {
        let config = AgentConfig {
            agent_type: "claude".into(),
            session_name: Some("my-workflow-run".to_string()),
            ..Default::default()
        };
        let mut handle = spawn_echo_agent(config).await;
        let _ = handle.wait().await;
        let output = handle.collected_output();
        assert!(
            output.contains("--name"),
            "expected --name in output: {output}"
        );
        assert!(
            output.contains("my-workflow-run"),
            "expected session name in output: {output}"
        );
    }
}

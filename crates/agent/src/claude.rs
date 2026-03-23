use async_trait::async_trait;
use portable_pty::CommandBuilder;
use tokio::sync::broadcast;
use tracing::info;

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
pub struct ClaudeAgent {
    config: ClaudeAgentConfig,
}

impl ClaudeAgent {
    pub fn new(config: ClaudeAgentConfig) -> Self {
        Self { config }
    }
}

impl Default for ClaudeAgent {
    fn default() -> Self {
        Self::new(ClaudeAgentConfig::default())
    }
}

#[async_trait]
impl Agent for ClaudeAgent {
    fn agent_type(&self) -> &str {
        "claude"
    }

    async fn spawn(&self, config: AgentConfig) -> Result<Box<dyn AgentHandle>, AgentError> {
        let mut cmd = CommandBuilder::new(&self.config.binary);

        for arg in &self.config.default_args {
            cmd.arg(arg);
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

        if let Some(dir) = &config.working_dir {
            cmd.cwd(dir);
        }

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        if !config.context.is_empty() {
            if let Ok(ctx_json) = serde_json::to_string(&config.context) {
                cmd.env("CONDUCTOR_CONTEXT", &ctx_json);
            }
        }

        info!("spawning Claude CLI: {:?}", cmd);

        let pty = PtyProcess::spawn(cmd)?;
        Ok(Box::new(ClaudeAgentHandle { pty }))
    }
}

/// Handle to a running Claude CLI session
struct ClaudeAgentHandle {
    pty: PtyProcess,
}

#[async_trait]
impl AgentHandle for ClaudeAgentHandle {
    fn subscribe(&self) -> broadcast::Receiver<AgentOutput> {
        self.pty.subscribe()
    }

    async fn write_stdin(&self, data: &[u8]) -> Result<(), AgentError> {
        self.pty.write_stdin(data).await
    }

    async fn wait(&mut self) -> Result<i32, AgentError> {
        self.pty.wait().await
    }

    async fn kill(&mut self) -> Result<(), AgentError> {
        self.pty.kill().await
    }

    fn is_running(&self) -> bool {
        self.pty.is_running()
    }

    fn collected_output(&self) -> String {
        self.pty.collected_output_sync()
    }
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
}

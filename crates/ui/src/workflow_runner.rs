use std::path::PathBuf;
use std::sync::Arc;

use fridi_agent::claude::ClaudeAgentConfig;
use fridi_agent::definition::load_agent_definitions;
use fridi_cli::spawner::OrchestratorSpawner;
use fridi_core::dag::WorkflowDag;
use fridi_core::engine::{Engine, EngineEvent};
use fridi_core::orchestrator::Orchestrator;
use fridi_core::schema::Workflow;
use fridi_core::session::{Session, SessionStore};
use tokio::sync::{Mutex, broadcast};
use tracing::{error, info};

/// Encapsulates starting a workflow execution in a background tokio task.
#[derive(Clone)]
pub(crate) struct WorkflowRunner {
    agents_dir: PathBuf,
    sessions_dir: PathBuf,
}

impl WorkflowRunner {
    pub(crate) fn new(agents_dir: PathBuf, sessions_dir: PathBuf) -> Self {
        Self {
            agents_dir,
            sessions_dir,
        }
    }

    /// Start a workflow execution in a background task.
    /// Returns a broadcast receiver for engine events.
    pub(crate) async fn start(
        &self,
        workflow: Workflow,
        session: Session,
        store: SessionStore,
    ) -> anyhow::Result<broadcast::Receiver<EngineEvent>> {
        let dag = WorkflowDag::from_workflow(&workflow)?;
        info!(
            "built DAG for '{}' with {} steps",
            workflow.name,
            dag.step_count()
        );

        let repo = workflow
            .config
            .repo
            .clone()
            .or_else(|| session.repo.clone())
            .unwrap_or_else(|| {
                tracing::warn!("no repo configured in workflow or session, defaulting to empty");
                String::new()
            });

        let session_dir = self.sessions_dir.join(session.id.as_str());
        let orchestrator = Orchestrator::from_agents_dir(
            session,
            store,
            &self.agents_dir,
            &repo,
            session_dir.clone(),
        )?;
        let orchestrator = Arc::new(Mutex::new(orchestrator));

        let (engine, event_rx) = Engine::new();

        let agent_definitions = if self.agents_dir.exists() {
            load_agent_definitions(&self.agents_dir).unwrap_or_else(|e| {
                tracing::warn!(
                    "failed to load agent definitions from {:?}: {}",
                    self.agents_dir,
                    e
                );
                Vec::new()
            })
        } else {
            Vec::new()
        };

        let mcp_socket_path = session_dir.join("mcp.sock").to_string_lossy().into_owned();
        let spawner = OrchestratorSpawner::new(
            Arc::clone(&orchestrator),
            ClaudeAgentConfig::default(),
            mcp_socket_path,
            session_dir,
            agent_definitions,
        );

        let workflow_name = workflow.name.clone();
        tokio::spawn(async move {
            info!("starting engine execution for '{}'", workflow_name);
            if let Err(e) = engine.execute(&workflow, &dag, &spawner).await {
                error!("workflow '{}' execution failed: {}", workflow_name, e);
            }
        });

        Ok(event_rx)
    }
}

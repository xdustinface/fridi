use std::path::PathBuf;
use std::sync::Arc;

use fridi_agent::claude::ClaudeAgentConfig;
use fridi_agent::definition::load_agent_definitions;
use fridi_cli::spawner::OrchestratorSpawner;
use fridi_core::dag::WorkflowDag;
use fridi_core::engine::{Engine, EngineEvent};
use fridi_core::orchestrator::Orchestrator;
use fridi_core::schema::{Step, Workflow, WorkflowConfig};
use fridi_core::session::{Session, SessionStore};
use tokio::sync::{Mutex, broadcast};
use tracing::{error, info};

use crate::components::session_creator::SessionSource;

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
            .unwrap_or_default();

        let session_dir = self.sessions_dir.join(session.id.as_str());
        let orchestrator = Orchestrator::from_agents_dir(
            session,
            store,
            &self.agents_dir,
            &repo,
            session_dir.clone(),
        )?;
        let orchestrator = Arc::new(Mutex::new(orchestrator));

        let (engine, _initial_rx) = Engine::new();
        // Subscribe before spawning execution so no events are lost
        let event_rx = engine.subscribe();

        let agent_definitions = if self.agents_dir.exists() {
            load_agent_definitions(&self.agents_dir).unwrap_or_default()
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

/// Build a single-step workflow from the user's session source.
pub(crate) fn workflow_from_source(source: &SessionSource, repo: &str) -> Workflow {
    let default_config = || WorkflowConfig {
        repo: Some(repo.into()),
        ..Default::default()
    };

    match source {
        SessionSource::Prompt { text } => Workflow {
            name: "prompt".into(),
            description: Some("User prompt".into()),
            config: default_config(),
            triggers: vec![],
            notifications: Default::default(),
            steps: vec![Step {
                name: "execute".into(),
                agent: Some("claude".into()),
                prompt: Some(text.clone()),
                ..default_step()
            }],
        },
        SessionSource::Issue { number, title } => Workflow {
            name: format!("issue-{number}"),
            description: Some(format!("Issue #{number}: {title}")),
            config: default_config(),
            triggers: vec![],
            notifications: Default::default(),
            steps: vec![Step {
                name: "work-on-issue".into(),
                agent: Some("claude".into()),
                prompt: Some(format!(
                    "Work on issue #{number} in repo {repo}:\n\n\
                     Title: {title}\n\n\
                     Analyze the issue, plan the implementation, and execute it."
                )),
                ..default_step()
            }],
        },
        SessionSource::PR {
            number,
            title,
            head_ref,
        } => Workflow {
            name: format!("pr-{number}"),
            description: Some(format!("PR #{number}: {title}")),
            config: default_config(),
            triggers: vec![],
            notifications: Default::default(),
            steps: vec![Step {
                name: "work-on-pr".into(),
                agent: Some("claude".into()),
                prompt: Some(format!(
                    "Work on PR #{number} ({title}) in repo {repo}:\n\n\
                     Branch: {head_ref}\n\n\
                     Review the PR, fix any issues, and ensure CI passes."
                )),
                ..default_step()
            }],
        },
    }
}

fn default_step() -> Step {
    Step {
        name: String::new(),
        agent: None,
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
    }
}

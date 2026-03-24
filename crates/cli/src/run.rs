use std::path::PathBuf;
use std::sync::Arc;

use fridi_agent::claude::ClaudeAgentConfig;
use fridi_agent::definition::load_agent_definitions;
use fridi_core::dag::WorkflowDag;
use fridi_core::engine::{Engine, EngineEvent, StepStatus};
use fridi_core::orchestrator::Orchestrator;
use fridi_core::schema::Workflow;
use fridi_core::session::{Session, SessionId, SessionStore};
use tokio::sync::Mutex;

use crate::spawner::OrchestratorSpawner;

pub(crate) async fn execute(
    workflow_path: PathBuf,
    repo: Option<String>,
    agents_dir: PathBuf,
    sessions_dir: PathBuf,
) -> anyhow::Result<()> {
    let workflow = Workflow::from_file(&workflow_path)?;
    tracing::info!("loaded workflow: {}", workflow.name);

    let dag = WorkflowDag::from_workflow(&workflow)?;
    tracing::info!("built DAG with {} steps", dag.step_count());

    let session_id = SessionId::new(&workflow.name);
    let repo_name = repo.or_else(|| workflow.config.repo.clone());
    let session = Session::new(
        session_id.clone(),
        workflow.name.clone(),
        workflow_path.to_string_lossy().into(),
        repo_name.clone(),
    );
    let store = SessionStore::new(&sessions_dir);
    store.save(&session)?;
    tracing::info!("created session: {}", session_id);

    let session_dir = sessions_dir.join(session_id.as_str());
    let orchestrator = Orchestrator::from_agents_dir(
        session,
        store,
        &agents_dir,
        repo_name.as_deref().unwrap_or(""),
        session_dir.clone(),
    )?;
    let orchestrator = Arc::new(Mutex::new(orchestrator));

    let (engine, mut event_rx) = Engine::new();

    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match &event {
                EngineEvent::WorkflowStarted { workflow_name } => {
                    println!("[workflow] started: {}", workflow_name);
                }
                EngineEvent::StepStatusChanged { step_name, status } => {
                    let status_str = match status {
                        StepStatus::Pending => "pending",
                        StepStatus::Running => "running",
                        StepStatus::Completed => "completed",
                        StepStatus::Failed(reason) => {
                            println!("[step] {} -> failed: {}", step_name, reason);
                            continue;
                        }
                        StepStatus::Skipped => "skipped",
                    };
                    println!("[step] {} -> {}", step_name, status_str);
                }
                EngineEvent::WorkflowCompleted { workflow_name } => {
                    println!("[workflow] completed: {}", workflow_name);
                }
                EngineEvent::WorkflowFailed {
                    workflow_name,
                    reason,
                } => {
                    println!("[workflow] failed: {} -- {}", workflow_name, reason);
                }
                EngineEvent::NotificationRequired {
                    step_name, message, ..
                } => {
                    println!("[notify] {} -- {}", step_name, message);
                }
                EngineEvent::AgentOutput { step_name, data } => {
                    tracing::trace!(
                        step = %step_name,
                        bytes = data.len(),
                        "agent output received"
                    );
                }
            }
        }
    });

    let agent_definitions = if agents_dir.exists() {
        load_agent_definitions(&agents_dir).unwrap_or_default()
    } else {
        Vec::new()
    };

    let mcp_socket_path = session_dir.join("mcp.sock").to_string_lossy().into_owned();
    let spawner = OrchestratorSpawner::new(
        orchestrator,
        ClaudeAgentConfig::default(),
        mcp_socket_path,
        session_dir,
        agent_definitions,
    )
    .with_event_sender(engine.event_sender());

    println!(
        "running workflow: {} (session: {})",
        workflow.name, session_id
    );
    let result = engine.execute(&workflow, &dag, &spawner).await;

    match result {
        Ok(_ctx) => {
            println!("workflow completed successfully.");
            Ok(())
        }
        Err(e) => {
            eprintln!("workflow failed: {}", e);
            std::process::exit(1);
        }
    }
}

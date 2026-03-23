use std::collections::HashMap;
use std::path::PathBuf;

use conductor_core::engine::StepStatus;
use conductor_core::schema::Workflow;

/// Represents a loaded workflow and its current run state
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkflowState {
    pub(crate) workflow: Workflow,
    pub(crate) file_path: PathBuf,
    pub(crate) run_state: RunState,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum RunState {
    #[default]
    Idle,
    Running {
        step_statuses: HashMap<String, StepStatus>,
        started_at: std::time::Instant,
    },
    Completed,
    Failed(String),
}

/// Load all workflow YAML files from a directory
pub(crate) fn load_workflows(dir: &std::path::Path) -> Vec<WorkflowState> {
    let mut workflows = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Ok(wf) = Workflow::from_file(&path) {
                    workflows.push(WorkflowState {
                        workflow: wf,
                        file_path: path,
                        run_state: RunState::Idle,
                    });
                }
            }
        }
    }
    workflows
}

use std::path::PathBuf;

use fridi_core::schema::Workflow;
use fridi_core::session::{SessionId, SessionStatus, SessionStore, SessionSummary};

/// Information about a tab displayed in the tab bar
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabInfo {
    pub(crate) session_id: SessionId,
    pub(crate) workflow_name: String,
    pub(crate) status: SessionStatus,
}

impl From<&SessionSummary> for TabInfo {
    fn from(summary: &SessionSummary) -> Self {
        Self {
            session_id: summary.id.clone(),
            workflow_name: summary.workflow_name.clone(),
            status: summary.status.clone(),
        }
    }
}

/// Load all workflow YAML files from a directory
pub(crate) fn load_workflows(dir: &std::path::Path) -> Vec<(Workflow, PathBuf)> {
    let mut workflows = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                if let Ok(wf) = Workflow::from_file(&path) {
                    workflows.push((wf, path));
                }
            }
        }
    }
    workflows
}

/// Load session summaries from the session store
pub(crate) fn load_sessions(store: &SessionStore) -> Vec<SessionSummary> {
    store.list().unwrap_or_default()
}

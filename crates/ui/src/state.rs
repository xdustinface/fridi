use std::path::{Path, PathBuf};

use fridi_core::schema::Workflow;
use fridi_core::session::{Session, SessionId, SessionStatus, SessionStore};
use fridi_core::window_state::WindowState;

/// Information about a tab displayed in the tab bar
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabInfo {
    pub(crate) session_id: SessionId,
    pub(crate) workflow_name: String,
    pub(crate) status: SessionStatus,
}

/// Load all workflow files from a directory, returning each workflow with its source path
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

/// Restore tabs from window state, filtering to sessions that still exist.
/// Returns the tabs and the index of the active tab.
pub(crate) fn restore_tabs(
    window_state: &WindowState,
    sessions: &[Session],
    repo: &str,
) -> (Vec<TabInfo>, Option<usize>) {
    let Some(info) = window_state.windows.get(repo) else {
        return (Vec::new(), None);
    };

    let mut tabs = Vec::new();
    let mut active_idx = None;

    for sid in &info.open_sessions {
        if let Some(session) = sessions.iter().find(|s| s.id.as_str() == sid) {
            let idx = tabs.len();
            if info.active_tab.as_deref() == Some(sid.as_str()) {
                active_idx = Some(idx);
            }
            tabs.push(TabInfo {
                session_id: session.id.clone(),
                workflow_name: session.workflow_name.clone(),
                status: session.status.clone(),
            });
        }
    }

    if active_idx.is_none() && !tabs.is_empty() {
        active_idx = Some(0);
    }

    (tabs, active_idx)
}

/// Load all sessions with recovery (marks running sessions as interrupted).
pub(crate) fn load_sessions_with_recovery(store: &SessionStore) -> Vec<Session> {
    match store.load_all_and_recover() {
        Ok(sessions) => sessions,
        Err(e) => {
            eprintln!("failed to load sessions: {e}");
            Vec::new()
        }
    }
}

/// Load window state from the given path.
pub(crate) fn load_window_state(state_path: &Path) -> WindowState {
    WindowState::load(state_path)
}

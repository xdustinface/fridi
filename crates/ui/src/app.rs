use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::engine::EngineEvent;
use fridi_core::schema::interpolate_with_repo;
use fridi_core::session::{Session, SessionId, SessionStore};
use fridi_core::window_state::WindowState;
use tokio::sync::broadcast;

use crate::components::home_dashboard::HomeDashboard;
use crate::components::session_creator::{SessionCreator, SessionSource};
use crate::components::tab_bar::TabBar;
use crate::components::workflow_view::WorkflowView;
use crate::engine_bridge::use_engine_events;
use crate::state::{self, TabInfo};
use crate::workflow_runner::WorkflowRunner;

const SESSIONS_DIR: &str = ".fridi/sessions";
const AGENTS_DIR: &str = "agents";
const STATE_FILE: &str = ".fridi/fridi-state.json";

/// Repo detected at startup, provided via Dioxus context.
#[derive(Clone)]
pub(crate) struct DetectedRepo(pub(crate) Option<String>);

#[component]
pub(crate) fn App() -> Element {
    let workflows_dir = PathBuf::from("./workflows");
    let workflows = use_signal(|| state::load_workflows(&workflows_dir));

    let store = use_signal(|| SessionStore::new(SESSIONS_DIR));
    let state_path = use_signal(|| PathBuf::from(STATE_FILE));

    // Retrieve the repo detected once at startup
    let detected_repo = use_context::<DetectedRepo>().0;

    let default_repo: Option<String> = detected_repo
        .or_else(|| {
            let repo_val = "";
            workflows
                .read()
                .iter()
                .find_map(|(wf, _)| {
                    wf.config
                        .repo
                        .as_ref()
                        .map(|r| interpolate_with_repo(r, repo_val))
                })
                .filter(|r| !r.is_empty())
        })
        .filter(|r| !r.is_empty());

    // On startup: load window state and recover sessions
    let mut window_state = use_signal(|| state::load_window_state(&state_path.read()));

    let mut tabs = use_signal(|| {
        let sessions = state::load_sessions_with_recovery(&store.read());
        let repo_key = default_repo.clone().unwrap_or_default();
        let ws = state::load_window_state(&state_path.read());
        let (restored, _) = state::restore_tabs(&ws, &sessions, &repo_key);
        if restored.is_empty() {
            // Fall back to showing all sessions as tabs
            sessions
                .iter()
                .map(|s| TabInfo {
                    session_id: s.id.clone(),
                    workflow_name: s.workflow_name.clone(),
                    status: s.status.clone(),
                })
                .collect::<Vec<_>>()
        } else {
            restored
        }
    });

    // Track which tab is active; None means home tab
    let mut active_tab = use_signal(|| {
        let sessions = state::load_sessions_with_recovery(&store.read());
        let repo_key = default_repo.clone().unwrap_or_default();
        let ws = state::load_window_state(&state_path.read());
        let (restored, active_idx) = state::restore_tabs(&ws, &sessions, &repo_key);
        if restored.is_empty() {
            let t = tabs.read();
            if t.is_empty() { None } else { Some(0) }
        } else {
            active_idx
        }
    });

    // Derive home state from active_tab rather than maintaining a separate signal
    let is_home = use_memo(move || active_tab.read().is_none());

    let mut showing_creator = use_signal(|| false);

    // Engine event bridge: receiver is set when a workflow starts
    let mut engine_rx: Signal<Option<broadcast::Receiver<EngineEvent>>> = use_signal(|| None);
    let live_state = use_engine_events(engine_rx);

    // Workflow runner for starting engine executions in background tasks
    let runner =
        use_signal(|| WorkflowRunner::new(PathBuf::from(AGENTS_DIR), PathBuf::from(SESSIONS_DIR)));

    // Helper to persist window state after tab changes
    let save_window_state = move |ws: &WindowState| {
        if let Err(e) = ws.save(&state_path.read()) {
            eprintln!("failed to save window state: {e}");
        }
    };

    // Load the full session for the active tab
    let active_session: Option<Session> = {
        let tabs_read = tabs.read();
        let active = *active_tab.read();
        active.and_then(|idx| {
            tabs_read
                .get(idx)
                .and_then(|tab| store.read().load(&tab.session_id).ok())
        })
    };

    let on_select_home = move |()| {
        active_tab.set(None);
    };

    let repo_for_select = default_repo.clone();
    let on_select_tab = move |idx: usize| {
        active_tab.set(Some(idx));
        // Persist active tab change
        let tabs_read = tabs.read();
        if let Some(tab) = tabs_read.get(idx) {
            let repo_key = repo_for_select.clone().unwrap_or_default();
            let mut ws = window_state.write();
            ws.set_active(&repo_key, tab.session_id.as_str());
            save_window_state(&ws);
        }
    };

    let repo_for_close = default_repo.clone();
    let on_close_tab = move |idx: usize| {
        let mut t = tabs.write();
        if idx < t.len() {
            let closed_session_id = t[idx].session_id.as_str().to_string();
            t.remove(idx);
            let len = t.len();

            // Persist tab removal
            let repo_key = repo_for_close.clone().unwrap_or_default();
            let mut ws = window_state.write();
            ws.update_tab(&repo_key, &closed_session_id, false);
            save_window_state(&ws);
            drop(ws);

            drop(t);
            if len == 0 {
                // No session tabs left, go to home
                active_tab.set(None);
            } else {
                let current = active_tab.read().unwrap_or(0);
                if current >= len {
                    active_tab.set(Some(len - 1));
                } else if current > idx {
                    active_tab.set(Some(current - 1));
                }
            }
        }
    };

    let on_new_tab = move |()| {
        showing_creator.set(true);
    };

    let create_session = {
        let default_repo = default_repo.clone();
        move |source: SessionSource| {
            let (workflow_name, context_label) = match &source {
                SessionSource::Issue { number, title } => (
                    format!("issue-{number}"),
                    format!("Issue #{number}: {title}"),
                ),
                SessionSource::PR { number, title, .. } => {
                    (format!("pr-{number}"), format!("PR #{number}: {title}"))
                }
                SessionSource::Prompt { text } => {
                    let short = if text.len() > 40 {
                        let truncated = text
                            .char_indices()
                            .nth(40)
                            .map_or(text.as_str(), |(i, _)| &text[..i]);
                        format!("{truncated}...")
                    } else {
                        text.clone()
                    };
                    ("prompt".to_string(), short)
                }
            };

            let session_id = SessionId::new(&workflow_name);
            let repo = default_repo.clone();
            let repo_str = repo.clone().unwrap_or_default();

            let workflow = crate::workflow_runner::workflow_from_source(&source, &repo_str);

            // Serialize the in-memory workflow to a YAML file so WorkflowView can read it
            let session_dir = PathBuf::from(SESSIONS_DIR).join(session_id.as_str());
            if let Err(e) = std::fs::create_dir_all(&session_dir) {
                eprintln!("failed to create session dir: {e}");
                return;
            }
            let workflow_path = session_dir.join("workflow.yaml");
            let workflow_yaml = match serde_yaml::to_string(&workflow) {
                Ok(y) => y,
                Err(e) => {
                    eprintln!("failed to serialize workflow: {e}");
                    return;
                }
            };
            if let Err(e) = std::fs::write(&workflow_path, &workflow_yaml) {
                eprintln!("failed to write workflow file: {e}");
                return;
            }

            let workflow_file = workflow_path.to_string_lossy().into_owned();

            let session = Session::new(
                session_id.clone(),
                context_label.clone(),
                workflow_file,
                repo,
            );

            let current_store = store.read().clone();
            if let Err(e) = current_store.save(&session) {
                eprintln!("failed to save session: {e}");
                return;
            }

            let runner_clone = runner.read().clone();
            let session_clone = session.clone();
            let store_clone = current_store;
            spawn(async move {
                match runner_clone
                    .start(workflow, session_clone, store_clone)
                    .await
                {
                    Ok(rx) => {
                        engine_rx.set(Some(rx));
                    }
                    Err(e) => {
                        eprintln!("failed to start workflow execution: {e}");
                    }
                }
            });

            let tab = TabInfo {
                session_id: session_id.clone(),
                workflow_name: context_label,
                status: session.status.clone(),
            };
            let mut t = tabs.write();
            let new_idx = t.len();
            t.push(tab);
            drop(t);
            active_tab.set(Some(new_idx));
            showing_creator.set(false);

            let repo_key = default_repo.clone().unwrap_or_default();
            let mut ws = window_state.write();
            ws.update_tab(&repo_key, session_id.as_str(), true);
            ws.set_active(&repo_key, session_id.as_str());
            save_window_state(&ws);
        }
    };

    let on_create_session = create_session.clone();
    let on_pick_issue = create_session;

    let on_cancel_creator = move |()| {
        showing_creator.set(false);
    };

    rsx! {
        div { class: "app-layout",
            TabBar {
                tabs: tabs.read().clone(),
                active: *active_tab.read(),
                home_active: *is_home.read(),
                on_select: on_select_tab,
                on_select_home,
                on_close: on_close_tab,
                on_new: on_new_tab,
            }
            div { class: "main-content",
                if *is_home.read() {
                    HomeDashboard {
                        key: "{default_repo.clone().unwrap_or_default()}",
                        repo: default_repo.clone(),
                        on_pick_issue,
                        // TODO: wire to a dedicated PR picker once available
                        on_show_pr_picker: on_new_tab,
                        on_show_creator: on_new_tab,
                    }
                } else if let Some(session) = active_session {
                    WorkflowView {
                        session,
                        live_state: Some(live_state.read().clone()),
                    }
                } else {
                    div { class: "empty-state", "Click + to start a new session" }
                }
            }
            if *showing_creator.read() {
                SessionCreator {
                    repo: default_repo.clone(),
                    on_create: on_create_session,
                    on_cancel: on_cancel_creator,
                }
            }
        }
    }
}

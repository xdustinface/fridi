use std::collections::HashMap;
use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::schema::interpolate_with_repo;
use fridi_core::session::{Session, SessionId, SessionStore};
use fridi_core::window_state::WindowState;
use serde_yaml::to_string as to_yaml;
use tracing::error;

use crate::components::backlog_tab::BacklogTab;
use crate::components::home_dashboard::HomeDashboard;
use crate::components::quick_capture::QuickCapture;
use crate::components::session_creator::{SessionCreator, SessionSource};
use crate::components::tab_bar::TabBar;
use crate::components::toast::{ToastContainer, ToastLevel, Toasts, push_toast};
use crate::components::workflow_view::WorkflowView;
use crate::engine_bridge::use_engine_events;
use crate::state::{self, TabInfo};
use crate::workflow_runner::WorkflowRunner;

const SESSIONS_DIR: &str = ".fridi/sessions";
const AGENTS_DIR: &str = "agents";
const STATE_FILE: &str = ".fridi/fridi-state.json";

/// Which pane is currently shown in the main content area.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActivePane {
    Home,
    Backlog,
    Session(usize),
}

/// Repo detected at startup, provided via Dioxus context.
#[derive(Clone)]
pub(crate) struct DetectedRepo(pub(crate) Option<String>);

#[component]
pub(crate) fn App() -> Element {
    // Global toast notification state
    let toast_signal = use_signal(Vec::new);
    use_context_provider(|| Toasts(toast_signal));

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

    let mut active_pane = use_signal(|| {
        let sessions = state::load_sessions_with_recovery(&store.read());
        let repo_key = default_repo.clone().unwrap_or_default();
        let ws = state::load_window_state(&state_path.read());
        let (restored, active_idx) = state::restore_tabs(&ws, &sessions, &repo_key);
        if restored.is_empty() {
            let t = tabs.read();
            if t.is_empty() {
                ActivePane::Home
            } else {
                ActivePane::Session(0)
            }
        } else {
            match active_idx {
                Some(idx) => ActivePane::Session(idx),
                None => ActivePane::Home,
            }
        }
    });

    let mut showing_creator = use_signal(|| false);
    let mut showing_quick_capture = use_signal(|| false);

    // Per-session engine event receivers and live state
    let mut engine_receivers: crate::engine_bridge::EngineReceivers = use_signal(HashMap::new);
    let mut live_states = use_engine_events(engine_receivers);

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
        let pane = *active_pane.read();
        match pane {
            ActivePane::Session(idx) => tabs_read
                .get(idx)
                .and_then(|tab| store.read().load(&tab.session_id).ok()),
            _ => None,
        }
    };

    let on_select_home = move |()| {
        active_pane.set(ActivePane::Home);
    };

    let on_select_backlog = move |()| {
        active_pane.set(ActivePane::Backlog);
    };

    let repo_for_select = default_repo.clone();
    let on_select_tab = move |idx: usize| {
        active_pane.set(ActivePane::Session(idx));
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
            let closed_sid = t[idx].session_id.clone();
            let closed_session_id = closed_sid.as_str().to_string();
            t.remove(idx);

            // Clean up per-session engine state
            engine_receivers.write().remove(&closed_sid);
            live_states.write().remove(&closed_sid);
            let len = t.len();

            // Persist tab removal
            let repo_key = repo_for_close.clone().unwrap_or_default();
            let mut ws = window_state.write();
            ws.update_tab(&repo_key, &closed_session_id, false);
            save_window_state(&ws);
            drop(ws);

            drop(t);
            if len == 0 {
                active_pane.set(ActivePane::Home);
            } else {
                let current = match *active_pane.read() {
                    ActivePane::Session(i) => i,
                    _ => 0,
                };
                if current >= len {
                    active_pane.set(ActivePane::Session(len - 1));
                } else if current > idx {
                    active_pane.set(ActivePane::Session(current - 1));
                }
            }
        }
    };

    let on_new_tab = move |()| {
        showing_creator.set(true);
    };

    let mut toasts = use_context::<Toasts>();

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
                error!("failed to create session dir: {e}");
                return;
            }
            let workflow_path = session_dir.join("workflow.yaml");
            let workflow_yaml = match to_yaml(&workflow) {
                Ok(y) => y,
                Err(e) => {
                    error!("failed to serialize workflow: {e}");
                    return;
                }
            };
            if let Err(e) = std::fs::write(&workflow_path, &workflow_yaml) {
                error!("failed to write workflow file: {e}");
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
            let sid = session_id.clone();
            spawn(async move {
                match runner_clone
                    .start(workflow, session_clone, store_clone)
                    .await
                {
                    Ok(rx) => {
                        engine_receivers.write().insert(sid, rx);
                    }
                    Err(e) => {
                        eprintln!("failed to start workflow execution: {e}");
                    }
                }
            });

            push_toast(
                &mut toasts.0,
                format!("Session started: {context_label}"),
                ToastLevel::Info,
            );

            let tab = TabInfo {
                session_id: session_id.clone(),
                workflow_name: context_label,
                status: session.status.clone(),
            };
            let mut t = tabs.write();
            let new_idx = t.len();
            t.push(tab);
            drop(t);
            active_pane.set(ActivePane::Session(new_idx));
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

    let current_pane = *active_pane.read();
    let is_home = current_pane == ActivePane::Home;
    let is_backlog = current_pane == ActivePane::Backlog;
    let session_idx = match current_pane {
        ActivePane::Session(idx) => Some(idx),
        _ => None,
    };

    // Derive context label for quick capture from the active session tab
    let quick_capture_context: Option<String> = {
        let tabs_read = tabs.read();
        session_idx.and_then(|idx| tabs_read.get(idx).map(|tab| tab.workflow_name.clone()))
    };

    // Global keyboard shortcut listener
    let repo_for_keys = default_repo.clone();
    use_coroutine(move |_: UnboundedReceiver<()>| {
        let repo_for_keys = repo_for_keys.clone();
        async move {
            let mut eval = document::eval(
                r#"
            document.addEventListener('keydown', function(e) {
                var mod = e.metaKey || e.ctrlKey;
                if (mod && e.key === 'b') {
                    e.preventDefault();
                    dioxus.send('toggle-quick-capture');
                } else if (mod && e.key === 't') {
                    e.preventDefault();
                    dioxus.send('new-session');
                } else if (mod && e.key === 'w') {
                    e.preventDefault();
                    dioxus.send('close-tab');
                } else if (mod && e.key >= '1' && e.key <= '9') {
                    e.preventDefault();
                    dioxus.send('tab-' + e.key);
                } else if (e.key === 'Escape') {
                    e.preventDefault();
                    dioxus.send('escape');
                }
            });
            "#,
            );
            loop {
                let Ok(msg) = eval.recv::<String>().await else {
                    break;
                };
                match msg.as_str() {
                    "toggle-quick-capture" => {
                        showing_quick_capture.toggle();
                    }
                    "new-session" => {
                        showing_creator.set(true);
                    }
                    "close-tab" => {
                        let current = *active_pane.read();
                        if let ActivePane::Session(idx) = current {
                            let t = tabs.read();
                            if idx < t.len() {
                                let closed_sid = t[idx].session_id.clone();
                                let closed_id_str = closed_sid.as_str().to_string();
                                drop(t);

                                tabs.write().remove(idx);
                                engine_receivers.write().remove(&closed_sid);
                                live_states.write().remove(&closed_sid);

                                let len = tabs.read().len();
                                let repo_key = repo_for_keys.clone().unwrap_or_default();
                                let mut ws = window_state.write();
                                ws.update_tab(&repo_key, &closed_id_str, false);
                                save_window_state(&ws);
                                drop(ws);

                                if len == 0 {
                                    active_pane.set(ActivePane::Home);
                                } else if idx >= len {
                                    active_pane.set(ActivePane::Session(len - 1));
                                }
                            }
                        }
                    }
                    "escape" => {
                        if *showing_quick_capture.read() {
                            showing_quick_capture.set(false);
                        } else if *showing_creator.read() {
                            showing_creator.set(false);
                        }
                    }
                    other => {
                        if let Some(digit) = other.strip_prefix("tab-") {
                            if let Ok(n) = digit.parse::<usize>() {
                                match n {
                                    1 => active_pane.set(ActivePane::Home),
                                    2 => active_pane.set(ActivePane::Backlog),
                                    _ => {
                                        let session_idx = n - 3;
                                        let t = tabs.read();
                                        if session_idx < t.len() {
                                            if let Some(tab) = t.get(session_idx) {
                                                let repo_key =
                                                    repo_for_keys.clone().unwrap_or_default();
                                                let mut ws = window_state.write();
                                                ws.set_active(&repo_key, tab.session_id.as_str());
                                                save_window_state(&ws);
                                                drop(ws);
                                            }
                                            drop(t);
                                            active_pane.set(ActivePane::Session(session_idx));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let on_dismiss_quick_capture = move |()| {
        showing_quick_capture.set(false);
    };

    rsx! {
        div { class: "app-layout",
            TabBar {
                tabs: tabs.read().clone(),
                active: session_idx,
                home_active: is_home,
                backlog_active: is_backlog,
                on_select: on_select_tab,
                on_select_home,
                on_select_backlog,
                on_close: on_close_tab,
                on_new: on_new_tab,
            }
            div { class: "main-content",
                if is_home {
                    HomeDashboard {
                        key: "{default_repo.clone().unwrap_or_default()}",
                        repo: default_repo.clone(),
                        on_pick_issue,
                        // TODO: wire to a dedicated PR picker once available
                        on_show_pr_picker: on_new_tab,
                        on_show_creator: on_new_tab,
                    }
                } else if is_backlog {
                    BacklogTab {}
                } else if let Some(session) = active_session {
                    {
                        let session_live = live_states
                            .read()
                            .get(&session.id)
                            .cloned();
                        rsx! {
                            WorkflowView {
                                session,
                                live_state: session_live,
                            }
                        }
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
            if *showing_quick_capture.read() {
                QuickCapture {
                    context: quick_capture_context.clone(),
                    on_dismiss: on_dismiss_quick_capture,
                }
            }
            ToastContainer {}
        }
    }
}

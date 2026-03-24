use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::session::{Session, SessionId, SessionStore};

use crate::components::session_creator::{SessionCreator, SessionSource};
use crate::components::tab_bar::TabBar;
use crate::components::workflow_view::WorkflowView;
use crate::state::{self, TabInfo};
use crate::styles;

const SESSIONS_DIR: &str = ".fridi/sessions";

#[component]
pub(crate) fn App() -> Element {
    let workflows_dir = PathBuf::from("./workflows");
    let workflows = use_signal(|| state::load_workflows(&workflows_dir));

    let store = use_signal(|| SessionStore::new(SESSIONS_DIR));

    let mut tabs = use_signal(|| {
        let summaries = state::load_sessions(&store.read());
        summaries.iter().map(TabInfo::from).collect::<Vec<_>>()
    });

    let mut active_tab = use_signal(|| {
        let t = tabs.read();
        if t.is_empty() { None } else { Some(0) }
    });

    let mut showing_creator = use_signal(|| false);

    // Derive repo from the first workflow that has one configured
    let default_repo: Option<String> = workflows
        .read()
        .iter()
        .find_map(|(wf, _)| wf.config.repo.clone())
        .filter(|r| !r.is_empty());

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

    let on_select_tab = move |idx: usize| {
        active_tab.set(Some(idx));
    };

    let on_close_tab = move |idx: usize| {
        let mut t = tabs.write();
        if idx < t.len() {
            t.remove(idx);
            let len = t.len();
            drop(t);
            if len == 0 {
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

    let on_create_session = move |source: SessionSource| {
        let (workflow_name, context_label) = match &source {
            SessionSource::Issue { number, title } => (
                format!("issue-{number}"),
                format!("Issue #{number}: {title}"),
            ),
            SessionSource::PR { number, title } => {
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
        let repo = workflows
            .read()
            .iter()
            .find_map(|(wf, _)| wf.config.repo.clone())
            .filter(|r| !r.is_empty());

        let session = Session::new(
            session_id.clone(),
            context_label.clone(),
            String::new(),
            repo,
        );

        if let Err(e) = store.read().save(&session) {
            eprintln!("failed to save session: {e}");
            return;
        }

        let tab = TabInfo {
            session_id,
            workflow_name: context_label,
            status: session.status.clone(),
        };
        let mut t = tabs.write();
        let new_idx = t.len();
        t.push(tab);
        drop(t);
        active_tab.set(Some(new_idx));
        showing_creator.set(false);
    };

    let on_cancel_creator = move |()| {
        showing_creator.set(false);
    };

    rsx! {
        document::Style { {styles::APP_CSS} }
        div { class: "app-layout",
            TabBar {
                tabs: tabs.read().clone(),
                active: *active_tab.read(),
                on_select: on_select_tab,
                on_close: on_close_tab,
                on_new: on_new_tab,
            }
            div { class: "main-content",
                if let Some(session) = active_session {
                    WorkflowView { session: session }
                } else {
                    div { class: "empty-state",
                        "Click + to start a new session"
                    }
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

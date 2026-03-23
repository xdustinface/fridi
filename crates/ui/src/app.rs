use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::engine::StepStatus;
use fridi_core::session::{Session, SessionId, SessionStore};

use crate::components::tab_bar::TabBar;
use crate::components::workflow_picker::WorkflowPicker;
use crate::components::workflow_view::WorkflowView;
use crate::state::{self, TabInfo};
use crate::styles;

const SESSIONS_DIR: &str = ".fridi/sessions";

#[component]
pub(crate) fn App() -> Element {
    let workflows_dir = PathBuf::from("./workflows");
    let workflows = use_signal(|| {
        state::load_workflows(&workflows_dir)
            .into_iter()
            .map(|(wf, _path)| wf)
            .collect::<Vec<_>>()
    });

    let store = use_signal(|| SessionStore::new(SESSIONS_DIR));

    let mut tabs = use_signal(|| {
        let summaries = state::load_sessions(&store.read());
        summaries.iter().map(TabInfo::from).collect::<Vec<_>>()
    });

    let mut active_tab = use_signal(|| {
        let t = tabs.read();
        if t.is_empty() { None } else { Some(0) }
    });

    let mut showing_picker = use_signal(|| false);

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
        showing_picker.set(true);
    };

    let on_pick_workflow = move |wf: fridi_core::schema::Workflow| {
        let session_id = SessionId::new(&wf.name);
        let repo = wf.config.repo.clone();
        let session = Session::new(
            session_id.clone(),
            wf.name.clone(),
            format!("./workflows/{}.yaml", wf.name),
            repo,
        );
        // Save the new session
        let _ = store.read().save(&session);

        let tab = TabInfo {
            session_id,
            workflow_name: wf.name.clone(),
            status: session.status.clone(),
        };
        let mut t = tabs.write();
        let new_idx = t.len();
        t.push(tab);
        drop(t);
        active_tab.set(Some(new_idx));
        showing_picker.set(false);
    };

    let on_cancel_picker = move |()| {
        showing_picker.set(false);
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
                        "Click + to start a new workflow"
                    }
                }
            }
            if *showing_picker.read() {
                WorkflowPicker {
                    workflows: workflows.read().clone(),
                    on_select: on_pick_workflow,
                    on_cancel: on_cancel_picker,
                }
            }
        }
    }
}

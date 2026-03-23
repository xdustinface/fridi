use std::collections::HashMap;
use std::path::PathBuf;

use conductor_core::engine::StepStatus;
use dioxus::prelude::*;

use crate::components::sidebar::Sidebar;
use crate::components::workflow_view::WorkflowView;
use crate::state::{self, RunState, WorkflowState};
use crate::styles;

#[component]
pub(crate) fn App() -> Element {
    let workflows_dir = PathBuf::from("./workflows");
    let mut workflows = use_signal(|| state::load_workflows(&workflows_dir));
    let mut selected = use_signal(|| Option::<usize>::None);

    let on_select = move |idx: usize| {
        selected.set(Some(idx));
    };

    let on_run = move |()| {
        if let Some(idx) = *selected.read() {
            let mut wfs = workflows.write();
            if let Some(ws) = wfs.get_mut(idx) {
                let step_statuses: HashMap<String, StepStatus> = ws
                    .workflow
                    .steps
                    .iter()
                    .map(|s| (s.name.clone(), StepStatus::Pending))
                    .collect();
                ws.run_state = RunState::Running {
                    step_statuses,
                    started_at: std::time::Instant::now(),
                };
            }
        }
    };

    let selected_ws: Option<WorkflowState> = {
        let wfs = workflows.read();
        (*selected.read()).and_then(|idx| wfs.get(idx).cloned())
    };

    rsx! {
        document::Style { {styles::APP_CSS} }
        div { class: "app-layout",
            Sidebar {
                workflows: workflows.read().clone(),
                selected_index: *selected.read(),
                on_select: on_select,
            }
            div { class: "main-content",
                if let Some(ws) = selected_ws {
                    WorkflowView {
                        workflow_state: ws,
                        on_run: on_run,
                    }
                } else {
                    div { class: "empty-state",
                        "Select a workflow to view details"
                    }
                }
            }
        }
    }
}

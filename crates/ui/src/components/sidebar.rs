use dioxus::prelude::*;

use crate::state::{RunState, WorkflowState};

#[component]
pub(crate) fn Sidebar(
    workflows: Vec<WorkflowState>,
    selected_index: Option<usize>,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header",
                h1 { "conductor" }
                p { "AI Workflow Orchestrator" }
            }
            div { class: "workflow-list",
                for (idx, ws) in workflows.iter().enumerate() {
                    {
                        let is_selected = selected_index == Some(idx);
                        let item_class = if is_selected {
                            "workflow-item selected"
                        } else {
                            "workflow-item"
                        };
                        let status_class = match &ws.run_state {
                            RunState::Idle => "idle",
                            RunState::Running { .. } => "running",
                            RunState::Completed => "completed",
                            RunState::Failed(_) => "failed",
                        };
                        rsx! {
                            div {
                                key: "{ws.workflow.name}",
                                class: "{item_class}",
                                onclick: move |_| on_select.call(idx),
                                div { class: "workflow-item-header",
                                    div { class: "status-dot {status_class}" }
                                    span { class: "workflow-item-name", "{ws.workflow.name}" }
                                }
                                if let Some(desc) = &ws.workflow.description {
                                    div { class: "workflow-item-desc", "{desc}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

use dioxus::prelude::*;
use fridi_core::engine::StepStatus;
use fridi_core::schema::Trigger;

use crate::components::step_card::StepCard;
use crate::state::{RunState, WorkflowState};

#[component]
pub(crate) fn WorkflowView(workflow_state: WorkflowState, on_run: EventHandler<()>) -> Element {
    let wf = &workflow_state.workflow;
    let is_running = matches!(workflow_state.run_state, RunState::Running { .. });

    let trigger_tags: Vec<String> = wf
        .triggers
        .iter()
        .map(|t| match t {
            Trigger::Cron { schedule } => format!("cron: {schedule}"),
            Trigger::Manual => "manual".to_string(),
        })
        .collect();

    rsx! {
        div { class: "workflow-view",
            div { class: "workflow-header",
                h2 { "{wf.name}" }
                if let Some(desc) = &wf.description {
                    p { "{desc}" }
                }
                if !trigger_tags.is_empty() {
                    div { class: "workflow-meta",
                        for tag in &trigger_tags {
                            span { class: "meta-tag", "{tag}" }
                        }
                    }
                }
            }

            div { class: "workflow-actions",
                button {
                    class: "btn-run",
                    disabled: is_running,
                    onclick: move |_| on_run.call(()),
                    if is_running { "Running..." } else { "Run" }
                }
            }

            div { class: "steps-section",
                h3 { "Steps" }
                div { class: "steps-list",
                    for step in &wf.steps {
                        {
                            let status = match &workflow_state.run_state {
                                RunState::Running { step_statuses, .. } => {
                                    step_statuses.get(&step.name).cloned()
                                }
                                RunState::Completed => Some(StepStatus::Completed),
                                RunState::Failed(_) => None,
                                RunState::Idle => None,
                            };
                            rsx! {
                                StepCard {
                                    key: "{step.name}",
                                    step: step.clone(),
                                    status: status,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

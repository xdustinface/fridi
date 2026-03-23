use dioxus::prelude::*;
use fridi_core::engine::StepStatus;
use fridi_core::schema::Trigger;
use fridi_core::session::Session;

use crate::components::step_card::StepCard;

#[component]
pub(crate) fn WorkflowView(session: Session) -> Element {
    // Load the workflow from disk to get trigger/description metadata
    let workflow =
        fridi_core::schema::Workflow::from_file(std::path::Path::new(&session.workflow_file)).ok();

    let (description, trigger_tags, steps) = match &workflow {
        Some(wf) => {
            let tags: Vec<String> = wf
                .triggers
                .iter()
                .map(|t| match t {
                    Trigger::Cron { schedule } => format!("cron: {schedule}"),
                    Trigger::Manual => "manual".to_string(),
                })
                .collect();
            (wf.description.clone(), tags, wf.steps.clone())
        }
        None => (None, Vec::new(), Vec::new()),
    };

    rsx! {
        div { class: "workflow-view",
            div { class: "workflow-header",
                h2 { "{session.workflow_name}" }
                if let Some(desc) = &description {
                    p { "{desc}" }
                }
                if !trigger_tags.is_empty() {
                    div { class: "workflow-meta",
                        for tag in &trigger_tags {
                            span { class: "meta-tag", "{tag}" }
                        }
                    }
                }
                if let Some(repo) = &session.repo {
                    div { class: "workflow-meta",
                        span { class: "meta-tag", "repo: {repo}" }
                    }
                }
            }

            div { class: "steps-section",
                h3 { "Steps" }
                div { class: "steps-list",
                    for step in &steps {
                        {
                            let status = session.steps.values()
                                .find(|ss| ss.step_name == step.name)
                                .map(|ss| ss.status.clone());
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

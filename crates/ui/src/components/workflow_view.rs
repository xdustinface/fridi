use dioxus::prelude::*;
use fridi_core::engine::StepStatus;
use fridi_core::schema::Trigger;
use fridi_core::session::Session;

use crate::components::step_card::StepCard;
use crate::components::terminal_view::TerminalView;

/// Information about the currently selected step, used by the terminal view.
struct SelectedStepInfo {
    name: String,
    attempt: u32,
    status_label: String,
    output: String,
}

#[component]
pub(crate) fn WorkflowView(session: Session) -> Element {
    let mut selected_step = use_signal(|| Option::<String>::None);

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

    // Resolve info for the selected step
    let selected_info: Option<SelectedStepInfo> = {
        let sel = selected_step.read();
        sel.as_ref().map(|name| {
            let step_state = session
                .steps
                .values()
                .filter(|ss| ss.step_name == *name)
                .max_by_key(|ss| ss.attempt);
            let (attempt, status_label, output) = match step_state {
                Some(ss) => {
                    let label = match &ss.status {
                        StepStatus::Pending => "Pending".to_string(),
                        StepStatus::Running => "Running".to_string(),
                        StepStatus::Completed => "Completed".to_string(),
                        StepStatus::Failed(reason) => format!("Failed: {reason}"),
                        StepStatus::Skipped => "Skipped".to_string(),
                    };
                    let output = ss
                        .output_summary
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    (ss.attempt, label, output)
                }
                None => (1, "Pending".to_string(), String::new()),
            };
            SelectedStepInfo {
                name: name.clone(),
                attempt,
                status_label,
                output,
            }
        })
    };

    let dag_view = rsx! {
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
                                .filter(|ss| ss.step_name == step.name)
                                .max_by_key(|ss| ss.attempt)
                                .map(|ss| ss.status.clone());
                            let is_selected = selected_step.read().as_deref() == Some(&step.name);
                            rsx! {
                                StepCard {
                                    key: "{step.name}",
                                    step: step.clone(),
                                    status: status,
                                    selected: is_selected,
                                    on_select: move |name: String| {
                                        let current = selected_step.read().clone();
                                        if current.as_deref() == Some(name.as_str()) {
                                            selected_step.set(None);
                                        } else {
                                            selected_step.set(Some(name));
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let terminal = selected_info.map(|info| {
        rsx! {
            TerminalView {
                step_name: info.name,
                attempt: info.attempt,
                status: info.status_label,
                output: info.output,
            }
        }
    });

    rsx! {
        crate::components::split_pane::SplitPane {
            top: dag_view,
            bottom: terminal,
        }
    }
}

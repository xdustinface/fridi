use dioxus::prelude::*;
use fridi_core::engine::StepStatus;
use fridi_core::schema::Step;

#[component]
pub(crate) fn StepCard(
    step: Step,
    status: Option<StepStatus>,
    selected: Option<bool>,
    on_select: Option<EventHandler<String>>,
) -> Element {
    let status = status.unwrap_or(StepStatus::Pending);
    let is_selected = selected.unwrap_or(false);

    let (status_class, status_label) = match &status {
        StepStatus::Pending => ("pending", "Pending".to_string()),
        StepStatus::Running => ("running", "Running".to_string()),
        StepStatus::Completed => ("completed", "Completed".to_string()),
        StepStatus::Failed(reason) => ("failed", format!("Failed: {reason}")),
        StepStatus::Skipped => ("skipped", "Skipped".to_string()),
    };

    let selected_class = if is_selected { " selected" } else { "" };
    let card_class = format!("step-card {status_class}{selected_class}");

    let agent_display = step.agent.as_deref().unwrap_or("-");
    let skill_display = step.skill.as_deref().unwrap_or("-");

    let step_name = step.name.clone();

    rsx! {
        div {
            class: "{card_class}",
            onclick: move |_| {
                if let Some(handler) = &on_select {
                    handler.call(step_name.clone());
                }
            },
            div { class: "step-card-header",
                div { class: "status-dot {status_class}" }
                span { class: "step-name", "{step.name}" }
                span { class: "step-status-text {status_class}", "{status_label}" }
            }
            div { class: "step-details",
                span {
                    span { class: "step-detail-label", "agent: " }
                    "{agent_display}"
                }
                span {
                    span { class: "step-detail-label", "skill: " }
                    "{skill_display}"
                }
            }
            if !step.depends_on.is_empty() {
                div { class: "step-deps",
                    "depends on: {step.depends_on.join(\", \")}"
                }
            }
        }
    }
}

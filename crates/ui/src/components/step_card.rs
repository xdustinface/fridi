use conductor_core::engine::StepStatus;
use conductor_core::schema::Step;
use dioxus::prelude::*;

#[component]
pub(crate) fn StepCard(step: Step, status: Option<StepStatus>) -> Element {
    let status = status.unwrap_or(StepStatus::Pending);

    let (status_class, status_label) = match &status {
        StepStatus::Pending => ("pending", "Pending".to_string()),
        StepStatus::Running => ("running", "Running".to_string()),
        StepStatus::Completed => ("completed", "Completed".to_string()),
        StepStatus::Failed(reason) => ("failed", format!("Failed: {reason}")),
        StepStatus::Skipped => ("skipped", "Skipped".to_string()),
    };

    let card_class = format!("step-card {status_class}");

    let agent_display = step.agent.as_deref().unwrap_or("-");
    let skill_display = step.skill.as_deref().unwrap_or("-");

    rsx! {
        div { class: "{card_class}",
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
            if matches!(status, StepStatus::Completed) {
                div { class: "step-hint",
                    "click to view terminal (coming soon)"
                }
            }
        }
    }
}

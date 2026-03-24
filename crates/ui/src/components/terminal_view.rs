use dioxus::prelude::*;

#[component]
pub(crate) fn TerminalView(
    step_name: String,
    attempt: u32,
    status: String,
    output: Vec<u8>,
) -> Element {
    let status_class = match status.as_str() {
        "Running" => "running",
        "Completed" => "completed",
        "Pending" => "pending",
        "Skipped" => "skipped",
        s if s.starts_with("Failed") => "failed",
        _ => "pending",
    };

    let display_text = String::from_utf8_lossy(&output);

    rsx! {
        div { class: "terminal-view",
            div { class: "terminal-header",
                span { class: "terminal-step-name", "{step_name}" }
                span { class: "terminal-attempt", "attempt #{attempt}" }
                div { class: "terminal-status-indicator",
                    div { class: "status-dot {status_class}" }
                    span { class: "terminal-status-text {status_class}", "{status}" }
                }
            }
            div { class: "terminal-output",
                pre { class: "terminal-output-text",
                    if display_text.is_empty() {
                        "No output yet."
                    } else {
                        "{display_text}"
                    }
                }
            }
        }
    }
}

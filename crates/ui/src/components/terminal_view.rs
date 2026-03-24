use base64::Engine;
use dioxus::prelude::*;

/// Encodes raw bytes as base64 for safe transport into JavaScript strings.
fn encode_for_js(data: &[u8]) -> String { base64::engine::general_purpose::STANDARD.encode(data) }

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

    // Stable terminal ID derived from step name (replace non-alphanumeric chars)
    let terminal_id = format!(
        "terminal-{}",
        step_name
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
    );

    // Track how many bytes we have already written to this xterm instance so we
    // only send the delta on each render.
    let mut written_len = use_signal(|| 0usize);

    // Track which terminal ID is currently initialized so we can detect step switches.
    let mut active_id = use_signal(String::new);

    // Initialize xterm.js on the container div once it is mounted.
    let tid = terminal_id.clone();
    let _init = use_resource(move || {
        let tid = tid.clone();
        async move {
            // Small delay to ensure the DOM element exists before we call open().
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let js = format!(
                r#"
                (function() {{
                    let el = document.getElementById('{tid}');
                    if (!el || window.fridiTerminals['{tid}']) return;
                    let term = new Terminal({{
                        theme: {{
                            background: '#0c0e12',
                            foreground: '#e2e8f0',
                            cursor: '#6b9e6b',
                            selectionBackground: 'rgba(107, 158, 107, 0.3)',
                        }},
                        fontSize: 13,
                        fontFamily: 'JetBrains Mono, SF Mono, Fira Code, monospace',
                        cursorStyle: 'underline',
                        scrollback: 10000,
                        convertEol: true,
                        allowTransparency: true,
                    }});
                    term.open(el);
                    window.fridiTerminals['{tid}'] = term;
                }})();
                "#,
            );
            let _ = document::eval(&js).await;
        }
    });

    // When the step changes, reset tracking state and destroy old terminal.
    if *active_id.read() != terminal_id {
        // Destroy old terminal if it existed under a different ID
        let old_id = active_id.read().clone();
        if !old_id.is_empty() {
            let js = format!(
                r#"
                (function() {{
                    let t = window.fridiTerminals['{old_id}'];
                    if (t) {{ t.dispose(); delete window.fridiTerminals['{old_id}']; }}
                }})();
                "#,
            );
            spawn(async move {
                let _ = document::eval(&js).await;
            });
        }
        active_id.set(terminal_id.clone());
        written_len.set(0);
    }

    // Write new output data to the xterm instance (only the delta since last write).
    let current_len = output.len();
    let already_written = *written_len.read();
    if current_len > already_written {
        let new_data = &output[already_written..current_len];
        let b64 = encode_for_js(new_data);
        let tid = terminal_id.clone();
        written_len.set(current_len);
        spawn(async move {
            let js = format!(
                r#"
                (function() {{
                    let t = window.fridiTerminals['{tid}'];
                    if (!t) return;
                    let raw = atob('{b64}');
                    t.write(raw);
                }})();
                "#,
            );
            let _ = document::eval(&js).await;
        });
    }

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
            div {
                id: "{terminal_id}",
                class: "terminal-xterm-container",
            }
        }
    }
}

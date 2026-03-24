use base64::Engine;
use dioxus::prelude::*;

/// Hex-encodes the step name to produce a collision-free terminal element ID.
/// Plain sanitization (replacing non-alphanumeric chars with `-`) would collide
/// for names like `build.foo` vs `build/foo`.
fn terminal_id_for(step_name: &str) -> String {
    let hex: String = step_name
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("terminal-{hex}")
}

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

    let terminal_id = use_memo(use_reactive!(|step_name| terminal_id_for(&step_name)));

    // Track how many bytes we have already written to this xterm instance so we
    // only send the delta on each render.
    let mut written_len = use_signal(|| 0usize);

    // Track which terminal ID is currently initialized so we can detect step switches.
    let mut active_id = use_signal(String::new);

    // Whether the xterm instance for the current terminal_id has been created.
    let mut terminal_ready = use_signal(|| false);

    // When the step changes, reset tracking state and destroy old terminal.
    if *active_id.read() != *terminal_id.read() {
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
        active_id.set(terminal_id.read().clone());
        written_len.set(0);
        terminal_ready.set(false);
    }

    // Initialize xterm.js when the container div is mounted in the DOM.
    let tid_for_mount = terminal_id.read().clone();
    let on_mounted = move |_evt: MountedEvent| {
        let tid = tid_for_mount.clone();
        spawn(async move {
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
                    let fitAddon = new FitAddon.FitAddon();
                    term.loadAddon(fitAddon);
                    fitAddon.fit();
                    new ResizeObserver(() => fitAddon.fit()).observe(el);
                    window.fridiTerminals['{tid}'] = term;
                }})();
                "#,
            );
            let _ = document::eval(&js).await;
            terminal_ready.set(true);
        });
    };

    // Write new output data to the xterm instance (only the delta since last write).
    let current_len = output.len();
    let already_written = *written_len.read();

    // Handle output shrinking (e.g., step re-run replaces buffer with shorter content).
    if current_len < already_written {
        written_len.set(0);
        let tid = terminal_id.read().clone();
        if *terminal_ready.read() {
            spawn(async move {
                let js = format!(
                    r#"
                    (function() {{
                        let t = window.fridiTerminals['{tid}'];
                        if (t) t.clear();
                    }})();
                    "#,
                );
                let _ = document::eval(&js).await;
            });
        }
    } else if current_len > already_written && *terminal_ready.read() {
        let new_data = &output[already_written..current_len];
        let b64 = base64::engine::general_purpose::STANDARD.encode(new_data);
        let tid = terminal_id.read().clone();
        written_len.set(current_len);
        spawn(async move {
            let js = format!(
                r#"
                (function() {{
                    let t = window.fridiTerminals['{tid}'];
                    if (!t) return;
                    let binary = atob('{b64}');
                    let bytes = new Uint8Array(binary.length);
                    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
                    t.write(bytes);
                }})();
                "#,
            );
            let _ = document::eval(&js).await;
        });
    }

    // Dispose the xterm instance when the component unmounts to avoid leaking memory.
    let cleanup_id = terminal_id.read().clone();
    use_drop(move || {
        let js = format!(
            r#"
            (function() {{
                let t = window.fridiTerminals['{cleanup_id}'];
                if (t) {{ t.dispose(); delete window.fridiTerminals['{cleanup_id}']; }}
            }})();
            "#,
        );
        // Fire-and-forget cleanup eval.
        spawn(async move {
            let _ = document::eval(&js).await;
        });
    });

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
                onmounted: on_mounted,
            }
        }
    }
}

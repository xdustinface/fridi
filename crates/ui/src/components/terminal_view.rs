use base64::Engine;
use dioxus::prelude::*;
use fridi_agent::pty;

/// Hex-encodes the session ID and step name to produce a collision-free terminal
/// element ID. Including the session ID prevents collisions when multiple
/// sessions run steps with the same name.
fn terminal_id_for(session_id: &str, step_name: &str) -> String {
    let hex: String = format!("{session_id}:{step_name}")
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("terminal-{hex}")
}

/// Returns the JS snippet that creates a new xterm.js Terminal, opens it in the
/// given DOM element, loads the FitAddon, and registers a ResizeObserver.
fn xterm_init_js(tid: &str) -> String {
    format!(
        r#"
        (function() {{
            let el = document.getElementById('{tid}');
            if (!el) return;
            let old = window.fridiTerminals['{tid}'];
            if (old) {{ old.dispose(); delete window.fridiTerminals['{tid}']; }}
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
                overviewRulerLanes: 0,
                scrollbarStyle: 'overlay',
            }});
            term.open(el);
            let fitAddon = new FitAddon.FitAddon();
            term.loadAddon(fitAddon);
            function syncSize() {{
                let p = el.parentElement;
                if (p && p.clientWidth > 0 && p.clientHeight > 0) {{
                    el.style.width = p.clientWidth + 'px';
                    el.style.height = (p.clientHeight - el.offsetTop) + 'px';
                    fitAddon.fit();
                    return true;
                }}
                return false;
            }}
            function doFit() {{
                if (!syncSize()) {{
                    requestAnimationFrame(doFit);
                }}
            }}
            requestAnimationFrame(doFit);
            new ResizeObserver(() => syncSize()).observe(el.parentElement);
            window.fridiTerminals['{tid}'] = term;
        }})();
        "#,
    )
}

#[component]
pub(crate) fn TerminalView(
    session_id: String,
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

    let terminal_id = use_memo(use_reactive!(|session_id, step_name| terminal_id_for(
        &session_id,
        &step_name
    )));

    // Track how many bytes we have already written to this xterm instance so we
    // only send the delta on each render.
    let mut written_len = use_signal(|| 0usize);

    // Track which terminal ID is currently initialized so we can detect step switches.
    let mut active_id = use_signal(String::new);

    // Whether the xterm instance for the current terminal_id has been created.
    let mut terminal_ready = use_signal(|| false);

    // Spawns a resize event channel for the given terminal. The JS calls
    // `dioxus.send()` on every xterm resize (and once immediately with the
    // initial size). Each new xterm instance needs its own channel.
    let resize_key = format!("{}:{}", session_id, step_name);
    let start_resize_channel = {
        let resize_key = resize_key.clone();
        move |tid: String| {
            let step = resize_key.clone();
            spawn(async move {
                let js = format!(
                    r#"
                    var t = window.fridiTerminals['{tid}'];
                    if (t) {{
                        dioxus.send({{ cols: t.cols, rows: t.rows }});
                        var resizeTimer = null;
                        t.onResize(function(size) {{
                            if (resizeTimer) clearTimeout(resizeTimer);
                            resizeTimer = setTimeout(function() {{
                                dioxus.send({{ cols: size.cols, rows: size.rows }});
                            }}, 150);
                        }});
                    }}
                    "#
                );

                let mut eval = document::eval(&js);

                tracing::info!("PTY resize channel established for step {}", step);

                loop {
                    match eval.recv::<serde_json::Value>().await {
                        Ok(val) => {
                            if let (Some(cols), Some(rows)) = (
                                val.get("cols").and_then(|v| v.as_u64()),
                                val.get("rows").and_then(|v| v.as_u64()),
                            ) {
                                let cols = cols as u16;
                                let rows = rows as u16;
                                tracing::info!(
                                    "PTY resize event: {}x{} for step {}",
                                    cols,
                                    rows,
                                    step
                                );
                                if let Some(resizer) = pty::get_resizer(&step) {
                                    resizer.resize(cols, rows);
                                } else {
                                    let mut retries = 0;
                                    while pty::get_resizer(&step).is_none() && retries < 30 {
                                        tokio::time::sleep(std::time::Duration::from_millis(100))
                                            .await;
                                        retries += 1;
                                    }
                                    if let Some(resizer) = pty::get_resizer(&step) {
                                        resizer.resize(cols, rows);
                                    } else {
                                        tracing::warn!(
                                            "PTY resizer not found after retries for step {}",
                                            step
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("PTY resize channel closed for step {}: {}", step, e);
                            break;
                        }
                    }
                }
            });
        }
    };

    // When the step changes, reset tracking state, destroy old terminal, and
    // create a new xterm instance. Dioxus reuses the DOM div so `onmounted`
    // won't fire again — we must re-initialize xterm ourselves.
    if *active_id.read() != *terminal_id.read() {
        let old_id = active_id.read().clone();
        let new_id = terminal_id.read().clone();
        active_id.set(new_id.clone());
        written_len.set(0);
        terminal_ready.set(false);

        if !old_id.is_empty() {
            let init_js = xterm_init_js(&new_id);
            let tid = new_id.clone();
            let start_resize = start_resize_channel.clone();
            spawn(async move {
                // Dispose the old terminal first.
                let dispose_js = format!(
                    r#"
                    (function() {{
                        let t = window.fridiTerminals['{old_id}'];
                        if (t) {{ t.dispose(); delete window.fridiTerminals['{old_id}']; }}
                    }})();
                    "#,
                );
                let _ = document::eval(&dispose_js).await;
                // Initialize xterm on the reused DOM element.
                let _ = document::eval(&init_js).await;
                terminal_ready.set(true);
                start_resize(tid);
            });
        }
    }

    // Initialize xterm.js when the container div is first mounted in the DOM.
    let tid_for_mount = terminal_id.read().clone();
    let start_resize_on_mount = start_resize_channel.clone();
    let on_mounted = move |_evt: MountedEvent| {
        written_len.set(0);
        terminal_ready.set(false);
        let js = xterm_init_js(&tid_for_mount);
        let tid = tid_for_mount.clone();
        let start_resize = start_resize_on_mount.clone();
        spawn(async move {
            let _ = document::eval(&js).await;
            terminal_ready.set(true);
            start_resize(tid);
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

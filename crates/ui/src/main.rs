mod app;
mod components;
mod engine_bridge;
mod state;
mod styles;
mod workflow_runner;

fn main() {
    // Detect repo once at startup to avoid repeated subprocess calls and
    // unsafe env mutation during rendering.
    let repo = fridi_core::github::detect_repo();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("fridi"))
                .with_custom_head(format!(
                    r#"<style>{css}</style>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.css" />
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/lib/xterm.js"></script>
<script>window.fridiTerminals = {{}};</script>"#,
                    css = styles::APP_CSS,
                )),
        )
        .with_context(app::DetectedRepo(repo))
        .launch(app::App);
}

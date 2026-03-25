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
<style>{xterm_css}</style>
<script>{xterm_js}</script>
<script>{xterm_fit_js}</script>
<script>window.fridiTerminals = {{}};</script>"#,
                    css = styles::APP_CSS,
                    xterm_css = include_str!("../assets/xterm.css"),
                    xterm_js = include_str!("../assets/xterm.js"),
                    xterm_fit_js = include_str!("../assets/xterm-addon-fit.min.js"),
                )),
        )
        .with_context(app::DetectedRepo(repo))
        .launch(app::App);
}

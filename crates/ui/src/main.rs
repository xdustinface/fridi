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
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("fridi")),
        )
        .with_context(app::DetectedRepo(repo))
        .launch(app::App);
}

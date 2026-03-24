mod app;
mod components;
mod engine_bridge;
mod state;
mod styles;
mod workflow_runner;

fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("fridi")),
        )
        .launch(app::App);
}

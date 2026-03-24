mod app;
mod components;
mod engine_bridge;
mod state;
mod styles;
mod workflow_runner;

fn main() { dioxus::launch(app::App); }

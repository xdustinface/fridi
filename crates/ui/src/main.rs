use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            class: "app",
            h1 { "conductor" }
            p { "AI Workflow Orchestrator" }
        }
    }
}

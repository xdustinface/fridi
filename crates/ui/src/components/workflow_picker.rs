use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::schema::Workflow;

#[component]
pub(crate) fn WorkflowPicker(
    workflows: Vec<(Workflow, PathBuf)>,
    on_select: EventHandler<(Workflow, PathBuf)>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "picker-overlay",
            onclick: move |_| on_cancel.call(()),
            div {
                class: "picker-modal",
                onclick: move |evt| evt.stop_propagation(),
                h3 { "Start a new workflow" }
                div { class: "picker-list",
                    for (wf, path) in &workflows {
                        {
                            let wf_clone = wf.clone();
                            let path_clone = path.clone();
                            rsx! {
                                div {
                                    key: "{wf.name}",
                                    class: "picker-item",
                                    onclick: move |_| on_select.call((wf_clone.clone(), path_clone.clone())),
                                    span { class: "picker-item-name", "{wf.name}" }
                                    if let Some(desc) = &wf.description {
                                        span { class: "picker-item-desc", "{desc}" }
                                    }
                                }
                            }
                        }
                    }
                    if workflows.is_empty() {
                        div { class: "picker-empty",
                            "No workflows found in ./workflows/"
                        }
                    }
                }
            }
        }
    }
}

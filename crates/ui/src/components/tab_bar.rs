use conductor_core::session::SessionStatus;
use dioxus::prelude::*;

use crate::state::TabInfo;

#[component]
pub(crate) fn TabBar(
    tabs: Vec<TabInfo>,
    active: Option<usize>,
    on_select: EventHandler<usize>,
    on_close: EventHandler<usize>,
    on_new: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "tab-bar",
            for (idx, tab) in tabs.iter().enumerate() {
                {
                    let is_active = active == Some(idx);
                    let tab_class = if is_active { "tab active" } else { "tab" };
                    let status_class = match &tab.status {
                        SessionStatus::Running => "running",
                        SessionStatus::Completed => "completed",
                        SessionStatus::Failed => "failed",
                        SessionStatus::Interrupted => "failed",
                    };
                    rsx! {
                        div {
                            key: "{tab.session_id}",
                            class: "{tab_class}",
                            onclick: move |_| on_select.call(idx),
                            div { class: "status-dot {status_class}" }
                            span { class: "tab-name", "{tab.workflow_name}" }
                            button {
                                class: "tab-close",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    on_close.call(idx);
                                },
                                "x"
                            }
                        }
                    }
                }
            }
            button {
                class: "tab-new",
                onclick: move |_| on_new.call(()),
                "+"
            }
        }
    }
}

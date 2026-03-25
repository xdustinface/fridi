use dioxus::prelude::*;
use fridi_core::session::SessionStatus;

use crate::state::TabInfo;

#[component]
pub(crate) fn TabBar(
    tabs: Vec<TabInfo>,
    active: Option<usize>,
    home_active: bool,
    backlog_active: bool,
    on_select: EventHandler<usize>,
    on_select_home: EventHandler<()>,
    on_select_backlog: EventHandler<()>,
    on_close: EventHandler<usize>,
    on_new: EventHandler<()>,
) -> Element {
    let home_class = if home_active {
        "tab home-tab active"
    } else {
        "tab home-tab"
    };

    let backlog_class = if backlog_active {
        "tab home-tab active"
    } else {
        "tab home-tab"
    };

    rsx! {
        div { class: "tab-bar",
            div {
                class: "{home_class}",
                onclick: move |_| on_select_home.call(()),
                span { class: "tab-name", "Home" }
            }
            div {
                class: "{backlog_class}",
                onclick: move |_| on_select_backlog.call(()),
                span { class: "tab-name", "Backlog" }
            }
            for (idx , tab) in tabs.iter().enumerate() {
                {
                    let is_active = !home_active && !backlog_active && active == Some(idx);
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
                            span { class: "tab-session-id", "{tab.session_id}" }
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
            button { class: "tab-new", onclick: move |_| on_new.call(()), "+" }
        }
    }
}

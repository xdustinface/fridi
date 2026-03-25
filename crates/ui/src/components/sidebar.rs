use dioxus::prelude::*;
use fridi_core::session::SessionStatus;

use crate::state::TabInfo;

#[component]
pub(crate) fn Sidebar(
    pinned: Signal<bool>,
    tabs: Vec<TabInfo>,
    active_session_idx: Option<usize>,
    on_session_select: EventHandler<usize>,
    on_new_session: EventHandler<()>,
) -> Element {
    let mut hovered = use_signal(|| false);
    let is_pinned = *pinned.read();
    let is_expanded = is_pinned || *hovered.read();

    let sidebar_class = if is_pinned {
        "sidebar expanded pinned"
    } else if is_expanded {
        "sidebar expanded"
    } else {
        "sidebar"
    };

    let pin_label = if is_pinned { "Unpin" } else { "Pin" };

    rsx! {
        // Backdrop when expanded but not pinned
        if is_expanded && !is_pinned {
            div {
                class: "sidebar-backdrop",
                onclick: move |_| {
                    hovered.set(false);
                },
            }
        }

        // Hover edge strip (always visible)
        div {
            class: "sidebar-edge",
            onmouseenter: move |_| {
                hovered.set(true);
            },
        }

        // Sidebar panel
        div {
            class: "{sidebar_class}",
            onmouseenter: move |_| {
                hovered.set(true);
            },
            onmouseleave: move |_| {
                if !is_pinned {
                    hovered.set(false);
                }
            },

            // Header
            div { class: "sidebar-header",
                span { class: "sidebar-brand", "fridi" }
                button {
                    class: "sidebar-pin-btn",
                    title: "{pin_label}",
                    onclick: move |_| {
                        let current = *pinned.read();
                        pinned.set(!current);
                    },
                    if is_pinned { "||" } else { ">>" }
                }
            }

            // Sessions section
            div { class: "sidebar-section",
                div { class: "sidebar-section-header",
                    span { "Sessions" }
                    button {
                        class: "sidebar-add-btn",
                        onclick: move |_| on_new_session.call(()),
                        "+"
                    }
                }
                if tabs.is_empty() {
                    div { class: "sidebar-empty", "No sessions" }
                } else {
                    div { class: "sidebar-list",
                        for (idx , tab) in tabs.iter().enumerate() {
                            {
                                let is_active = active_session_idx == Some(idx);
                                let item_class = if is_active {
                                    "sidebar-item active"
                                } else {
                                    "sidebar-item"
                                };
                                let status_class = match &tab.status {
                                    SessionStatus::Running => "running",
                                    SessionStatus::Completed => "completed",
                                    SessionStatus::Failed => "failed",
                                    SessionStatus::Interrupted => "failed",
                                };
                                rsx! {
                                    div {
                                        key: "{tab.session_id}",
                                        class: "{item_class}",
                                        onclick: move |_| on_session_select.call(idx),
                                        div { class: "status-dot {status_class}" }
                                        span { class: "sidebar-item-name", "{tab.workflow_name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Footer with keyboard hint
            div { class: "sidebar-footer",
                span { class: "sidebar-hint", "Esc to close" }
            }
        }
    }
}

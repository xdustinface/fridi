use dioxus::prelude::*;

#[component]
pub(crate) fn ConfirmDialog(
    title: String,
    message: String,
    confirm_label: String,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "confirm-dialog-overlay",
            onclick: move |_| on_cancel.call(()),
            div {
                class: "confirm-dialog",
                onclick: move |evt| evt.stop_propagation(),
                h3 { "{title}" }
                p { class: "confirm-dialog-message", "{message}" }
                div { class: "confirm-dialog-actions",
                    button {
                        class: "confirm-dialog-btn cancel",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "confirm-dialog-btn confirm",
                        onclick: move |_| on_confirm.call(()),
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}

use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::backlog::Backlog;
use tracing::error;

const BACKLOG_PATH: &str = ".fridi/backlog.md";

#[component]
pub(crate) fn QuickCapture(context: Option<String>, on_dismiss: EventHandler<()>) -> Element {
    let mut input_text = use_signal(String::new);

    let save_and_dismiss = {
        let context = context.clone();
        move |_| {
            let text = input_text.read().trim().to_string();
            if !text.is_empty() {
                let path = PathBuf::from(BACKLOG_PATH);
                match Backlog::load(&path) {
                    Ok(mut backlog) => {
                        backlog.add(&text, context.as_deref());
                        if let Err(e) = backlog.save() {
                            error!("failed to save backlog: {e}");
                        }
                    }
                    Err(e) => {
                        error!("failed to load backlog: {e}");
                    }
                }
            }
            on_dismiss.call(());
        }
    };

    let on_key = {
        let save = save_and_dismiss.clone();
        move |evt: KeyboardEvent| match evt.key() {
            Key::Enter => {
                evt.prevent_default();
                save(());
            }
            Key::Escape => {
                on_dismiss.call(());
            }
            _ => {}
        }
    };

    rsx! {
        div {
            class: "quick-capture-overlay",
            onclick: move |_| on_dismiss.call(()),
            div {
                class: "quick-capture-modal",
                onclick: move |evt| evt.stop_propagation(),
                onkeydown: on_key,
                if let Some(ctx) = &context {
                    div { class: "quick-capture-context", "{ctx}" }
                }
                input {
                    class: "quick-capture-input",
                    placeholder: "Capture an idea... (supports #tags and !/!! priority)",
                    value: "{input_text}",
                    oninput: move |evt| input_text.set(evt.value()),
                    onmounted: move |evt: MountedEvent| {
                        spawn(async move {
                            let _ = evt.set_focus(true).await;
                        });
                    },
                }
                div { class: "quick-capture-hint", "Enter to save, Esc to dismiss" }
            }
        }
    }
}

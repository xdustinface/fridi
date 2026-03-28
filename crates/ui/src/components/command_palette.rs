use dioxus::prelude::*;

use crate::components::session_creator::SessionSource;

#[component]
pub(crate) fn CommandPalette(
    on_create: EventHandler<SessionSource>,
    on_dismiss: EventHandler<()>,
) -> Element {
    let mut input_text = use_signal(String::new);

    let submit = move |_| {
        let text = input_text.read().trim().to_string();
        if text.is_empty() {
            return;
        }
        let source = parse_input(&text);
        on_create.call(source);
    };

    let on_key = {
        move |evt: KeyboardEvent| match evt.key() {
            Key::Enter => {
                evt.prevent_default();
                submit(());
            }
            Key::Escape => {
                on_dismiss.call(());
            }
            _ => {}
        }
    };

    rsx! {
        div {
            class: "command-palette-backdrop",
            onclick: move |_| on_dismiss.call(()),
            div {
                class: "command-palette",
                onclick: move |evt| evt.stop_propagation(),
                onkeydown: on_key,
                input {
                    class: "command-palette-input",
                    placeholder: "Describe what you want to work on... (#issue, !pr)",
                    value: "{input_text}",
                    oninput: move |evt| input_text.set(evt.value()),
                    onmounted: move |evt: MountedEvent| {
                        spawn(async move {
                            let _ = evt.set_focus(true).await;
                        });
                    },
                }
                div { class: "command-palette-hint",
                    "Enter to start \u{00b7} Esc to close \u{00b7} #N for issue \u{00b7} !N for PR"
                }
            }
        }
    }
}

fn parse_input(text: &str) -> SessionSource {
    if let Some(rest) = text.strip_prefix('#') {
        if let Ok(number) = rest.parse::<u64>() {
            return SessionSource::Issue {
                number,
                title: format!("Issue #{number}"),
            };
        }
    }
    if let Some(rest) = text.strip_prefix('!') {
        if let Ok(number) = rest.parse::<u64>() {
            return SessionSource::PR {
                number,
                title: format!("PR #{number}"),
                head_ref: String::new(),
            };
        }
    }
    SessionSource::Prompt {
        text: text.to_string(),
    }
}

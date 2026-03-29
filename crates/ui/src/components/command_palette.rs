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
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('#') {
        if let Ok(number) = rest.trim().parse::<u64>() {
            return SessionSource::Issue {
                number,
                title: format!("Issue #{number}"),
            };
        }
    }
    if let Some(rest) = text.strip_prefix('!') {
        if let Ok(number) = rest.trim().parse::<u64>() {
            return SessionSource::PR {
                number,
                title: format!("PR #{number}"),
                head_ref: format!("pr/{number}"),
            };
        }
    }
    SessionSource::Prompt {
        text: text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_issue_number() {
        let result = parse_input("#123");
        assert!(matches!(result, SessionSource::Issue { number: 123, .. }));
    }

    #[test]
    fn parse_pr_number() {
        let result = parse_input("!45");
        match result {
            SessionSource::PR {
                number, head_ref, ..
            } => {
                assert_eq!(number, 45);
                assert_eq!(head_ref, "pr/45");
            }
            other => panic!("expected PR, got {other:?}"),
        }
    }

    #[test]
    fn parse_plain_text() {
        let result = parse_input("fix the login bug");
        match result {
            SessionSource::Prompt { text } => assert_eq!(text, "fix the login bug"),
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn parse_whitespace_around_prefix() {
        assert!(matches!(
            parse_input("  #123  "),
            SessionSource::Issue { number: 123, .. }
        ));
        assert!(matches!(
            parse_input("  !45  "),
            SessionSource::PR { number: 45, .. }
        ));
    }

    #[test]
    fn parse_whitespace_after_prefix() {
        assert!(matches!(
            parse_input("# 123"),
            SessionSource::Issue { number: 123, .. }
        ));
        assert!(matches!(
            parse_input("! 45"),
            SessionSource::PR { number: 45, .. }
        ));
    }

    #[test]
    fn parse_non_numeric_after_prefix_falls_back_to_prompt() {
        assert!(matches!(parse_input("#abc"), SessionSource::Prompt { .. }));
        assert!(matches!(parse_input("!xyz"), SessionSource::Prompt { .. }));
    }

    #[test]
    fn parse_empty_and_whitespace_only_returns_prompt() {
        match parse_input("") {
            SessionSource::Prompt { text } => assert_eq!(text, ""),
            other => panic!("expected Prompt, got {other:?}"),
        }
        match parse_input("   ") {
            SessionSource::Prompt { text } => assert_eq!(text, ""),
            other => panic!("expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn parse_prefix_alone_falls_back_to_prompt() {
        assert!(matches!(parse_input("#"), SessionSource::Prompt { .. }));
        assert!(matches!(parse_input("!"), SessionSource::Prompt { .. }));
    }
}

use dioxus::prelude::*;
use fridi_core::window_state::WindowState;

/// Data for a single recent repo entry.
#[derive(Clone, PartialEq)]
pub(crate) struct RecentRepo {
    pub(crate) name: String,
    pub(crate) session_count: usize,
}

/// Extracts recent repos from window state. Each key in the windows map
/// is a repo identifier (e.g. "owner/repo").
pub(crate) fn recent_repos_from_state(state: &WindowState) -> Vec<RecentRepo> {
    let mut repos: Vec<RecentRepo> = state
        .windows
        .iter()
        .filter(|(key, _)| !key.is_empty())
        .map(|(key, info)| RecentRepo {
            name: key.clone(),
            session_count: info.open_sessions.len(),
        })
        .collect();
    repos.sort_by(|a, b| a.name.cmp(&b.name));
    repos
}

#[component]
pub(crate) fn WelcomeScreen(repos: Vec<RecentRepo>, on_new_session: EventHandler<()>) -> Element {
    rsx! {
        div { class: "welcome-screen",
            div { class: "welcome-card",
                div { class: "welcome-logo", "fridi" }
                div { class: "welcome-tagline", "AI workflow orchestrator" }

                if !repos.is_empty() {
                    div { class: "welcome-section",
                        div { class: "welcome-section-title", "Recent Repos" }
                        div { class: "welcome-repo-list",
                            for repo in &repos {
                                {
                                    let suffix = if repo.session_count != 1 { "s" } else { "" };
                                    let count = repo.session_count;
                                    rsx! {
                                        div {
                                            key: "{repo.name}",
                                            class: "welcome-repo-row",
                                            div { class: "welcome-repo-name", "{repo.name}" }
                                            div { class: "welcome-repo-meta",
                                                "{count} session{suffix}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "welcome-actions",
                    button {
                        class: "welcome-btn primary",
                        onclick: move |_| on_new_session.call(()),
                        "New Session"
                    }
                }

                div { class: "welcome-hint", "Press Cmd+T to start a new session" }
            }
        }
    }
}

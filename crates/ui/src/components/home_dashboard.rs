use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::github::CiStatus;
use fridi_core::project_overview::ProjectOverview;
use fridi_core::session::SessionStore;

const SESSIONS_DIR: &str = ".fridi/sessions";
const POLL_INTERVAL_SECS: u64 = 60;

#[derive(Clone)]
enum FetchState {
    Loading,
    Loaded(ProjectOverview),
    Error(String),
}

/// Parses an ISO 8601 date string and returns a human-readable relative time.
fn relative_time(iso: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return iso.to_string();
    };
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(dt);

    if delta.num_days() > 0 {
        let days = delta.num_days();
        if days == 1 {
            "1d ago".into()
        } else {
            format!("{days}d ago")
        }
    } else if delta.num_hours() > 0 {
        let hours = delta.num_hours();
        if hours == 1 {
            "1h ago".into()
        } else {
            format!("{hours}h ago")
        }
    } else {
        let mins = delta.num_minutes().max(1);
        if mins == 1 {
            "1m ago".into()
        } else {
            format!("{mins}m ago")
        }
    }
}

fn ci_badge_class(status: &CiStatus) -> &'static str {
    match status {
        CiStatus::Passed => "ci-badge passed",
        CiStatus::Failed => "ci-badge failed",
        CiStatus::Pending => "ci-badge pending",
        CiStatus::None => "ci-badge none",
    }
}

fn ci_badge_label(status: &CiStatus) -> &'static str {
    match status {
        CiStatus::Passed => "passed",
        CiStatus::Failed => "failed",
        CiStatus::Pending => "pending",
        CiStatus::None => "",
    }
}

#[component]
pub(crate) fn HomeDashboard(repo: Option<String>) -> Element {
    let mut state = use_signal(|| FetchState::Loading);
    let repo_clone = repo.clone();

    // Fetch on mount and poll every 60s
    use_coroutine(move |_: UnboundedReceiver<()>| {
        let repo = repo_clone.clone();
        async move {
            loop {
                let overview = {
                    let repo = repo.clone();
                    tokio::task::spawn_blocking(move || {
                        let repo_str = repo.as_deref().unwrap_or("");
                        let work_dir = PathBuf::from(".");
                        let store = SessionStore::new(SESSIONS_DIR);
                        fridi_core::project_overview::fetch_project_overview(
                            repo_str, &work_dir, &store,
                        )
                    })
                    .await
                };

                match overview {
                    Ok(Ok(data)) => state.set(FetchState::Loaded(data)),
                    Ok(Err(e)) => state.set(FetchState::Error(e.to_string())),
                    Err(e) => state.set(FetchState::Error(e.to_string())),
                }

                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    });

    let current = state.read().clone();

    match current {
        FetchState::Loading => rsx! {
            div { class: "dashboard",
                div { class: "dashboard-loading", "Loading project overview..." }
            }
        },
        FetchState::Error(msg) => rsx! {
            div { class: "dashboard",
                div { class: "dashboard-error", "Failed to load overview: {msg}" }
            }
        },
        FetchState::Loaded(overview) => rsx! {
            div { class: "dashboard",
                // PRs section
                DashboardSection {
                    title: "Open Pull Requests",
                    empty_msg: "No open PRs",
                    count: overview.open_prs.len(),
                    children: rsx! {
                        for pr in &overview.open_prs {
                            div { class: "dashboard-row", key: "pr-{pr.number}",
                                span { class: "dashboard-number", "#{pr.number}" }
                                span { class: "dashboard-title", "{pr.title}" }
                                span { class: "dashboard-branch", "{pr.branch}" }
                                span { class: ci_badge_class(&pr.ci_status), "{ci_badge_label(&pr.ci_status)}" }
                                span { class: "dashboard-time", "{relative_time(&pr.updated_at)}" }
                            }
                        }
                    },
                }

                // Issues section
                DashboardSection {
                    title: "Open Issues",
                    empty_msg: "No open issues",
                    count: overview.open_issues.len(),
                    children: rsx! {
                        for issue in &overview.open_issues {
                            div { class: "dashboard-row", key: "issue-{issue.number}",
                                span { class: "dashboard-number", "#{issue.number}" }
                                span { class: "dashboard-title", "{issue.title}" }
                                if !issue.labels.is_empty() {
                                    span { class: "dashboard-labels",
                                        for label in &issue.labels {
                                            span { class: "dashboard-label", "{label}" }
                                        }
                                    }
                                }
                                span { class: "dashboard-time", "{relative_time(&issue.updated_at)}" }
                            }
                        }
                    },
                }

                // Running sessions section
                DashboardSection {
                    title: "Running Sessions",
                    empty_msg: "No running sessions",
                    count: overview.running_sessions,
                    children: rsx! {
                        div { class: "dashboard-row",
                            span { class: "dashboard-title", "{overview.running_sessions} session(s) currently running" }
                        }
                    },
                }
            }
        },
    }
}

#[component]
fn DashboardSection(title: String, empty_msg: String, count: usize, children: Element) -> Element {
    rsx! {
        div { class: "dashboard-section",
            div { class: "dashboard-section-header",
                h3 { "{title}" }
                span { class: "dashboard-count", "{count}" }
            }
            if count == 0 {
                div { class: "dashboard-empty", "{empty_msg}" }
            } else {
                div { class: "dashboard-list", {children} }
            }
        }
    }
}

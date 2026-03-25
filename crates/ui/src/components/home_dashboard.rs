use std::collections::HashSet;
use std::path::PathBuf;

use dioxus::prelude::*;
use fridi_core::github::{self, CiStatus};
use fridi_core::project_overview::ProjectOverview;
use fridi_core::session::SessionStore;

use crate::components::session_creator::SessionSource;

const SESSIONS_DIR: &str = ".fridi/sessions";
const POLL_INTERVAL_SECS: u64 = 60;

#[derive(Clone)]
enum FetchState {
    Loading,
    Loaded(ProjectOverview),
    Error(String),
}

#[derive(Clone, PartialEq)]
enum PickState {
    Idle,
    Loading,
    Error(String),
}

/// Parses an ISO 8601 date string and returns a human-readable relative time.
fn relative_time(iso: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return iso.to_string();
    };
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(dt);

    if delta.num_seconds() < 0 {
        return "just now".into();
    }

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
pub(crate) fn HomeDashboard(
    repo: Option<String>,
    on_pick_issue: EventHandler<SessionSource>,
    on_show_pr_picker: EventHandler<()>,
    on_show_creator: EventHandler<()>,
) -> Element {
    let mut state = use_signal(|| FetchState::Loading);
    let mut pick_state = use_signal(|| PickState::Idle);
    let mut removing_labels: Signal<HashSet<u64>> = use_signal(HashSet::new);
    let repo_clone = repo.clone();
    let work_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Fetch on mount and poll every 60s
    use_coroutine(move |_: UnboundedReceiver<()>| {
        let repo = repo_clone.clone();
        let work_dir = work_dir.clone();
        async move {
            loop {
                let overview = {
                    let repo = repo.clone();
                    let work_dir = work_dir.clone();
                    tokio::task::spawn_blocking(move || {
                        let repo_str = repo.as_deref().unwrap_or("");
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
        FetchState::Loaded(overview) => {
            let has_repo = repo.as_ref().is_some_and(|r| !r.is_empty());
            let is_picking = *pick_state.read() == PickState::Loading;

            rsx! {
                div { class: "dashboard",
                    // Quick actions strip
                    div { class: "quick-actions",
                        button {
                            class: "quick-action-btn primary",
                            disabled: !has_repo || is_picking,
                            onclick: {
                                let repo_str = repo.clone().unwrap_or_default();
                                move |_| {
                                    let r = repo_str.clone();
                                    pick_state.set(PickState::Loading);
                                    spawn(async move {
                                        let result = tokio::task::spawn_blocking(move || {
                                            github::auto_pick_issue(&r)
                                        }).await;
                                        match result {
                                            Ok(Ok(Some(issue))) => {
                                                pick_state.set(PickState::Idle);
                                                on_pick_issue.call(SessionSource::Issue {
                                                    number: issue.number,
                                                    title: issue.title,
                                                });
                                            }
                                            Ok(Ok(None)) => {
                                                pick_state.set(PickState::Error("No open issues found".into()));
                                            }
                                            Ok(Err(e)) => {
                                                pick_state.set(PickState::Error(e.to_string()));
                                            }
                                            Err(e) => {
                                                pick_state.set(PickState::Error(e.to_string()));
                                            }
                                        }
                                    });
                                }
                            },
                            match &*pick_state.read() {
                                PickState::Loading => rsx! { "Finding issue..." },
                                PickState::Error(msg) => rsx! { span { class: "quick-action-error", "{msg}" } },
                                PickState::Idle => rsx! { "Pick up highest priority issue" },
                            }
                        }
                        button {
                            class: "quick-action-btn",
                            disabled: !has_repo,
                            onclick: move |_| on_show_pr_picker.call(()),
                            "Review open PRs"
                        }
                        button {
                            class: "quick-action-btn",
                            onclick: move |_| on_show_creator.call(()),
                            "Run workflow..."
                        }
                    }

                    // PRs section
                    DashboardSection {
                        title: "Open Pull Requests",
                        empty_msg: "No open PRs",
                        count: overview.open_prs.len(),
                        children: rsx! {
                            for pr in &overview.open_prs {
                                {
                                    let needs_human = pr.labels.iter().any(|l| l == "needs-human");
                                    let pr_number = pr.number;
                                    let is_removing = removing_labels.read().contains(&pr_number);
                                    rsx! {
                                        div { class: "dashboard-row", key: "pr-{pr_number}",
                                            span { class: "dashboard-number", "#{pr.number}" }
                                            span { class: "dashboard-title", "{pr.title}" }
                                            span { class: "dashboard-branch", "{pr.branch}" }
                                            span { class: ci_badge_class(&pr.ci_status), "{ci_badge_label(&pr.ci_status)}" }
                                            if needs_human {
                                                button {
                                                    class: "dashboard-ready-btn",
                                                    disabled: is_removing || !has_repo,
                                                    onclick: {
                                                        let repo_str = repo.clone().unwrap_or_default();
                                                        move |_| {
                                                            let r = repo_str.clone();
                                                            removing_labels.write().insert(pr_number);
                                                            spawn(async move {
                                                                let result = tokio::task::spawn_blocking(move || {
                                                                    github::remove_pr_label(&r, pr_number, "needs-human")
                                                                }).await;
                                                                removing_labels.write().remove(&pr_number);
                                                                if let Ok(Ok(())) = result {
                                                                    if let FetchState::Loaded(ref mut data) = *state.write() {
                                                                        if let Some(pr) = data.open_prs.iter_mut().find(|p| p.number == pr_number) {
                                                                            pr.labels.retain(|l| l != "needs-human");
                                                                        }
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    },
                                                    if is_removing { "Removing..." } else { "Ready" }
                                                }
                                            }
                                            span { class: "dashboard-time", "{relative_time(&pr.updated_at)}" }
                                        }
                                    }
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
                                span { class: "dashboard-title",
                                    if overview.running_sessions == 1 {
                                        "1 session currently running"
                                    } else {
                                        "{overview.running_sessions} sessions currently running"
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_invalid_input() {
        assert_eq!(relative_time("not-a-date"), "not-a-date");
    }

    #[test]
    fn relative_time_days_ago() {
        let two_days_ago = (chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339();
        assert_eq!(relative_time(&two_days_ago), "2d ago");
    }

    #[test]
    fn relative_time_one_day_ago() {
        let one_day_ago = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        assert_eq!(relative_time(&one_day_ago), "1d ago");
    }

    #[test]
    fn relative_time_hours_ago() {
        let three_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(3)).to_rfc3339();
        assert_eq!(relative_time(&three_hours_ago), "3h ago");
    }

    #[test]
    fn relative_time_one_hour_ago() {
        let one_hour_ago = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(relative_time(&one_hour_ago), "1h ago");
    }

    #[test]
    fn relative_time_minutes_ago() {
        let five_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        assert_eq!(relative_time(&five_min_ago), "5m ago");
    }

    #[test]
    fn relative_time_one_minute_ago() {
        let one_min_ago = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
        assert_eq!(relative_time(&one_min_ago), "1m ago");
    }

    #[test]
    fn relative_time_seconds_ago_shows_one_minute() {
        let ten_sec_ago = (chrono::Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
        assert_eq!(relative_time(&ten_sec_ago), "1m ago");
    }

    #[test]
    fn relative_time_future_timestamp() {
        let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(relative_time(&future), "just now");
    }
}

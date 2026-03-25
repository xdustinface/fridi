use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use dioxus::prelude::*;
use fridi_core::github::{self, CiStatus};
use fridi_core::project_overview::{IssueSummary, ProjectOverview};
use fridi_core::session::SessionStore;

use crate::components::session_creator::SessionSource;
use crate::components::toast::{ToastLevel, ToastMessage, Toasts, push_toast};

static CACHED_OVERVIEW: OnceLock<Mutex<Option<ProjectOverview>>> = OnceLock::new();

const SESSIONS_DIR: &str = ".fridi/sessions";
const POLL_INTERVAL_SECS: u64 = 60;

/// Pre-populate the overview cache in the background so the dashboard is
/// instant on first visit.
pub(crate) fn warm_overview_cache(repo: Option<String>) {
    let work_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    dioxus::prelude::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let repo_str = repo.as_deref().unwrap_or("");
            let store = SessionStore::new(SESSIONS_DIR);
            fridi_core::project_overview::fetch_project_overview(repo_str, &work_dir, &store)
        })
        .await;
        if let Ok(Ok(data)) = result {
            let cache = CACHED_OVERVIEW.get_or_init(|| Mutex::new(None));
            *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(data);
        }
    });
}

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

/// A parsed checkbox line from an issue body.
struct CheckboxLine {
    checked: bool,
    text: String,
    line_index: usize,
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

/// Returns the CSS class for the sync dot based on state.
fn dot_state_class(is_refreshing: bool, failed: bool, seconds_since_fetch: u32) -> &'static str {
    if is_refreshing {
        "warning"
    } else if failed || seconds_since_fetch > 120 {
        "error"
    } else {
        "success"
    }
}

/// Returns the detail text displayed below the status label.
fn sync_detail(is_refreshing: bool, failed: bool, seconds_since_fetch: u32) -> String {
    if is_refreshing {
        "Refreshing...".to_string()
    } else if failed {
        "Last sync failed".to_string()
    } else if seconds_since_fetch > 120 {
        let mins = seconds_since_fetch / 60;
        format!("Stale \u{2014} {mins}m ago")
    } else {
        format!("Updated {seconds_since_fetch}s ago")
    }
}

/// Returns the short label text displayed next to the status dot.
fn sync_label(is_refreshing: bool, failed: bool, seconds_since_fetch: u32) -> &'static str {
    if is_refreshing {
        "Syncing..."
    } else if failed || seconds_since_fetch > 120 {
        "Stale"
    } else {
        "Synced"
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

fn check_icon_and_class(conclusion: &Option<String>, status: &str) -> (&'static str, &'static str) {
    match conclusion.as_deref() {
        Some("success") => ("ok", "check-passed"),
        Some("failure" | "error" | "timed_out" | "cancelled") => ("x", "check-failed"),
        _ if status == "completed" => ("?", "check-pending"),
        _ => ("~", "check-pending"),
    }
}

fn review_badge_class(decision: &str) -> &'static str {
    match decision {
        "APPROVED" => "review-badge approved",
        "CHANGES_REQUESTED" => "review-badge changes-requested",
        _ => "review-badge review-required",
    }
}

fn parse_checkboxes(body: &str) -> Vec<CheckboxLine> {
    body.lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed
                .strip_prefix("- [x] ")
                .or_else(|| trimmed.strip_prefix("- [X] "))
            {
                Some(CheckboxLine {
                    checked: true,
                    text: rest.to_string(),
                    line_index,
                })
            } else {
                trimmed.strip_prefix("- [ ] ").map(|rest| CheckboxLine {
                    checked: false,
                    text: rest.to_string(),
                    line_index,
                })
            }
        })
        .collect()
}

fn toggle_checkbox_in_body(body: &str, line_index: usize) -> String {
    let had_trailing_newline = body.ends_with('\n');
    let mut result: String = body
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if i == line_index {
                let trimmed = line.trim_start();
                let prefix = &line[..line.len() - trimmed.len()];
                if let Some(rest) = trimmed
                    .strip_prefix("- [x] ")
                    .or_else(|| trimmed.strip_prefix("- [X] "))
                {
                    format!("{prefix}- [ ] {rest}")
                } else if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
                    format!("{prefix}- [x] {rest}")
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if had_trailing_newline {
        result.push('\n');
    }
    result
}

fn open_url(url: &str) {
    if !url.starts_with("https://") {
        return;
    }
    let url = url.to_string();
    spawn(async move {
        match tokio::task::spawn_blocking(move || open::that(&url)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("Failed to open URL: {e}"),
            Err(e) => tracing::warn!("Failed to open URL: {e}"),
        }
    });
}

/// Return the pending body if one exists, otherwise the issue's original body.
fn get_effective_body(issue: &IssueSummary, pending: &HashMap<u64, String>) -> String {
    pending
        .get(&issue.number)
        .cloned()
        .unwrap_or_else(|| issue.body.clone().unwrap_or_default())
}

#[component]
pub(crate) fn HomeDashboard(
    repo: Option<String>,
    on_pick_issue: EventHandler<SessionSource>,
    on_show_pr_picker: EventHandler<()>,
    on_show_creator: EventHandler<()>,
) -> Element {
    let initial_state = {
        let cache = CACHED_OVERVIEW.get_or_init(|| Mutex::new(None));
        match cache.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            Some(data) => FetchState::Loaded(data),
            None => FetchState::Loading,
        }
    };
    let mut state = use_signal(|| initial_state);
    let mut pick_state = use_signal(|| PickState::Idle);
    let mut removing_labels: Signal<HashSet<u64>> = use_signal(HashSet::new);
    let mut pending_bodies: Signal<HashMap<u64, String>> = use_signal(HashMap::new);
    let mut saving_issues: Signal<HashSet<u64>> = use_signal(HashSet::new);
    let mut is_refreshing = use_signal(|| true);
    let mut seconds_since_fetch: Signal<u32> = use_signal(|| 0);
    let mut fetch_failed = use_signal(|| false);
    let mut expanded_prs: Signal<HashSet<u64>> = use_signal(HashSet::new);
    let mut expanded_issues: Signal<HashSet<u64>> = use_signal(HashSet::new);
    let mut toasts = use_context::<Toasts>().0;
    let repo_clone = repo.clone();
    let work_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Fetch on mount and poll every 60s; also re-fetches when seconds_since_fetch
    // is manually reset to 0 (via the refresh button) while a poll sleep is active.
    use_coroutine(move |_: UnboundedReceiver<()>| {
        let repo = repo_clone.clone();
        let work_dir = work_dir.clone();
        async move {
            loop {
                is_refreshing.set(true);
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
                    Ok(Ok(data)) => {
                        let cache = CACHED_OVERVIEW.get_or_init(|| Mutex::new(None));
                        *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(data.clone());
                        state.set(FetchState::Loaded(data));
                        seconds_since_fetch.set(0);
                        fetch_failed.set(false);
                    }
                    Ok(Err(e)) => {
                        state.set(FetchState::Error(e.to_string()));
                        fetch_failed.set(true);
                    }
                    Err(e) => {
                        state.set(FetchState::Error(e.to_string()));
                        fetch_failed.set(true);
                    }
                }
                is_refreshing.set(false);

                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    });

    // Tick every second to update the countdown ring
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if !*is_refreshing.read() {
                let current = *seconds_since_fetch.read();
                seconds_since_fetch.set(current.saturating_add(1));
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

            let refreshing = *is_refreshing.read();
            let secs = *seconds_since_fetch.read();
            let failed = *fetch_failed.read();

            let dot_class = dot_state_class(refreshing, failed, secs);
            let detail_text = sync_detail(refreshing, failed, secs);
            let label_text = sync_label(refreshing, failed, secs);

            rsx! {
                div { class: "dashboard",
                    // Sync status indicator with dot, label, and detail text
                    div {
                        class: "sync-status",
                        onclick: {
                            let repo = repo.clone();
                            move |_| {
                                if *is_refreshing.read() {
                                    return;
                                }
                                seconds_since_fetch.set(0);
                                is_refreshing.set(true);
                                let repo = repo.clone();
                                let work_dir = std::env::current_dir()
                                    .unwrap_or_else(|_| PathBuf::from("."));
                                spawn(async move {
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
                                        Ok(Ok(data)) => {
                                            let cache = CACHED_OVERVIEW
                                                .get_or_init(|| Mutex::new(None));
                                            *cache.lock().unwrap_or_else(|e| e.into_inner()) =
                                                Some(data.clone());
                                            state.set(FetchState::Loaded(data));
                                            seconds_since_fetch.set(0);
                                            fetch_failed.set(false);
                                        }
                                        Ok(Err(e)) => {
                                            state.set(FetchState::Error(e.to_string()));
                                            fetch_failed.set(true);
                                        }
                                        Err(e) => {
                                            state.set(FetchState::Error(e.to_string()));
                                            fetch_failed.set(true);
                                        }
                                    }
                                    is_refreshing.set(false);
                                });
                            }
                        },
                        div { class: "sync-dot {dot_class}" }
                        div { class: "sync-text",
                            span { class: "sync-label", "{label_text}" }
                            span { class: "sync-detail", "{detail_text}" }
                        }
                    }

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
                                    let is_expanded = expanded_prs.read().contains(&pr_number);
                                    let chevron_class = if is_expanded { "expand-chevron expanded" } else { "expand-chevron" };
                                    let row_class = "dashboard-row dashboard-row-clickable";
                                    let pr_url = pr.url.clone();
                                    let pr_clone = pr.clone();
                                    rsx! {
                                        div { key: "pr-{pr_number}",
                                            div {
                                                class: "{row_class}",
                                                onclick: move |_| {
                                                    let mut set = expanded_prs.write();
                                                    if !set.remove(&pr_number) {
                                                        set.insert(pr_number);
                                                    }
                                                },
                                                span { class: "{chevron_class}", ">" }
                                                span {
                                                    class: "dashboard-link",
                                                    onclick: {
                                                        let url = pr_url.clone();
                                                        move |evt: Event<MouseData>| {
                                                            evt.stop_propagation();
                                                            open_url(&url);
                                                        }
                                                    },
                                                    "#{pr_number}"
                                                }
                                                span { class: "dashboard-title", "{pr_clone.title}" }
                                                span { class: "dashboard-branch", "{pr_clone.branch}" }
                                                span { class: ci_badge_class(&pr_clone.ci_status), "{ci_badge_label(&pr_clone.ci_status)}" }
                                                if needs_human {
                                                    button {
                                                        class: "dashboard-ready-btn",
                                                        disabled: is_removing || !has_repo,
                                                        onclick: {
                                                            let repo_str = repo.clone().unwrap_or_default();
                                                            move |evt: Event<MouseData>| {
                                                                evt.stop_propagation();
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
                                                span { class: "dashboard-time", "{relative_time(&pr_clone.updated_at)}" }
                                            }
                                            if is_expanded {
                                                {render_pr_detail(&pr_clone)}
                                            }
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
                                {
                                    let issue_number = issue.number;
                                    let is_expanded = expanded_issues.read().contains(&issue_number);
                                    let chevron_class = if is_expanded { "expand-chevron expanded" } else { "expand-chevron" };
                                    let issue_url = issue.url.clone();
                                    let issue_clone = issue.clone();
                                    let has_pending = pending_bodies.read().contains_key(&issue_number);
                                    rsx! {
                                        div { key: "issue-{issue_number}",
                                            div {
                                                class: "dashboard-row dashboard-row-clickable",
                                                onclick: move |_| {
                                                    let mut set = expanded_issues.write();
                                                    if !set.remove(&issue_number) {
                                                        set.insert(issue_number);
                                                    }
                                                },
                                                span { class: "{chevron_class}", ">" }
                                                span {
                                                    class: "dashboard-link",
                                                    onclick: {
                                                        let url = issue_url.clone();
                                                        move |evt: Event<MouseData>| {
                                                            evt.stop_propagation();
                                                            open_url(&url);
                                                        }
                                                    },
                                                    "#{issue_number}"
                                                }
                                                span { class: "dashboard-title", "{issue_clone.title}" }
                                                if !issue_clone.labels.is_empty() {
                                                    span { class: "dashboard-labels",
                                                        for label in &issue_clone.labels {
                                                            span { class: "dashboard-label", "{label}" }
                                                        }
                                                    }
                                                }
                                                if let Some((done, total)) = issue_clone.task_progress {
                                                    span { class: "task-progress-text", "{done}/{total}" }
                                                }
                                                if has_pending {
                                                    span { class: "dashboard-pending-dot" }
                                                }
                                                span { class: "dashboard-time", "{relative_time(&issue_clone.updated_at)}" }
                                            }
                                            if is_expanded {
                                                {render_issue_detail(
                                                    &issue_clone,
                                                    &repo,
                                                    has_repo,
                                                    &mut state,
                                                    &mut pending_bodies,
                                                    &mut saving_issues,
                                                    &mut toasts,
                                                    on_pick_issue,
                                                )}
                                            }
                                        }
                                    }
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

fn render_pr_detail(pr: &fridi_core::project_overview::PrSummary) -> Element {
    let additions = pr.additions;
    let deletions = pr.deletions;
    let changed_files = pr.changed_files;

    rsx! {
        div { class: "card-detail",
            div { class: "card-stat",
                span { class: "card-stat-label", "Files changed" }
                span { class: "card-stat-value", "{changed_files} files (+{additions} / -{deletions})" }
            }
            div { class: "card-stat",
                span { class: "card-stat-label", "Branch" }
                span { class: "card-stat-value", "{pr.branch}" }
            }
            if let Some(ref decision) = pr.review_decision {
                div { class: "card-stat",
                    span { class: "card-stat-label", "Review" }
                    span { class: review_badge_class(decision), "{decision}" }
                }
            }
            if !pr.checks.is_empty() {
                div { class: "card-stat",
                    span { class: "card-stat-label", "CI checks" }
                    div {
                        for check in &pr.checks {
                            {
                                let (icon, class) = check_icon_and_class(&check.conclusion, &check.status);
                                rsx! {
                                    div { class: "check-item",
                                        span { class: "{class}", "{icon}" }
                                        span { "{check.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !pr.labels.is_empty() {
                div { class: "card-stat",
                    span { class: "card-stat-label", "Labels" }
                    span { class: "dashboard-labels",
                        for label in &pr.labels {
                            span { class: "dashboard-label", "{label}" }
                        }
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

#[allow(clippy::too_many_arguments)]
fn render_issue_detail(
    issue: &IssueSummary,
    repo: &Option<String>,
    has_repo: bool,
    state: &mut Signal<FetchState>,
    pending_bodies: &mut Signal<HashMap<u64, String>>,
    saving_issues: &mut Signal<HashSet<u64>>,
    toasts: &mut Signal<Vec<ToastMessage>>,
    on_pick_issue: EventHandler<SessionSource>,
) -> Element {
    let issue_number = issue.number;
    let body = get_effective_body(issue, &pending_bodies.read());
    let checkboxes = parse_checkboxes(&body);
    let has_pending = pending_bodies.read().contains_key(&issue_number);
    let is_saving = saving_issues.read().contains(&issue_number);

    let mut pending_bodies = *pending_bodies;
    let mut saving_issues = *saving_issues;
    let mut state = *state;
    let mut toasts = *toasts;

    rsx! {
        div { class: "card-detail",
            if let Some((done, total)) = issue.task_progress {
                {
                    let pct = if total > 0 { done * 100 / total } else { 0 };
                    rsx! {
                        div { class: "card-stat",
                            span { class: "card-stat-label", "Progress" }
                            div { style: "display: flex; align-items: center; gap: 8px; flex: 1;",
                                div { class: "task-progress-bar",
                                    div {
                                        class: "task-progress-fill",
                                        style: "width: {pct}%",
                                    }
                                }
                                span { class: "task-progress-text", "{done}/{total} done" }
                            }
                        }
                    }
                }
            }
            if !issue.labels.is_empty() {
                div { class: "card-stat",
                    span { class: "card-stat-label", "Labels" }
                    span { class: "dashboard-labels",
                        for label in &issue.labels {
                            span { class: "dashboard-label", "{label}" }
                        }
                    }
                }
            }
            if !issue.assignees.is_empty() {
                div { class: "card-stat",
                    span { class: "card-stat-label", "Assignees" }
                    div { style: "display: flex; gap: 4px; flex-wrap: wrap;",
                        for assignee in &issue.assignees {
                            span { class: "assignee-badge", "{assignee}" }
                        }
                    }
                }
            }
            if !checkboxes.is_empty() {
                div { class: "card-stat", style: "flex-direction: column; align-items: flex-start;",
                    span { class: "card-stat-label", "Tasks" }
                    div { class: "issue-detail-tasks",
                        for cb in &checkboxes {
                            {
                                let line_idx = cb.line_index;
                                let checkbox_class = if has_pending {
                                    "issue-checkbox-row checkbox-pending"
                                } else {
                                    "issue-checkbox-row"
                                };
                                rsx! {
                                    div {
                                        class: "{checkbox_class}",
                                        key: "cb-{issue_number}-{line_idx}",
                                        input {
                                            class: "issue-checkbox",
                                            r#type: "checkbox",
                                            checked: cb.checked,
                                            disabled: is_saving,
                                            onchange: move |_| {
                                                let current_body = {
                                                    let pending = pending_bodies.read();
                                                    pending.get(&issue_number).cloned()
                                                };
                                                let base_body = current_body.unwrap_or_else(|| {
                                                    if let FetchState::Loaded(ref overview) = *state.read() {
                                                        overview
                                                            .open_issues
                                                            .iter()
                                                            .find(|i| i.number == issue_number)
                                                            .and_then(|i| i.body.clone())
                                                            .unwrap_or_default()
                                                    } else {
                                                        String::new()
                                                    }
                                                });
                                                let new_body = toggle_checkbox_in_body(&base_body, line_idx);
                                                pending_bodies.write().insert(issue_number, new_body);
                                            },
                                        }
                                        span { class: "issue-checkbox-label", "{cb.text}" }
                                    }
                                }
                            }
                        }
                    }
                }
                if has_pending {
                    div { class: "issue-detail-actions",
                        button {
                            class: "save-btn",
                            disabled: is_saving || !has_repo,
                            onclick: {
                                let repo_str = repo.clone().unwrap_or_default();
                                move |evt: Event<MouseData>| {
                                    evt.stop_propagation();
                                    let body_to_save = {
                                        let pending = pending_bodies.read();
                                        pending.get(&issue_number).cloned()
                                    };
                                    let Some(body) = body_to_save else { return };
                                    saving_issues.write().insert(issue_number);
                                    let r = repo_str.clone();
                                    let b = body.clone();
                                    spawn(async move {
                                        let result = tokio::task::spawn_blocking(move || {
                                            github::update_issue_body(&r, issue_number, &b)
                                        })
                                        .await;
                                        saving_issues.write().remove(&issue_number);
                                        match result {
                                            Ok(Ok(())) => {
                                                let saved_body = pending_bodies
                                                    .write()
                                                    .remove(&issue_number)
                                                    .unwrap_or(body);
                                                if let FetchState::Loaded(ref mut overview) =
                                                    *state.write()
                                                {
                                                    if let Some(issue) = overview
                                                        .open_issues
                                                        .iter_mut()
                                                        .find(|i| i.number == issue_number)
                                                    {
                                                        issue.task_progress = fridi_core::project_overview::parse_task_progress(&saved_body);
                                                        issue.body = Some(saved_body);
                                                    }
                                                }
                                                push_toast(&mut toasts, "Tasks saved", ToastLevel::Success);
                                            }
                                            Ok(Err(e)) => {
                                                push_toast(&mut toasts, format!("Failed to save: {e}"), ToastLevel::Error);
                                            }
                                            Err(e) => {
                                                push_toast(&mut toasts, format!("Failed to save: {e}"), ToastLevel::Error);
                                            }
                                        }
                                    });
                                }
                            },
                            if is_saving { "Saving..." } else { "Save" }
                        }
                    }
                }
            }
            div { class: "card-detail-actions",
                button {
                    class: "card-detail-btn",
                    onclick: {
                        let title = issue.title.clone();
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            on_pick_issue.call(SessionSource::Issue {
                                number: issue_number,
                                title: title.clone(),
                            });
                        }
                    },
                    "Analyze"
                }
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

    #[test]
    fn sync_label_states() {
        assert_eq!(sync_label(true, false, 0), "Syncing...");
        assert_eq!(sync_label(false, false, 30), "Synced");
        assert_eq!(sync_label(false, false, 60), "Synced");
        assert_eq!(sync_label(false, false, 90), "Synced");
        assert_eq!(sync_label(false, false, 130), "Stale");
        assert_eq!(sync_label(false, true, 10), "Stale");
    }

    #[test]
    fn dot_state_class_states() {
        assert_eq!(dot_state_class(true, false, 0), "warning");
        assert_eq!(dot_state_class(false, false, 30), "success");
        assert_eq!(dot_state_class(false, false, 90), "success");
        assert_eq!(dot_state_class(false, false, 130), "error");
        assert_eq!(dot_state_class(false, true, 10), "error");
    }

    #[test]
    fn sync_detail_states() {
        assert_eq!(sync_detail(true, false, 0), "Refreshing...");
        assert_eq!(sync_detail(false, false, 45), "Updated 45s ago");
        assert_eq!(sync_detail(false, false, 130), "Stale \u{2014} 2m ago");
        assert_eq!(sync_detail(false, true, 10), "Last sync failed");
    }

    #[test]
    fn parse_checkboxes_empty() {
        let result = parse_checkboxes("No checkboxes here");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_checkboxes_mixed() {
        let body = "- [x] done\n- [ ] todo\n- [X] also done\nsome text";
        let result = parse_checkboxes(body);
        assert_eq!(result.len(), 3);
        assert!(result[0].checked);
        assert_eq!(result[0].text, "done");
        assert!(!result[1].checked);
        assert_eq!(result[1].text, "todo");
        assert!(result[2].checked);
        assert_eq!(result[2].text, "also done");
    }

    #[test]
    fn toggle_checkbox_unchecked_to_checked() {
        let body = "- [ ] task 1\n- [ ] task 2";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "- [x] task 1\n- [ ] task 2");
    }

    #[test]
    fn toggle_checkbox_checked_to_unchecked() {
        let body = "- [x] task 1\n- [ ] task 2";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "- [ ] task 1\n- [ ] task 2");
    }

    #[test]
    fn toggle_checkbox_preserves_indentation() {
        let body = "  - [ ] indented task\n- [x] normal task";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "  - [x] indented task\n- [x] normal task");
    }

    #[test]
    fn toggle_checkbox_uppercase_x() {
        let body = "- [X] task";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "- [ ] task");
    }

    #[test]
    fn parse_checkboxes_indented() {
        let body = "  - [x] indented done\n  - [ ] indented todo";
        let result = parse_checkboxes(body);
        assert_eq!(result.len(), 2);
        assert!(result[0].checked);
        assert_eq!(result[0].text, "indented done");
        assert!(!result[1].checked);
        assert_eq!(result[1].text, "indented todo");
    }

    #[test]
    fn parse_checkboxes_empty_body() {
        let result = parse_checkboxes("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_checkboxes_ignores_non_checkbox_lines() {
        let body = "Some text\n- [x] real task\n- not a checkbox\n- [invalid] nope";
        let result = parse_checkboxes(body);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "real task");
        assert_eq!(result[0].line_index, 1);
    }

    #[test]
    fn toggle_checkbox_round_trip() {
        let body = "- [ ] task\n- [x] done";
        let toggled = toggle_checkbox_in_body(body, 0);
        assert_eq!(toggled, "- [x] task\n- [x] done");
        let back = toggle_checkbox_in_body(&toggled, 0);
        assert_eq!(back, "- [ ] task\n- [x] done");
    }

    #[test]
    fn toggle_checkbox_non_checkbox_line_unchanged() {
        let body = "just text\n- [ ] task";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "just text\n- [ ] task");
    }

    #[test]
    fn parse_checkboxes_tracks_line_indices() {
        let body = "heading\n\n- [x] first\nsome text\n- [ ] second";
        let result = parse_checkboxes(body);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line_index, 2);
        assert_eq!(result[1].line_index, 4);
    }

    #[test]
    fn toggle_checkbox_preserves_trailing_newline() {
        let body = "- [ ] task 1\n- [ ] task 2\n";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "- [x] task 1\n- [ ] task 2\n");
    }

    #[test]
    fn toggle_checkbox_no_trailing_newline_stays_without() {
        let body = "- [ ] task 1\n- [ ] task 2";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "- [x] task 1\n- [ ] task 2");
    }

    #[test]
    fn parse_checkboxes_special_characters_in_text() {
        let body = "- [x] task with `code` and **bold**\n- [ ] task with [link](url)";
        let result = parse_checkboxes(body);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "task with `code` and **bold**");
        assert_eq!(result[1].text, "task with [link](url)");
    }

    #[test]
    fn parse_checkboxes_only_whitespace_body() {
        let result = parse_checkboxes("   \n  \n");
        assert!(result.is_empty());
    }

    #[test]
    fn toggle_checkbox_single_line_body() {
        let body = "- [ ] only task";
        let result = toggle_checkbox_in_body(body, 0);
        assert_eq!(result, "- [x] only task");
    }

    #[test]
    fn get_effective_body_uses_pending() {
        let issue = IssueSummary {
            number: 42,
            title: "test".into(),
            labels: vec![],
            updated_at: String::new(),
            body: Some("original".into()),
            assignees: vec![],
            url: String::new(),
            task_progress: None,
        };
        let mut pending = HashMap::new();
        assert_eq!(get_effective_body(&issue, &pending), "original");
        pending.insert(42, "modified".into());
        assert_eq!(get_effective_body(&issue, &pending), "modified");
    }
}

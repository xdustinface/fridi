use dioxus::prelude::*;
use fridi_core::github::{self, GitHubIssue, GitHubPR};

#[derive(Clone, PartialEq)]
enum CreatorMode {
    SelectMode,
    PickIssue,
    PickPR,
    NewPrompt,
}

#[derive(Clone, PartialEq)]
enum LoadState<T: Clone + PartialEq> {
    Loading,
    Loaded(T),
    Error(String),
}

/// Describes which source triggered session creation.
#[derive(Clone, Debug)]
pub(crate) enum SessionSource {
    Issue { number: u64, title: String },
    PR { number: u64, title: String },
    Prompt { text: String },
}

#[component]
pub(crate) fn SessionCreator(
    repo: Option<String>,
    on_create: EventHandler<SessionSource>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut mode = use_signal(|| CreatorMode::SelectMode);
    let mut issues = use_signal(|| None::<LoadState<Vec<GitHubIssue>>>);
    let mut prs = use_signal(|| None::<LoadState<Vec<GitHubPR>>>);
    let mut prompt_text = use_signal(String::new);
    let mut search_filter = use_signal(String::new);

    let repo_str = repo.clone().unwrap_or_default();

    let has_repo = repo.is_some() && !repo_str.is_empty();

    rsx! {
        div { class: "picker-overlay",
            onclick: move |_| on_cancel.call(()),
            div {
                class: "picker-modal session-creator",
                onclick: move |evt| evt.stop_propagation(),
                match &*mode.read() {
                    CreatorMode::SelectMode => rsx! {
                        h3 { "New Session" }
                        div { class: "mode-grid",
                            button {
                                class: "mode-btn",
                                disabled: !has_repo,
                                onclick: {
                                    let repo_val = repo_str.clone();
                                    move |_| {
                                        mode.set(CreatorMode::PickIssue);
                                        issues.set(Some(LoadState::Loading));
                                        search_filter.set(String::new());
                                        let r = repo_val.clone();
                                        let mut issues_sig = issues;
                                        // Blocking fetch is acceptable for desktop app
                                        match github::fetch_issues(&r) {
                                            Ok(list) => issues_sig.set(Some(LoadState::Loaded(list))),
                                            Err(e) => issues_sig.set(Some(LoadState::Error(e.to_string()))),
                                        }
                                    }
                                },
                                div { class: "mode-btn-title", "Pick Issue" }
                                div { class: "mode-btn-desc", "Browse open issues" }
                            }
                            button {
                                class: "mode-btn",
                                disabled: !has_repo,
                                onclick: {
                                    let repo_val = repo_str.clone();
                                    move |_| {
                                        let r = repo_val.clone();
                                        match github::auto_pick_issue(&r) {
                                            Ok(Some(issue)) => {
                                                on_create.call(SessionSource::Issue {
                                                    number: issue.number,
                                                    title: issue.title,
                                                });
                                            }
                                            Ok(None) => {
                                                // No issues found — show issue picker with empty state
                                                mode.set(CreatorMode::PickIssue);
                                                issues.set(Some(LoadState::Loaded(Vec::new())));
                                            }
                                            Err(e) => {
                                                mode.set(CreatorMode::PickIssue);
                                                issues.set(Some(LoadState::Error(e.to_string())));
                                            }
                                        }
                                    }
                                },
                                div { class: "mode-btn-title", "Auto-pick" }
                                div { class: "mode-btn-desc", "Highest priority issue" }
                            }
                            button {
                                class: "mode-btn",
                                disabled: !has_repo,
                                onclick: {
                                    let repo_val = repo_str.clone();
                                    move |_| {
                                        mode.set(CreatorMode::PickPR);
                                        prs.set(Some(LoadState::Loading));
                                        search_filter.set(String::new());
                                        let r = repo_val.clone();
                                        let mut prs_sig = prs;
                                        match github::fetch_prs(&r) {
                                            Ok(list) => prs_sig.set(Some(LoadState::Loaded(list))),
                                            Err(e) => prs_sig.set(Some(LoadState::Error(e.to_string()))),
                                        }
                                    }
                                },
                                div { class: "mode-btn-title", "Pick PR" }
                                div { class: "mode-btn-desc", "Browse open PRs" }
                            }
                            button {
                                class: "mode-btn",
                                onclick: move |_| {
                                    mode.set(CreatorMode::NewPrompt);
                                    prompt_text.set(String::new());
                                },
                                div { class: "mode-btn-title", "New by Prompt" }
                                div { class: "mode-btn-desc", "Describe the task" }
                            }
                        }
                        if !has_repo {
                            div { class: "creator-hint",
                                "Configure a repo in workflow config to enable GitHub modes"
                            }
                        }
                    },
                    CreatorMode::PickIssue => rsx! {
                        div { class: "creator-header",
                            button {
                                class: "creator-back",
                                onclick: move |_| mode.set(CreatorMode::SelectMode),
                                "< Back"
                            }
                            h3 { "Pick Issue" }
                        }
                        input {
                            class: "creator-search",
                            placeholder: "Filter issues...",
                            value: "{search_filter}",
                            oninput: move |evt| search_filter.set(evt.value()),
                        }
                        div { class: "picker-list",
                            match &*issues.read() {
                                Some(LoadState::Loading) => rsx! {
                                    div { class: "creator-loading", "Loading issues..." }
                                },
                                Some(LoadState::Error(e)) => rsx! {
                                    div { class: "creator-error", "Error: {e}" }
                                },
                                Some(LoadState::Loaded(list)) => {
                                    let filter = search_filter.read().to_lowercase();
                                    let filtered: Vec<_> = list.iter().filter(|i| {
                                        filter.is_empty()
                                            || i.title.to_lowercase().contains(&filter)
                                            || i.number.to_string().contains(&filter)
                                    }).collect();
                                    if filtered.is_empty() {
                                        rsx! {
                                            div { class: "picker-empty", "No issues found" }
                                        }
                                    } else {
                                        rsx! {
                                            for issue in filtered {
                                                {
                                                    let num = issue.number;
                                                    let title = issue.title.clone();
                                                    let labels: Vec<String> = issue.labels.iter().map(|l| l.name.clone()).collect();
                                                    rsx! {
                                                        div {
                                                            key: "issue-{num}",
                                                            class: "picker-item",
                                                            onclick: move |_| {
                                                                on_create.call(SessionSource::Issue {
                                                                    number: num,
                                                                    title: title.clone(),
                                                                });
                                                            },
                                                            span { class: "picker-item-number", "#{num}" }
                                                            span { class: "picker-item-name", "{issue.title}" }
                                                            if !labels.is_empty() {
                                                                div { class: "picker-item-labels",
                                                                    for label in &labels {
                                                                        span { class: "picker-label", "{label}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                                None => rsx! {},
                            }
                        }
                    },
                    CreatorMode::PickPR => rsx! {
                        div { class: "creator-header",
                            button {
                                class: "creator-back",
                                onclick: move |_| mode.set(CreatorMode::SelectMode),
                                "< Back"
                            }
                            h3 { "Pick PR" }
                        }
                        input {
                            class: "creator-search",
                            placeholder: "Filter PRs...",
                            value: "{search_filter}",
                            oninput: move |evt| search_filter.set(evt.value()),
                        }
                        div { class: "picker-list",
                            match &*prs.read() {
                                Some(LoadState::Loading) => rsx! {
                                    div { class: "creator-loading", "Loading pull requests..." }
                                },
                                Some(LoadState::Error(e)) => rsx! {
                                    div { class: "creator-error", "Error: {e}" }
                                },
                                Some(LoadState::Loaded(list)) => {
                                    let filter = search_filter.read().to_lowercase();
                                    let filtered: Vec<_> = list.iter().filter(|pr| {
                                        filter.is_empty()
                                            || pr.title.to_lowercase().contains(&filter)
                                            || pr.number.to_string().contains(&filter)
                                            || pr.head_ref_name.to_lowercase().contains(&filter)
                                    }).collect();
                                    if filtered.is_empty() {
                                        rsx! {
                                            div { class: "picker-empty", "No pull requests found" }
                                        }
                                    } else {
                                        rsx! {
                                            for pr in filtered {
                                                {
                                                    let num = pr.number;
                                                    let title = pr.title.clone();
                                                    rsx! {
                                                        div {
                                                            key: "pr-{num}",
                                                            class: "picker-item",
                                                            onclick: move |_| {
                                                                on_create.call(SessionSource::PR {
                                                                    number: num,
                                                                    title: title.clone(),
                                                                });
                                                            },
                                                            span { class: "picker-item-number", "#{num}" }
                                                            span { class: "picker-item-name", "{pr.title}" }
                                                            span { class: "picker-item-branch", "{pr.head_ref_name}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                                None => rsx! {},
                            }
                        }
                    },
                    CreatorMode::NewPrompt => rsx! {
                        div { class: "creator-header",
                            button {
                                class: "creator-back",
                                onclick: move |_| mode.set(CreatorMode::SelectMode),
                                "< Back"
                            }
                            h3 { "New by Prompt" }
                        }
                        textarea {
                            class: "creator-textarea",
                            placeholder: "Describe the task...",
                            value: "{prompt_text}",
                            oninput: move |evt| prompt_text.set(evt.value()),
                        }
                        button {
                            class: "creator-submit",
                            disabled: prompt_text.read().trim().is_empty(),
                            onclick: move |_| {
                                let text = prompt_text.read().trim().to_string();
                                if !text.is_empty() {
                                    on_create.call(SessionSource::Prompt { text });
                                }
                            },
                            "Create Session"
                        }
                    },
                }
            }
        }
    }
}

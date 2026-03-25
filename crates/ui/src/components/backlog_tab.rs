use std::path::PathBuf;

use chrono::Utc;
use dioxus::prelude::*;
use fridi_core::backlog::{Backlog, Priority};

const BACKLOG_FILE: &str = ".fridi/backlog.md";

/// Compute a human-readable relative time from a chrono DateTime.
fn relative_time_from_dt(dt: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
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

/// Index into the sorted display order, mapped back to the original backlog index.
#[derive(Clone, PartialEq)]
struct DisplayItem {
    original_index: usize,
    text: String,
    tags: Vec<String>,
    priority: Priority,
    context: Option<String>,
    completed: bool,
    time_label: String,
}

fn priority_sort_key(p: &Priority) -> u8 {
    match p {
        Priority::Urgent => 0,
        Priority::Important => 1,
        Priority::Normal => 2,
    }
}

fn priority_class(p: &Priority) -> &'static str {
    match p {
        Priority::Urgent => "backlog-priority-urgent",
        Priority::Important => "backlog-priority-important",
        Priority::Normal => "",
    }
}

fn priority_label(p: &Priority) -> &'static str {
    match p {
        Priority::Urgent => "!!",
        Priority::Important => "!",
        Priority::Normal => "",
    }
}

/// Build sorted display items from a backlog. Sorted by priority (urgent first),
/// then by recency within the same priority level.
fn build_display_items(backlog: &Backlog) -> Vec<DisplayItem> {
    let mut items: Vec<DisplayItem> = backlog
        .items()
        .iter()
        .enumerate()
        .map(|(i, item)| DisplayItem {
            original_index: i,
            text: item.text.clone(),
            tags: item.tags.clone(),
            priority: item.priority.clone(),
            context: item.context.clone(),
            completed: item.completed,
            time_label: relative_time_from_dt(item.created_at),
        })
        .collect();

    items.sort_by(|a, b| {
        let pa = priority_sort_key(&a.priority);
        let pb = priority_sort_key(&b.priority);
        pa.cmp(&pb)
            .then(a.original_index.cmp(&b.original_index).reverse())
    });

    items
}

#[component]
pub(crate) fn BacklogTab() -> Element {
    let backlog_path = PathBuf::from(BACKLOG_FILE);
    let mut backlog = use_signal(|| {
        Backlog::load(&backlog_path).unwrap_or_else(|_| {
            // Fall back to an empty in-memory backlog on load error
            Backlog::load("/dev/null/nonexistent").unwrap()
        })
    });
    let mut input_text = use_signal(String::new);

    let display_items = use_memo(move || build_display_items(&backlog.read()));

    let on_add = move |_: FormEvent| {
        let text = input_text.read().trim().to_string();
        if text.is_empty() {
            return;
        }
        backlog.write().add(&text, None);
        input_text.set(String::new());
        if let Err(e) = backlog.read().save() {
            eprintln!("failed to save backlog: {e}");
        }
    };

    let items = display_items.read();
    let is_empty = items.is_empty();

    rsx! {
        div { class: "backlog-tab",
            form {
                class: "backlog-input-form",
                onsubmit: on_add,
                input {
                    class: "backlog-input",
                    r#type: "text",
                    placeholder: "Add idea... (prefix !! for urgent, ! for important, #tag for tags)",
                    value: "{input_text}",
                    oninput: move |evt| input_text.set(evt.value()),
                }
            }
            if is_empty {
                div { class: "backlog-empty",
                    "No ideas yet — add one above"
                }
            } else {
                div { class: "backlog-list",
                    for item in items.iter() {
                        {
                            let idx = item.original_index;
                            let p_class = priority_class(&item.priority);
                            let p_label = priority_label(&item.priority);
                            let completed_class = if item.completed { " completed" } else { "" };
                            rsx! {
                                div {
                                    class: "backlog-item{completed_class}",
                                    key: "{idx}",
                                    input {
                                        class: "backlog-checkbox",
                                        r#type: "checkbox",
                                        checked: item.completed,
                                        onchange: move |_| {
                                            if let Err(e) = backlog.write().toggle(idx) {
                                                eprintln!("failed to toggle backlog item: {e}");
                                            }
                                            if let Err(e) = backlog.read().save() {
                                                eprintln!("failed to save backlog: {e}");
                                            }
                                        },
                                    }
                                    if !p_label.is_empty() {
                                        span { class: "{p_class}", "{p_label}" }
                                    }
                                    span { class: "backlog-text", "{item.text}" }
                                    for tag in &item.tags {
                                        span { class: "backlog-tag", "#{tag}" }
                                    }
                                    if let Some(ctx) = &item.context {
                                        span { class: "backlog-context", "{ctx}" }
                                    }
                                    span { class: "backlog-time", "{item.time_label}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

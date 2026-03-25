use std::time::Instant;

use dioxus::prelude::*;

const AUTO_DISMISS_SECS: u64 = 5;
const POLL_INTERVAL_MS: u64 = 250;
const EXIT_ANIMATION_MS: u64 = 150;

static NEXT_TOAST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    fn css_class(self) -> &'static str {
        match self {
            ToastLevel::Info => "info",
            ToastLevel::Success => "success",
            ToastLevel::Warning => "warning",
            ToastLevel::Error => "error",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            ToastLevel::Info => "i",
            ToastLevel::Success => "ok",
            ToastLevel::Warning => "!",
            ToastLevel::Error => "x",
        }
    }
}

#[derive(Clone)]
pub(crate) struct ToastMessage {
    pub(crate) id: u64,
    pub(crate) message: String,
    pub(crate) level: ToastLevel,
    pub(crate) created_at: Instant,
    /// Set when the toast starts its exit animation.
    pub(crate) exiting: bool,
}

/// Shared toast state provided via Dioxus context.
#[derive(Clone, Copy)]
pub(crate) struct Toasts(pub(crate) Signal<Vec<ToastMessage>>);

/// Allocate the next unique toast id.
fn next_toast_id() -> u64 { NEXT_TOAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }

/// Push a new toast into the given signal.
pub(crate) fn push_toast(
    toasts: &mut Signal<Vec<ToastMessage>>,
    message: impl Into<String>,
    level: ToastLevel,
) {
    let id = next_toast_id();
    toasts.write().push(ToastMessage {
        id,
        message: message.into(),
        level,
        created_at: Instant::now(),
        exiting: false,
    });
}

/// Container that renders the toast stack and manages auto-dismiss.
#[component]
pub(crate) fn ToastContainer() -> Element {
    let mut toasts = use_context::<Toasts>().0;

    // Auto-dismiss coroutine: marks non-error toasts as exiting after the
    // timeout, then removes them once the exit animation completes.
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
            let now = Instant::now();
            let mut list = toasts.write();

            // Mark expired non-error toasts as exiting
            for toast in list.iter_mut() {
                if toast.level != ToastLevel::Error
                    && !toast.exiting
                    && now.duration_since(toast.created_at).as_secs() >= AUTO_DISMISS_SECS
                {
                    toast.exiting = true;
                    toast.created_at = now; // reuse as exit-start timestamp
                }
            }

            // Remove toasts whose exit animation has completed
            list.retain(|t| {
                if t.exiting {
                    now.duration_since(t.created_at).as_millis() < EXIT_ANIMATION_MS as u128
                } else {
                    true
                }
            });
        }
    });

    let list = toasts.read();
    if list.is_empty() {
        return rsx! {};
    }

    rsx! {
        div { class: "toast-container",
            for toast in list.iter() {
                {
                    let id = toast.id;
                    let level_class = toast.level.css_class();
                    let icon = toast.level.icon();
                    let anim_class = if toast.exiting { "exiting" } else { "entering" };
                    let elapsed = Instant::now().duration_since(toast.created_at).as_secs();
                    let remaining = if toast.exiting || toast.level == ToastLevel::Error {
                        0.0
                    } else {
                        let frac = elapsed as f64 / AUTO_DISMISS_SECS as f64;
                        (1.0 - frac).max(0.0)
                    };
                    let progress_width = format!("{:.0}%", remaining * 100.0);
                    rsx! {
                        div {
                            key: "{id}",
                            class: "toast-card {level_class} {anim_class}",
                            div { class: "toast-body",
                                span { class: "toast-icon", "{icon}" }
                                span { class: "toast-message", "{toast.message}" }
                                button {
                                    class: "toast-dismiss",
                                    onclick: move |_| {
                                        toasts.write().retain(|t| t.id != id);
                                    },
                                    "x"
                                }
                            }
                            if toast.level != ToastLevel::Error && !toast.exiting {
                                div { class: "toast-progress",
                                    div {
                                        class: "toast-progress-bar {level_class}",
                                        style: "width: {progress_width}",
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

use dioxus::prelude::*;

const DEFAULT_SPLIT_RATIO: f64 = 0.6;
const MIN_RATIO: f64 = 0.2;
const MAX_RATIO: f64 = 0.8;

#[component]
pub(crate) fn SplitPane(top: Element, bottom: Option<Element>) -> Element {
    let mut split_ratio = use_signal(|| DEFAULT_SPLIT_RATIO);
    let mut dragging = use_signal(|| false);
    let mut collapsed = use_signal(|| false);
    let mut ratio_before_collapse = use_signal(|| DEFAULT_SPLIT_RATIO);

    let has_bottom = bottom.is_some();
    let show_bottom = has_bottom && !*collapsed.read();

    let top_percent = if show_bottom {
        *split_ratio.read() * 100.0
    } else {
        100.0
    };
    let bottom_percent = if show_bottom {
        (1.0 - *split_ratio.read()) * 100.0
    } else {
        0.0
    };

    let top_style = format!("height: {top_percent}%; overflow: auto;");
    let bottom_style = format!("height: {bottom_percent}%; overflow: auto;");

    rsx! {
        div {
            class: "split-pane-container",
            onmousemove: move |evt: MouseEvent| {
                if *dragging.read() {
                    let y = evt.page_coordinates().y;
                    // Use eval to get the container's bounding rect
                    let eval = document::eval(
                        r#"
                        let el = document.querySelector('.split-pane-container');
                        let rect = el.getBoundingClientRect();
                        return [rect.top, rect.height];
                        "#,
                    );
                    spawn(async move {
                        if let Ok(val) = eval.await {
                            if let Some(arr) = val.as_array() {
                                let container_top = arr[0].as_f64().unwrap_or(0.0);
                                let container_height = arr[1].as_f64().unwrap_or(800.0);
                                if container_height > 0.0 {
                                    let relative_y = y - container_top;
                                    let ratio = (relative_y / container_height)
                                        .clamp(MIN_RATIO, MAX_RATIO);
                                    split_ratio.set(ratio);
                                }
                            }
                        }
                    });
                }
            },
            onmouseup: move |_| {
                dragging.set(false);
            },
            onmouseleave: move |_| {
                dragging.set(false);
            },
            div {
                class: "split-pane-top",
                style: "{top_style}",
                {top}
            }
            if has_bottom {
                div {
                    class: if *dragging.read() { "split-pane-divider dragging" } else { "split-pane-divider" },
                    onmousedown: move |evt: MouseEvent| {
                        evt.prevent_default();
                        if !*collapsed.read() {
                            dragging.set(true);
                        }
                    },
                    ondoubleclick: move |_| {
                        if *collapsed.read() {
                            collapsed.set(false);
                            split_ratio.set(*ratio_before_collapse.read());
                        } else {
                            ratio_before_collapse.set(*split_ratio.read());
                            collapsed.set(true);
                        }
                    },
                    div { class: "split-pane-divider-handle" }
                }
            }
            if show_bottom {
                div {
                    class: "split-pane-bottom",
                    style: "{bottom_style}",
                    {bottom}
                }
            }
        }
    }
}

mod app;
mod components;
mod engine_bridge;
mod state;
mod styles;
mod workflow_runner;

use std::borrow::Cow;

use dioxus::desktop::wry::http::Response;

/// Serve bundled assets via a custom `fridi://` protocol instead of inlining
/// them in the document head. This avoids WebKit navigation policy stack
/// traces that fire when large inline `<script>` tags are parsed.
fn asset_protocol_handler(
    _id: dioxus::desktop::wry::WebViewId,
    request: dioxus::desktop::wry::http::Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path().trim_start_matches('/');
    match path {
        "app.css" => Response::builder()
            .header("Content-Type", "text/css")
            .body(Cow::Borrowed(styles::APP_CSS.as_bytes()))
            .unwrap(),
        "xterm.css" => Response::builder()
            .header("Content-Type", "text/css")
            .body(Cow::Borrowed(
                include_bytes!("../assets/xterm.css").as_slice(),
            ))
            .unwrap(),
        "xterm.js" => Response::builder()
            .header("Content-Type", "application/javascript")
            .body(Cow::Borrowed(
                include_bytes!("../assets/xterm.js").as_slice(),
            ))
            .unwrap(),
        "xterm-addon-fit.js" => Response::builder()
            .header("Content-Type", "application/javascript")
            .body(Cow::Borrowed(
                include_bytes!("../assets/xterm-addon-fit.min.js").as_slice(),
            ))
            .unwrap(),
        _ => Response::builder()
            .status(404)
            .body(Cow::Borrowed(b"not found" as &[u8]))
            .unwrap(),
    }
}

fn main() {
    // Detect repo once at startup to avoid repeated subprocess calls and
    // unsafe env mutation during rendering.
    let repo = fridi_core::github::detect_repo();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(dioxus::desktop::WindowBuilder::new().with_title("fridi"))
                .with_custom_protocol("fridi", asset_protocol_handler)
                .with_custom_head(
                    r#"<link rel="stylesheet" href="fridi://localhost/app.css">
<link rel="stylesheet" href="fridi://localhost/xterm.css">
<script src="fridi://localhost/xterm.js"></script>
<script src="fridi://localhost/xterm-addon-fit.js"></script>
<script>window.fridiTerminals = {};</script>"#
                        .to_string(),
                ),
        )
        .with_context(app::DetectedRepo(repo))
        .launch(app::App);
}

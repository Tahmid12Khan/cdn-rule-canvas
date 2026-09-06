//! Popup placement wrapper. Appends a `<div class="rre-popup" role="dialog">`
//! (with backdrop + close button) to `<body>` and injects its CSS once.
//! Idempotent per component via a marker attribute.

use crate::domain::applier::html_sanitizer::sanitize;
use crate::domain::applier::ApplyError;
use crate::infra::backend_client::ActiveComponent;
use serde_json::Value;

const STYLE_MARKER: &str = r#"data-rre="popup""#;
const WRAPPER_MARKER_ATTR: &str = "data-rre-popup";

/// Wrap a sanitized component body in a popup dialog and append to `<body>`.
pub fn render(
    html: &str,
    component: &ActiveComponent,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<String, ApplyError> {
    let body = component
        .config
        .get("html_body")
        .and_then(Value::as_str)
        .unwrap_or("");
    let sanitized = sanitize(sanitizer, body);
    let marker = format!("c-{}", component.id);
    Ok(wrap_sanitized(html, &sanitized, &marker))
}

/// Wrap ALREADY-sanitized inner HTML in a popup dialog + append to `<body>`,
/// injecting the CSS once. Idempotent via `marker`. Shared by `render` and the
/// `component_ref` HTML renderer (which renders + sanitizes the resolved template
/// first, then wraps).
pub fn wrap_sanitized(html: &str, sanitized_inner: &str, marker: &str) -> String {
    if html.contains(&format!(r#"{WRAPPER_MARKER_ATTR}="{marker}""#)) {
        return html.to_string();
    }
    let wrapper = format!(
        r#"<div class="rre-popup-backdrop" {WRAPPER_MARKER_ATTR}="{marker}"><div class="rre-popup" role="dialog" aria-label="Notice"><button class="rre-popup-close" aria-label="Close">&times;</button>{sanitized_inner}</div></div>"#
    );
    let mut out = inject_style_once(html.to_string());
    out = append_to_body(out, &wrapper);
    out
}

fn inject_style_once(html: String) -> String {
    if html.contains(STYLE_MARKER) {
        return html;
    }
    let style = format!(
        r#"<style {STYLE_MARKER}>.rre-popup-backdrop{{position:fixed;inset:0;background:rgba(0,0,0,.5);z-index:10000;display:flex;align-items:center;justify-content:center;}}.rre-popup{{background:#fff;border-radius:8px;padding:24px;max-width:480px;position:relative;}}.rre-popup-close{{position:absolute;top:8px;right:8px;border:0;background:none;font-size:20px;cursor:pointer;}}</style>"#
    );
    match html.find("</head>") {
        Some(idx) => {
            let mut out = String::with_capacity(html.len() + style.len());
            out.push_str(&html[..idx]);
            out.push_str(&style);
            out.push_str(&html[idx..]);
            out
        }
        None => format!("{style}{html}"),
    }
}

fn append_to_body(html: String, fragment: &str) -> String {
    match html.rfind("</body>") {
        Some(idx) => {
            let mut out = String::with_capacity(html.len() + fragment.len());
            out.push_str(&html[..idx]);
            out.push_str(fragment);
            out.push_str(&html[idx..]);
            out
        }
        None => format!("{html}{fragment}"),
    }
}

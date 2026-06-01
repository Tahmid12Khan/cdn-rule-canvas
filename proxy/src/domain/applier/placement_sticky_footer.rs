//! Sticky-footer placement wrapper. Appends a `<div class="rre-sticky-footer">`
//! to `<body>` and injects its CSS once (guarded by a `<style data-rre>` marker).
//! Idempotent per component via a marker attribute.

use crate::domain::applier::html_sanitizer::sanitize;
use crate::domain::applier::ApplyError;
use crate::infra::backend_client::ActiveComponent;
use serde_json::Value;

const STYLE_MARKER: &str = r#"data-rre="sticky-footer""#;
const WRAPPER_MARKER_ATTR: &str = "data-rre-sticky";

/// Wrap a sanitized component body in a sticky-footer and append to `<body>`.
pub fn render(
    html: &str,
    component: &ActiveComponent,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<String, ApplyError> {
    let marker = format!("c-{}", component.id);
    if html.contains(&format!(r#"{WRAPPER_MARKER_ATTR}="{marker}""#)) {
        return Ok(html.to_string());
    }

    let body = component
        .config
        .get("html_body")
        .and_then(Value::as_str)
        .unwrap_or("");
    let sanitized = sanitize(sanitizer, body);

    let wrapper = format!(
        r#"<div class="rre-sticky-footer" {WRAPPER_MARKER_ATTR}="{marker}">{sanitized}</div>"#
    );

    let mut out = inject_style_once(html.to_string());
    out = append_to_body(out, &wrapper);
    Ok(out)
}

fn inject_style_once(html: String) -> String {
    if html.contains(STYLE_MARKER) {
        return html;
    }
    let style = format!(
        r#"<style {STYLE_MARKER}>.rre-sticky-footer{{position:fixed;left:0;right:0;bottom:0;z-index:9999;background:#fff;box-shadow:0 -2px 8px rgba(0,0,0,.15);padding:12px;}}</style>"#
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

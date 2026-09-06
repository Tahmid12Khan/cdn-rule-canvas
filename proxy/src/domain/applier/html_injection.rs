//! `HtmlInjectionRenderer`. Sanitizes `html_body`, then injects it relative to
//! `target_selector` via a lol_html element handler. Idempotent: a marker
//! attribute on the injected wrapper makes a second pass a no-op.

use lol_html::{element, rewrite_str, RewriteStrSettings};
use serde_json::Value;

use crate::domain::applier::html_sanitizer::sanitize;
use crate::domain::applier::{validate_selector, ApplyError, ComponentRenderer};
use crate::infra::backend_client::ActiveComponent;

/// Marker attribute making injections idempotent.
const MARKER_ATTR: &str = "data-rre-injected";

pub struct HtmlInjectionRenderer;

impl ComponentRenderer for HtmlInjectionRenderer {
    fn render(
        &self,
        html: &str,
        component: &ActiveComponent,
        sanitizer: &ammonia::Builder<'static>,
    ) -> Result<String, ApplyError> {
        let cfg = &component.config;
        let target_selector = str_field(cfg, "target_selector").ok_or(ApplyError::Selector)?;
        let placement_mode = str_field(cfg, "placement_mode").unwrap_or("append");
        let html_body = str_field(cfg, "html_body").unwrap_or("");

        let sanitized = sanitize(sanitizer, html_body);
        let marker = component_marker(component);
        inject_html(html, target_selector, placement_mode, &sanitized, &marker)
    }
}

/// Reusable injection core: validate the (untrusted) selector, sanitize-once
/// caller-supplied `sanitized_inner` (already cleaned), wrap it in a marker `<div>`
/// for idempotency, and inject relative to `target_selector` per `placement_mode`.
/// If the marker is already present anywhere in `html`, a second pass is a no-op
/// (idempotency). Shared by the `html_injection` component renderer and the
/// `apply_component` expression action (design §4.3).
pub fn inject_html(
    html: &str,
    target_selector: &str,
    placement_mode: &str,
    sanitized_inner: &str,
    marker: &str,
) -> Result<String, ApplyError> {
    validate_selector(target_selector)?;

    // Idempotency: if our marker is already present anywhere, skip.
    if html.contains(marker) {
        return Ok(html.to_string());
    }

    let wrapped = format!(r#"<div {MARKER_ATTR}="{marker}">{sanitized_inner}</div>"#);

    // lol_html 2 requires element handlers to be `'static`, so the closure must
    // OWN everything it touches. Move the owned `wrapped` String + placement in.
    let placement_mode = placement_mode.to_string();
    let element_handler = element!(target_selector, move |el| {
        match placement_mode.as_str() {
            "replace" => {
                el.set_inner_content(&wrapped, lol_html::html_content::ContentType::Html);
            }
            "prepend" => {
                el.prepend(&wrapped, lol_html::html_content::ContentType::Html);
            }
            "before" => {
                el.before(&wrapped, lol_html::html_content::ContentType::Html);
            }
            "after" => {
                el.after(&wrapped, lol_html::html_content::ContentType::Html);
            }
            // "append" and any unknown mode default to append.
            _ => {
                el.append(&wrapped, lol_html::html_content::ContentType::Html);
            }
        }
        Ok(())
    });

    rewrite_str(
        html,
        RewriteStrSettings {
            element_content_handlers: vec![element_handler],
            ..RewriteStrSettings::default()
        },
    )
    .map_err(|_| ApplyError::Html)
}

/// Stable per-component marker value (idempotency key).
fn component_marker(component: &ActiveComponent) -> String {
    format!("c-{}", component.id)
}

fn str_field<'a>(cfg: &'a Value, key: &str) -> Option<&'a str> {
    cfg.get(key).and_then(Value::as_str)
}

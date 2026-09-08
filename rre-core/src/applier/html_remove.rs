//! `HtmlRemoveRenderer`. Deletes matched content instead of injecting any —
//! nothing is added to the page, so there is no marker wrapper and no
//! `html_body`.
//!
//! `include_selector` picks what "remove" means:
//!   * `false` (default) — empty the matched element, keeping the element itself
//!     (`<div id="x">…</div>` → `<div id="x"></div>`).
//!   * `true` — remove the matched element AND its contents (the tag is gone).
//!
//! Naturally idempotent: a second pass finds nothing left to remove.

use lol_html::{element, rewrite_str, RewriteStrSettings};
use serde_json::Value;

use crate::applier::{validate_selector, ApplyError, ComponentRenderer};
use crate::bundle::ActiveComponent;

pub struct HtmlRemoveRenderer;

impl ComponentRenderer for HtmlRemoveRenderer {
    fn render(
        &self,
        html: &str,
        component: &ActiveComponent,
        _sanitizer: &ammonia::Builder<'static>,
    ) -> Result<String, ApplyError> {
        let cfg = &component.config;
        let target_selector = cfg
            .get("target_selector")
            .and_then(Value::as_str)
            .ok_or(ApplyError::Selector)?;
        let include_selector = cfg
            .get("include_selector")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        remove_html(html, target_selector, include_selector)
    }
}

/// Reusable removal core: validate the (untrusted) selector, then either empty
/// the matched elements or remove them outright.
pub fn remove_html(
    html: &str,
    target_selector: &str,
    include_selector: bool,
) -> Result<String, ApplyError> {
    validate_selector(target_selector)?;

    let element_handler = element!(target_selector, move |el| {
        if include_selector {
            el.remove();
        } else {
            el.set_inner_content("", lol_html::html_content::ContentType::Html);
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

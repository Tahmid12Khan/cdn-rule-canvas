//! The applier: turns an outcome's components into HTML modifications. Every
//! renderer is PURE and IDEMPOTENT — `render(render(html)) == render(html)`.
//!
//! `mod.rs` owns the shared `ComponentRenderer` trait + result/error types; the
//! orchestrator dispatches components to renderers and runs them in order.

pub mod component_ref;
pub mod component_render;
pub mod content_truncation;
pub mod html_injection;
pub mod html_remove;
pub mod html_sanitizer;
pub mod json_apply;
pub mod json_path;
pub mod orchestrator;
pub mod placement_popup;
pub mod placement_sticky_footer;

use crate::bundle::ActiveComponent;

/// Result of applying an outcome to an HTML body.
pub struct ModificationResult {
    pub html: String,
    pub applied: bool,
}

/// Result of applying an outcome to a JSON body (parallel to `ModificationResult`).
pub struct JsonModificationResult {
    pub json: serde_json::Value,
    pub applied: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("malformed html")]
    Html,
    #[error("selector rejected")]
    Selector,
    #[error("invalid json path: {0}")]
    JsonPath(#[from] json_path::ParsePathError),
    /// A Component template failed to compile/render (unbalanced mustache, etc.).
    /// The caller fails open (skips the component, serves the body untouched).
    #[error("component render failed")]
    Render,
}

/// One component renderer. Pure + idempotent.
pub trait ComponentRenderer {
    fn render(
        &self,
        html: &str,
        component: &ActiveComponent,
        sanitizer: &ammonia::Builder<'static>,
    ) -> Result<String, ApplyError>;
}

/// Max length of a user-authored CSS selector (selector-injection guard).
pub const MAX_SELECTOR_LEN: usize = 200;

/// Validate an untrusted CSS selector: length cap + character whitelist.
/// Returns `Err(ApplyError::Selector)` on violation (component is then skipped).
pub fn validate_selector(selector: &str) -> Result<(), ApplyError> {
    if selector.is_empty() || selector.len() > MAX_SELECTOR_LEN {
        return Err(ApplyError::Selector);
    }
    let allowed = |c: char| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '_' | '-' | '[' | ']' | '=' | '"' | '\'' | '.' | '#' | ':' | ' ' | ',' | '>'
            )
    };
    if !selector.chars().all(allowed) {
        return Err(ApplyError::Selector);
    }
    Ok(())
}

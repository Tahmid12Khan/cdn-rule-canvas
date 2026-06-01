//! Applier orchestrator. Runs an outcome's components in order (order_index ASC,
//! inline components first, then placement-specific). Each component is rendered
//! via its renderer; a per-component error (selector reject / malformed HTML) is
//! caught and that component is skipped — the overall result is still `Ok`.
//! ShowContent (builtin) is short-circuited by the caller, not here.

use crate::domain::applier::content_truncation::ContentTruncationRenderer;
use crate::domain::applier::html_injection::HtmlInjectionRenderer;
use crate::domain::applier::{placement_popup, placement_sticky_footer};
use crate::domain::applier::{ApplyError, ComponentRenderer, ModificationResult};
use crate::infra::backend_client::{ActiveComponent, ActiveOutcome, Placement};

/// Apply an outcome's components to `html`. Always returns `Ok` (per-component
/// failures fail open); only an unexpected/global failure returns `Err`.
pub fn apply_outcome(
    html: String,
    outcome: &ActiveOutcome,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<ModificationResult, ApplyError> {
    let mut ordered: Vec<&ActiveComponent> = outcome.components.iter().collect();
    // Inline first, then placement (sticky_footer/popup); within each, order_index ASC.
    ordered.sort_by_key(|c| (placement_rank(c.placement), c.order_index));

    let mut current = html;
    let mut applied = false;

    for component in ordered {
        match render_component(&current, component, sanitizer) {
            Ok(next) => {
                if next != current {
                    applied = true;
                }
                current = next;
            }
            Err(e) => {
                // Fail open: skip this component, keep going.
                tracing::warn!(component_id = %component.id, error = %e, "component skipped");
            }
        }
    }

    Ok(ModificationResult {
        html: current,
        applied,
    })
}

/// Inline placement sorts before sticky_footer/popup.
fn placement_rank(p: Placement) -> u8 {
    match p {
        Placement::Inline => 0,
        Placement::StickyFooter => 1,
        Placement::Popup => 2,
    }
}

/// Dispatch a single component to its renderer based on type + placement.
fn render_component(
    html: &str,
    component: &ActiveComponent,
    sanitizer: &ammonia::Builder<'static>,
) -> Result<String, ApplyError> {
    match component.placement {
        Placement::StickyFooter => placement_sticky_footer::render(html, component, sanitizer),
        Placement::Popup => placement_popup::render(html, component, sanitizer),
        Placement::Inline => {
            let renderer = renderer_for(component);
            renderer.render(html, component, sanitizer)
        }
    }
}

/// Pick the inline renderer for a component type. Unknown types are no-ops
/// (returned via a passthrough renderer) so eval never fails the request.
fn renderer_for(component: &ActiveComponent) -> Box<dyn ComponentRenderer> {
    match component.r#type.as_str() {
        "html_injection" => Box::new(HtmlInjectionRenderer),
        "content_truncation" => Box::new(ContentTruncationRenderer),
        _ => Box::new(PassthroughRenderer),
    }
}

/// No-op renderer for unknown component types.
struct PassthroughRenderer;

impl ComponentRenderer for PassthroughRenderer {
    fn render(
        &self,
        html: &str,
        component: &ActiveComponent,
        _sanitizer: &ammonia::Builder<'static>,
    ) -> Result<String, ApplyError> {
        tracing::warn!(component_type = %component.r#type, "unknown component type, skipped");
        Ok(html.to_string())
    }
}

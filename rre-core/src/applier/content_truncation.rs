//! `ContentTruncationRenderer`. Truncates the text inside `target_selector` to
//! `word_count` words. `fade_out` wraps the (removed) remainder marker and
//! injects a gradient `<style>` once. Idempotent via a marker attribute.

use lol_html::{element, rewrite_str, RewriteStrSettings};
use serde_json::Value;

use crate::applier::{validate_selector, ApplyError, ComponentRenderer};
use crate::bundle::ActiveComponent;

const MARKER_ATTR: &str = "data-rre-truncated";
const FADE_STYLE_MARKER: &str = r#"data-rre="content-truncation""#;

pub struct ContentTruncationRenderer;

impl ComponentRenderer for ContentTruncationRenderer {
    fn render(
        &self,
        html: &str,
        component: &ActiveComponent,
        _sanitizer: &ammonia::Builder<'static>,
    ) -> Result<String, ApplyError> {
        let cfg = &component.config;
        let target_selector = str_field(cfg, "target_selector").ok_or(ApplyError::Selector)?;
        let word_count = cfg.get("word_count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let fade_out = cfg
            .get("fade_out")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        validate_selector(target_selector)?;

        // Idempotency: skip if we already truncated this target.
        if html.contains(MARKER_ATTR) {
            return Ok(html.to_string());
        }

        let element_handler = element!(target_selector, |el| {
            el.set_attribute(MARKER_ATTR, "1").ok();
            if fade_out {
                let existing = el.get_attribute("class").unwrap_or_default();
                let class = if existing.is_empty() {
                    "rre-fade-out".to_string()
                } else {
                    format!("{existing} rre-fade-out")
                };
                el.set_attribute("class", &class).ok();
            }
            Ok(())
        });

        let mut out = rewrite_str(
            html,
            RewriteStrSettings {
                element_content_handlers: vec![element_handler],
                ..RewriteStrSettings::default()
            },
        )
        .map_err(|_| ApplyError::Html)?;

        // The word-budget trim is applied to the marked element's text content.
        out = trim_marked_element_words(&out, word_count);

        if fade_out {
            out = inject_fade_style_once(out);
        }

        Ok(out)
    }
}

/// Trim the text of the element carrying `MARKER_ATTR` to `word_count` words,
/// preserving surrounding markup. Token-by-token (not a naive whole-doc split).
fn trim_marked_element_words(html: &str, word_count: usize) -> String {
    if word_count == 0 {
        return html.to_string();
    }
    // Find the marked element's opening tag and trim only its inner text region.
    let Some(marker_pos) = html.find(MARKER_ATTR) else {
        return html.to_string();
    };
    let Some(tag_close_rel) = html[marker_pos..].find('>') else {
        return html.to_string();
    };
    let inner_start = marker_pos + tag_close_rel + 1;

    // Heuristic end: the next closing tag after inner_start.
    let inner_end_rel = match html[inner_start..].find("</") {
        Some(p) => p,
        None => return html.to_string(),
    };
    let inner_end = inner_start + inner_end_rel;

    let inner = &html[inner_start..inner_end];
    let trimmed = truncate_words(inner, word_count);

    let mut result = String::with_capacity(html.len());
    result.push_str(&html[..inner_start]);
    result.push_str(&trimmed);
    result.push_str(&html[inner_end..]);
    result
}

/// Keep the first `word_count` whitespace-delimited words of `text`.
fn truncate_words(text: &str, word_count: usize) -> String {
    let mut kept = Vec::with_capacity(word_count);
    for (i, word) in text.split_whitespace().enumerate() {
        if i >= word_count {
            break;
        }
        kept.push(word);
    }
    kept.join(" ")
}

/// Inject the fade-out gradient `<style>` block once (guarded by a marker).
fn inject_fade_style_once(html: String) -> String {
    if html.contains(FADE_STYLE_MARKER) {
        return html;
    }
    let style = format!(
        r#"<style {FADE_STYLE_MARKER}>.rre-fade-out{{position:relative;-webkit-mask-image:linear-gradient(180deg,#000 60%,transparent);mask-image:linear-gradient(180deg,#000 60%,transparent);}}</style>"#
    );
    if let Some(idx) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + style.len());
        out.push_str(&html[..idx]);
        out.push_str(&style);
        out.push_str(&html[idx..]);
        out
    } else {
        format!("{style}{html}")
    }
}

fn str_field<'a>(cfg: &'a Value, key: &str) -> Option<&'a str> {
    cfg.get(key).and_then(Value::as_str)
}

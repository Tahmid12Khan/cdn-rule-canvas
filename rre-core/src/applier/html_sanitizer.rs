//! HTML sanitizer. Builds an `ammonia::Builder` from the allow-list YAML and
//! sanitizes component-authored `html_body` before injection.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SanitizerConfig {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    attributes: HashMap<String, Vec<String>>,
    #[serde(default)]
    url_schemes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SanitizerError {
    #[error("reading sanitizer config {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("parsing sanitizer config: {0}")]
    Parse(#[from] serde_yaml::Error),
}

/// The allow-list compiled into the binary. The ONLY sanitizer available inside
/// a Wasm guest, which cannot read files, and the fallback the proxy uses when
/// its configured path is unreadable — so both hosts sanitize identically.
const EMBEDDED_ALLOWLIST: &str = include_str!("../../config/sanitizer.yaml");

/// Build the sanitizer from the embedded allow-list.
///
/// Never panics: the embedded file is covered by a unit test, so a parse failure
/// here would be a build error in disguise, and falling back to ammonia's own
/// (stricter) defaults is safer than aborting a reader's request.
pub fn default_sanitizer() -> ammonia::Builder<'static> {
    match serde_yaml::from_str::<SanitizerConfig>(EMBEDDED_ALLOWLIST) {
        Ok(cfg) => build_from_config(cfg),
        Err(e) => {
            tracing::error!(error = %e, "embedded sanitizer allow-list failed to parse");
            ammonia::Builder::default()
        }
    }
}

/// Load the sanitizer allow-list from YAML into an owned `ammonia::Builder`.
///
/// We leak the allow-list strings to get `'static` lifetimes (the builder is
/// built exactly once at startup and lives for the whole process).
pub fn load_sanitizer(path: &str) -> Result<ammonia::Builder<'static>, SanitizerError> {
    let raw = std::fs::read_to_string(path).map_err(|source| SanitizerError::Read {
        path: path.to_string(),
        source,
    })?;
    let cfg: SanitizerConfig = serde_yaml::from_str(&raw)?;
    Ok(build_from_config(cfg))
}

fn build_from_config(cfg: SanitizerConfig) -> ammonia::Builder<'static> {
    let mut builder = ammonia::Builder::default();

    let tags: HashSet<&'static str> = cfg
        .tags
        .into_iter()
        .map(|t| &*Box::leak(t.into_boxed_str()))
        .collect();
    builder.tags(tags);

    // Generic ("*") attributes apply to all tags; per-tag attributes are scoped.
    if let Some(generic) = cfg.attributes.get("*") {
        let set: HashSet<&'static str> = generic
            .iter()
            .map(|a| &*Box::leak(a.clone().into_boxed_str()))
            .collect();
        builder.generic_attributes(set);
    }
    // Build the full per-tag map first; `tag_attributes` REPLACES the map, so it
    // must be set exactly once with all tags present.
    let mut tag_attrs: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();
    for (tag, attrs) in cfg.attributes.iter() {
        if tag == "*" {
            continue;
        }
        let tag_static: &'static str = Box::leak(tag.clone().into_boxed_str());
        let set: HashSet<&'static str> = attrs
            .iter()
            .map(|a| &*Box::leak(a.clone().into_boxed_str()))
            .collect();
        tag_attrs.insert(tag_static, set);
    }
    if !tag_attrs.is_empty() {
        builder.tag_attributes(tag_attrs);
    }

    let schemes: HashSet<&'static str> = cfg
        .url_schemes
        .into_iter()
        .map(|s| &*Box::leak(s.into_boxed_str()))
        .collect();
    builder.url_schemes(schemes);

    builder
}

/// Sanitize raw HTML against the configured allow-list.
pub fn sanitize(b: &ammonia::Builder<'static>, raw: &str) -> String {
    b.clean(raw).to_string()
}

#[cfg(test)]
mod embedded_tests {
    use super::*;

    /// Compute cannot read files, so the allow-list must be compiled in AND
    /// must actually parse — a silent fallback to ammonia's defaults would
    /// change what components are allowed to render.
    #[test]
    fn default_sanitizer_is_embedded_and_strips_scripts() {
        let cfg: SanitizerConfig =
            serde_yaml::from_str(EMBEDDED_ALLOWLIST).expect("the embedded allow-list must parse");
        assert!(!cfg.tags.is_empty(), "embedded allow-list has no tags");

        let b = default_sanitizer();
        let out = sanitize(&b, "<p>keep</p><script>steal()</script>");
        assert!(out.contains("keep"));
        assert!(
            !out.contains("script"),
            "script tag must not survive: {out}"
        );
    }
}

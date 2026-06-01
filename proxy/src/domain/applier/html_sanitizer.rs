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

/// Load the sanitizer allow-list from YAML into an owned `ammonia::Builder`.
///
/// We leak the allow-list strings to get `'static` lifetimes (the builder is
/// built exactly once at startup and lives for the whole process).
pub fn load_sanitizer(path: &str) -> anyhow::Result<ammonia::Builder<'static>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading sanitizer config {path}: {e}"))?;
    let cfg: SanitizerConfig = serde_yaml::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parsing sanitizer config {path}: {e}"))?;
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

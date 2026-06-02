//! `EvaluationContext` and its `Send` carrier `EvaluationContextParts`.
//!
//! `scraper::Html` is `!Send + !Sync`, and zen's `with_adapter` requires the
//! adapter (which holds an `Arc<EvaluationContext>`) to be `Send + Sync`. So the
//! `Parts` carrier holds the raw HTML `String`, and `into_context` parses it ONCE
//! to extract the `<meta name=… content=…>` tags into a plain (Sync) map — the
//! parsed `scraper::Html` never escapes that call.

use std::collections::HashMap;

use http::HeaderMap;
use serde_json::{json, Value};

/// Device classification derived once from the User-Agent header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceType {
    Mobile,
    Desktop,
    Tablet,
}

impl DeviceType {
    /// Classify from a User-Agent string: tablet markers win over mobile.
    pub fn from_user_agent(ua: &str) -> Self {
        if ua.contains("iPad") || ua.contains("Tablet") {
            DeviceType::Tablet
        } else if ua.contains("Mobi") || ua.contains("Android") || ua.contains("iPhone") {
            DeviceType::Mobile
        } else {
            DeviceType::Desktop
        }
    }

    /// Lowercase wire form used in JSON projections and processor comparisons.
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceType::Mobile => "mobile",
            DeviceType::Desktop => "desktop",
            DeviceType::Tablet => "tablet",
        }
    }
}

/// Read-only, per-request evaluation context. Processors read this directly via
/// the adapter. Everything here is `Send + Sync` (no `scraper::Html`), so the
/// adapter can be shared with zen's `with_adapter`. The response's meta tags are
/// pre-extracted into `meta_tags` (`name` → `content`) when the context is built.
#[derive(Debug)]
pub struct EvaluationContext {
    pub request_headers: HeaderMap,
    pub request_path: String,
    pub request_cookies: HashMap<String, String>,
    pub device: DeviceType,
    /// `<meta name="X" content="Y">` tags from the response, keyed by `name`.
    /// Empty for JSON responses.
    pub meta_tags: HashMap<String, String>,
    /// Parsed response body for a JSON response. `serde_json::Value` is
    /// `Send + Sync` (unlike `scraper::Html`), so the adapter stays shareable.
    /// `None` for HTML responses or when the JSON body fails to parse.
    pub response_json: Option<Value>,
}

/// `Send` carrier: everything in `EvaluationContext` except the `!Send`
/// `scraper::Html`, which is carried as a raw `String` and parsed in the closure.
#[derive(Clone, Debug)]
pub struct EvaluationContextParts {
    pub request_headers: HeaderMap,
    pub request_path: String,
    pub request_cookies: HashMap<String, String>,
    pub device: DeviceType,
    /// Raw response body (HTML or JSON, per `is_json`).
    pub html: String,
    /// When true, `html` carries a JSON body: `into_context` parses it into
    /// `response_json` and leaves `meta_tags` empty (no HTML parse).
    pub is_json: bool,
    /// Only parse the HTML DOM for `<meta>` tags when the canvas being
    /// evaluated actually contains a `meta_tags` node. `false` skips the full
    /// `scraper::Html` parse and leaves `meta_tags` empty. Ignored for JSON.
    pub needs_meta_tags: bool,
}

impl EvaluationContextParts {
    /// Build the `Send` carrier from request data. `body` is kept as a `String`;
    /// `is_json` selects how `into_context` interprets it.
    pub fn from_request(
        headers: &HeaderMap,
        path: &str,
        cookies: &HashMap<String, String>,
        body: String,
        is_json: bool,
    ) -> Self {
        let device = headers
            .get(http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(DeviceType::from_user_agent)
            .unwrap_or(DeviceType::Desktop);

        Self {
            request_headers: headers.clone(),
            request_path: path.to_string(),
            request_cookies: cookies.clone(),
            device,
            html: body,
            is_json,
            // Default to parsing meta tags; the hot path overrides this via
            // `needs_meta_tags` once it knows whether the canvas has a meta_tags
            // node. Direct callers (tests) keep the previous always-parse behavior.
            needs_meta_tags: true,
        }
    }

    /// Reconstruct the full context. For JSON, parse the body into
    /// `response_json` (meta tags empty). For HTML, parse it ONCE to extract meta
    /// tags (the `scraper::Html` is local to this call and never escapes, keeping
    /// the returned `EvaluationContext` `Send + Sync`).
    pub fn into_context(self) -> EvaluationContext {
        if self.is_json {
            let response_json = serde_json::from_str(&self.html).ok();
            return EvaluationContext {
                request_headers: self.request_headers,
                request_path: self.request_path,
                request_cookies: self.request_cookies,
                device: self.device,
                meta_tags: HashMap::new(),
                response_json,
            };
        }
        // Skip the full DOM parse when the canvas has no meta_tags node.
        let meta_tags = if self.needs_meta_tags {
            extract_meta_tags(&self.html)
        } else {
            HashMap::new()
        };
        EvaluationContext {
            request_headers: self.request_headers,
            request_path: self.request_path,
            request_cookies: self.request_cookies,
            device: self.device,
            meta_tags,
            response_json: None,
        }
    }

    /// JSON projection (path/headers/device/cookies) for the top-level evaluate
    /// `Variable`, so switch conditions/expressions can also read the context.
    pub fn to_input_value(&self) -> Value {
        let headers: HashMap<String, String> = self
            .request_headers
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        json!({
            "path": self.request_path,
            "headers": headers,
            "device": self.device.as_str(),
            "cookies": self.request_cookies,
        })
    }
}

/// Parse `html` and collect every `<meta name="X" content="Y">` into a
/// `name → content` map. Returns an empty map on parse/selector failure.
fn extract_meta_tags(html: &str) -> HashMap<String, String> {
    let doc = scraper::Html::parse_document(html);
    let selector = match scraper::Selector::parse("meta[name]") {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let mut map = HashMap::new();
    for el in doc.select(&selector) {
        if let (Some(name), Some(content)) = (el.value().attr("name"), el.value().attr("content")) {
            map.insert(name.to_string(), content.to_string());
        }
    }
    map
}

impl EvaluationContext {
    /// JSON projection for the top-level evaluate `Variable`.
    pub fn to_input_value(&self) -> Value {
        let headers: HashMap<String, String> = self
            .request_headers
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        json!({
            "path": self.request_path,
            "headers": headers,
            "device": self.device.as_str(),
            "cookies": self.request_cookies,
        })
    }
}

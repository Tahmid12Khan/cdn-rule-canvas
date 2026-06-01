//! User classifier. Reads the `rre_user_type` cookie and maps it to exactly one
//! `Canvas`. This is the canvas-isolation source: an anonymous request can only
//! ever evaluate `rule_graph.anonymous`.

use http::HeaderMap;

use crate::domain::graph::Canvas;

const USER_TYPE_COOKIE: &str = "rre_user_type";

/// Classify a request into exactly one canvas. Defaults to `Anonymous` when the
/// cookie is absent or unrecognized.
pub fn classify(headers: &HeaderMap) -> Canvas {
    let Some(value) = cookie_value(headers, USER_TYPE_COOKIE) else {
        return Canvas::Anonymous;
    };
    match value.as_str() {
        "registered" => Canvas::Registered,
        "customer" => Canvas::Customer,
        _ => Canvas::Anonymous,
    }
}

/// Extract a single cookie value from the `Cookie` header(s).
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    for header in headers.get_all(http::header::COOKIE).iter() {
        let Ok(s) = header.to_str() else { continue };
        for pair in s.split(';') {
            let pair = pair.trim();
            if let Some((k, v)) = pair.split_once('=') {
                if k.trim() == name {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

//! Visitor identity: whether the request is logged in, and which product
//! labels the visitor holds. Resolved once per request from a cookie first,
//! falling back to a header (Product Catalogue / Outcomes Library design).
//! Never logs raw cookie/header values.

use std::collections::{HashMap, HashSet};

use http::HeaderMap;

#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub logged_in: bool,
    pub products: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct IdentitySettings {
    pub user_cookie: String,
    pub products_cookie: String,
    pub user_header: String,
    pub products_header: String,
}

/// Resolve identity: cookie first, header fallback. `logged_in` is true when
/// EITHER source has a non-empty user value; `products` is parsed from
/// WHICHEVER source produced a non-empty value, cookie checked first (never
/// merged across sources).
pub fn resolve(
    headers: &HeaderMap,
    cookies: &HashMap<String, String>,
    settings: &IdentitySettings,
) -> Identity {
    let user_from_cookie = cookies.get(&settings.user_cookie).filter(|v| !v.is_empty());
    let user_from_header = header_value(headers, &settings.user_header);
    let logged_in = user_from_cookie.is_some() || user_from_header.is_some();

    let products_source = cookies
        .get(&settings.products_cookie)
        .filter(|v| !v.is_empty())
        .cloned()
        .or_else(|| header_value(headers, &settings.products_header));

    let products = products_source
        .map(|raw| parse_products(&raw))
        .unwrap_or_default();

    Identity {
        logged_in,
        products,
    }
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn parse_products(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn settings() -> IdentitySettings {
        IdentitySettings {
            user_cookie: "rre_user".into(),
            products_cookie: "rre_products".into(),
            user_header: "x-rre-user".into(),
            products_header: "x-rre-products".into(),
        }
    }

    #[test]
    fn cookie_present_marks_logged_in_and_parses_products() {
        let headers = HeaderMap::new();
        let mut cookies = HashMap::new();
        cookies.insert("rre_user".to_string(), "u1".to_string());
        cookies.insert("rre_products".to_string(), "premium, Sports ,".to_string());
        let identity = resolve(&headers, &cookies, &settings());
        assert!(identity.logged_in);
        assert!(identity.products.contains("premium"));
        assert!(identity.products.contains("sports"));
        assert_eq!(identity.products.len(), 2);
    }

    #[test]
    fn header_fallback_when_cookie_absent() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rre-user", "u1".parse().unwrap());
        headers.insert("x-rre-products", "gold".parse().unwrap());
        let identity = resolve(&headers, &HashMap::new(), &settings());
        assert!(identity.logged_in);
        assert!(identity.products.contains("gold"));
    }

    #[test]
    fn absent_everywhere_is_anonymous() {
        let identity = resolve(&HeaderMap::new(), &HashMap::new(), &settings());
        assert!(!identity.logged_in);
        assert!(identity.products.is_empty());
    }

    #[test]
    fn cookie_takes_priority_over_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-rre-products", "header-product".parse().unwrap());
        let mut cookies = HashMap::new();
        cookies.insert("rre_user".to_string(), "u1".to_string());
        cookies.insert("rre_products".to_string(), "cookie-product".to_string());
        let identity = resolve(&headers, &cookies, &settings());
        assert!(identity.products.contains("cookie-product"));
        assert!(!identity.products.contains("header-product"));
    }
}

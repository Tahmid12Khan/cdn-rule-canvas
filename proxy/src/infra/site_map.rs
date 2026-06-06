//! Site map: the dynamic `source host:port -> destination` routing table.
//!
//! On demand the proxy fetches the full Site list from the RRE backend
//! (`GET {base}/api/v1/sites?page_size=100`) and indexes it by normalized
//! `source_host:source_port` (lowercase host). The compiled index is cached in a
//! `moka::future::Cache` with the SAME TTL mechanism the active-version cache
//! uses (`infra::backend_client::BackendClient`). Any error returns an empty
//! index so the forwarder fails open (falls back to `upstream_base_url`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use serde::Deserialize;

/// One Site routing rule — the proxy-facing projection of the backend `SiteRead`
/// DTO (spec §3/§4). Only the fields the proxy needs to route + tag the request.
/// serde defaults / no `deny_unknown_fields` so extra backend columns are ignored.
#[derive(Deserialize, Clone, Debug)]
pub struct Site {
    pub slug: String,
    pub source_host: String,
    pub source_port: i32,
    pub dest_protocol: String,
    pub dest_host: String,
    pub dest_port: i32,
}

impl Site {
    /// The upstream authority+scheme this Site routes to:
    /// `dest_protocol://dest_host:dest_port`.
    pub fn dest_base_url(&self) -> String {
        format!(
            "{}://{}:{}",
            self.dest_protocol, self.dest_host, self.dest_port
        )
    }
}

/// The `Page<SiteRead>` envelope (BACKEND CONTRACT §5 pagination). The proxy only
/// reads `items`; the rest is ignored.
#[derive(Deserialize)]
struct SitePage {
    items: Vec<Site>,
}

/// A compiled, immutable index: normalized `host:port` -> `Site`.
#[derive(Debug, Default)]
pub struct SiteIndex {
    map: HashMap<String, Site>,
}

impl SiteIndex {
    fn from_sites(sites: Vec<Site>) -> Self {
        let mut map = HashMap::with_capacity(sites.len());
        for site in sites {
            let key = normalize_key(&site.source_host, site.source_port);
            map.insert(key, site);
        }
        Self { map }
    }

    /// Look up a Site by an already-normalized `host:port` key.
    fn get(&self, key: &str) -> Option<&Site> {
        self.map.get(key)
    }

    /// Look up a Site by an inbound `Host` header value. `https` selects the
    /// default port 443 when the header omits one, else 80 (http). The host is
    /// lowercased; the port from the header (when present) wins over the default.
    pub fn lookup(&self, host_header: &str, is_https: bool) -> Option<&Site> {
        let (host, port) = parse_host_header(host_header, is_https);
        self.get(&normalize_key(&host, port))
    }
}

/// Builds + caches the [`SiteIndex`] fetched from the backend.
pub struct SiteMap {
    http: reqwest::Client,
    base: String,
    cache: Cache<(), Arc<SiteIndex>>,
}

impl SiteMap {
    /// `ttl_secs` mirrors the active-version cache TTL (`active_version_ttl_secs`).
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(ttl_secs))
            .max_capacity(1)
            .build();
        Self { http, base, cache }
    }

    /// Return the current (TTL-cached) site index, fetching from the backend on a
    /// miss. A successful fetch (including a genuinely empty 200) is cached for the
    /// TTL. Any transport / non-2xx / decode error yields a FRESH empty index that
    /// is NOT cached — the forwarder fails open (falls back to `upstream_base_url`)
    /// but the next request re-fetches once the backend recovers, instead of
    /// serving a sticky empty index for the full TTL. Never panics.
    pub async fn index(&self) -> Arc<SiteIndex> {
        if let Some(hit) = self.cache.get(&()).await {
            return hit;
        }
        match self.fetch().await {
            Ok(index) => {
                let index = Arc::new(index);
                self.cache.insert((), index.clone()).await;
                index
            }
            // Do NOT cache transient failures: return an uncached empty index so a
            // subsequent request retries the backend immediately on recovery.
            Err(()) => Arc::new(SiteIndex::default()),
        }
    }

    /// `Ok(index)` on a successful 2xx (cacheable). `Err(())` on a transport /
    /// non-2xx / decode error (fail-open, but must NOT be cached).
    async fn fetch(&self) -> Result<SiteIndex, ()> {
        let url = format!(
            "{}/api/v1/sites?page_size=100",
            self.base.trim_end_matches('/')
        );
        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "site_map=miss (transport error)");
                return Err(());
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "site_map=miss (non-2xx)");
            return Err(());
        }
        match resp.json::<SitePage>().await {
            Ok(page) => Ok(SiteIndex::from_sites(page.items)),
            Err(e) => {
                tracing::warn!(error = %e, "site_map=miss (decode error)");
                Err(())
            }
        }
    }
}

/// Normalize a `(host, port)` into the index key: `lowercasehost:port`.
fn normalize_key(host: &str, port: i32) -> String {
    format!("{}:{}", host.trim().to_ascii_lowercase(), port)
}

/// Parse an inbound `Host` header into `(lowercase host, port)`. A port in the
/// header wins; otherwise the scheme default applies (443 https, 80 http). An
/// unparseable / empty port falls back to the scheme default. IPv6 literals
/// (`[::1]:port`) are handled by splitting on the LAST colon only.
fn parse_host_header(host_header: &str, is_https: bool) -> (String, i32) {
    let default_port = if is_https { 443 } else { 80 };
    let trimmed = host_header.trim();
    // IPv6 literal: `[::1]` or `[::1]:9000`.
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let host = &rest[..close];
            let after = &rest[close + 1..];
            let port = after
                .strip_prefix(':')
                .and_then(|p| p.trim().parse::<i32>().ok())
                .unwrap_or(default_port);
            return (format!("[{}]", host.to_ascii_lowercase()), port);
        }
    }
    match trimmed.rsplit_once(':') {
        Some((host, port)) => {
            let port = port.trim().parse::<i32>().unwrap_or(default_port);
            (host.to_ascii_lowercase(), port)
        }
        None => (trimmed.to_ascii_lowercase(), default_port),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> SiteIndex {
        SiteIndex::from_sites(vec![
            Site {
                slug: "demo-localhost".to_string(),
                source_host: "localhost".to_string(),
                source_port: 9000,
                dest_protocol: "http".to_string(),
                dest_host: "demo-upstream".to_string(),
                dest_port: 8081,
            },
            Site {
                slug: "secure".to_string(),
                source_host: "Secure.Example.COM".to_string(),
                source_port: 443,
                dest_protocol: "https".to_string(),
                dest_host: "origin.example.com".to_string(),
                dest_port: 8443,
            },
        ])
    }

    #[test]
    fn matches_explicit_host_and_port() {
        let idx = index();
        let site = idx.lookup("localhost:9000", false).expect("match");
        assert_eq!(site.slug, "demo-localhost");
        assert_eq!(site.dest_base_url(), "http://demo-upstream:8081");
    }

    #[test]
    fn host_lookup_is_case_insensitive() {
        let idx = index();
        let site = idx.lookup("LOCALHOST:9000", false).expect("match");
        assert_eq!(site.slug, "demo-localhost");
    }

    #[test]
    fn source_host_indexed_case_insensitively() {
        let idx = index();
        // The stored source host has mixed case; lookup with lowercase + the
        // https default port (443) must still match.
        let site = idx.lookup("secure.example.com", true).expect("match");
        assert_eq!(site.slug, "secure");
        assert_eq!(site.dest_base_url(), "https://origin.example.com:8443");
    }

    #[test]
    fn http_default_port_80_when_absent() {
        let idx = SiteIndex::from_sites(vec![Site {
            slug: "plain".to_string(),
            source_host: "plain.test".to_string(),
            source_port: 80,
            dest_protocol: "http".to_string(),
            dest_host: "origin".to_string(),
            dest_port: 8080,
        }]);
        // No port in the header + http scheme -> default 80 -> match.
        assert!(idx.lookup("plain.test", false).is_some());
        // Same host with the https default (443) must NOT match the :80 entry.
        assert!(idx.lookup("plain.test", true).is_none());
    }

    #[test]
    fn https_default_port_443_when_absent() {
        let idx = index();
        // secure.example.com is registered on :443; an https request with no
        // explicit port resolves to 443 and matches.
        assert!(idx.lookup("secure.example.com", true).is_some());
    }

    #[test]
    fn unknown_host_is_fallback() {
        let idx = index();
        assert!(idx.lookup("other.test:9000", false).is_none());
        assert!(idx.lookup("localhost:9999", false).is_none());
    }

    #[test]
    fn ipv6_literal_host_parses() {
        let idx = SiteIndex::from_sites(vec![Site {
            slug: "v6".to_string(),
            source_host: "[::1]".to_string(),
            source_port: 9000,
            dest_protocol: "http".to_string(),
            dest_host: "origin".to_string(),
            dest_port: 8080,
        }]);
        assert!(idx.lookup("[::1]:9000", false).is_some());
        assert!(idx.lookup("[::1]", false).is_none()); // defaults to :80, no match
    }

    /// A transient backend error must NOT be cached: the first request hits a 500
    /// (fail-open => empty index, uncached), and the very next request — still
    /// within the TTL — re-fetches and sees the now-healthy backend's data,
    /// without waiting for the TTL to expire.
    #[tokio::test]
    async fn error_response_is_not_cached() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        // First call: backend is down (500). `expect(1)` pins it to exactly one hit.
        Mock::given(method("GET"))
            .and(path("/api/v1/sites"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        // A long TTL proves recovery is NOT gated on TTL expiry: if the empty
        // error result were cached, the second call would never reach the backend.
        let map = SiteMap::new(reqwest::Client::new(), server.uri(), 3600);

        let first = map.index().await;
        assert!(
            first.get("localhost:9000").is_none(),
            "error fails open to an empty index"
        );

        // Backend recovers (200 with one site). Mounted after the 500 mock is
        // exhausted, so the next fetch sees it.
        Mock::given(method("GET"))
            .and(path("/api/v1/sites"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{
                    "slug": "demo-localhost",
                    "source_host": "localhost",
                    "source_port": 9000,
                    "dest_protocol": "http",
                    "dest_host": "demo-upstream",
                    "dest_port": 8081
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let second = map.index().await;
        assert!(
            second.get("localhost:9000").is_some(),
            "the error result was not cached, so the recovered data is served immediately"
        );

        // Successful result IS cached: a third call within the TTL serves from cache
        // (the 200 mock's `expect(1)` would fail on a second backend hit).
        let third = map.index().await;
        assert!(third.get("localhost:9000").is_some());
    }
}

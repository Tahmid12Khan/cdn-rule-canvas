//! Golden fixtures: the CROSS-HOST PARITY CONTRACT for `rre_core::apply`.
//!
//! `apply.rs` builds its bundles as typed Rust values, so it cannot catch a
//! change to the WIRE format — a renamed serde field, a changed default, a new
//! required key. These fixtures are committed JSON parsed by the same
//! `Deserialize` impls the Fastly exporter writes for, so a skew between what
//! the backend exports and what a host reads fails here first.
//!
//! Bodies are compared as EXACT strings: normalising whitespace would hide
//! exactly the `lol_html` / `ammonia` output drift a golden exists to catch.
//!
//! See `tests/golden/README.md` for how to add a case.

use std::collections::HashMap;
use std::path::PathBuf;

use rre_core::edge::{BodyKind, EdgeBundle, RequestFacts, SkipReason};
use rre_core::http::{HeaderMap, HeaderName, HeaderValue};
use rre_core::identity::Identity;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    /// Why this case exists. Printed on failure so a red golden says what broke.
    description: String,
    body_kind: FixtureBodyKind,
    facts: FixtureFacts,
    bundle: EdgeBundle,
    body: String,
    expected: Expected,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum FixtureBodyKind {
    Html,
    Json,
}

impl From<FixtureBodyKind> for BodyKind {
    fn from(k: FixtureBodyKind) -> Self {
        match k {
            FixtureBodyKind::Html => BodyKind::Html,
            FixtureBodyKind::Json => BodyKind::Json,
        }
    }
}

/// Serde-friendly mirror of `RequestFacts`, which holds an `http::HeaderMap` and
/// an `Identity` — neither of which is `Deserialize`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureFacts {
    path: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    cookies: HashMap<String, String>,
    #[serde(default)]
    site: Option<String>,
    #[serde(default)]
    identity: FixtureIdentity,
}

/// The host resolves identity from cookies/headers before calling `apply`, so a
/// fixture states the RESOLVED identity directly rather than re-deriving it.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FixtureIdentity {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    products: Vec<String>,
}

impl FixtureFacts {
    fn into_request_facts(self) -> RequestFacts {
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .unwrap_or_else(|e| panic!("bad header name `{name}`: {e}"));
            let value = HeaderValue::from_str(value)
                .unwrap_or_else(|e| panic!("bad header value for `{name}`: {e}"));
            headers.insert(name, value);
        }
        RequestFacts {
            headers,
            path: self.path,
            cookies: self.cookies,
            site: self.site,
            identity: Identity {
                logged_in: self.identity.logged_in,
                products: self.identity.products.into_iter().collect(),
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    body: String,
    changed: bool,
    features: Vec<ExpectedFeature>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFeature {
    feature_id: String,
    version_number: i32,
    matched: bool,
    changed: bool,
    /// `SkipReason::as_str` wire form, or absent for a feature that ran.
    #[serde(default)]
    skipped_reason: Option<String>,
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Every `.json` under `tests/golden`, sorted so a failure names a stable case.
fn fixture_paths() -> Vec<PathBuf> {
    let dir = golden_dir();
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("golden fixture dir {} is unreadable: {e}", dir.display()));

    let mut paths: Vec<PathBuf> = entries
        .map(|e| e.expect("reading a golden dir entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    // An empty glob that silently passes is how a golden suite dies unnoticed.
    assert!(
        !paths.is_empty(),
        "no golden fixtures found in {}",
        dir.display()
    );
    paths
}

#[test]
fn golden_fixtures_pin_apply() {
    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let fixture: Fixture =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name}: parse failed: {e}"));
        let ctx = format!("{name} ({})", fixture.description);

        let facts = fixture.facts.into_request_facts();
        let outcome = rre_core::apply(
            &fixture.bundle,
            &facts,
            fixture.body_kind.into(),
            fixture.body,
            &rre_core::default_sanitizer(),
        );

        // The `rre` telemetry block carries measured times, so it can never be
        // pinned byte-for-byte. Strip it, compare the RULE output (what a
        // fixture is for), then assert the block separately below.
        let (body, telemetry_seen) = strip_telemetry(&outcome.body, fixture.body_kind.into());
        assert_eq!(body, fixture.expected.body, "{ctx}: body");
        // A feature that changed the body MUST carry an entry, and one that
        // changed nothing must not inject at all.
        assert_eq!(
            telemetry_seen,
            !outcome.feature_expressions.is_empty(),
            "{ctx}: telemetry present iff a feature changed the body"
        );
        for (feature_id, entry) in &outcome.feature_expressions {
            assert!(
                !entry.expressions.is_empty(),
                "{ctx}: `{feature_id}` entry with no expressions"
            );
            assert!(
                entry.time_took_ms.contains('.'),
                "{ctx}: `{feature_id}` time_took_ms is not `d.dd`: {}",
                entry.time_took_ms
            );
        }
        assert_eq!(outcome.changed, fixture.expected.changed, "{ctx}: changed");
        assert_eq!(
            outcome.features.len(),
            fixture.expected.features.len(),
            "{ctx}: feature report count"
        );

        for (got, want) in outcome.features.iter().zip(&fixture.expected.features) {
            let ctx = format!("{ctx}: feature `{}`", want.feature_id);
            assert_eq!(got.feature_id, want.feature_id, "{ctx}: id");
            assert_eq!(got.version_number, want.version_number, "{ctx}: version");
            assert_eq!(got.matched, want.matched, "{ctx}: matched");
            assert_eq!(got.changed, want.changed, "{ctx}: changed");
            assert_eq!(
                got.skipped_reason.map(SkipReason::as_str),
                want.skipped_reason.as_deref(),
                "{ctx}: skipped_reason"
            );
        }
    }
}

/// Remove the injected `rre` telemetry from a body so the rule output can be
/// compared exactly. Returns the stripped body and whether anything was found.
fn strip_telemetry(body: &str, kind: BodyKind) -> (String, bool) {
    match kind {
        BodyKind::Json => {
            let Ok(mut v) = serde_json::from_str::<serde_json::Value>(body) else {
                return (body.to_string(), false);
            };
            let removed = v.as_object_mut().and_then(|o| o.remove("rre")).is_some();
            if !removed {
                // Return the body VERBATIM when there was nothing to strip: a
                // re-serialize would reorder keys and fail the exact compare
                // for a body `apply` never touched.
                return (body.to_string(), false);
            }
            (
                serde_json::to_string(&v).unwrap_or_else(|_| body.to_string()),
                true,
            )
        }
        BodyKind::Html => match body.find("<script>window.rre=") {
            Some(start) => {
                let end = body[start..]
                    .find("</script>")
                    .map(|i| start + i + "</script>".len())
                    .unwrap_or(body.len());
                (format!("{}{}", &body[..start], &body[end..]), true)
            }
            None => (body.to_string(), false),
        },
    }
}

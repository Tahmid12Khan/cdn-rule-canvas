use rre_proxy::infra::feature_map::{FeatureMap, FeatureMapEntry};

fn map() -> FeatureMap {
    FeatureMap::from_entries(vec![FeatureMapEntry {
        host: "localhost:9000".to_string(),
        path_glob: "/article*".to_string(),
        feature_id: "dn-article".to_string(),
    }])
    .unwrap()
}

#[test]
fn resolves_matching_host_and_path() {
    let m = map();
    assert_eq!(
        m.resolve("localhost:9000", "/article/123"),
        Some("dn-article".to_string())
    );
    assert_eq!(
        m.resolve("localhost:9000", "/articles"),
        Some("dn-article".to_string())
    );
}

#[test]
fn host_match_is_case_insensitive() {
    let m = map();
    assert_eq!(
        m.resolve("LOCALHOST:9000", "/article/1"),
        Some("dn-article".to_string())
    );
}

#[test]
fn misses_on_wrong_host_or_path() {
    let m = map();
    assert_eq!(m.resolve("other:9000", "/article/1"), None);
    assert_eq!(m.resolve("localhost:9000", "/news/1"), None);
}

/// resolve_all returns EVERY matching feature, in map order, de-duplicated.
/// Two features registered on overlapping globs both apply to a matching path.
#[test]
fn resolve_all_returns_every_match_in_order_deduped() {
    let m = FeatureMap::from_entries(vec![
        FeatureMapEntry {
            host: "localhost:9000".to_string(),
            path_glob: "/**".to_string(),
            feature_id: "dn-article".to_string(),
        },
        FeatureMapEntry {
            host: "localhost:9000".to_string(),
            path_glob: "/**".to_string(),
            feature_id: "dn-json-article".to_string(),
        },
        // Duplicate feature on a narrower glob — must NOT appear twice.
        FeatureMapEntry {
            host: "localhost:9000".to_string(),
            path_glob: "/proxy/**".to_string(),
            feature_id: "dn-article".to_string(),
        },
    ])
    .unwrap();

    assert_eq!(
        m.resolve_all("localhost:9000", "/proxy/v2/content/2-1-1997318"),
        vec!["dn-article".to_string(), "dn-json-article".to_string()]
    );
    // No host match -> empty (pass-through).
    assert!(m.resolve_all("other:9000", "/proxy/x").is_empty());
}

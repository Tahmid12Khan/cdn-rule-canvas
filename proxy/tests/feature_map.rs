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

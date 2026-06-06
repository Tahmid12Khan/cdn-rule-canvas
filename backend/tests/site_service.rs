//! Service-level integration tests for the site domain (real Postgres via
//! testcontainers). Covers happy path + error paths per service method,
//! including the `name ILIKE '%q%'` picker filter and the 409 conflict mapping.

mod common;

use std::collections::HashMap;

use rre_backend::{
    error::AppError,
    schemas::{
        pagination::PageParams,
        site::{SiteCreate, SiteUpdate},
    },
    services::site_service,
};

/// Build a header map from `(name, value)` pairs.
fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn create_input(slug: &str, name: &str) -> SiteCreate {
    SiteCreate {
        slug: slug.to_string(),
        name: name.to_string(),
        source_protocol: "http".to_string(),
        source_host: "localhost".to_string(),
        source_port: 9000,
        dest_protocol: "http".to_string(),
        dest_host: "demo-upstream".to_string(),
        dest_port: 8081,
        headers: None,
    }
}

#[tokio::test]
async fn create_then_get_roundtrips() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let created = site_service::create(pool, create_input("demo-localhost", "Demo (localhost)"))
        .await
        .expect("create");
    assert_eq!(created.slug, "demo-localhost");
    assert_eq!(created.source_host, "localhost");
    assert_eq!(created.dest_port, 8081);

    let fetched = site_service::get(pool, "demo-localhost")
        .await
        .expect("get");
    assert_eq!(fetched.slug, created.slug);
    assert_eq!(fetched.created_at, created.created_at);
}

#[tokio::test]
async fn create_duplicate_slug_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    site_service::create(pool, create_input("demo-localhost", "First"))
        .await
        .expect("first create");

    // Same slug, different name/source → still a PK conflict.
    let mut dup = create_input("demo-localhost", "Second");
    dup.source_port = 9100;
    let err = site_service::create(pool, dup)
        .await
        .expect_err("expected slug conflict");
    assert!(matches!(err, AppError::SlugConflict(_)), "got {err:?}");
}

#[tokio::test]
async fn create_duplicate_source_conflicts() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    site_service::create(pool, create_input("site-a", "Site A"))
        .await
        .expect("first create");

    // Different slug + name but same (source_host, source_port) → unique-index
    // conflict mapped to 409.
    let err = site_service::create(pool, create_input("site-b", "Site B"))
        .await
        .expect_err("expected source conflict");
    assert!(matches!(err, AppError::SlugConflict(_)), "got {err:?}");
}

#[tokio::test]
async fn create_invalid_slug_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = site_service::create(pool, create_input("AB", "Bad"))
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn list_filters_by_name_q() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut a = create_input("alpha-site", "Alpha News");
    a.source_port = 9001;
    site_service::create(pool, a).await.expect("create alpha");

    let mut b = create_input("beta-site", "Beta Times");
    b.source_port = 9002;
    site_service::create(pool, b).await.expect("create beta");

    // Case-insensitive substring match on name.
    let filtered = site_service::list(pool, &PageParams::default(), Some("alpha"))
        .await
        .expect("list filtered");
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.items[0].slug, "alpha-site");

    // Empty q is treated as no filter.
    let all = site_service::list(pool, &PageParams::default(), Some(""))
        .await
        .expect("list all");
    assert_eq!(all.total, 2);

    let none = site_service::list(pool, &PageParams::default(), None)
        .await
        .expect("list none-filter");
    assert_eq!(none.total, 2);
}

#[tokio::test]
async fn update_partial_changes_only_named_fields() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    site_service::create(pool, create_input("demo-localhost", "Old"))
        .await
        .expect("create");

    let updated = site_service::update(
        pool,
        "demo-localhost",
        SiteUpdate {
            dest_host: Some("other-upstream".to_string()),
            dest_port: Some(8082),
            ..SiteUpdate::default()
        },
    )
    .await
    .expect("update");
    assert_eq!(updated.name, "Old", "name unchanged");
    assert_eq!(updated.dest_host, "other-upstream");
    assert_eq!(updated.dest_port, 8082);
    assert_eq!(updated.source_host, "localhost", "source unchanged");
}

#[tokio::test]
async fn update_empty_is_noop_returning_current() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    site_service::create(pool, create_input("demo-localhost", "Keep"))
        .await
        .expect("create");

    let updated = site_service::update(pool, "demo-localhost", SiteUpdate::default())
        .await
        .expect("noop update");
    assert_eq!(updated.name, "Keep");
}

#[tokio::test]
async fn update_missing_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = site_service::update(
        pool,
        "ghost",
        SiteUpdate {
            name: Some("X".to_string()),
            ..SiteUpdate::default()
        },
    )
    .await
    .expect_err("expected not found");
    assert!(matches!(err, AppError::SiteNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn delete_then_get_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    site_service::create(pool, create_input("demo-localhost", "Doomed"))
        .await
        .expect("create");

    site_service::delete(pool, "demo-localhost")
        .await
        .expect("delete");

    let err = site_service::get(pool, "demo-localhost")
        .await
        .expect_err("expected not found");
    assert!(matches!(err, AppError::SiteNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn delete_missing_is_not_found() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let err = site_service::delete(pool, "ghost")
        .await
        .expect_err("expected not found");
    assert!(matches!(err, AppError::SiteNotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn create_with_headers_roundtrips() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut input = create_input("demo-localhost", "Demo");
    input.headers = Some(headers(&[
        ("X-Forwarded-Host", "news.example.com"),
        ("X-RRE-Tenant", "acme"),
    ]));

    let created = site_service::create(pool, input).await.expect("create");
    assert_eq!(
        created.headers.get("X-Forwarded-Host").map(String::as_str),
        Some("news.example.com")
    );
    assert_eq!(created.headers.len(), 2);

    let fetched = site_service::get(pool, "demo-localhost")
        .await
        .expect("get");
    assert_eq!(fetched.headers, created.headers);
}

#[tokio::test]
async fn create_without_headers_defaults_to_empty_map() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let created = site_service::create(pool, create_input("demo-localhost", "Demo"))
        .await
        .expect("create");
    assert!(created.headers.is_empty());
}

#[tokio::test]
async fn create_with_crlf_header_value_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut input = create_input("demo-localhost", "Demo");
    input.headers = Some(headers(&[("X-Foo", "a\r\nInjected: 1")]));

    let err = site_service::create(pool, input)
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn create_with_invalid_header_name_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut input = create_input("demo-localhost", "Demo");
    input.headers = Some(headers(&[("Bad Name", "1")]));

    let err = site_service::create(pool, input)
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn create_with_too_many_headers_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let many: HashMap<String, String> = (0..33)
        .map(|i| (format!("X-H-{i}"), "v".to_string()))
        .collect();
    let mut input = create_input("demo-localhost", "Demo");
    input.headers = Some(many);

    let err = site_service::create(pool, input)
        .await
        .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn patch_headers_replaces_map_and_none_leaves_unchanged() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    let mut input = create_input("demo-localhost", "Demo");
    input.headers = Some(headers(&[("X-One", "1"), ("X-Two", "2")]));
    site_service::create(pool, input).await.expect("create");

    // PATCH with headers replaces the whole map.
    let replaced = site_service::update(
        pool,
        "demo-localhost",
        SiteUpdate {
            headers: Some(headers(&[("X-Three", "3")])),
            ..SiteUpdate::default()
        },
    )
    .await
    .expect("update headers");
    assert_eq!(replaced.headers, headers(&[("X-Three", "3")]));

    // PATCH without headers (changing only name) leaves the map unchanged.
    let untouched = site_service::update(
        pool,
        "demo-localhost",
        SiteUpdate {
            name: Some("Renamed".to_string()),
            ..SiteUpdate::default()
        },
    )
    .await
    .expect("update name only");
    assert_eq!(untouched.name, "Renamed");
    assert_eq!(untouched.headers, headers(&[("X-Three", "3")]));
}

#[tokio::test]
async fn patch_with_invalid_header_value_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    site_service::create(pool, create_input("demo-localhost", "Demo"))
        .await
        .expect("create");

    let err = site_service::update(
        pool,
        "demo-localhost",
        SiteUpdate {
            headers: Some(headers(&[("X-Foo", "bad\nvalue")])),
            ..SiteUpdate::default()
        },
    )
    .await
    .expect_err("expected validation error");
    assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
}

#[tokio::test]
async fn create_with_forbidden_header_name_is_validation_error() {
    let db = common::setup().await;
    let pool = &db.state.pool;

    // Overriding a proxy-managed routing/framing header is rejected
    // (case-insensitive) before any persistence.
    for name in ["Host", "Content-Length"] {
        let mut input = create_input("demo-localhost", "Demo");
        input.headers = Some(headers(&[(name, "evil")]));

        let err = site_service::create(pool, input)
            .await
            .expect_err("expected validation error");
        assert!(matches!(err, AppError::Validation { .. }), "got {err:?}");
    }
}

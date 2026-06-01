use http::{HeaderMap, HeaderValue};
use rre_proxy::forwarder::strip_hop_by_hop;

#[test]
fn strips_hop_by_hop_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("connection", HeaderValue::from_static("keep-alive"));
    headers.insert("keep-alive", HeaderValue::from_static("timeout=5"));
    headers.insert("transfer-encoding", HeaderValue::from_static("chunked"));
    headers.insert("content-type", HeaderValue::from_static("text/html"));

    strip_hop_by_hop(&mut headers);

    assert!(headers.get("connection").is_none());
    assert!(headers.get("keep-alive").is_none());
    assert!(headers.get("transfer-encoding").is_none());
    // Non-hop-by-hop headers are preserved.
    assert_eq!(
        headers.get("content-type").and_then(|v| v.to_str().ok()),
        Some("text/html")
    );
}

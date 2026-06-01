use http::HeaderMap;
use rre_proxy::domain::classifier::classify;
use rre_proxy::domain::graph::Canvas;

fn headers_with_cookie(cookie: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(http::header::COOKIE, cookie.parse().unwrap());
    h
}

#[test]
fn defaults_to_anonymous_without_cookie() {
    assert_eq!(classify(&HeaderMap::new()), Canvas::Anonymous);
}

#[test]
fn classifies_registered_and_customer() {
    assert_eq!(
        classify(&headers_with_cookie("rre_user_type=registered")),
        Canvas::Registered
    );
    assert_eq!(
        classify(&headers_with_cookie("a=1; rre_user_type=customer; b=2")),
        Canvas::Customer
    );
}

#[test]
fn unknown_value_falls_back_to_anonymous() {
    assert_eq!(
        classify(&headers_with_cookie("rre_user_type=banana")),
        Canvas::Anonymous
    );
}

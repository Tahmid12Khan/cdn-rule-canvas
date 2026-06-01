//! Trace-id middleware. Ensures every request carries an `X-RRE-Trace-Id`
//! (UUID v4, generated if absent), binds it to a tracing span, stamps it on
//! request extensions, and echoes it on the response.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

pub const TRACE_HEADER: &str = "x-rre-trace-id";

/// A trace id stored on request extensions so downstream handlers can read it.
#[derive(Clone, Debug)]
pub struct TraceId(pub String);

/// Build the middleware layer applied to the whole router. Returned as a
/// `from_fn` layer; callers pass it straight to `Router::layer`.
#[allow(clippy::type_complexity)]
pub fn layer() -> axum::middleware::FromFnLayer<fn(Request, Next) -> TraceFuture, (), (Request,)> {
    let f: fn(Request, Next) -> TraceFuture = trace_mw;
    axum::middleware::from_fn(f)
}

/// The boxed future returned by the middleware function.
pub type TraceFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>;

/// Per-request middleware: resolve/generate the trace id, run the inner service
/// inside a span, and echo the trace id on the response.
fn trace_mw(mut req: Request, next: Next) -> TraceFuture {
    Box::pin(async move {
        let trace_id = req
            .headers()
            .get(TRACE_HEADER)
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        req.extensions_mut().insert(TraceId(trace_id.clone()));

        let span = tracing::info_span!("request", trace_id = %trace_id);
        let mut res: Response = next.run(req).instrument(span).await;

        if let Ok(value) = HeaderValue::from_str(&trace_id) {
            res.headers_mut()
                .insert(HeaderName::from_static(TRACE_HEADER), value);
        }
        res
    })
}

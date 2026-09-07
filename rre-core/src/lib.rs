//! Shared RRE rule evaluation and response-body application.
//!
//! One crate, two hosts: the RRE proxy (native, tokio, moka caches) and the
//! Fastly Compute edge service (`wasm32-wasip1`, single-threaded, no I/O, no
//! filesystem). Everything a rule decision depends on lives here so the Test
//! panel, the proxy and the edge can never disagree about what a canvas means.
//!
//! MUST NOT depend on tokio, reqwest, moka or any I/O crate — `make
//! rre-core-check` builds it for `wasm32-wasip1` to enforce that.

pub mod adapter;
pub mod applier;
pub mod bundle;
pub mod context;
pub mod edge;
pub mod evaluator;
pub mod graph;
pub mod identity;
pub mod processors;
pub mod translator;

pub use applier::html_sanitizer::{default_sanitizer, load_sanitizer, SanitizerError};
pub use edge::apply;

/// Re-exported so a host builds its `RequestFacts` with the SAME `http`
/// version this crate compiled against. A host that adds its own `http`
/// dependency can silently get a different major version, and the resulting
/// type mismatch is reported far from its cause.
pub use http;
pub use evaluator::{
    compile, evaluate_sync, evaluate_with_trace_sync, EvalTrace, MatchedAction, TraceStep,
};

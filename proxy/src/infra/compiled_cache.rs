//! Compiled-graph cache. Keyed by `(feature_id, version_number)`.
//! Bounded LRU (<= 256). Stores `Arc<DecisionContent>` post `.compile()`.

use std::sync::Arc;

use moka::sync::Cache;
use zen_engine::model::DecisionContent;

pub struct CompiledCache {
    inner: Cache<(String, i32), Arc<DecisionContent>>,
}

impl CompiledCache {
    /// Build a bounded LRU cache (capacity capped at 256 per the contract).
    pub fn new(capacity: u64) -> Self {
        let capacity = capacity.min(256);
        Self {
            inner: Cache::builder().max_capacity(capacity).build(),
        }
    }

    /// Cache hit -> clone the `Arc`. Miss -> run `build`, insert. `build` is
    /// expected to return an ALREADY-COMPILED graph (`rre_core::compile` does).
    pub fn get_or_compile(
        &self,
        feature_id: &str,
        version_number: i32,
        build: impl FnOnce() -> DecisionContent,
    ) -> Arc<DecisionContent> {
        let key = (feature_id.to_string(), version_number);
        if let Some(hit) = self.inner.get(&key) {
            return hit;
        }
        let arc = Arc::new(build());
        self.inner.insert(key, arc.clone());
        arc
    }
}

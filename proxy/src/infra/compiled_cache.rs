//! Compiled-graph cache. Keyed by `(feature_id, version_number, Canvas)` so the
//! three canvases of one version are cached independently (canvas isolation).
//! Bounded LRU (<= 256). Stores `Arc<DecisionContent>` post `.compile()`.

use std::sync::Arc;

use moka::sync::Cache;
use zen_engine::model::DecisionContent;

use crate::domain::graph::Canvas;

pub struct CompiledCache {
    inner: Cache<(String, i32, Canvas), Arc<DecisionContent>>,
}

impl CompiledCache {
    /// Build a bounded LRU cache (capacity capped at 256 per the contract).
    pub fn new(capacity: u64) -> Self {
        let capacity = capacity.min(256);
        Self {
            inner: Cache::builder().max_capacity(capacity).build(),
        }
    }

    /// Cache hit -> clone the `Arc`. Miss -> run `build`, `.compile()`, insert.
    pub fn get_or_compile(
        &self,
        feature_id: &str,
        version_number: i32,
        canvas: Canvas,
        build: impl FnOnce() -> DecisionContent,
    ) -> Arc<DecisionContent> {
        let key = (feature_id.to_string(), version_number, canvas);
        if let Some(hit) = self.inner.get(&key) {
            return hit;
        }
        let mut content = build();
        content.compile();
        let arc = Arc::new(content);
        self.inner.insert(key, arc.clone());
        arc
    }
}

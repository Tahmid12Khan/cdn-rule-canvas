//! Backend HTTP client + the `ActiveVersionRead` DTO mirror.
//!
//! The active-version payload is fetched from the RRE backend and cached in a
//! `moka::future::Cache` (TTL configured at startup). Any error / 404 /
//! NO_LIVE_VERSION returns `None` so the forwarder fails open (pass-through).

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::graph::{Canvas, CanvasGraph, RuleGraph};

/// Placement mirror (BACKEND CONTRACT §2). snake_case wire form.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Inline,
    StickyFooter,
    Popup,
}

/// Active-version payload — EXACT shape from BACKEND CONTRACT §5.
#[derive(Deserialize, Clone, Debug)]
pub struct ActiveVersionRead {
    pub version_number: i32,
    pub rule_graph: RuleGraph,
    pub outcomes: Vec<ActiveOutcome>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ActiveOutcome {
    pub id: Uuid,
    pub title: String,
    pub is_builtin: bool,
    pub order_index: i32,
    pub components: Vec<ActiveComponent>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ActiveComponent {
    pub id: Uuid,
    pub slug: String,
    pub r#type: String,
    pub config: serde_json::Value,
    pub placement: Placement,
    pub order_index: i32,
}

impl ActiveVersionRead {
    /// The `CanvasGraph` for the classified user class (canvas isolation).
    pub fn canvas(&self, c: Canvas) -> &CanvasGraph {
        self.rule_graph.canvas(c)
    }

    /// Find an outcome by id (the eval result `outcomeId`).
    pub fn find_outcome(&self, id: Uuid) -> Option<&ActiveOutcome> {
        self.outcomes.iter().find(|o| o.id == id)
    }
}

impl ActiveOutcome {
    /// True if this is the builtin "ShowContent" outcome (a no-op terminal).
    pub fn is_builtin_show_content(&self) -> bool {
        self.is_builtin
    }
}

/// Environment selector for the active-version fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Env {
    Live,
    Staging,
}

impl Env {
    fn as_query(&self) -> &'static str {
        match self {
            Env::Live => "live",
            Env::Staging => "staging",
        }
    }
}

pub struct BackendClient {
    http: reqwest::Client,
    base: String,
    cache: Cache<(String, Env), Arc<ActiveVersionRead>>,
}

impl BackendClient {
    pub fn new(http: reqwest::Client, base: String, ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(ttl_secs))
            .max_capacity(1024)
            .build();
        Self { http, base, cache }
    }

    /// `GET {base}/api/v1/features/{id}/active-version?env=...`, TTL-cached.
    /// Returns `None` on 404 / NO_LIVE_VERSION / any transport or decode error
    /// (fail-open per the proxy contract).
    pub async fn active_version(
        &self,
        feature_id: &str,
        env: Env,
    ) -> Option<Arc<ActiveVersionRead>> {
        let key = (feature_id.to_string(), env);
        if let Some(hit) = self.cache.get(&key).await {
            return Some(hit);
        }

        let url = format!(
            "{}/api/v1/features/{}/active-version?env={}",
            self.base.trim_end_matches('/'),
            feature_id,
            env.as_query()
        );

        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, feature_id, "active_version=miss (transport error)");
                return None;
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), feature_id, "active_version=miss (non-2xx)");
            return None;
        }

        match resp.json::<ActiveVersionRead>().await {
            Ok(av) => {
                let arc = Arc::new(av);
                self.cache.insert(key, arc.clone()).await;
                Some(arc)
            }
            Err(e) => {
                tracing::warn!(error = %e, feature_id, "active_version=miss (decode error)");
                None
            }
        }
    }
}

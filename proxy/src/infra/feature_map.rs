//! Feature map: `(host, path glob) -> feature_id`. Loaded once from YAML at
//! startup; `resolve` is an in-memory, sync, hot-path lookup (target < 50us).

use globset::{Glob, GlobMatcher};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureMapEntry {
    pub host: String,
    pub path_glob: String,
    pub feature_id: String,
}

#[derive(Debug)]
pub struct FeatureMap {
    entries: Vec<(GlobMatcher, FeatureMapEntry)>,
}

impl FeatureMap {
    /// Load and compile the feature map from a YAML file.
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading feature map {path}: {e}"))?;
        let parsed: Vec<FeatureMapEntry> = serde_yaml::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parsing feature map {path}: {e}"))?;
        Self::from_entries(parsed)
    }

    /// Build a feature map from already-parsed entries (used by tests).
    pub fn from_entries(parsed: Vec<FeatureMapEntry>) -> anyhow::Result<Self> {
        let mut entries = Vec::with_capacity(parsed.len());
        for entry in parsed {
            let glob = Glob::new(&entry.path_glob)
                .map_err(|e| anyhow::anyhow!("invalid path_glob {:?}: {e}", entry.path_glob))?;
            entries.push((glob.compile_matcher(), entry));
        }
        Ok(Self { entries })
    }

    /// Resolve a `(host, path)` to a feature id. First matching entry wins.
    /// Host match is exact (case-insensitive); path uses glob matching.
    pub fn resolve(&self, host: &str, path: &str) -> Option<String> {
        self.entries.iter().find_map(|(matcher, entry)| {
            if entry.host.eq_ignore_ascii_case(host) && matcher.is_match(path) {
                Some(entry.feature_id.clone())
            } else {
                None
            }
        })
    }

    /// Resolve a `(host, path)` to ALL matching feature ids, in map order,
    /// de-duplicated. A request can fan out to several features — e.g. an HTML
    /// paywall feature AND a JSON-shape feature both registered on `/**`. The
    /// forwarder applies each in order, chaining the body; each feature gates
    /// itself by response content-kind / applicability / eval, so a feature that
    /// doesn't apply is a no-op. Empty vec = no feature -> pass-through.
    pub fn resolve_all(&self, host: &str, path: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (matcher, entry) in &self.entries {
            if entry.host.eq_ignore_ascii_case(host)
                && matcher.is_match(path)
                && !out.contains(&entry.feature_id)
            {
                out.push(entry.feature_id.clone());
            }
        }
        out
    }
}

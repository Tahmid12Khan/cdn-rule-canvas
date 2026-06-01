-- 0003_versions: rre.versions + deferrable FKs on features + partial unique
-- indexes (one live / one staging per feature). BACKEND CONTRACT §3.
CREATE TABLE rre.versions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    feature_id      VARCHAR(64) NOT NULL
                       REFERENCES rre.features(id) ON DELETE CASCADE,
    version_number  INTEGER NOT NULL,
    description     TEXT NULL,
    status          rre.version_status NOT NULL DEFAULT 'draft',
    rule_graph      JSONB NOT NULL DEFAULT '{"anonymous":{"nodes":[],"edges":[],"root_node_id":null},"registered":{"nodes":[],"edges":[],"root_node_id":null},"customer":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb,
    created_by      VARCHAR(200) NOT NULL DEFAULT 'system',
    last_updated_by VARCHAR(200) NOT NULL DEFAULT 'system',
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT versions_feature_vnum_uniq UNIQUE (feature_id, version_number)
);

CREATE INDEX versions_feature_status_idx ON rre.versions (feature_id, status);

-- Wire the deferrable FK columns on features now that versions exists.
ALTER TABLE rre.features
  ADD CONSTRAINT features_staging_version_fk
      FOREIGN KEY (staging_version_id) REFERENCES rre.versions(id)
      ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
  ADD CONSTRAINT features_live_version_fk
      FOREIGN KEY (live_version_id) REFERENCES rre.versions(id)
      ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED;

-- Enforce "at most one LIVE and one STAGING per feature".
CREATE UNIQUE INDEX versions_one_live_per_feature
  ON rre.versions (feature_id) WHERE status = 'live';
CREATE UNIQUE INDEX versions_one_staging_per_feature
  ON rre.versions (feature_id) WHERE status = 'staging';

-- 0002_features: the rre.features table (BACKEND CONTRACT §3).
-- The staging/live version FK columns are created nullable here; the deferrable
-- FKs to rre.versions are wired in 0003 (versions table does not exist yet).
CREATE TABLE rre.features (
    id                 VARCHAR(64)  PRIMARY KEY,
    name               VARCHAR(200) NOT NULL,
    "type"             rre.feature_type NOT NULL,
    staging_version_id UUID NULL,
    live_version_id    UUID NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

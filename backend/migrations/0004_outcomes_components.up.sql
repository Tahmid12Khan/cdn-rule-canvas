-- 0004_outcomes_components: outcomes + components tables (BACKEND CONTRACT §3).
CREATE TABLE rre.outcomes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id  UUID NOT NULL REFERENCES rre.versions(id) ON DELETE CASCADE,
    title       VARCHAR(100) NOT NULL,
    description VARCHAR(500) NULL,
    is_builtin  BOOLEAN NOT NULL DEFAULT false,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX outcomes_version_order_idx ON rre.outcomes (version_id, order_index);

CREATE TABLE rre.components (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    outcome_id  UUID NOT NULL REFERENCES rre.outcomes(id) ON DELETE CASCADE,
    slug        VARCHAR(120) NOT NULL,
    type        VARCHAR(60)  NOT NULL,                 -- 'html_injection' | 'content_truncation'
    config      JSONB NOT NULL DEFAULT '{}'::jsonb,
    placement   rre.placement NOT NULL DEFAULT 'inline',
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX components_outcome_order_idx ON rre.components (outcome_id, order_index);

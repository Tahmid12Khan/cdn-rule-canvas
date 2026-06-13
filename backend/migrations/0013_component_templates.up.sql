-- 0013_component_templates: the Component Editor library — a GLOBAL set of
-- reusable, INDEPENDENTLY-versioned HTML (mustache) templates that hold ONLY a
-- result payload. Rules supply the control flow (where to inject/replace), pick
-- the version, and fill in variable values. Not feature/version-scoped.
--
-- `default_mode` controls how a rule's `version = "default"` resolves: `latest`
-- tracks the highest `version_number` (self-healing), `pinned` resolves to
-- `default_version_id`. The default_version FK is added AFTER the versions table
-- exists and is DEFERRABLE INITIALLY DEFERRED so the service can INSERT the
-- component then its v1 then set the pointer inside one transaction (mirrors the
-- features↔versions deferrable FK in 0003_versions).
CREATE TABLE rre.component_templates (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug               VARCHAR(120) NOT NULL UNIQUE,
    name               VARCHAR(200) NOT NULL,
    description        TEXT,
    default_mode       VARCHAR(8) NOT NULL DEFAULT 'latest',
    default_version_id UUID,                 -- FK added below (deferrable)
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT component_templates_default_mode_chk CHECK (default_mode IN ('latest', 'pinned'))
);

-- List pagination sorts by `created_at DESC, slug ASC` (mirror sites/test_presets).
CREATE INDEX component_templates_created_at_idx
    ON rre.component_templates (created_at DESC, slug ASC);

CREATE TABLE rre.component_template_versions (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    component_id   UUID NOT NULL
                       REFERENCES rre.component_templates(id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL,
    description    TEXT,
    html_body      TEXT NOT NULL DEFAULT '',
    variables      JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT component_template_versions_cid_vnum_uniq UNIQUE (component_id, version_number)
);

-- Wire the deferrable default-version FK now that the versions table exists.
ALTER TABLE rre.component_templates
    ADD CONSTRAINT component_templates_default_version_fk
        FOREIGN KEY (default_version_id) REFERENCES rre.component_template_versions(id)
        ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED;

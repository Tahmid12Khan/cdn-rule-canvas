-- 0011_test_presets: the rre.test_presets table — a GLOBAL library of reusable
-- inputs for the rule-builder Test panels (not feature/version-scoped). A
-- `rule` preset holds synthetic "Test a rule" inputs; a `url` preset holds
-- "Test with a live URL" inputs. `payload` is an opaque JSON object owned by the
-- frontend test panels (the service validates only that it is a JSON object
-- within a serialized-size cap).
CREATE TABLE rre.test_presets (
    slug       VARCHAR(64)  PRIMARY KEY,
    name       VARCHAR(200) NOT NULL UNIQUE,
    kind       VARCHAR(8)   NOT NULL,
    payload    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT test_presets_kind_known CHECK (kind IN ('rule', 'url'))
);

-- List pagination: `test_preset_repository::list_paged` sorts by
-- `created_at DESC, slug ASC` (mirror sites_created_at_idx).
CREATE INDEX test_presets_created_at_idx ON rre.test_presets (created_at DESC, slug ASC);

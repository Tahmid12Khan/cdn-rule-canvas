-- 0006_version_applicability: version-level applicability gate + JSON component
-- types (BACKEND CONTRACT §3/§5).
--
-- (a) Add the applicability JSONB column to versions. Additive, NOT NULL with a
-- DB default of the empty object so existing rows backfill to "apply whenever
-- the response content-type matches the feature type".
ALTER TABLE rre.versions
  ADD COLUMN applicability JSONB NOT NULL DEFAULT '{}'::jsonb;

-- (b) Extend the component `type` discriminator CHECK (added in 0005) to also
-- allow the JSON full-body mutation component types. Drop & recreate the named
-- constraint so the allowed set stays in one place.
ALTER TABLE rre.components DROP CONSTRAINT IF EXISTS components_type_known;
ALTER TABLE rre.components
  ADD CONSTRAINT components_type_known
  CHECK (type IN (
    'html_injection',
    'content_truncation',
    'json_remove',
    'json_set',
    'json_replace'
  ));

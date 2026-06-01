-- Reverse of 0006_version_applicability.
-- Restore the 0005 component-type CHECK (drop the JSON variants), then drop the
-- applicability column.
ALTER TABLE rre.components DROP CONSTRAINT IF EXISTS components_type_known;
ALTER TABLE rre.components
  ADD CONSTRAINT components_type_known
  CHECK (type IN ('html_injection', 'content_truncation'));

ALTER TABLE rre.versions DROP COLUMN applicability;

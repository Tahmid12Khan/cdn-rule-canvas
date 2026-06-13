-- Reverse of 0014_component_ref_component.
-- Restore the 0006 component-type CHECK (drop the two component-ref variants).
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

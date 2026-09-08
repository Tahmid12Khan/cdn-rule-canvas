-- Reverse of 0018_html_remove_component.
-- Restore the 0014 component-type CHECK (drop the html_remove variant).
ALTER TABLE rre.components DROP CONSTRAINT IF EXISTS components_type_known;
ALTER TABLE rre.components
  ADD CONSTRAINT components_type_known
  CHECK (type IN (
    'html_injection',
    'content_truncation',
    'json_remove',
    'json_set',
    'json_replace',
    'component_ref',
    'component_ref_json'
  ));

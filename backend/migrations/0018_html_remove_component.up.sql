-- 0018_html_remove_component: extend the component `type` discriminator CHECK
-- (last set in 0014) to ALSO allow `html_remove`.
--
-- `html_remove { target_selector, include_selector }` deletes matched content
-- and injects nothing — either emptying the matched element or removing the
-- element itself. HTML features only.
--
-- Drop & recreate the named constraint so the allowed set stays in one place.
-- Mirrors migration 0014 style. No column change.
ALTER TABLE rre.components DROP CONSTRAINT IF EXISTS components_type_known;
ALTER TABLE rre.components
  ADD CONSTRAINT components_type_known
  CHECK (type IN (
    'html_injection',
    'content_truncation',
    'html_remove',
    'json_remove',
    'json_set',
    'json_replace',
    'component_ref',
    'component_ref_json'
  ));

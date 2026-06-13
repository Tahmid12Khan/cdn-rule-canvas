-- 0014_component_ref_component: extend the component `type` discriminator CHECK
-- (last set in 0006) to ALSO allow the two library-Component reference types.
--
-- These let an outcome's component reference the versioned component-template
-- library (`rre.component_templates`) and populate its mustache variables:
--   * `component_ref`      — HTML features (inject the rendered template)
--   * `component_ref_json` — JSON features (set the rendered HTML string at a path)
--
-- Drop & recreate the named constraint so the allowed set stays in one place.
-- Mirrors migration 0006 style. No column change.
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

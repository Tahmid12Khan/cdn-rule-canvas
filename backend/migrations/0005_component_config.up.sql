-- 0005_component_config: DB backstop for the component `type` discriminator
-- (BACKEND CONTRACT §3). The typed `ComponentConfig` (#[serde(tag="type")]) is
-- validated server-side; this CHECK asserts the discriminator key is one of the
-- known component kinds. No column change.
ALTER TABLE rre.components
  ADD CONSTRAINT components_type_known
  CHECK (type IN ('html_injection', 'content_truncation'));

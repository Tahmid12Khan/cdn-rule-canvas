-- 0013_component_templates (reverse): drop the deferrable default-version FK on
-- the parent first (it references the versions table), then the versions table,
-- then the parent. The indexes drop with their tables.
ALTER TABLE rre.component_templates
    DROP CONSTRAINT IF EXISTS component_templates_default_version_fk;
DROP TABLE IF EXISTS rre.component_template_versions;
DROP TABLE IF EXISTS rre.component_templates;

CREATE TABLE rre.saved_outcomes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug VARCHAR(120) NOT NULL UNIQUE,
  name VARCHAR(200) NOT NULL UNIQUE,
  component_id UUID NOT NULL REFERENCES rre.component_templates(id) ON DELETE RESTRICT,
  version_number INTEGER,
  variables JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX saved_outcomes_created_at_idx ON rre.saved_outcomes (created_at DESC, slug ASC);

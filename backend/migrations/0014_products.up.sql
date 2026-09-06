CREATE TABLE rre.products (
  label VARCHAR(64) PRIMARY KEY,
  name VARCHAR(200) NOT NULL UNIQUE,
  description VARCHAR(500),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX products_created_at_idx ON rre.products (created_at DESC, label ASC);
ALTER TABLE rre.products ADD CONSTRAINT products_label_snake_case
  CHECK (label ~ '^[a-z0-9]+(_[a-z0-9]+)*$');

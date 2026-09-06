ALTER TABLE rre.versions
  ALTER COLUMN rule_graph SET DEFAULT '{"canvas":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb;

UPDATE rre.versions
  SET rule_graph = jsonb_build_object('canvas', rule_graph->'anonymous');

ALTER TABLE rre.versions
  ALTER COLUMN rule_graph SET DEFAULT
    '{"anonymous":{"nodes":[],"edges":[],"root_node_id":null},"registered":{"nodes":[],"edges":[],"root_node_id":null},"customer":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb;

UPDATE rre.versions
  SET rule_graph = jsonb_build_object(
    'anonymous', rule_graph->'canvas',
    'registered', '{"nodes":[],"edges":[],"root_node_id":null}'::jsonb,
    'customer', '{"nodes":[],"edges":[],"root_node_id":null}'::jsonb
  );

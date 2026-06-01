-- Reverse of 0007_expression_nodes (best-effort). Per canvas:
--   * expression{action:{type:"apply_outcome", outcome_id}} -> outcome node
--     {kind:"outcome", id, outcome_id, position}.
--   * drop the injected start node (id "start") and end node (id "end").
--   * drop the injected edges: id "e_start" and any id starting with "e_end_".
--   * restore root_node_id to the old root (the target of the dropped e_start
--     edge), else null.
--
-- Non-apply_outcome expression nodes (trim_json/add_attribute) have no Outcome
-- equivalent; they are dropped on the way down (best-effort, as documented).
-- rre.outcomes / rre.components are untouched.

CREATE OR REPLACE FUNCTION rre.revert_canvas_0007(canvas jsonb)
RETURNS jsonb
LANGUAGE plpgsql
AS $func$
DECLARE
  node       jsonb;
  edge       jsonb;
  new_nodes  jsonb := '[]'::jsonb;
  new_edges  jsonb := '[]'::jsonb;
  old_root   text := NULL;
  src        text;
  tgt        text;
  eid        text;
BEGIN
  IF canvas IS NULL
     OR jsonb_array_length(COALESCE(canvas->'nodes', '[]'::jsonb)) = 0 THEN
    RETURN canvas;
  END IF;

  -- Recover the old root = target of the injected e_start edge (if present).
  SELECT e->>'target_node_id'
    INTO old_root
    FROM jsonb_array_elements(COALESCE(canvas->'edges', '[]'::jsonb)) e
   WHERE e->>'id' = 'e_start'
   LIMIT 1;

  -- Nodes: drop injected start/end; apply_outcome expression -> outcome; drop
  -- other expression kinds (no Outcome equivalent).
  FOR node IN SELECT * FROM jsonb_array_elements(canvas->'nodes')
  LOOP
    IF node->>'kind' = 'start' AND node->>'id' = 'start' THEN
      CONTINUE;
    ELSIF node->>'kind' = 'end' AND node->>'id' = 'end' THEN
      CONTINUE;
    ELSIF node->>'kind' = 'expression' THEN
      IF node->'action'->>'type' = 'apply_outcome' THEN
        new_nodes := new_nodes || jsonb_build_object(
          'kind', 'outcome',
          'id', node->>'id',
          'outcome_id', node->'action'->>'outcome_id',
          'position', node->'position'
        );
      END IF;  -- non-apply_outcome expressions are dropped
    ELSE
      new_nodes := new_nodes || node;
    END IF;
  END LOOP;

  -- Edges: drop the injected e_start and every e_end_* edge.
  FOR edge IN SELECT * FROM jsonb_array_elements(COALESCE(canvas->'edges', '[]'::jsonb))
  LOOP
    eid := edge->>'id';
    IF eid = 'e_start' OR eid LIKE 'e_end_%' THEN
      CONTINUE;
    END IF;
    src := edge->>'source_node_id';
    tgt := edge->>'target_node_id';
    -- Drop edges that referenced the removed start/end nodes.
    IF src = 'start' OR tgt = 'start' OR src = 'end' OR tgt = 'end' THEN
      CONTINUE;
    END IF;
    new_edges := new_edges || edge;
  END LOOP;

  RETURN jsonb_build_object(
    'nodes', new_nodes,
    'edges', new_edges,
    'root_node_id', to_jsonb(old_root)
  );
END;
$func$;

UPDATE rre.versions
SET rule_graph = jsonb_build_object(
      'anonymous',  rre.revert_canvas_0007(rule_graph->'anonymous'),
      'registered', rre.revert_canvas_0007(rule_graph->'registered'),
      'customer',   rre.revert_canvas_0007(rule_graph->'customer')
    )
WHERE rule_graph IS NOT NULL;

DROP FUNCTION rre.revert_canvas_0007(jsonb);

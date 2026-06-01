-- 0007_expression_nodes: migrate every versions.rule_graph canvas from the old
-- Outcome-node taxonomy to the new Start/Decision/Expression/End pipeline (spec
-- §6). DDL-free DATA migration via a transient PL/pgSQL helper; idempotent.
--
-- Per canvas (anonymous/registered/customer) of every row:
--   1. Outcome -> Expression: {kind:"outcome", id, outcome_id, position}
--        becomes {kind:"expression", id, action:{type:"apply_outcome",
--        outcome_id:<uuid>}, position}.
--   2. Add END: if >=1 node and no `end` node, append {kind:"end", id:"end",
--        position}, plus one edge from EACH former-outcome (now expression)
--        node -> end (branch "yes", id "e_end_<exprId>").
--   3. Add START: if >=1 node and no `start` node, find the old root (the unique
--        pre-migration node with no incoming edge); append {kind:"start",
--        id:"start", position} + edge start -> oldRoot (branch "yes", id
--        "e_start"). Ambiguous root -> anchor START at the first node.
--   4. Set root_node_id to "start".
--
-- Idempotency: a canvas that already contains a `start` node is left untouched,
-- so re-running is a no-op. rre.outcomes / rre.components are NOT dropped.

CREATE OR REPLACE FUNCTION rre.migrate_canvas_0007(canvas jsonb)
RETURNS jsonb
LANGUAGE plpgsql
AS $func$
DECLARE
  node          jsonb;
  edge          jsonb;
  new_nodes     jsonb := '[]'::jsonb;
  new_edges     jsonb := '[]'::jsonb;
  former_outcome_ids text[] := ARRAY[]::text[];
  has_incoming  text[] := ARRAY[]::text[];
  all_ids       text[] := ARRAY[]::text[];
  old_root      text;
  candidate     text;
  oid           text;
  end_y         numeric := 0;
  start_y       numeric := 0;
BEGIN
  -- Empty canvas (no nodes) stays valid and untouched.
  IF canvas IS NULL
     OR jsonb_array_length(COALESCE(canvas->'nodes', '[]'::jsonb)) = 0 THEN
    RETURN canvas;
  END IF;

  -- Already migrated? (a `start` node present) -> no-op for idempotency.
  IF EXISTS (
    SELECT 1 FROM jsonb_array_elements(canvas->'nodes') n
    WHERE n->>'kind' = 'start'
  ) THEN
    RETURN canvas;
  END IF;

  -- Pass 1: rewrite nodes (outcome -> expression), collect ids + the
  -- former-outcome ids, and track a vertical anchor for the injected end node.
  FOR node IN SELECT * FROM jsonb_array_elements(canvas->'nodes')
  LOOP
    all_ids := all_ids || (node->>'id');
    IF node->>'kind' = 'outcome' THEN
      oid := node->>'outcome_id';
      former_outcome_ids := former_outcome_ids || (node->>'id');
      new_nodes := new_nodes || jsonb_build_object(
        'kind', 'expression',
        'id', node->>'id',
        'action', jsonb_build_object('type', 'apply_outcome', 'outcome_id', oid),
        'position', node->'position'
      );
    ELSE
      new_nodes := new_nodes || node;
    END IF;
    end_y := GREATEST(end_y, COALESCE((node->'position'->>'y')::numeric, 0));
  END LOOP;

  -- Carry edges across unchanged, and collect which node ids have an incoming
  -- edge (for old-root detection).
  FOR edge IN SELECT * FROM jsonb_array_elements(COALESCE(canvas->'edges', '[]'::jsonb))
  LOOP
    new_edges := new_edges || edge;
    has_incoming := has_incoming || (edge->>'target_node_id');
  END LOOP;

  -- Step 2: add END + edges from each former outcome -> end (only when there is
  -- no existing end node, which the start guard already implies for un-migrated
  -- canvases).
  new_nodes := new_nodes || jsonb_build_object(
    'kind', 'end',
    'id', 'end',
    'position', jsonb_build_object('x', 1000, 'y', end_y + 120)
  );
  FOREACH oid IN ARRAY former_outcome_ids
  LOOP
    new_edges := new_edges || jsonb_build_object(
      'id', 'e_end_' || oid,
      'source_node_id', oid,
      'target_node_id', 'end',
      'branch', 'yes'
    );
  END LOOP;

  -- Step 3: find the old root = unique pre-migration node id with no incoming
  -- edge. Ambiguous (zero or many) -> anchor at the first node.
  old_root := NULL;
  FOREACH candidate IN ARRAY all_ids
  LOOP
    IF NOT (candidate = ANY(has_incoming)) THEN
      IF old_root IS NULL THEN
        old_root := candidate;
      ELSE
        old_root := NULL;  -- more than one no-incoming node => ambiguous
        EXIT;
      END IF;
    END IF;
  END LOOP;
  IF old_root IS NULL THEN
    old_root := all_ids[1];
  END IF;

  -- Anchor START to the left of / above the old root's position.
  SELECT COALESCE((n->'position'->>'y')::numeric, 0)
    INTO start_y
    FROM jsonb_array_elements(new_nodes) n
   WHERE n->>'id' = old_root
   LIMIT 1;

  new_nodes := new_nodes || jsonb_build_object(
    'kind', 'start',
    'id', 'start',
    'position', jsonb_build_object('x', -120, 'y', COALESCE(start_y, 0))
  );
  new_edges := new_edges || jsonb_build_object(
    'id', 'e_start',
    'source_node_id', 'start',
    'target_node_id', old_root,
    'branch', 'yes'
  );

  -- Step 4: rebuild the canvas, anchoring root_node_id at the new start.
  RETURN jsonb_build_object(
    'nodes', new_nodes,
    'edges', new_edges,
    'root_node_id', 'start'
  );
END;
$func$;

UPDATE rre.versions
SET rule_graph = jsonb_build_object(
      'anonymous',  rre.migrate_canvas_0007(rule_graph->'anonymous'),
      'registered', rre.migrate_canvas_0007(rule_graph->'registered'),
      'customer',   rre.migrate_canvas_0007(rule_graph->'customer')
    )
WHERE rule_graph IS NOT NULL;

DROP FUNCTION rre.migrate_canvas_0007(jsonb);

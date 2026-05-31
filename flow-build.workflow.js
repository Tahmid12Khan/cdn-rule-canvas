export const meta = {
  name: 'zen-flow-editor-build',
  description: 'Add a React Flow visual JDM flowchart editor (nodes+edges, switch branching, trace highlighting) to the zen playground',
  phases: [
    { title: 'Build', detail: 'backend trace API, React Flow editor page, seed example + docs in parallel' },
    { title: 'Verify', detail: 'cargo build + static asset sanity' },
  ],
}

// ---------- seed flowchart (traffic light, uses a switch node) ----------
const SEED_JDM = {
  contentType: 'application/vnd.gorules.decision',
  nodes: [
    { id: 'in', type: 'inputNode', name: 'Request', position: { x: 40, y: 200 } },
    { id: 'sw', type: 'switchNode', name: 'Traffic Light', position: { x: 340, y: 200 }, content: {
      hitPolicy: 'first',
      statements: [
        { id: 's1', condition: "color == 'red'" },
        { id: 's2', condition: "color == 'yellow'" },
        { id: 's3', condition: '' },
      ],
    } },
    { id: 'e1', type: 'expressionNode', name: 'Stop', position: { x: 680, y: 60 }, content: { expressions: [ { id: 'x1', key: 'action', value: "'STOP'" }, { id: 'x2', key: 'canGo', value: 'false' } ] } },
    { id: 'e2', type: 'expressionNode', name: 'Slow', position: { x: 680, y: 200 }, content: { expressions: [ { id: 'x3', key: 'action', value: "'SLOW'" }, { id: 'x4', key: 'canGo', value: 'false' } ] } },
    { id: 'e3', type: 'expressionNode', name: 'Go', position: { x: 680, y: 340 }, content: { expressions: [ { id: 'x5', key: 'action', value: "'GO'" }, { id: 'x6', key: 'canGo', value: 'true' } ] } },
    { id: 'out', type: 'outputNode', name: 'Response', position: { x: 1020, y: 200 } },
  ],
  edges: [
    { id: 'ed0', type: 'edge', sourceId: 'in', targetId: 'sw' },
    { id: 'ed1', type: 'edge', sourceId: 'sw', targetId: 'e1', sourceHandle: 's1' },
    { id: 'ed2', type: 'edge', sourceId: 'sw', targetId: 'e2', sourceHandle: 's2' },
    { id: 'ed3', type: 'edge', sourceId: 'sw', targetId: 'e3', sourceHandle: 's3' },
    { id: 'ed4', type: 'edge', sourceId: 'e1', targetId: 'out' },
    { id: 'ed5', type: 'edge', sourceId: 'e2', targetId: 'out' },
    { id: 'ed6', type: 'edge', sourceId: 'e3', targetId: 'out' },
  ],
}
const SEED_CTX = { color: 'red' }
const seedJdmJson = JSON.stringify(SEED_JDM, null, 2)
const seedCtxJson = JSON.stringify(SEED_CTX, null, 2)

const API_CONTRACT = [
  'HTTP API contract (already implemented by backend after this build):',
  '  POST /api/evaluate   Content-Type: application/json',
  '    request body:  { "jdm": <JDM graph object>, "context": <input object>, "trace": true }',
  '    success: { "ok": true, "result": <output>, "performance": "<str>", "trace": { "<nodeId>": { "input":..., "output":..., "name":"...", "id":"<nodeId>", "performance":"...", "order": <int> }, ... } }',
  '    failure: { "ok": false, "error": "<message>" }',
  '  trace is present only when request trace=true. trace keys are the ids of the nodes that fired.',
  '',
  'JDM graph shape:',
  '  { "nodes": [ { "id", "type", "name", "position": {x,y}, "content"? } ], "edges": [ { "id", "type":"edge", "sourceId", "targetId", "sourceHandle"? } ] }',
  '  node types: inputNode (no content; output only), outputNode (no content; input only),',
  '    expressionNode  content: { expressions: [ { id, key, value } ] }',
  '    switchNode      content: { hitPolicy: "first"|"collect", statements: [ { id, condition } ] }  (empty condition = else)',
  '    decisionTableNode content: { hitPolicy, inputs:[{id,name,field,type:"expression"}], outputs:[{id,name,field,type:"expression"}], rules:[{ _id, "<inputId>":"<unary expr>", "<outputId>":"<expr>" }] }',
  '    functionNode    content: { source: "export const handler = async (input) => ({ ... })" }',
  '  IMPORTANT: an edge leaving a switchNode MUST set sourceHandle = the statement id it branches from.',
].join('\n')

const NEW_MAIN_RS = [
  'use std::sync::Arc;',
  '',
  'use axum::{routing::post, Json, Router};',
  'use serde::Deserialize;',
  'use serde_json::{json, Value};',
  'use tower_http::cors::CorsLayer;',
  'use tower_http::services::ServeDir;',
  'use zen_engine::model::DecisionContent;',
  'use zen_engine::{DecisionEngine, EvaluationOptions};',
  'use zen_expression::variable::Variable;',
  '',
  '#[derive(Deserialize)]',
  'struct EvalRequest {',
  '    jdm: Value,',
  '    context: Value,',
  '    #[serde(default)]',
  '    trace: bool,',
  '}',
  '',
  '// zen Variable uses Rc internally and is !Send, so the whole evaluation',
  '// (parse -> evaluate -> serialize) runs inside spawn_blocking on a',
  '// current-thread runtime. Only Send serde_json::Value crosses the boundary.',
  'async fn evaluate(Json(req): Json<EvalRequest>) -> Json<Value> {',
  '    let EvalRequest { jdm, context, trace } = req;',
  '',
  '    let out = tokio::task::spawn_blocking(move || {',
  '        let rt = tokio::runtime::Builder::new_current_thread()',
  '            .enable_all()',
  '            .build()',
  '            .expect("failed to build runtime");',
  '',
  '        rt.block_on(async move {',
  '            let content: DecisionContent = match serde_json::from_value(jdm) {',
  '                Ok(c) => c,',
  '                Err(e) => return json!({ "ok": false, "error": format!("Invalid JDM: {e}") }),',
  '            };',
  '',
  '            let engine = DecisionEngine::default();',
  '            let decision = engine.create_decision(Arc::new(content));',
  '            let ctx = Variable::from(context);',
  '            let opts = EvaluationOptions { trace, max_depth: 10 };',
  '',
  '            match decision.evaluate_with_opts(ctx, opts).await {',
  '                Ok(resp) => {',
  '                    let mut v = serde_json::to_value(&resp).unwrap_or_else(|_| json!({}));',
  '                    if let Value::Object(ref mut m) = v {',
  '                        m.insert("ok".to_string(), Value::Bool(true));',
  '                    }',
  '                    v',
  '                }',
  '                Err(e) => json!({ "ok": false, "error": e.to_string() }),',
  '            }',
  '        })',
  '    })',
  '    .await',
  '    .unwrap_or_else(|e| json!({ "ok": false, "error": format!("evaluation task failed: {e}") }));',
  '',
  '    Json(out)',
  '}',
  '',
  '#[tokio::main]',
  'async fn main() {',
  '    let static_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/static");',
  '',
  '    let app = Router::new()',
  '        .route("/api/evaluate", post(evaluate))',
  '        .fallback_service(ServeDir::new(static_dir))',
  '        .layer(CorsLayer::permissive());',
  '',
  '    let addr = "127.0.0.1:3000";',
  '    let listener = tokio::net::TcpListener::bind(addr)',
  '        .await',
  '        .expect("failed to bind");',
  '    println!("ZEN playground running -> http://{addr}");',
  '    axum::serve(listener, app).await.expect("server error");',
  '}',
  '',
].join('\n')

const MARK = '<<<<<FILE>>>>>'

phase('Build')

const backend = () => agent(
  [
    'Update the backend of the ZEN playground to support execution trace. Workspace root: /Users/tahmid/IdeaProjects/zen.',
    '',
    'Overwrite the file /Users/tahmid/IdeaProjects/zen/playground/src/main.rs with EXACTLY the content between the ' + MARK + ' markers (do not include the marker lines):',
    MARK,
    NEW_MAIN_RS,
    MARK,
    '',
    'Do NOT change any other file. Do NOT run cargo. After writing, read it back to confirm it is exact. Report in 2 lines.',
    '',
    API_CONTRACT,
  ].join('\n'),
  { label: 'backend:trace-api', phase: 'Build' }
)

const editor = () => agent(
  [
    'Create a single self-contained HTML file (one <script type="module">, no build step) that is a VISUAL FLOWCHART EDITOR for ZEN JDM decision graphs, using React + React Flow loaded from a CDN. Write it to:',
    '  /Users/tahmid/IdeaProjects/zen/playground/static/flow.html',
    '',
    'Load deps via ESM CDN, pinning versions and deduping React (use EXACTLY these specifiers):',
    "  import React, { useState, useCallback, useRef, useEffect } from 'https://esm.sh/react@18.3.1'",
    "  import { createRoot } from 'https://esm.sh/react-dom@18.3.1/client'",
    "  import ReactFlow, { Background, Controls, MiniMap, addEdge, useNodesState, useEdgesState, Handle, Position, MarkerType, ReactFlowProvider } from 'https://esm.sh/reactflow@11.11.4?deps=react@18.3.1,react-dom@18.3.1&exports=default,Background,Controls,MiniMap,addEdge,useNodesState,useEdgesState,Handle,Position,MarkerType,ReactFlowProvider'",
    "  import htm from 'https://esm.sh/htm@3.1.1'",
    'And in the <head> include the React Flow stylesheet:',
    '  <link rel="stylesheet" href="https://esm.sh/reactflow@11.11.4/dist/style.css">',
    "Bind htm to React.createElement:  const html = htm.bind(React.createElement);  Use html`...` template syntax everywhere (NO JSX, there is no compiler).",
    '',
    'LAYOUT: full viewport (100vh). Left ~70% = React Flow canvas (give the wrapper height:100vh). Right ~30% = a side panel with: (a) editor for the currently selected node, (b) an input/context JSON <textarea>, (c) an "Evaluate" button, (d) a result area. A top toolbar over the canvas with buttons: "+ Input", "+ Output", "+ Expression", "+ Switch", "+ Decision Table", "+ Function", and "Export JDM", "Import JDM", "Reset". Dark, modern theme. Also a small link back to the JSON playground: <a href="/index.html">JSON Playground</a>.',
    '',
    'CUSTOM NODE TYPES (register via nodeTypes prop). Each is a small styled box showing node.data.name and a one-line summary. Handles:',
    '  - inputNode: ONLY a source Handle (Position.Right). Greenish. summary: "Request".',
    '  - outputNode: ONLY a target Handle (Position.Left). Reddish. summary: "Response".',
    '  - expressionNode: target Handle (Left) + source Handle (Right). summary: N expressions.',
    '  - switchNode: target Handle (Left) + ONE source Handle per statement (Position.Right), each Handle id = that statement.id, vertically distributed. Show each statement condition next to its handle (empty = "else"). This is what makes branching work.',
    '  - decisionTableNode: target (Left) + source (Right). summary: "table (R rules)".',
    '  - functionNode: target (Left) + source (Right). summary: "JS".',
    '',
    'STATE: keep nodes/edges via useNodesState/useEdgesState. Each node.data holds { name, content } (content per the JDM shape below). Node ids and statement ids: generate with crypto.randomUUID(). onConnect: addEdge with markerEnd arrow; KEEP params.sourceHandle on the edge object (needed for switch branches).',
    '',
    'SIDE PANEL NODE EDITOR (depends on selected node type):',
    '  - All: a text input for name.',
    '  - expressionNode: editable list of rows [key | value] with add/remove buttons; writes node.data.content.expressions = [{id,key,value}].',
    "  - switchNode: a hitPolicy <select> (first/collect) + editable list of statement rows [condition text]; each row has its own id (keep existing ids; new rows get crypto.randomUUID()). Adding/removing a statement must add/remove the matching source handle. Empty condition = else. writes content = { hitPolicy, statements:[{id,condition}] }.",
    '  - functionNode: a code <textarea> bound to content.source.',
    '  - decisionTableNode: to keep this robust, render a <textarea> bound to JSON.stringify(content) that the user can edit (a structured grid is optional/nice-to-have; the raw-JSON editor is the required fallback). Default content for a new table = { hitPolicy:"first", inputs:[{id:uuid,name:"Input",field:"input",type:"expression"}], outputs:[{id:uuid,name:"Output",field:"output",type:"expression"}], rules:[] }.',
    '  - ALSO for EVERY node, show an "Advanced: raw content JSON" collapsible <textarea> so any content is editable as a safety net (parse on change; show inline error if invalid).',
    '',
    'JDM <-> React Flow conversion:',
    '  flowToJdm(): nodes -> { id, type: node.type, name: node.data.name, position: node.position, content: node.data.content (omit for input/output) }; edges -> { id, type:"edge", sourceId: e.source, targetId: e.target, ...(e.sourceHandle ? { sourceHandle: e.sourceHandle } : {}) }. Wrap as { contentType:"application/vnd.gorules.decision", nodes, edges }.',
    '  jdmToFlow(jdm): node -> { id, type, position, data:{ name, content } }, with the right handles; edge -> { id, source: e.sourceId, target: e.targetId, sourceHandle: e.sourceHandle, markerEnd:{type:MarkerType.ArrowClosed} }.',
    '',
    'EVALUATE: read the context textarea (JSON.parse; show clear error on failure). POST to /api/evaluate with body { jdm: flowToJdm(), context, trace: true }. Then:',
    '  - If ok:false, show data.error in red in the result area.',
    '  - If ok:true: show the final result (data.result) pretty-printed. Use data.trace (a map keyed by node id) to HIGHLIGHT: give every node whose id is a key in data.trace a glowing/active style (e.g. add a class or set node.style with a bright border + box-shadow). ANIMATE edges whose BOTH endpoints are in the trace (set edge.animated=true and a bright stroke). Reset previous highlights at the start of each evaluate.',
    '  - Clicking a highlighted node shows that node\'s trace entry (its output, order, performance) in the side panel.',
    '  - Show a subtle "evaluating..." state on the button.',
    '',
    'SEED: on first load, initialize the canvas from this exact JDM (convert with jdmToFlow), and prefill the context textarea with the seed context. Embed both verbatim as JS consts:',
    MARK,
    'const SEED_JDM = ' + seedJdmJson + ';',
    'const SEED_CONTEXT = ' + seedCtxJson + ';',
    MARK,
    '',
    'Wrap the app in <ReactFlowProvider>. Mount with createRoot. Keep it a single file, ideally under ~750 lines. Make it look polished (rounded nodes, accent color, readable side panel, monospace for code/JSON). Handle fetch errors with try/catch.',
    '',
    API_CONTRACT,
    '',
    'After writing, read the file back and confirm: it is one self-contained HTML file, the ESM imports use the exact pinned specifiers above, SEED_JDM/SEED_CONTEXT are embedded verbatim, and switch nodes render one source handle per statement with id=statement.id. Report in 4 lines what you built.',
  ].join('\n'),
  { label: 'editor:flow-html', phase: 'Build' }
)

const docs = () => agent(
  [
    'Add an example + docs for the new ZEN flowchart editor. Workspace: /Users/tahmid/IdeaProjects/zen/playground.',
    '',
    '1) Write /Users/tahmid/IdeaProjects/zen/playground/rules/flow-traffic-light.json with EXACTLY this JSON (it is a valid JDM graph using a switch node for branching):',
    MARK,
    seedJdmJson,
    MARK,
    '',
    '2) Update /Users/tahmid/IdeaProjects/zen/playground/README.md: add a section titled "Flowchart editor" that explains:',
    '   - Open http://127.0.0.1:3000/flow.html for a visual node+edge editor (React Flow).',
    '   - You drag nodes, connect ports, edit each node, then Evaluate a JSON input; the executed path is highlighted using the engine trace.',
    '   - Switch nodes branch: each statement (condition) is its own output handle; the empty condition is the else branch.',
    '   - The seed graph is a traffic light: input color -> switch -> Stop/Slow/Go -> response. Try context { "color": "red" } vs { "color": "green" }.',
    '   - Note the API now accepts an optional "trace": true and returns a per-node "trace" map.',
    '   Keep additions concise; do not rewrite unrelated sections.',
    '',
    '3) In /Users/tahmid/IdeaProjects/zen/playground/static/index.html, add a small navigation link to the flowchart editor (an <a href="/flow.html">Flowchart editor</a>) near the top header. Read the file first, then make a MINIMAL, well-targeted edit (do not restructure the page).',
    '',
    'Do NOT run cargo. After writing, confirm the JSON file parses. Report in 3 lines.',
  ].join('\n'),
  { label: 'docs:seed+readme+nav', phase: 'Build' }
)

await parallel([backend, editor, docs])

phase('Verify')

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['built', 'errorSummary', 'flowHtmlOk'],
  properties: {
    built: { type: 'boolean', description: 'true if cargo build -p zen-playground succeeded' },
    errorSummary: { type: 'string', description: 'compiler errors verbatim, else empty' },
    flowHtmlOk: { type: 'boolean', description: 'true if flow.html exists, is a single HTML file, embeds SEED_JDM, and imports reactflow@11.11.4 from esm.sh' },
    notes: { type: 'string', description: 'anything notable (e.g. a minimal fix you applied), else empty' },
  },
}

const verify = await agent(
  [
    'Verify the zen-playground still builds after the trace + flow-editor changes. Workspace: /Users/tahmid/IdeaProjects/zen.',
    '',
    '1) Run: bash -lc \'source "$HOME/.cargo/env" && cd /Users/tahmid/IdeaProjects/zen && cargo build -p zen-playground 2>&1 | tail -80\'  (allow ~3 min).',
    '   If it fails, report exact compiler errors in errorSummary. You MAY apply a MINIMAL fix only for an obvious trivial issue (wrong import/path/feature), then rebuild once; note it in notes.',
    '2) Confirm playground/static/flow.html exists, is a single self-contained HTML file, contains the string "SEED_JDM", and references "reactflow@11.11.4" and "esm.sh". Set flowHtmlOk accordingly.',
    '3) Confirm playground/rules/flow-traffic-light.json parses as valid JSON.',
    '',
    'Return the structured result.',
  ].join('\n'),
  { label: 'verify:build+assets', phase: 'Verify', schema: VERIFY_SCHEMA }
)

return { verify }

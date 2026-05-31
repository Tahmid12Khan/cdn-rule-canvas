export const meta = {
  name: 'zen-playground-build',
  description: 'Build a Rust axum + HTML UI playground for the ZEN rules engine, incl. a datetime-picker rule',
  phases: [
    { title: 'Build', detail: 'backend crate, frontend UI, example rules authored in parallel' },
    { title: 'Verify', detail: 'cargo build -p zen-playground' },
  ],
}

// ---- canonical example JDM graphs (single source of truth) ----
const TIME_GATE = {
  nodes: [
    { id: 'in', type: 'inputNode', name: 'Request', position: { x: 100, y: 120 } },
    { id: 'ex', type: 'expressionNode', name: 'Time Gate', position: { x: 380, y: 120 }, content: { expressions: [
      { id: 'a', key: 'isBefore', value: "date(pickedTime) > date('now')" },
      { id: 'b', key: 'message', value: "date(pickedTime) > date('now') ? 'Current time is BEFORE the picked time' : 'Current time is AFTER the picked time'" },
      { id: 'c', key: 'nowEpoch', value: "date('now')" },
      { id: 'd', key: 'pickedEpoch', value: 'date(pickedTime)' },
    ] } },
    { id: 'out', type: 'outputNode', name: 'Response', position: { x: 660, y: 120 } },
  ],
  edges: [
    { id: 'e1', type: 'edge', sourceId: 'in', targetId: 'ex' },
    { id: 'e2', type: 'edge', sourceId: 'ex', targetId: 'out' },
  ],
}
const TIME_GATE_CTX = { pickedTime: '2030-01-01T00:00:00Z' }

const ADULT = {
  nodes: [
    { id: 'in', type: 'inputNode', name: 'Request', position: { x: 100, y: 120 } },
    { id: 'ex', type: 'expressionNode', name: 'Adult Check', position: { x: 380, y: 120 }, content: { expressions: [
      { id: 'a', key: 'isAdult', value: 'customer.age >= 18' },
      { id: 'b', key: 'category', value: "customer.age >= 18 ? 'adult' : 'minor'" },
    ] } },
    { id: 'out', type: 'outputNode', name: 'Response', position: { x: 660, y: 120 } },
  ],
  edges: [
    { id: 'e1', type: 'edge', sourceId: 'in', targetId: 'ex' },
    { id: 'e2', type: 'edge', sourceId: 'ex', targetId: 'out' },
  ],
}
const ADULT_CTX = { customer: { age: 20 } }

const FEE = {
  nodes: [
    { id: 'in', type: 'inputNode', name: 'Request', position: { x: 100, y: 120 } },
    { id: 'tb', type: 'decisionTableNode', name: 'Shipping Fee', position: { x: 380, y: 120 }, content: {
      hitPolicy: 'first',
      inputs: [
        { id: 'i1', name: 'Country', field: 'customer.country', type: 'expression' },
        { id: 'i2', name: 'Cart total', field: 'cart.total', type: 'expression' },
      ],
      outputs: [ { id: 'o1', name: 'Fee', field: 'fee', type: 'expression' } ],
      rules: [
        { _id: 'r1', i1: "'US'", i2: '> 1000', o1: '0' },
        { _id: 'r2', i1: "'US'", i2: '', o1: '30' },
        { _id: 'r3', i1: "'CA', 'MX'", i2: '', o1: '50' },
        { _id: 'r4', i1: '', i2: '', o1: '150' },
      ],
    } },
    { id: 'out', type: 'outputNode', name: 'Response', position: { x: 660, y: 120 } },
  ],
  edges: [
    { id: 'e1', type: 'edge', sourceId: 'in', targetId: 'tb' },
    { id: 'e2', type: 'edge', sourceId: 'tb', targetId: 'out' },
  ],
}
const FEE_CTX = { customer: { country: 'US' }, cart: { total: 1500 } }

const examplesJson = JSON.stringify({
  'time-gate': { label: 'Time Gate (datetime rule)', jdm: TIME_GATE, context: TIME_GATE_CTX },
  'adult-check': { label: 'Adult Check (expression)', jdm: ADULT, context: ADULT_CTX },
  'shipping-fee': { label: 'Shipping Fee (decision table)', jdm: FEE, context: FEE_CTX },
}, null, 2)

const API_CONTRACT = [
  'HTTP API contract (backend exposes this, frontend calls it):',
  '  POST /api/evaluate   Content-Type: application/json',
  '    request body:  { "jdm": <JDM graph object>, "context": <input object> }',
  '    response (HTTP 200 always):',
  '       success: { "ok": true, "result": <evaluated output>, "performance": "<string>" }',
  '       failure: { "ok": false, "error": "<message>" }',
  '  GET  /            -> serves static/index.html (and other static files)',
  '  Server listens on 127.0.0.1:3000. Run from workspace root: cargo run -p zen-playground',
].join('\n')

const MAIN_RS = [
  'use std::sync::Arc;',
  '',
  'use axum::{routing::post, Json, Router};',
  'use serde::Deserialize;',
  'use serde_json::{json, Value};',
  'use tower_http::cors::CorsLayer;',
  'use tower_http::services::ServeDir;',
  'use zen_engine::model::DecisionContent;',
  'use zen_engine::DecisionEngine;',
  'use zen_expression::variable::Variable;',
  '',
  '#[derive(Deserialize)]',
  'struct EvalRequest {',
  '    jdm: Value,',
  '    context: Value,',
  '}',
  '',
  '// zen Variable uses Rc internally and is !Send, so the whole evaluation',
  '// (parse -> evaluate -> serialize) runs inside spawn_blocking on a',
  '// current-thread runtime. Only Send serde_json::Value crosses the boundary.',
  'async fn evaluate(Json(req): Json<EvalRequest>) -> Json<Value> {',
  '    let EvalRequest { jdm, context } = req;',
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
  '',
  '            match decision.evaluate(ctx).await {',
  '                Ok(resp) => {',
  '                    let result = serde_json::to_value(&resp.result).unwrap_or(Value::Null);',
  '                    json!({ "ok": true, "result": result, "performance": resp.performance })',
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

const CARGO_TOML = [
  '[package]',
  'name = "zen-playground"',
  'version = "0.0.0"',
  'edition = "2021"',
  'publish = false',
  '',
  '[dependencies]',
  'zen-engine = { path = "../core/engine" }',
  'zen-expression = { path = "../core/expression" }',
  'tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros", "net"] }',
  'axum = "0.7"',
  'tower-http = { version = "0.5", features = ["fs", "cors"] }',
  'serde = { version = "1", features = ["derive"] }',
  'serde_json = "1"',
  '',
].join('\n')

phase('Build')

const MARK = '<<<<<FILE>>>>>'

const backend = () => agent(
  [
    'You are creating the backend of a web playground for the ZEN business-rules engine in this Rust workspace at /Users/tahmid/IdeaProjects/zen.',
    '',
    'Do EXACTLY these file operations, no more. The exact file contents are delimited by ' + MARK + ' lines (do NOT include the marker lines themselves):',
    '',
    '1) Write /Users/tahmid/IdeaProjects/zen/playground/Cargo.toml with exactly:',
    MARK,
    CARGO_TOML,
    MARK,
    '',
    '2) Write /Users/tahmid/IdeaProjects/zen/playground/src/main.rs with exactly:',
    MARK,
    MAIN_RS,
    MARK,
    '',
    '3) Edit /Users/tahmid/IdeaProjects/zen/Cargo.toml : the [workspace] members array is currently',
    '       members = [',
    '           "core/*",',
    '           "bindings/*"',
    '       ]',
    '   Change it to add the new crate so it becomes',
    '       members = [',
    '           "core/*",',
    '           "bindings/*",',
    '           "playground"',
    '       ]',
    '   Use the Edit tool with a precise unique old_string. Do NOT change anything else in that file.',
    '',
    'Do NOT run cargo. Do NOT create any other files. After writing, read back the three files to confirm content is exact, then report what you did in 3 lines.',
    '',
    API_CONTRACT,
  ].join('\n'),
  { label: 'backend:rust-crate', phase: 'Build' }
)

const frontend = () => agent(
  [
    'You are creating the FRONTEND of a web playground for the ZEN business-rules engine. Create ONE self-contained file (no external CDN/network deps, no build step) at:',
    '  /Users/tahmid/IdeaProjects/zen/playground/static/index.html',
    '',
    'It is a single HTML file with inline <style> and <script>. Modern, clean, dark theme. Two side-by-side cards (responsive: stack on narrow screens).',
    '',
    API_CONTRACT,
    '',
    'CARD 1 — "Time Gate" (headline custom rule: is the current time before a picked datetime?):',
    '  - Heading + one-sentence explanation.',
    '  - An input element: <input type="datetime-local" id="picker">.',
    '  - A "Check" button.',
    '  - On click: read picker.value (a local datetime string like "2030-01-01T12:30"); if empty, show a hint and stop. Convert to ISO UTC with new Date(picker.value).toISOString(). Build context = { pickedTime: <iso> }.',
    '  - POST to /api/evaluate with body { jdm: EXAMPLES["time-gate"].jdm, context }.',
    '  - Render result: a big colored badge — GREEN "Current time is BEFORE the picked time" when result.result.isBefore is true, RED "Current time is AFTER the picked time" when false. Also show result.result.message. Show raw JSON result in a <pre> below. On { ok:false }, show result.error in red.',
    '',
    'CARD 2 — "JDM Playground" (author + test any rule):',
    '  - A <select id="example"> populated from EXAMPLES: one <option> per key, text = its .label.',
    '  - Two <textarea>: rule JDM JSON (id="jdm") and input/context JSON (id="context"), both monospace, reasonable height, spellcheck off.',
    '  - When an example is selected (and on initial page load), fill the two textareas with JSON.stringify(example.jdm, null, 2) and JSON.stringify(example.context, null, 2).',
    '  - An "Evaluate" button: JSON.parse both textareas (on parse error show a clear inline error), POST { jdm, context } to /api/evaluate, then pretty-print the JSON response in a <pre id="out">. If response.ok is false, highlight the error.',
    '',
    'Embed this EXACT object literal in the script as the single source of truth (Card 1 uses EXAMPLES["time-gate"].jdm). Write it verbatim as a JS const named EXAMPLES assigned to this object:',
    MARK,
    'const EXAMPLES = ' + examplesJson + ';',
    MARK,
    '',
    'Implementation notes:',
    '  - Vanilla JS only; fetch() with await inside try/catch. A shared async function evaluate(jdm, context) returning the parsed JSON.',
    '  - Show a subtle "evaluating..." state on buttons while a request is in flight.',
    '  - Single file, well under 400 lines. Tasteful: system font stack, rounded cards, an accent color, good spacing. No frameworks.',
    '',
    'After writing, read it back to confirm it is valid self-contained HTML and that EXAMPLES is embedded verbatim. Report in 3 lines what you built.',
  ].join('\n'),
  { label: 'frontend:index-html', phase: 'Build' }
)

const rules = () => agent(
  [
    'You are creating example rule files + docs for the ZEN engine web playground at /Users/tahmid/IdeaProjects/zen/playground.',
    '',
    'Create these files. JSON files must be valid standalone JDM decision graphs, pretty-printed with 2-space indent, exactly the content shown between ' + MARK + ' markers (do not include the markers):',
    '',
    '1) /Users/tahmid/IdeaProjects/zen/playground/rules/time-gate.json',
    MARK,
    JSON.stringify(TIME_GATE, null, 2),
    MARK,
    '',
    '2) /Users/tahmid/IdeaProjects/zen/playground/rules/adult-check.json',
    MARK,
    JSON.stringify(ADULT, null, 2),
    MARK,
    '',
    '3) /Users/tahmid/IdeaProjects/zen/playground/rules/shipping-fee.json',
    MARK,
    JSON.stringify(FEE, null, 2),
    MARK,
    '',
    '4) /Users/tahmid/IdeaProjects/zen/playground/README.md : a concise, practical doc that explains:',
    '   - What this is: a small web UI to author + test ZEN rules, with a Rust axum backend wrapping zen-engine.',
    '   - How to run: from the workspace root, source the cargo env then run  cargo run -p zen-playground , then open http://127.0.0.1:3000',
    "   - The Time Gate custom rule: takes a picked datetime and returns isBefore = whether the current time is before the picked time, using the ZEN expression  date(pickedTime) > date('now') .",
    '   - The three example rules in rules/ with a one-line description each.',
    '   - A curl example hitting POST /api/evaluate with the time-gate rule (jdm + context = { "pickedTime": "2030-01-01T00:00:00Z" }).',
    '   - The API shape: { "jdm": ..., "context": ... } -> { "ok": true, "result": ... }.',
    '',
    'Do NOT create other files. Do NOT run cargo. After writing, confirm the 3 JSON files parse as valid JSON. Report in 3 lines.',
  ].join('\n'),
  { label: 'rules:examples+readme', phase: 'Build' }
)

await parallel([backend, frontend, rules])

phase('Verify')

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['built', 'errorSummary', 'filesPresent'],
  properties: {
    built: { type: 'boolean', description: 'true if cargo build -p zen-playground succeeded (exit 0)' },
    errorSummary: { type: 'string', description: 'compiler errors if any, else empty string' },
    warnings: { type: 'string', description: 'notable warnings, else empty' },
    filesPresent: { type: 'string', description: 'comma-separated list of expected files that exist' },
  },
}

const verify = await agent(
  [
    'Verify the zen-playground crate builds in the workspace at /Users/tahmid/IdeaProjects/zen.',
    '',
    'Steps:',
    '1) Confirm these files exist: playground/Cargo.toml, playground/src/main.rs, playground/static/index.html, playground/rules/time-gate.json, playground/README.md. Also confirm the root Cargo.toml [workspace] members now includes "playground".',
    "2) Run: bash -lc 'source \"$HOME/.cargo/env\" && cd /Users/tahmid/IdeaProjects/zen && cargo build -p zen-playground 2>&1 | tail -80'  (first build downloads axum/tower-http; allow up to ~6 minutes).",
    '3) If it fails to compile, report the exact compiler errors verbatim in errorSummary. Do NOT do large rewrites. You MAY make a MINIMAL fix ONLY for an obvious trivial issue (e.g. wrong import path such as zen_expression::variable::Variable vs zen_expression::Variable, or a missing tokio feature), then rebuild once. Keep any edit tiny and note it in warnings.',
    '',
    'Return the structured result.',
  ].join('\n'),
  { label: 'verify:cargo-build', phase: 'Verify', schema: VERIFY_SCHEMA }
)

return { verify, examples: ['time-gate', 'adult-check', 'shipping-fee'] }

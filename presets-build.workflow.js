export const meta = {
  name: 'zen-flow-presets',
  description: 'Add an all-node-types example + a palette of pre-configured preset nodes to the ZEN flow editor',
  phases: [
    { title: 'Build', detail: 'editor (examples dropdown + presets palette) and example/docs in parallel' },
    { title: 'Verify', detail: 'static asset sanity checks' },
  ],
}

// ---- verified all-node-types graph (input->function->expression->switch->decisionTable/expression->output) ----
const KITCHEN_JDM = {
  contentType: 'application/vnd.gorules.decision',
  nodes: [
    { id: 'req', type: 'inputNode', name: 'Request', position: { x: 40, y: 280 } },
    { id: 'fn', type: 'functionNode', name: 'Enrich (JS)', position: { x: 280, y: 280 }, content: { source: "export const handler = async (input) => ({ ...input, browser: (input.userAgent || '').includes('Firefox') ? 'firefox' : (input.userAgent || '').includes('Chrome') ? 'chrome' : 'other', isMobile: (input.userAgent || '').includes('Mobile') });" } },
    { id: 'sig', type: 'expressionNode', name: 'Signals', position: { x: 540, y: 280 }, content: { expressions: [
      { id: 's1', key: 'browser', value: 'browser' },
      { id: 's2', key: 'isMobile', value: 'isMobile' },
      { id: 's3', key: 'total', value: 'cart.total' },
      { id: 's4', key: 'isCheckout', value: "contains(url, 'checkout')" },
      { id: 's5', key: 'isToday', value: "year(date(requestedAt))==year(date('now')) and monthOfYear(date(requestedAt))==monthOfYear(date('now')) and dayOfMonth(date(requestedAt))==dayOfMonth(date('now'))" },
    ] } },
    { id: 'sw', type: 'switchNode', name: 'Route by browser', position: { x: 820, y: 280 }, content: { hitPolicy: 'first', statements: [
      { id: 'c_chrome', condition: "browser == 'chrome'" },
      { id: 'c_firefox', condition: "browser == 'firefox'" },
      { id: 'c_else', condition: '' },
    ] } },
    { id: 'tbl', type: 'decisionTableNode', name: 'Chrome pricing', position: { x: 1120, y: 120 }, content: {
      hitPolicy: 'first',
      inputs: [ { id: 'i1', name: 'Checkout', field: 'isCheckout', type: 'expression' }, { id: 'i2', name: 'Total', field: 'total', type: 'expression' } ],
      outputs: [ { id: 'o1', name: 'Channel', field: 'channel', type: 'expression' }, { id: 'o2', name: 'Discount', field: 'discount', type: 'expression' } ],
      rules: [
        { _id: 'r1', i1: 'true', i2: '> 1000', o1: "'chrome-vip'", o2: '15' },
        { _id: 'r2', i1: 'true', i2: '', o1: "'chrome-checkout'", o2: '10' },
        { _id: 'r3', i1: '', i2: '', o1: "'chrome'", o2: '0' },
      ],
    } },
    { id: 'ff', type: 'expressionNode', name: 'Firefox tag', position: { x: 1120, y: 300 }, content: { expressions: [ { id: 'f1', key: 'channel', value: "'firefox'" }, { id: 'f2', key: 'discount', value: '5' } ] } },
    { id: 'other', type: 'expressionNode', name: 'Other tag', position: { x: 1120, y: 460 }, content: { expressions: [ { id: 'ot1', key: 'channel', value: "'other'" }, { id: 'ot2', key: 'discount', value: '0' } ] } },
    { id: 'res', type: 'outputNode', name: 'Response', position: { x: 1420, y: 280 } },
  ],
  edges: [
    { id: 'e_req_fn', type: 'edge', sourceId: 'req', targetId: 'fn' },
    { id: 'e_fn_sig', type: 'edge', sourceId: 'fn', targetId: 'sig' },
    { id: 'e_sig_sw', type: 'edge', sourceId: 'sig', targetId: 'sw' },
    { id: 'e_sw_tbl', type: 'edge', sourceId: 'sw', targetId: 'tbl', sourceHandle: 'c_chrome' },
    { id: 'e_sw_ff', type: 'edge', sourceId: 'sw', targetId: 'ff', sourceHandle: 'c_firefox' },
    { id: 'e_sw_other', type: 'edge', sourceId: 'sw', targetId: 'other', sourceHandle: 'c_else' },
    { id: 'e_tbl_res', type: 'edge', sourceId: 'tbl', targetId: 'res' },
    { id: 'e_ff_res', type: 'edge', sourceId: 'ff', targetId: 'res' },
    { id: 'e_other_res', type: 'edge', sourceId: 'other', targetId: 'res' },
  ],
}
const KITCHEN_CTX = { userAgent: 'Mozilla/5.0 (Macintosh) Chrome/120 Safari/537', url: 'https://shop.example.com/checkout', cart: { total: 1500 }, requestedAt: '2026-05-29T08:00:00Z' }

// ---- preset nodes (every expression VERIFIED against the engine) ----
// Each preset inserts one node. Field names (timestamp/userAgent/url/cart.total/amount) are placeholders the user edits.
const PRESETS = [
  { group: 'Date/Time', label: 'beforeNow', type: 'expressionNode', name: 'beforeNow', note: 'input field: timestamp', content: { expressions: [ { id: 'p1', key: 'beforeNow', value: "date(timestamp) < date('now')" } ] } },
  { group: 'Date/Time', label: 'afterNow', type: 'expressionNode', name: 'afterNow', note: 'input field: timestamp', content: { expressions: [ { id: 'p1', key: 'afterNow', value: "date(timestamp) > date('now')" } ] } },
  { group: 'Date/Time', label: 'isToday', type: 'expressionNode', name: 'isToday', note: 'input field: timestamp (UTC day match)', content: { expressions: [ { id: 'p1', key: 'isToday', value: "year(date(timestamp))==year(date('now')) and monthOfYear(date(timestamp))==monthOfYear(date('now')) and dayOfMonth(date(timestamp))==dayOfMonth(date('now'))" } ] } },
  { group: 'Date/Time', label: 'isWeekend', type: 'expressionNode', name: 'isWeekend', note: 'input field: timestamp (Sat/Sun)', content: { expressions: [ { id: 'p1', key: 'isWeekend', value: 'dayOfWeek(date(timestamp)) > 5' } ] } },
  { group: 'Date/Time', label: 'expiresWithin7d', type: 'expressionNode', name: 'expiresWithin7d', note: 'input field: expiresAt', content: { expressions: [ { id: 'p1', key: 'expiringSoon', value: "date(expiresAt) > date('now') and date(expiresAt) < date('now') + duration('7d')" } ] } },

  { group: 'Browser', label: 'isChrome', type: 'expressionNode', name: 'isChrome', note: 'input field: userAgent', content: { expressions: [ { id: 'p1', key: 'isChrome', value: "contains(userAgent, 'Chrome') and not contains(userAgent, 'Edg')" } ] } },
  { group: 'Browser', label: 'isFirefox', type: 'expressionNode', name: 'isFirefox', note: 'input field: userAgent', content: { expressions: [ { id: 'p1', key: 'isFirefox', value: "contains(userAgent, 'Firefox')" } ] } },
  { group: 'Browser', label: 'isSafari', type: 'expressionNode', name: 'isSafari', note: 'input field: userAgent', content: { expressions: [ { id: 'p1', key: 'isSafari', value: "contains(userAgent, 'Safari') and not contains(userAgent, 'Chrome')" } ] } },
  { group: 'Browser', label: 'isEdge', type: 'expressionNode', name: 'isEdge', note: 'input field: userAgent', content: { expressions: [ { id: 'p1', key: 'isEdge', value: "contains(userAgent, 'Edg')" } ] } },
  { group: 'Browser', label: 'isMobile', type: 'expressionNode', name: 'isMobile', note: 'input field: userAgent', content: { expressions: [ { id: 'p1', key: 'isMobile', value: "contains(userAgent, 'Mobile')" } ] } },
  { group: 'Browser', label: 'Browser switch', type: 'switchNode', name: 'Browser', note: 'branches: chrome / firefox / safari / else (input field: userAgent)', content: { hitPolicy: 'first', statements: [ { id: 'b_chrome', condition: "contains(userAgent, 'Chrome') and not contains(userAgent, 'Edg')" }, { id: 'b_firefox', condition: "contains(userAgent, 'Firefox')" }, { id: 'b_safari', condition: "contains(userAgent, 'Safari') and not contains(userAgent, 'Chrome')" }, { id: 'b_else', condition: '' } ] } },

  { group: 'URL', label: 'url_contains', type: 'expressionNode', name: 'urlContains', note: "input field: url (edit 'checkout')", content: { expressions: [ { id: 'p1', key: 'urlContains', value: "contains(url, 'checkout')" } ] } },
  { group: 'URL', label: 'url_startsWith https', type: 'expressionNode', name: 'urlHttps', note: 'input field: url', content: { expressions: [ { id: 'p1', key: 'isHttps', value: "startsWith(url, 'https')" } ] } },
  { group: 'URL', label: 'url_matches (regex)', type: 'expressionNode', name: 'urlMatches', note: 'input field: url (regex)', content: { expressions: [ { id: 'p1', key: 'urlMatches', value: "matches(url, '^https://[^/]+\\\\.example\\\\.com')" } ] } },

  { group: 'Number', label: 'isHighValue', type: 'expressionNode', name: 'isHighValue', note: 'input field: cart.total', content: { expressions: [ { id: 'p1', key: 'isHighValue', value: 'cart.total > 1000' } ] } },
  { group: 'Number', label: 'inRange', type: 'expressionNode', name: 'inRange', note: 'input field: amount', content: { expressions: [ { id: 'p1', key: 'inRange', value: 'amount >= 100 and amount <= 500' } ] } },
]

const kitchenJdmJson = JSON.stringify(KITCHEN_JDM, null, 2)
const kitchenCtxJson = JSON.stringify(KITCHEN_CTX, null, 2)
const presetsJson = JSON.stringify(PRESETS, null, 2)

const MARK = '<<<<<BLOCK>>>>>'

phase('Build')

const editor = () => agent(
  [
    'Modify the existing single-file flowchart editor at /Users/tahmid/IdeaProjects/zen/playground/static/flow.html. READ IT FIRST to learn its structure (it already uses React + React Flow via esm.sh, htm template literals, SEED_JDM/SEED_CONTEXT consts, jdmToFlow/flowToJdm, useNodesState/useEdgesState, a toolbar with add-node buttons, and a side panel). PRESERVE all existing behaviour and styling. Make surgical additions only.',
    '',
    'ADD TWO THINGS:',
    '',
    'A) An "Examples" picker in the toolbar. Embed (near the existing SEED_JDM const) these new consts verbatim:',
    MARK,
    'const KITCHEN_JDM = ' + kitchenJdmJson + ';',
    'const KITCHEN_CONTEXT = ' + kitchenCtxJson + ';',
    'const EXAMPLES = {',
    "  'traffic-light': { label: 'Traffic Light (switch)', jdm: SEED_JDM, context: SEED_CONTEXT },",
    "  'kitchen-sink': { label: 'Kitchen Sink (all node types)', jdm: KITCHEN_JDM, context: KITCHEN_CONTEXT },",
    '};',
    MARK,
    '   Add a <select> labelled "Example" to the toolbar listing the EXAMPLES (option value = key, text = label). On change: load the chosen example by setting nodes/edges via jdmToFlow(example.jdm) and setting the context textarea to JSON.stringify(example.context, null, 2); also clear any trace/highlight/selection/result state (reuse the same resets the existing reset() / importJdm() use). Keep the existing seed-on-first-load behaviour (traffic-light) OR initialise from EXAMPLES["traffic-light"] equivalently.',
    '',
    'B) A "Presets" picker in the toolbar that INSERTS a pre-configured node onto the canvas. Embed this const verbatim (near the others):',
    MARK,
    'const PRESETS = ' + presetsJson + ';',
    MARK,
    '   Render a <select> (or grouped dropdown using <optgroup> by preset.group) whose options are the PRESETS (use the index or label as value). When the user picks one, insert ONE new React Flow node and reset the picker to its placeholder:',
    '     - id: crypto.randomUUID()',
    '     - type: preset.type',
    '     - position: place it somewhere visible (e.g. { x: 80 + (existing node count % 5)*40, y: 80 + (existing node count % 5)*40 }, or near the viewport centre).',
    '     - data: { name: preset.name, content: a DEEP CLONE of preset.content }. For switchNode presets, REGENERATE each statement id with crypto.randomUUID() in the clone (so handles are unique) — keep the conditions.',
    '   Append it via setNodes((ns) => ns.concat(newNode)) and select it (set the selected node id) so the side panel opens for editing. The user then wires it by dragging from another node\'s handle. Show the preset.note somewhere subtle (e.g. as the <option> title attribute or a small hint when selected) so the user knows which input field the expression expects.',
    '',
    'Do not break the switch-handle-per-statement rendering, the trace highlighting, or export/import. Keep it one self-contained file. After editing, READ the file back and confirm: EXAMPLES, KITCHEN_JDM, PRESETS are embedded; the toolbar has an Example select and a Presets select; existing imports/structure are intact. Report in 4 lines what you changed.',
  ].join('\n'),
  { label: 'editor:examples+presets', phase: 'Build' }
)

const docs = () => agent(
  [
    'Add an example file and docs for the ZEN flow editor. Workspace: /Users/tahmid/IdeaProjects/zen/playground.',
    '',
    '1) Write /Users/tahmid/IdeaProjects/zen/playground/rules/kitchen-sink.json with EXACTLY this JSON (a valid JDM graph that uses every node type: input, function, expression, switch, decisionTable, output). Content between ' + MARK + ' markers:',
    MARK,
    kitchenJdmJson,
    MARK,
    '',
    '2) Update /Users/tahmid/IdeaProjects/zen/playground/README.md: add a section "All node types example" describing the kitchen-sink graph (Request -> Enrich function (JS) -> Signals expression -> Route-by-browser switch -> Chrome decision table / Firefox+Other expressions -> Response) and that selecting "Kitchen Sink (all node types)" in the editor\'s Example dropdown loads it. Try userAgent with Chrome vs Firefox vs Safari, url containing "checkout", and cart.total > 1000.',
    '',
    '3) Also add a "Preset nodes" subsection to the README listing the available presets and the exact (verified) ZEN expression each uses, grouped. Use this exact list:',
    '   Date/Time:',
    "     - beforeNow:    date(timestamp) < date('now')",
    "     - afterNow:     date(timestamp) > date('now')",
    "     - isToday:      year(date(timestamp))==year(date('now')) and monthOfYear(...)==monthOfYear(date('now')) and dayOfMonth(...)==dayOfMonth(date('now'))",
    '     - isWeekend:    dayOfWeek(date(timestamp)) > 5',
    "     - expiresWithin7d: date(expiresAt) > date('now') and date(expiresAt) < date('now') + duration('7d')",
    '   Browser (field: userAgent):',
    "     - isChrome:  contains(userAgent,'Chrome') and not contains(userAgent,'Edg')",
    "     - isFirefox: contains(userAgent,'Firefox')",
    "     - isSafari:  contains(userAgent,'Safari') and not contains(userAgent,'Chrome')",
    "     - isEdge:    contains(userAgent,'Edg')",
    "     - isMobile:  contains(userAgent,'Mobile')",
    '     - Browser switch: branches chrome / firefox / safari / else',
    '   URL (field: url):',
    "     - url_contains:     contains(url,'checkout')",
    "     - url_startsWith:   startsWith(url,'https')",
    "     - url_matches:      matches(url,'^https://[^/]+\\.example\\.com')",
    '   Number:',
    '     - isHighValue: cart.total > 1000',
    '     - inRange:     amount >= 100 and amount <= 500',
    '   Note in the README that date functions (date/year/monthOfYear/dayOfMonth/dayOfWeek/duration) are FUNCTIONS, not methods (e.g. date(x).isToday() does NOT work in expression nodes; use the function form shown).',
    '',
    'Do NOT run cargo. After writing, confirm kitchen-sink.json parses as valid JSON. Report in 3 lines.',
  ].join('\n'),
  { label: 'docs:kitchen+presets-readme', phase: 'Build' }
)

await parallel([editor, docs])

phase('Verify')

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['flowHtmlOk', 'kitchenJsonOk', 'notes'],
  properties: {
    flowHtmlOk: { type: 'boolean', description: 'true if flow.html still one HTML file and now contains PRESETS, EXAMPLES, KITCHEN_JDM, an Example <select> and a Presets <select>, and still imports reactflow@11.11.4' },
    kitchenJsonOk: { type: 'boolean', description: 'true if playground/rules/kitchen-sink.json parses as valid JSON and has 8 nodes' },
    notes: { type: 'string', description: 'anything notable, else empty' },
  },
}

const verify = await agent(
  [
    'Verify the ZEN flow editor preset additions. Workspace: /Users/tahmid/IdeaProjects/zen.',
    '1) Check playground/static/flow.html is still a single self-contained HTML file, still references "reactflow@11.11.4", and now contains the strings "PRESETS", "EXAMPLES", "KITCHEN_JDM", plus a toolbar <select> for examples and one for presets. Set flowHtmlOk.',
    '2) Check playground/rules/kitchen-sink.json parses as valid JSON and has exactly 8 nodes. Set kitchenJsonOk.',
    'Do NOT run cargo (no Rust changed). Return the structured result.',
  ].join('\n'),
  { label: 'verify:assets', phase: 'Verify', schema: VERIFY_SCHEMA }
)

return { verify }

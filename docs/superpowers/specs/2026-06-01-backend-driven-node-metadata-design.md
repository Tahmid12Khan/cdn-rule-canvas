# Backend-driven node metadata, JSON config, and node visuals — Design

**Date:** 2026-06-01
**Status:** Approved (design); pending spec review
**Scope:** RRE app (`backend/`, `proxy/`, `frontend/`). Does not touch upstream zen crates.

## Goal

1. Each canvas node shows its **type label** at the top (e.g. `article_url` → "Article URL"), truncated to a fixed width with `…` when long.
2. **Hover** a node → tooltip showing the node label, every field (operator, value, …) with its current value, an input summary, and the output branches (with the node label).
3. **Backend declares node metadata; the frontend just renders it.** Adding a brand-new node type requires **zero frontend changes** and **zero backend Rust changes** — only a JSON manifest entry on the backend plus a proxy `CanvasProcessor` impl (the eval logic, which genuinely cannot be data).
4. **Config is read from layered JSON files** (Spring-Boot/FastAPI-style), with idiomatic Rust dependency injection, for both Rust services.

## Decisions (locked)

- **Frontend genericity:** fully data-driven — palette, node title, tooltip, AND config form all render from the manifest.
- **"Node name":** derived from node type (the manifest `label`). No per-node user-editable name.
- **Config system:** layered JSON via the `config` crate, applied to **both** `backend` and `proxy`. Env vars remain the top override layer for secrets.
- **Manifest ownership:** a backend-owned JSON file is the source of truth; backend serves it at `GET /api/v1/node-types`. Proxy keeps its Rust processors (does **not** read the manifest file).
- **Backend validation:** manifest-driven (the typed `ProcessorConfig` enum is removed). New node = JSON entry only on the backend.
- **Build orchestration:** spec → `writing-plans` → a Workflow that fans out backend/proxy/frontend engineers in parallel (worktree isolation), then adversarial review + a JSON-only-new-node verification gate.

## Canonical node identifier

Today three identifiers exist for one concept: the canvas `type` (snake_case: `meta_tags`), the proxy registry/JDM kind (camelCase: `metaTags`), bridged by `ProcessorConfig::kind_key()`.

**This design collapses them into one snake_case identifier** used uniformly: manifest `kind` == rule_graph processor `type` == proxy `CanvasProcessor::kind()` == JDM `CustomNode` kind. This removes `kind_key()` and the camelCase/snake_case split. JDM kind strings live only in memory (built per request), never persisted, so the change is internal and safe. Existing rule_graph JSONB already stores snake_case `type`; no data migration needed.

---

## Part A — Layered JSON config + DI

### What changes

Replace `envy::from_env::<Settings>()` + `dotenvy` with the **`config` crate** in both `backend/src/config.rs` and `proxy/src/config.rs`.

Layering (highest wins):
```
config/default.json                 # committed base values
config/{APP_ENV}.json               # dev | staging | prod profile, selected by APP_ENV (default "dev")
environment variables               # top layer — secrets (DATABASE_URL, ...) and per-deploy overrides
```

```rust
// sketch
pub fn load() -> anyhow::Result<Settings> {
    dotenvy::dotenv().ok();
    let env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".into());
    let s = config::Config::builder()
        .add_source(config::File::with_name("config/default"))
        .add_source(config::File::with_name(&format!("config/{env}")).required(false))
        .add_source(config::Environment::default().separator("__"))
        .build()?;
    Ok(s.try_deserialize()?)
}
```

- The `Settings` field set is unchanged in meaning; values move from `.env`/defaults into `config/default.json` + profile files. `DATABASE_URL` and other secrets stay env-only (not committed to JSON).
- Both crates are standalone; each gets its own `config/` directory. Config file paths are resolved relative to the working directory used in compose/dev (same convention as the proxy's existing YAML paths).
- `.env.example` files updated; docker-compose env wiring reviewed so the layered files are present in the image (config dirs copied in each Dockerfile).

### DI

Dependency injection stays the idiomatic Rust pattern already in use: a cloneable `AppState` holding `Arc<Settings>` (and the new `Arc<NodeManifest>`), injected into handlers via `State<AppState>` / the proxy's equivalent. No DI container is introduced — that would be un-idiomatic in Rust and adds nothing over `AppState`.

### Contract impact

`CONTRACTS.md §0` currently mandates `envy::from_env` + `dotenvy`. Update it to describe the layered-JSON `config`-crate approach, since the contract is authoritative and wins on conflicts.

---

## Part B — Node-type manifest

### File: `backend/config/node_types.json`

An ordered list of node-type specs plus palette categories. One spec per type:

```jsonc
{
  "kind": "article_url",                 // canonical snake_case identifier (see above)
  "label": "Article URL",                // title shown at top of the node
  "category": "content",                 // palette category id
  "summary": "Matches against the request article URL (path).",  // tooltip "input info"
  "fields": [
    { "name": "operator", "label": "Operator", "control": "select", "required": true,
      "default": "contains",
      "options": [
        { "value": "contains",    "label": "Contains" },
        { "value": "matches",     "label": "Matches (regex)" },
        { "value": "starts_with", "label": "Starts with" },
        { "value": "equals",      "label": "Equals" }
      ] },
    { "name": "value", "label": "Value", "control": "text", "required": true,
      "placeholder": "Enter a value to compare against the URL" }
  ],
  "output": { "branches": [ { "id": "yes", "label": "Yes" }, { "id": "no", "label": "No" } ] }
}
```

Field controls supported for MVP: `select` (with `options`), `text`, `number`.

Conditional requirement (covers meta_tags' "value required unless `exists`"):
```jsonc
{ "name": "value", "control": "text",
  "requiredUnless": { "field": "operator", "value": "exists" } }
```

Palette categories (including the disabled "Coming soon" ones) are also declared in the manifest, so even those are backend-controlled:
```jsonc
"categories": [
  { "id": "content",  "label": "Content" },
  { "id": "session",  "label": "Session" },
  { "id": "user",     "label": "User",  "comingSoon": true },
  ...
]
```

The three existing types (`meta_tags`, `device_type`, `article_url`) are ported verbatim from the current `processorSchemas.ts` + `nodeTemplates.ts` so behavior is identical.

### Loading + serving (backend)

- `NodeManifest` struct deserialized once at startup from `node_types.json`, wrapped `Arc<NodeManifest>`, added to `AppState`.
- `GET /api/v1/node-types` returns the manifest (utoipa-documented, in the uniform response style). Cheap, cacheable; frontend fetches via TanStack Query.

---

## Part C — Manifest-driven backend validation

### What changes in `backend/src/schemas/rule_graph.rs`

- Remove the `ProcessorConfig` enum and the per-type operator/value enums.
- A decision node's `processor` becomes a generic shape: a `type: String` discriminator plus the remaining fields captured as `serde_json::Value` (a map). It still serializes back to the same JSON the frontend/proxy exchange (`{ "type": "...", "operator": "...", "value": "..." }`).

### What changes in `backend/src/services/rule_graph_service.rs`

Processor validation becomes manifest-driven, run per decision node, emitting the same `{loc, msg, rule_id}` 422 details:

- `type` must be a known manifest `kind` → else detail (`rule_id: "processor_kind_known"`).
- For each manifest `field` with `required` (or `requiredUnless` unsatisfied): value must be present/non-empty (`rule_id: "processor_field_required"`).
- For `select` fields: value must be one of `options[].value` (`rule_id: "processor_field_option"`).
- Unknown extra fields: ignored (forward-compatible) — decision recorded in spec, not an error.

The structural rules (edges, outcome refs, root, etc.) are unchanged. The manifest (`Arc<NodeManifest>`) is threaded into `validate(...)` (currently a free function — it gains a `&NodeManifest` parameter, supplied by the service from `AppState`).

### Contract impact

Update `CONTRACTS.md §6` to describe processor config as a manifest-validated generic shape instead of the fixed enum. `§8.3`/zen-integration notes about `kind` are updated to the single snake_case identifier.

---

## Part D — Proxy stays minimal

### Generic processor reference in `proxy/src/domain/graph.rs`

- Replace the proxy's `ProcessorConfig` enum (+ `kind_key()` / `to_config_value()`) with a generic reference: `{ type: String (the kind), config: serde_json::Value }`. Deserializes any node type, including ones added later, without a graph.rs edit.

### Translator (`proxy/src/domain/translator.rs`)

- Use the processor's `type` **directly** as the JDM `CustomNode` kind (no `kind_key()` mapping) and pass the config map through unchanged. Processors already read config dynamically via `config.get(...)`.

### Processor registry

- `CanvasProcessor::kind()` returns the canonical snake_case identifier (e.g. `"article_url"`). Existing three processors updated from camelCase to snake_case kinds. Registry keys on `kind()` as today.

### Net: adding a new node type

| Layer | Work for a new node type |
|---|---|
| Frontend | **none** |
| Backend | **one entry** in `node_types.json` (no Rust) |
| Proxy | one `CanvasProcessor` impl + one `register()` line (eval logic — unavoidable) |

---

## Part E — Frontend, fully data-driven

### Data access

- `useNodeTypes()` — TanStack Query hook fetching `GET /api/v1/node-types`; cached for the session. A small client-side index (`kind → spec`) for O(1) lookup in nodes/forms.

### Node rendering — `frontend/src/components/canvas/nodes/DecisionNode.tsx`

- **Title at top** = manifest `label` for the node's `type` (so `article_url` → "Article URL"). Truncated to ~16 chars via CSS `truncate` (`max-w`) and `…`; full text available on hover.
- Diamond shape, handles, error/test-highlight behavior unchanged. The hardcoded `processorTitle()` / `processorSubLabel()` switch statements are removed; their text now derives from the manifest spec + current config.

### Hover tooltip

- On node hover, show: node `label`; each manifest field (label → current value) e.g. "Operator: Contains", "Value: /news/"; the `summary` (input info); and output branches ("Yes" / "No") under the node label.
- Implementation: a styled tooltip (existing UI tooltip primitive if present, else a lightweight positioned popover). Must not interfere with React Flow drag/handle interactions.

### Palette — `frontend/src/lib/canvas/nodeTemplates.ts` + palette components

- Categories and chips generated from the manifest (`categories` + each spec's `category`/`label`). Disabled "Coming soon" chips come from categories flagged `comingSoon`.
- Default dropped-config built from each field's `default` (missing → empty), so a freshly-dropped node still nudges the user to open the config drawer (same UX as today).
- Outcome chips remain dynamic (per current version's outcomes) — unchanged.
- The hardcoded `STATIC_CATEGORIES` decision chips and `DEFAULT_*` consts are removed.

### Config form — `frontend/src/components/canvas/config/NodeConfigDrawer.tsx` (+ a new generic form renderer)

- A generic renderer builds controls from `fields[]`: `select` → dropdown of `options`, `text` → input, `number` → number input.
- Validation derived from the manifest: `required`, `requiredUnless`, and `select` `options` membership — producing the same inline messages the hardcoded Zod schemas did (messages carried in the manifest where they were user-facing, e.g. placeholders/help).
- The hardcoded `processorSchemas.ts` Zod unions and per-type form components are removed; serialize/deserialize (`lib/canvas/serialize.ts` / `deserialize.ts`) continue to round-trip the generic `{ type, ...fields }` shape (largely unchanged, since the JSON shape is preserved).

### Type changes — `frontend/src/lib/canvas/types.ts`

- `ProcessorConfig` becomes a generic `{ type: string; [field: string]: unknown }` shape; the per-type unions/operator literal types are removed (the manifest is the runtime contract now).

---

## Testing & verification

- **Per-service gates:** `make backend-check`, `make proxy-check`, `make frontend-check` all green (clippy/fmt/cargo test, lint/typecheck/vitest).
- **Backend:** unit tests for manifest loading + manifest-driven validation (known/unknown kind, required, requiredUnless, option membership) replacing the enum-based tests.
- **Proxy:** translator + processor tests updated to the snake_case kinds; existing `dn-article` evaluation regression — anonymous→paywall, mobile→regwall — unchanged.
- **Frontend:** vitest for the generic form renderer (renders select/text, enforces required/requiredUnless), palette built from a manifest fixture, node title truncation, tooltip content. Playwright canvas round-trip still passes.
- **Headline acceptance — JSON-only new node:** add a test-only node type to a manifest fixture (no frontend/backend-Rust edit) and assert it appears in the palette, renders its label, validates, and (with a trivial test processor registered in the proxy) evaluates. Proves "frontend needs no change; backend change is enough."
- **E2E demo unchanged:** `make up` → proxy still applies rules at `:9000/article.html`.

## Out of scope

- No new node *types* beyond porting the existing three.
- Proxy `feature_map.yaml` / `sanitizer.yaml` stay YAML (separate config concern).
- No per-node custom naming UI.
- No DI framework/container.

## Rollout notes

- `CONTRACTS.md` §0 (config) and §6 (processor config) updated as part of the work — the contract is authoritative, so it must move with the code.
- Demo seed (`seed_demo`) and `feature_map.yaml` continue to reference `dn-article`; no data migration (rule_graph JSONB already snake_case `type`).

# Component Editor — Design Spec

> Status: DRAFT for review · 2026-06-07
> Source: `spec.md` (8 requirements) + clarifying answers captured during brainstorming.
> Authoritative engineering contract: `CONTRACTS.md` (this spec extends it; on any conflict CONTRACTS wins and must be updated in lock-step).

## 1. Summary

Add a **Component Editor** — a global library of reusable, **independently-versioned** HTML templates (mustache) that hold *only* a result payload. Rules supply the control flow (where to inject/replace), pick which version, and fill in the template's variable values. The rendered result is produced at request time by the proxy, so a component referenced as "default" auto-updates live output without republishing the feature.

This is **additive**. The existing version-scoped outcome/component system (`html_injection`, `apply_outcome`, …) is untouched and keeps working.

### Mapping to the 8 spec requirements

| spec | covered by |
|---|---|
| 1 — new section "Component Editor" | new top-level `/products/components` library + editor |
| 2 — versioned | `component_template_versions` (own version numbers + descriptions) |
| 3 — result only; rules provide control flow | component = `html_body` only; `target_selector`/`placement_mode`/`target_path` live on the rule node |
| 4 — pick version; default; default auto-updates | rule `version = "default" | N`; component has a movable default (track-latest or pinned) |
| 5 — any valid HTML + mustache; split tab shows extracted variables | CodeMirror HTML editor + auto-extracted `{{var}}` list |
| 6 — populate variables in rule; live preview both sides; save final outcome | dynamic Variables sub-form on the node; `<ComponentPreview>` in both editors; rule stores the **reference + values** |
| 7 — each variable has title, name, description | `variables[] = {name, title, description}` |
| 8 — highlight + errors/warnings, never blocks save | CodeMirror lint (DOMParser + mustache brace-balance), advisory only |

## 2. Domain model

A **Component** (UI label) = internal `ComponentTemplate`. HTML-only. Lives in a global library (not scoped to a feature/version).

```
ComponentTemplate
  id, slug, name, description?
  default_mode: 'latest' | 'pinned'     ← how "default" resolves
  default_version_id?: uuid             ← used only when default_mode = 'pinned'
  versions[] : ComponentTemplateVersion
       id, version_number, description?
       html_body: string                ← mustache template (the ONLY payload)
       variables: ComponentVariable[]    ← [{ name, title, description }]
```

- **Versions are editable in place** (edit `html_body` / `variables` / `description`). This is intentional: editing the version a rule follows changes live output (spec item 4 auto-update). Creating a *new* version clones the current default (or starts blank) so you can iterate off-line; "make default" is offered on create (default-on, since "latest is generally default").
- A component always has **≥1 version** (the last version cannot be deleted).
- `ComponentVariable.name` is the mustache key (`{{title}}` → `"title"`). `title` + `description` are author metadata that drive the rule-side population UI (item 7).

### "default" resolution (the movable pointer — controlled in the Component Editor)

| `default_mode` | default resolves to |
|---|---|
| `latest` | the highest `version_number` (auto-advances when a new version is created) |
| `pinned` | `default_version_id` |

A rule node's `version` is either a specific `version_number` **or** the string `"default"`. Every rule that follows `"default"` resolves to the *component's current default* — so changing the default in the editor (e.g. pin to v7, then later v5) re-points all default-following rules at once (spec item 4, confirmed).

### Deletion behavior (confirmed: fall back to current default)

- Deleting a **non-default** version: allowed.
- Deleting the **current default** version: allowed; the default re-points to the newest remaining version (mode stays `pinned`, or `latest` already self-heals).
- Cannot delete the **last** remaining version (component must keep ≥1).
- Proxy at request time: a rule pinned to a now-deleted version → render the component's **current default** instead (fall-back, not fail-open). A `"default"` rule whose default is somehow unresolvable → fail-open (component skipped). All resolve/render failures are fail-open (body served untouched, `X-RRE-Apply-Status: error`).

## 3. Backend

Layering unchanged (`api/v1 → services → repositories → models`; `schemas` for DTOs; runtime SQLx only).

### 3.1 Migration `0013_component_templates`

```sql
-- up
CREATE TABLE rre.component_templates (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug          VARCHAR(120) NOT NULL UNIQUE,
  name          VARCHAR(200) NOT NULL,
  description   TEXT,
  default_mode  VARCHAR(8) NOT NULL DEFAULT 'latest',
  default_version_id UUID,                 -- FK added below (deferrable)
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT component_templates_default_mode_chk CHECK (default_mode IN ('latest','pinned'))
);
CREATE INDEX component_templates_created_at_idx ON rre.component_templates (created_at DESC, slug ASC);

CREATE TABLE rre.component_template_versions (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  component_id  UUID NOT NULL REFERENCES rre.component_templates(id) ON DELETE CASCADE,
  version_number INTEGER NOT NULL,
  description   TEXT,
  html_body     TEXT NOT NULL DEFAULT '',
  variables     JSONB NOT NULL DEFAULT '[]'::jsonb,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (component_id, version_number)
);
ALTER TABLE rre.component_templates
  ADD CONSTRAINT component_templates_default_version_fk
  FOREIGN KEY (default_version_id) REFERENCES rre.component_template_versions(id)
  ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED;
```
Down drops both tables (FK first). Migration order is additive — no reorder of 0001–0012.

### 3.2 Models / schemas / enums

- `models/component_template.rs`: `ComponentTemplate`, `ComponentTemplateVersion` (`FromRow`).
- `models/enums.rs`: `DefaultMode { Latest, Pinned }` (`sqlx::Type`, `rename_all="lowercase"`).
- `schemas/component_template.rs`:
  - `ComponentVariable { name (1..=64, snake_case-ish), title (1..=100), description? (<=500) }`
  - `ComponentTemplateCreate { slug (SLUG_RE 3..=120), name (1..=200), description?, html_body?, variables?: Vec<ComponentVariable> }` (creates the component + its v1)
  - `ComponentTemplateUpdate { name?, description?, default_mode?, default_version_number? }` (manages the default pointer; `default_version_number` required when switching to `pinned`)
  - `ComponentTemplateRead { id, slug, name, description?, default_mode, default_version_number?, latest_version_number, versions: Vec<ComponentTemplateVersionSummary>, created_at, updated_at }`
  - `ComponentTemplateSummary` (list rows: id, slug, name, description?, default_version_number, latest_version_number, updated_at)
  - `VersionCreate { description?, html_body?, variables?, make_default?: bool }`, `VersionUpdate { description?, html_body?, variables? }`, `ComponentTemplateVersionRead { id, version_number, description?, html_body, variables, is_default, created_at, updated_at }`
  - `ResolvedComponentRead { version_number, html_body, variables }` (proxy-facing resolve payload)
- All DTOs `deny_unknown_fields`.

### 3.3 Errors

Add `ComponentVersionNotFound(String)` → `COMPONENT_VERSION_NOT_FOUND` (404). `COMPONENT_NOT_FOUND` + `SLUG_CONFLICT` already exist. `LAST_VERSION_PROTECTED` (409) when deleting the only version.

### 3.4 Service (`component_template_service`) — tx boundaries

- `create`: tx → INSERT component → INSERT v1 (`version_number=1`) → set `default_version_id=v1`, `default_mode='latest'` → commit. Dup slug → 409.
- `list` / `get_with_versions` / `get_version`: read paths (`&PgPool`).
- `update`: name/description/default. Switching to `pinned` validates `default_version_number` exists. Switching to `latest` clears the pin (resolution becomes dynamic).
- `create_version`: tx → `SELECT MAX(version_number) FOR UPDATE` → INSERT (clone current default's body/variables unless supplied) → if `make_default` (or `default_mode='latest'`) re-point default → commit.
- `update_version`: edit body/variables/description in place.
- `delete_version`: tx → forbid if it's the last version (`LAST_VERSION_PROTECTED`) → if it's the current pinned default, re-point default to newest remaining → DELETE → commit.
- `make_default`: set `default_mode='pinned'`, `default_version_id` = that version.
- `resolve(component_id, selector)`: read. `selector ∈ {Default, Version(n)}`. `Default` → latest or pinned per `default_mode`; `Version(n)` → that version, **falling back to current default if n is missing**. Returns `ResolvedComponentRead`.

### 3.5 Routes (`api/v1/component_templates.rs`)

```
GET    /api/v1/component-templates                              -> list (?q name filter + pagination)
POST   /api/v1/component-templates                              -> create (201)
GET    /api/v1/component-templates/{cid}                        -> get_with_versions
PATCH  /api/v1/component-templates/{cid}                        -> update
DELETE /api/v1/component-templates/{cid}                        -> delete (204)
GET    /api/v1/component-templates/{cid}/versions               -> list versions
POST   /api/v1/component-templates/{cid}/versions               -> create_version (201)
GET    /api/v1/component-templates/{cid}/versions/{vnum}        -> get_version
PATCH  /api/v1/component-templates/{cid}/versions/{vnum}        -> update_version
DELETE /api/v1/component-templates/{cid}/versions/{vnum}        -> delete_version (204)
POST   /api/v1/component-templates/{cid}/versions/{vnum}/make-default -> make_default
GET    /api/v1/component-templates/{cid}/resolve?version=default|N    -> resolve (proxy-facing)
```
`{cid}` = component UUID; `{vnum}` = `version_number` i32. No collision with existing `/components/{cid}`.

### 3.6 Node manifest (`backend/config/node_types.json`) — zero new backend Rust

Add two `node_kind: expression` specs (mirroring `apply_outcome`), plus two field controls:

```jsonc
{ "kind": "apply_component", "label": "Component", "category": "content",
  "applies_to": "html", "node_kind": "expression",
  "summary": "Render a Component template and inject it into the page.",
  "fields": [
    { "name": "component_id", "label": "Component", "control": "component_select", "required": true, "default": "", "required_message": "Pick a component" },
    { "name": "version", "label": "Version", "control": "component_version_select", "required": true, "default": "default" },
    { "name": "target_selector", "label": "Target selector", "control": "text", "required": true, "placeholder": "main .article-body", "required_message": "Enter a CSS selector" },
    { "name": "placement_mode", "label": "Placement", "control": "select", "required": true, "default": "append",
      "options": [ {"value":"replace","label":"replace"},{"value":"append","label":"append"},{"value":"prepend","label":"prepend"},{"value":"before","label":"before"},{"value":"after","label":"after"} ] }
  ],
  "output": { "branches": [ { "id": "out", "label": "Next" } ] } }

{ "kind": "apply_component_json", "label": "Component", "category": "json",
  "applies_to": "json", "node_kind": "expression",
  "summary": "Render a Component template to an HTML string and set it at a JSON path.",
  "fields": [
    { "name": "component_id", "label": "Component", "control": "component_select", "required": true, "default": "", "required_message": "Pick a component" },
    { "name": "version", "label": "Version", "control": "component_version_select", "required": true, "default": "default" },
    { "name": "target_path", "label": "Target path", "control": "text", "required": true, "placeholder": "$.content.html", "required_message": "Enter a JSON path" }
  ],
  "output": { "branches": [ { "id": "out", "label": "Next" } ] } }
```

- New `Control` variants: `ComponentSelect`, `ComponentVersionSelect` (both dynamic — options supplied client-side, **skip** option-membership validation, like `OutcomeSelect`).
- Variable **values** are NOT manifest fields. They are stored on the action as `variables: { name: value }` and rendered by a special-cased sub-form on the frontend (§5.4).

### 3.7 rule_graph validation (`rule_graph_service::validate`)

New stable `rule_id`s (run per expression node whose `action.type` is `apply_component`/`apply_component_json`):
- `apply_component_ref_exists` — `action.component_id` is a UUID present in `rre.component_templates`.
- `apply_component_version_valid` — `action.version` is either the string `"default"` or a positive integer (well-formedness only). A pinned version number that no longer exists is **not** a save-time error — version drift is handled fail-open at the proxy (fall back to current default, §2). `processor_field_required` still applies to `component_id` / `version` / `target_selector` / `target_path`.

`validate` already takes `&NodeManifest`; it gains a `&PgPool`/repo handle (or a pre-fetched set of component ids) to check `apply_component_ref_exists`, mirroring how `apply_outcome_ref_exists` checks outcome membership.

## 4. Proxy

### 4.1 Resolve + cache

- New `infra::component_cache` (moka SWR, TTL = `active_version_ttl_secs`, key `(component_id: Uuid, selector: VersionSelector)`), value `Arc<ResolvedComponent { version_number, html_body, variables }>`. SWR gives the spec-item-4 auto-update within the TTL window.
- `BackendClient::resolve_component(id, selector)` → `GET /api/v1/component-templates/{id}/resolve?version=default|N`. 404 → `None` (fail-open).

### 4.2 Render

- New dep: **`mustache = "0.9"`** (dynamic `MapBuilder`; `{{x}}` HTML-escaped, `{{{x}}}` / `{{&x}}` raw). Mustache scope = flat interpolation only (no sections/partials/lambdas), matching the flat variable model.
- New `domain::applier::component_render::render(html_body, variables_values) -> String`. Missing variables render empty. Render errors → propagate as `ApplyError` (fail-open upstream).

### 4.3 Apply integration (async pre-resolve, sync apply)

In `forwarder::apply_features_html/json` (and the test-eval / full-journey paths): after eval yields `actions`, **pre-resolve** every `apply_component`/`apply_component_json` action's `(component_id, version)` via `resolve_component(...).await` into a `HashMap<(Uuid,Selector), Arc<ResolvedComponent>>`. Pass that map into the (sync) `json_apply::apply_action_html/json`, which gains branches:
- `apply_component` (HTML): look up resolved template → `component_render::render` with `action.variables` → **ammonia sanitize** → inject at `target_selector` with `placement_mode` (reuse `html_injection` injection core; idempotency marker). Missing resolution → skip (fail-open).
- `apply_component_json` (JSON): render → produce the HTML **string** → `json_path` set at `target_path` (reuse `json_set` core). Missing → skip.

Hot-path budget unchanged: resolve is a cached await on the async side (like `active_version`); render+sanitize+inject are within the existing transform budget.

### 4.4 In-band timing / journey

`apply_component*` actions appear in `rre.feature_expressions[*].expressions` and the test-panel `journey[]` exactly like `apply_outcome` (they are matched expression nodes). `describeStep` gets an `apply_component` case ("Rendered component {name} → {selector|path}").

## 5. Frontend

Stack reuse: Next 16 App Router, TanStack Query, Zustand, Radix, Zod 4, DOMPurify/`SafeHtml`. New deps: `mustache` (client render), `@uiw/react-codemirror` + `@codemirror/lang-html` + `@codemirror/lint` + `@codemirror/view`.

### 5.1 Section + routes
```
/products/components                 -> ComponentLibraryClient (list + create)
/products/components/[slug]          -> ComponentEditorPage (editor + versions)
```
Add "Components" to the products nav (alongside Features / Sites / Test Presets).

### 5.2 API client + schemas
`lib/api/componentTemplates.ts` (NOT `components.ts`, which is the outcome sub-component client) + zod `lib/schemas/componentTemplates.ts` mirroring §3.2.

### 5.3 Component Editor (split layout — spec items 5/7/8)
- **Left**: CodeMirror 6 HTML editor (`html()` language). A `lintEngine` (pure fn) produces diagnostics shown in the lint gutter + an "Issues" list: (a) HTML parse errors via `DOMParser('text/html')` / unbalanced-tag heuristic, (b) unbalanced/empty `{{ }}` mustache. **Advisory only — never blocks Save** (item 8).
- **Right**: **Variables** panel. `extractVariables(html_body)` (regex `/\{\{\{?\s*([\w.]+)\s*\}?\}\}/g`) lists each `{{name}}`; the author annotates `title` + `description`. Names present in body but not annotated are auto-added; annotated names absent from body are flagged "unused". Persisted as `variables[]`.
- **Live preview**: `<ComponentPreview html variables values />` renders mustache (client `mustache.render`) → `SafeHtml`. Editor uses sample/placeholder values (variable `title` as placeholder).
- **Version bar**: switch version, create version (modal: description + "make default"), delete version (guard last), set default; default control offers **Track latest** vs **Pin to version**. Per-version description editable.

### 5.4 Rule-node integration (spec item 6)
`NodeConfigDrawer` + `GenericNodeForm` gain the two dynamic controls and a special-cased Variables sub-form when `action.type ∈ {apply_component, apply_component_json}`:
- `component_select` → dropdown from `GET /api/v1/component-templates` (id + name).
- `component_version_select` → "Default (follows component)" + each `version_number` of the chosen component.
- **Variables sub-form**: fetch the chosen (component, resolved-version) → render one input per declared variable, labelled by `title`, helped by `description` (item 7). Values stored in `action.variables`.
- **Live preview** of the rendered result with the entered values (`<ComponentPreview>` again).
- The node caches a denormalized `componentName` for display (mirrors `outcomeTitle`).

### 5.5 What the rule saves
`action = { type: "apply_component", component_id, version: "default"|N, variables: {name:value}, target_selector, placement_mode }` (or `apply_component_json` with `target_path`). **The reference + values — never frozen HTML** (so default/auto-update works).

## 6. Testing

- Backend: `tests/component_templates.rs` (CRUD, versions, default modes, make-default, delete-default re-point, last-version guard, resolve default|pinned|missing-fallback, slug conflict, validation rule_ids). Reuse the testcontainers harness (`DOCKER_HOST` env).
- Proxy: `component_render` unit tests (escaped/raw/missing vars); applier integration (`apply_component` inject + `apply_component_json` set-at-path); resolve cache + missing-version fall-back-to-default; fail-open on render/resolve error; idempotency.
- Frontend: vitest for `extractVariables`, `lintEngine`, `ComponentPreview` render; component tests for editor save (lint never blocks) + node Variables sub-form. A Playwright E2E: author a component → use it in an HTML rule → see live preview → full-journey reflects rendered output.
- Each touched module ≥80% on new code. `make check` (all per-service gates) green before done.

## 7. Out of scope (YAGNI)
- JSON-body components (only HTML components; JSON features consume them as serialized strings).
- Mustache sections/partials/lambdas; variable values referencing request context (literals only).
- Component tags/folders, bulk ops, usage analytics, audit-history UI.

## 8. Implementation plan (phases)

1. **Backend data + service + routes** — migration 0013, models/enums/schemas, repos, `component_template_service`, handlers, errors, OpenAPI; `tests/component_templates.rs`. Gate: `make backend-check`.
2. **Manifest + validation** — two node specs + two controls in `node_types.json`; `apply_component_ref_exists` + field-required validation; backend tests for the new rule_ids. Gate: backend-check.
3. **Proxy render + apply** — `mustache` dep, `component_render`, `component_cache`, `BackendClient::resolve_component`, applier branches, pre-resolve in forwarder + test/full-journey paths, `describeStep` proxy-side; unit+integration tests; latency smoke. Gate: `make proxy-check`. (Rebuild + restart the proxy binary — serialized structs changed.)
4. **Frontend library + editor** — deps, API client + zod, `/products/components` list + `ComponentEditorPage` (CodeMirror + lint + variables + preview + versions/default), nav entry; vitest. Gate: `make frontend-check`.
5. **Frontend rule-node integration** — dynamic controls + Variables sub-form + preview in `NodeConfigDrawer`/`GenericNodeForm`; vitest + Playwright E2E. Gate: frontend-check.
6. **Seed + docs** — seed one demo component (e.g. "Paywall CTA" with `{{headline}}`,`{{cta}}`) wired into the demo feature; update `CONTRACTS.md` (new tables/routes/manifest/rule_ids) + `RRE_README.md`. Gate: `make check` (full).

Each phase ends green on its gate; the whole stack compiles once at the end per BUILDABILITY RULES.

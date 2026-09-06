# Single Rule Canvas + Product Catalogue + Global Outcomes Library

Date: 2026-09-06
Status: approved

## Problem

Today `rule_graph` carries three parallel canvases (`anonymous`/`registered`/`customer`), classified
by a single `rre_user_type` cookie. The three-canvas model adds authoring and maintenance overhead
without matching how RRE customers actually gate content: by login state and by which product(s) a
user holds. There is also no first-class way to author a reusable "outcome" (a Library component +
version + variable values, given a name) that can be dropped into any rule as a `saved_outcome`
reference — today `apply_component`/`apply_component_json` re-enter variable values on every node, and
the per-version `rre.outcomes` mechanism is version-scoped, not a global library.

## Decision

1. Collapse `rule_graph` to a single canvas, renamed **Rule Canvas** in the UI.
2. Add two new decision nodes — `logged_in` and `has_product` — backed by a new backend-owned
   **Product Catalogue** (`rre.products`).
3. Add a global **Outcomes** library: `rre.saved_outcomes` (name + Library component + version pin +
   variable values), with a live preview editor and two new action nodes,
   `apply_saved_outcome` / `apply_saved_outcome_json`, that reference a saved outcome by id.
4. Identity signals (login + products) are read from a **cookie first, header fallback** at the proxy
   boundary, computed once per request.

## Non-goals

- Do not touch the existing per-version `rre.outcomes` / `rre.components` mechanism, the
  `transformation/[outcomeId]` route, the `apply_outcome` node, or the builtin `ShowContent` outcome.
  Both outcome concepts coexist; "saved outcome" / "Outcomes library" always refers to the new one in
  UI copy to avoid confusion.
- No entitlements API / JWT decoding. No product hierarchies, tiers, or bundles — a product is a flat
  label.
- No migration of *content* from the old `registered`/`customer` canvases — only `anonymous` survives
  (see Migration below). This is a dev-only database (seed/demo data); no production data exists yet.
- No change to `apply_component`/`apply_component_json` (the per-node variable-values mechanism stays
  as an option alongside saved outcomes).

## Migration (rule_graph shape)

New migration `0015_single_canvas`:
- `ALTER TABLE rre.versions ALTER COLUMN rule_graph SET DEFAULT '{"canvas":{"nodes":[],"edges":[],"root_node_id":null}}'::jsonb`
- `UPDATE rre.versions SET rule_graph = jsonb_build_object('canvas', rule_graph->'anonymous')` (drops
  `registered`/`customer` content; `anonymous` is preserved as `canvas`).
- Down: restores the old three-key default and rewrites `rule_graph = jsonb_build_object('anonymous',
  rule_graph->'canvas', 'registered', '{"nodes":[],"edges":[],"root_node_id":null}'::jsonb, 'customer',
  same-empty)`.

`backend/src/schemas/rule_graph.rs`: `RuleGraph { pub canvas: CanvasGraph }`, `canvases()` returns a
single `("canvas", &graph)` pair (kept as an iterator so `rule_graph_service::validate` and the
`loc` string builder need no shape-specific changes beyond the key name).

`proxy/src/domain/graph.rs`: `RuleGraph { pub canvas: CanvasGraph }`; `RuleGraph::canvas()` returns the
one field directly (no `Canvas` enum parameter). `proxy/src/domain/classifier.rs` is deleted.
`proxy/src/domain/graph.rs`'s `enum Canvas` is deleted. Every call site that threads a `Canvas` value
(`forwarder.rs`, `evaluator.rs`, `infra/compiled_cache.rs` cache key, observability log field
`canvas`) drops that parameter/field. `EvaluationContext`/`EvaluationContextParts` gain the new
`identity: Identity` field (see below) in the same pass, since both touch the same call sites.

Frontend: `CanvasKey` type, `CanvasSlider`, the `canvases: Record<CanvasKey, …>` store shape,
`serialize.ts`/`deserialize.ts` three-key mapping, `diff.ts`/`compare/*` three-way compare all
collapse to a single canvas. `ruleBuilderStore` keeps one `canvas: {nodes, edges, rootNodeId}` plus
`selected` is removed (no tab state). The Rule Canvas view removes `CanvasSlider` entirely; the canvas
header label changes from "Testing" tab copy where it said "Anonymous/Registered/Customer" (none
currently do per the copy map audit — confirm during implementation and adjust only what exists).

## Identity signals (proxy)

`proxy/src/config.rs` `Settings` gains:
```rust
pub identity_user_cookie: String,      // default "rre_user"
pub identity_products_cookie: String,  // default "rre_products"
pub identity_user_header: String,      // default "x-rre-user"
pub identity_products_header: String,  // default "x-rre-products"
```
(env-overridable like every other `Settings` field; added to `proxy/config/default.json`).

New `proxy/src/domain/identity.rs`:
```rust
pub struct Identity { pub logged_in: bool, pub products: HashSet<String> }
pub fn resolve(headers: &HeaderMap, cookies: &HashMap<String, String>, settings: &IdentitySettings) -> Identity
```
- `logged_in`: true if `cookies[user_cookie]` is present and non-empty; else true if the
  `user_header` request header is present and non-empty; else false.
- `products`: parsed from whichever source produced a non-empty value, cookie checked first — split on
  `,`, trim, lowercase, drop empties, collect into a `HashSet<String>`.
- Called once in `forwarder::forward` (same place `classifier::classify` was called) and stored as
  `EvaluationContext.identity` / threaded through `EvaluationContextParts`.
- `/__rre/eval`, `/__rre/eval-url`, `/__rre/eval-full-journey` request bodies gain optional
  `logged_in?: bool` (default false) and `products?: string[]` (default `[]`) on `EvalContext`,
  constructing `Identity` directly (bypassing cookie/header parsing) so the Test panels can simulate
  either state. Frontend Test panel gains a "Logged in" toggle and a products multi-select (reusing the
  product catalogue's `q` search) in the same panel as device/UA/path.

## New decision nodes (manifest + processors)

`backend/config/node_types.json` additions. The `user` category (currently `coming_soon: true`) is
flipped to active and becomes the home for both:

```jsonc
{ "kind": "logged_in", "label": "Logged In", "category": "user", "node_kind": "decision",
  "applies_to": "all", "summary": "Branch on whether the visitor is logged in",
  "fields": [], "output": { "branches": [{"id":"yes"},{"id":"no"}] } }

{ "kind": "has_product", "label": "Has Product", "category": "user", "node_kind": "decision",
  "applies_to": "all", "summary": "Branch on whether the visitor holds a chosen product",
  "fields": [{ "name": "product", "label": "Product", "control": "product_select", "required": true,
               "placeholder": "Search products…" }],
  "output": { "branches": [{"id":"yes"},{"id":"no"}] } }
```

Proxy: `proxy/src/domain/processors/logged_in.rs` (`LoggedInProcessor`, branches on
`ctx.identity.logged_in`) and `proxy/src/domain/processors/has_product.rs` (`HasProductProcessor`,
branches `Yes` iff `ctx.identity.products.contains(&config.product)`), both registered in
`default_registry()`, mirroring `site_match.rs`'s shape exactly (config extraction, `ProcessorError`,
unit tests with a `ctx()` helper that now also sets `identity`).

Backend validation: new rule_id `has_product_ref_exists` — a Decision node with
`processor.type == "has_product"` must have `processor.product` present in the current
`rre.products` label set (threaded into `validate` the same way `valid_component_ids` already is,
as a `valid_product_labels: &HashSet<String>` fetched once per validate call). `product_select`
(new `Control` variant) skips option-membership checks like `site_select`, relying on this rule
instead.

## Product Catalogue

Migration `0016_products`:
```sql
CREATE TABLE rre.products (
  label VARCHAR(64) PRIMARY KEY,               -- snake_case, immutable
  name VARCHAR(200) NOT NULL UNIQUE,
  description VARCHAR(500),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX products_created_at_idx ON rre.products (created_at DESC, label ASC);
ALTER TABLE rre.products ADD CONSTRAINT products_label_snake_case
  CHECK (label ~ '^[a-z0-9]+(_[a-z0-9]+)*$');
```
Down drops the table.

Model `backend/src/models/product.rs`: `Product { label, name, description: Option<String>,
created_at, updated_at }` (`FromRow`), pattern-identical to `Site`.

Schemas `backend/src/schemas/product.rs`: `ProductCreate { label (regex, 1..=64), name (1..=200),
description? }`, `ProductUpdate { name?, description? }` (label immutable), `ProductRead` (mirrors
model), `deny_unknown_fields` on both writes. New `PRODUCT_LABEL_RE` static regex (same pattern as
`has_product_ref_exists` validates against, and as the node manifest's `product` field value).

Service `backend/src/services/product_service.rs`: `create/list(q, page, page_size)/get/update/delete`.
Dup label/name → 409 `SLUG_CONFLICT`. Missing → 404 `PRODUCT_NOT_FOUND`. Deleting a product is NOT
blocked by usage in a `has_product` node — `has_product_ref_exists` is a save-time-only guard (same as
`apply_outcome_ref_exists`/`apply_component_ref_exists`), and the proxy's `HasProductProcessor` falls
back to `Branch::No` fail-open when `product` doesn't match any current label (mirrors `site_match`'s
existing fail-open behavior on a stale reference). This avoids a new class of delete-time
cross-table JSONB scan for a label that could appear in any version's rule_graph.

Routes (`backend/src/api/v1/products.rs`), mounted at `/api/v1/products`:
```
POST   /api/v1/products          -> create   (201)
GET    /api/v1/products          -> list     (?q, ?page, ?page_size)
GET    /api/v1/products/{label}  -> get
PATCH  /api/v1/products/{label}  -> update
DELETE /api/v1/products/{label}  -> delete   (204)
```

Frontend: `lib/api/products.ts` + `lib/schemas/products.ts` (zod mirror). New route
`/products/catalogue`: `ProductCatalogueClient.tsx` (list + search `q` + create modal), no editor
page needed beyond inline edit (name/description) in a row — reuses the Sites list page's shape
(`ProductCard` or a simple table; implementer's choice, matching existing list-page conventions).
`product_select` control: new `ProductSelectControl.tsx` (searchable single-select, copy of
`SiteSelectControl.tsx` querying `GET /api/v1/products?q=`). Add `"product_select"` to
`NodeFieldControl` zod enum (`lib/api/nodeTypes.ts`) and to the `GenericNodeForm.tsx` control switch.
Nav: add "Catalogue" link to `TopNav.tsx` (label via `uiCopy.ts`).

## Global Outcomes Library

Migration `0017_saved_outcomes`:
```sql
CREATE TABLE rre.saved_outcomes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug VARCHAR(120) NOT NULL UNIQUE,
  name VARCHAR(200) NOT NULL UNIQUE,
  component_id UUID NOT NULL REFERENCES rre.component_templates(id) ON DELETE RESTRICT,
  version_number INTEGER,                 -- NULL = "Latest" (tracks the component's current default... 
                                           -- see resolution rule below)
  variables JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX saved_outcomes_created_at_idx ON rre.saved_outcomes (created_at DESC, slug ASC);
```
`ON DELETE RESTRICT` on `component_id`: deleting a Library component used by a saved outcome is
blocked at the DB level; the service catches the FK violation and returns 409 `COMPONENT_IN_USE`
(mirrors the existing `LAST_VERSION_PROTECTED` style of guard).

**Version resolution**: `version_number = NULL` means "Latest", which reuses the *existing*
`component_template_service::resolve` endpoint's `version=default` path verbatim — i.e. it tracks
whatever the component's own default pointer is set to (`latest` or `pinned`), giving the same
spec-item-4 auto-update semantics `apply_component`'s `"default"` string already has. A saved outcome
with `version_number = NULL` calls `resolve?version=default`; with `version_number = N` it calls
`resolve?version=N`. This avoids adding a second resolution semantic to the backend.

Model `backend/src/models/saved_outcome.rs`, schemas `backend/src/schemas/saved_outcome.rs`:
`SavedOutcomeCreate { slug (SLUG_RE, 3..=120), name (1..=200), component_id, version_number?: i32,
variables?: HashMap<String,String> }`, `SavedOutcomeUpdate { name?, version_number?: Option<i32>,
variables? }`. Field omitted → unchanged; field present with a value → pin that version; field present
as explicit JSON `null` → clear to Latest. Use the double-option pattern already established elsewhere
in the schemas crate if one exists (grep for `Option<Option<`), otherwise add
`#[serde(default, with = "::serde_with::rust::double_option")]` (the `serde_with` crate; add it as a
backend dependency if not already present). `SavedOutcomeRead { id,
slug, name, component_id, component_name, version_number: Option<i32>, variables, created_at,
updated_at }` (denormalized `component_name` for list display, joined at read time).
`ResolvedSavedOutcomeRead { html_body: String, variables_used: HashMap<String,String> }` — the
proxy-facing resolved-and-rendered shape (rendering happens in the SAME place `component_render`
already renders mustache — no new render logic, just: resolve the component version, then mustache
against `saved_outcome.variables` instead of action-level variables).

Routes (`backend/src/api/v1/saved_outcomes.rs`), mounted at `/api/v1/saved-outcomes`:
```
POST   /api/v1/saved-outcomes                    -> create               (201)
GET    /api/v1/saved-outcomes                    -> list                 (?q, ?page, ?page_size)
GET    /api/v1/saved-outcomes/{id}                -> get
PATCH  /api/v1/saved-outcomes/{id}                -> update
DELETE /api/v1/saved-outcomes/{id}                -> delete               (204)
GET    /api/v1/saved-outcomes/{id}/resolve        -> resolve              (proxy-facing; 200 + ResolvedSavedOutcomeRead, 404 if id/component/version unresolvable)
```
Dup slug/name → 409 `SLUG_CONFLICT`. Missing → 404 `SAVED_OUTCOME_NOT_FOUND`.

**Validation** (`rule_graph_service::validate`): new rule_id `saved_outcome_ref_exists` — an
Expression node whose `action.type` is `apply_saved_outcome`/`apply_saved_outcome_json` must have
`action.saved_outcome_id` present in the version's known saved-outcome id set (same
pre-fetch-set pattern as `valid_component_ids`).

**Two new expression node types** in the manifest, category `content` (HTML) and `json` (JSON):
```jsonc
{ "kind": "apply_saved_outcome", "label": "Apply Saved Outcome", "category": "content",
  "applies_to": "html", "node_kind": "expression",
  "fields": [
    { "name": "saved_outcome_id", "label": "Outcome", "control": "saved_outcome_select", "required": true },
    { "name": "target_selector", "label": "Target selector", "control": "text", "required": true },
    { "name": "placement_mode", "label": "Placement", "control": "select", "required": true,
      "options": [{"value":"replace"},{"value":"append"},{"value":"prepend"},{"value":"before"},{"value":"after"}] }
  ], "output": { "branches": [{"id":"out","label":"Next"}] } }

{ "kind": "apply_saved_outcome_json", "label": "Apply Saved Outcome (JSON)", "category": "json",
  "applies_to": "json", "node_kind": "expression",
  "fields": [
    { "name": "saved_outcome_id", "label": "Outcome", "control": "saved_outcome_select", "required": true },
    { "name": "target_path", "label": "Target path", "control": "text", "required": true }
  ], "output": { "branches": [{"id":"out","label":"Next"}] } }
```
No `variables` field on either node — values live on the saved outcome, not the action (this is the
whole point: editing the saved outcome propagates to every rule referencing it, same "movable
reference" pattern as `apply_component`'s `"default"` version string).

**Rendering happens on the backend, in the `resolve` route** — `saved_outcome_service::resolve` fetches
the component version's `html_body`, renders it against `saved_outcome.variables` with the `mustache`
crate (add `mustache` to `backend/Cargo.toml`; same flat-interpolation semantics as
`proxy/src/domain/applier/component_render.rs`), and returns the already-rendered `html_body` in
`ResolvedSavedOutcomeRead`. This keeps rendering logic in exactly one place per layer (frontend preview
renders client-side with the JS `mustache` package, same as `ComponentPreview` does for plain
components; backend renders server-side for the proxy to consume) and keeps the proxy applier free of
rendering: `apply_saved_outcome` reuses `html_injection` core exactly like `apply_component`, injecting
the ALREADY-RENDERED `html_body` as-is; `apply_saved_outcome_json` reuses `json_apply::json_set` the
same way `apply_component_json` does.

**Proxy**: `infra::saved_outcome_cache` — a moka SWR cache identical in shape to `component_cache.rs`,
keyed `Uuid` (saved_outcome id), value `Arc<ResolvedSavedOutcome { html_body }>`, calling
`BackendClient::resolve_saved_outcome(id) -> GET /api/v1/saved-outcomes/{id}/resolve`, same TTL setting
(`active_version_ttl_secs`) as `component_cache`. Pre-resolution happens in the forwarder's existing
pre-resolve pass (`forwarder.rs:295/461/608-660`) alongside `apply_component*` pre-resolution — one
more branch on `action.type`. All failures fail-open (serve body untouched), matching every other
applier.

**Frontend**: new section `/products/outcomes`:
```
/products/outcomes            -> SavedOutcomeLibraryClient (list + create)
/products/outcomes/[slug]     -> SavedOutcomeEditorPage (editor)
```
Editor: searchable Library component picker (`ComponentSelectControl`-equivalent, or reuse
`GenericNodeForm`'s existing `component_select` dropdown fetch), version picker ("Latest" + each
`version_number`, mirrors `component_version_select`), a Variables form generated from the resolved
version's declared `variables[]` (title/description-labelled inputs, same shape as
`GenericNodeForm`'s Variables sub-form — extract that sub-form into a shared
`VariablesForm.tsx` component used by BOTH the node drawer and this new editor to avoid duplicating
the render-input-per-variable logic), and a live `<ComponentPreview>` fed by the currently-selected
component version + entered variable values (same client `mustache.render` path `ComponentPreview`
already uses — no new preview logic). Save posts `SavedOutcomeCreate`/`Update`.

`saved_outcome_select` control: new `SavedOutcomeSelectControl.tsx` (searchable single-select
querying `GET /api/v1/saved-outcomes?q=`), added to `NodeFieldControl` enum and `GenericNodeForm`'s
control switch alongside `product_select`. Nav: add "Outcomes" link to `TopNav.tsx`.

## Testing

- Backend: `tests/products.rs` (CRUD, slug/name conflict, label regex, 404), `tests/saved_outcomes.rs`
  (CRUD, component FK restrict → 409, resolve default|pinned, slug conflict, validation rule_ids),
  `rule_graph_service` unit tests for `has_product_ref_exists` / `saved_outcome_ref_exists`, migration
  round-trip test for `0015_single_canvas` (up then down restores three-key shape with anonymous
  content preserved).
- Proxy: `identity.rs` unit tests (cookie-present, header-fallback, both-absent, malformed csv),
  `logged_in`/`has_product` processor unit tests (mirroring `site_match.rs`'s test style),
  `saved_outcome_cache` unit tests mirroring `component_cache.rs`'s existing SWR test suite, applier
  integration tests for `apply_saved_outcome`/`apply_saved_outcome_json` fail-open paths.
- Frontend: vitest for `ProductSelectControl`/`SavedOutcomeSelectControl`, `VariablesForm` extraction
  (used from two call sites, must not regress the existing node-drawer variables test), Rule Canvas
  single-canvas store/serialize/deserialize/diff (delete the three-canvas test fixtures, replace with
  single-canvas fixtures), catalogue + outcomes-library list/create/edit flows. Update every existing
  test that references `CanvasKey`/`CanvasSlider`/`anonymous`/`registered`/`customer` canvas literals.
- `make check` (all per-service gates) green before done — this is the only gate; no CI.

## Out of scope (YAGNI)

- Product tiers/hierarchies, bundles, or expiry.
- JWT/entitlements-API identity sources.
- Bulk import/export for products or saved outcomes.
- A "which rules use this saved outcome / product" usage-analytics view.
- Renaming `variables` shape or the mustache render engine (still flat interpolation, no
  sections/partials/lambdas, matching the existing Component Editor spec's YAGNI list).

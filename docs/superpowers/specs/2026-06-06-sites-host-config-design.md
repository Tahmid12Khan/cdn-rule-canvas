# Sites (host config) + site-match decision node — Design Spec

Date: 2026-06-06
Status: Approved (build in progress)
Branch: `feat/journey-step-diff` (current) → feature work

## 1. Problem

RRE proxy currently forwards every request to a single static upstream
(`upstream_base_url`, default `http://demo-upstream:8081`) and decides which rule
features apply via a hardcoded `proxy/config/feature_map.yaml`
(`localhost:9000 → dn-article, dn-json-article`). There is **no** `www.dn.no`
literal anywhere — "the dn hardcode" = those feature_map entries + the static
upstream + the `dn-*` seed feature ids.

We want **Sites**: user-configurable `source → destination` routing, persisted in
DB. A request arriving on a Site's source host:port is reverse-proxied to that
Site's destination protocol://host:port, with all rule features applied to the
rewritten response. Plus a new **site-match decision node** that branches yes/no
on whether the current request belongs to a chosen Site.

## 2. Decisions (locked with user)

- **Routing model**: Site matched by incoming `Host` → picks the upstream
  destination. **All** features are evaluated for every request (no host/path
  gating). Per-site scoping is achieved only via the new site-match decision node.
  → `feature_map.yaml` + `feature_map.rs` are **removed**.
- **Demo**: seed a demo Site (`localhost:9000 → demo-upstream:8081`) and keep the
  demo rule feature, now DB-driven. Rename `dn-article`/`dn-json-article` →
  `demo-article`/`demo-json-article`; update all references.
- **"redirect" = reverse-proxy forward** (fetch upstream, rewrite body, return),
  not an HTTP 302.
- **Single listen port**: the proxy keeps binding its one configured port.
  `source_host:source_port` is matched against the incoming **Host header**; the
  proxy does **not** bind a new OS port per Site. Source `protocol` is
  stored/validated but cosmetic. **Destination** protocol/host/port fully drive
  the upstream scheme+authority.

## 3. Data model — migration `0009_sites`

Table `rre.sites`:

| column | type | constraints |
|---|---|---|
| `slug` | `VARCHAR(64)` | PRIMARY KEY, kebab-case |
| `name` | `VARCHAR(200)` | NOT NULL, UNIQUE |
| `source_protocol` | `VARCHAR(8)` | NOT NULL, CHECK in (`http`,`https`) |
| `source_host` | `VARCHAR(255)` | NOT NULL |
| `source_port` | `INTEGER` | NOT NULL, CHECK 1..65535 |
| `dest_protocol` | `VARCHAR(8)` | NOT NULL, CHECK in (`http`,`https`) |
| `dest_host` | `VARCHAR(255)` | NOT NULL |
| `dest_port` | `INTEGER` | NOT NULL, CHECK 1..65535 |
| `created_at` | `TIMESTAMPTZ` | NOT NULL DEFAULT now() |
| `updated_at` | `TIMESTAMPTZ` | NOT NULL DEFAULT now() |

Indexes:
- `sites_source_unique` UNIQUE on `(source_host, source_port)` — deterministic routing.
- `sites_created_at_idx` on `(created_at DESC, slug ASC)` — list pagination (mirror 0008).

`0009_sites.down.sql` drops the table (+ indexes).

## 4. Backend — copy the Feature resource template

Files (mirror `feature.rs` across each layer):
- `models/site.rs` — `Site` (`#[derive(sqlx::FromRow)]`), all columns.
- `schemas/site.rs` — `SiteCreate` / `SiteUpdate` / `SiteRead` (+ `From<Site>`).
- `repositories/site_repository.rs` — runtime SQLx (`query_as` + `AssertSqlSafe`,
  `COLS` const): `insert`, `list_paged(limit,offset,q)`, `count(q)`, `get(slug)`,
  `update(slug, ...)`, `delete(slug)`. `q` = optional case-insensitive `name ILIKE
  '%q%'` filter for the picker.
- `services/site_service.rs` — validate DTOs; map pg `23505` → `409` conflict.
- `api/v1/sites.rs` — `pub fn router()`; merge in `api/v1/mod.rs`.

Routes:
- `POST /api/v1/sites` → 201 `SiteRead`
- `GET /api/v1/sites?page&page_size&q` → 200 `Page<SiteRead>`
- `GET /api/v1/sites/{slug}` → 200 `SiteRead`
- `PATCH /api/v1/sites/{slug}` → 200 `SiteRead`
- `DELETE /api/v1/sites/{slug}` → 204

Validation (`SiteCreate`, `deny_unknown_fields`):
- `slug`: len 3..=64, regex `^[a-z0-9]+(?:-[a-z0-9]+)*$`
- `name`: len 1..=200
- `source_protocol`/`dest_protocol`: in {`http`,`https`}
- `source_host`/`dest_host`: len 1..=255
- `source_port`/`dest_port`: range 1..=65535

`SiteUpdate`: all fields optional except slug (slug is immutable / path-derived).

Conflict mapping: reuse the 409 conflict pattern. A `23505` on `sites` may hit the
slug PK or the name/source unique index — inspect the constraint name where easy;
otherwise return a single 409 `CONFLICT` with a message naming slug/name/source.

### 4b. Node manifest — add `site_match`

Add to the backend node-type manifest source (the JSON/struct served by
`GET /api/v1/node-types`; find via the `node_types` module/`config`):

```json
{
  "kind": "site_match",
  "label": "Site Match",
  "category": "request",
  "summary": "Branch on whether the request's site matches a chosen site",
  "applies_to": ["html", "json"],
  "node_kind": "decision",
  "fields": [
    { "name": "site", "label": "Site", "control": "site_select", "required": true,
      "placeholder": "Search sites by name…" }
  ],
  "output": { "branches": [ { "id": "yes" }, { "id": "no" } ] }
}
```

(Use the existing manifest's exact category ids — reuse a "request" category if one
exists, else add it. Match the existing field/branch field names exactly.)

## 5. Proxy

- **`infra/site_map.rs`** (new): on demand, `GET {backend_base_url}/api/v1/sites?page_size=100`,
  build `HashMap<String, Site>` keyed by normalized `source_host:source_port`
  (lowercase host). Cache with the same TTL mechanism as the active-version cache.
- **forwarder.rs**:
  - Parse incoming `Host` header → `host:port` (default port by request scheme:
    80/http, 443/https). Look up the Site. If found → upstream authority+scheme =
    `dest_protocol://dest_host:dest_port`. If not found → fall back to
    `settings.upstream_base_url` (preserves dev + tests).
  - When forwarding, set the upstream `Host` header to the destination host
    (existing code strips inbound `host`; set the new authority's host).
  - Tag the request's `EvaluationContext.site = Some(matched_slug)` (None when
    falling back).
  - **Remove** `feature_map.resolve_all(...)`. Instead fetch the feature list
    (`GET {backend_base_url}/api/v1/features?page_size=100`, cached w/ TTL) and run
    the existing per-feature pipeline for **every** feature id (active-version 404 →
    skip, as today).
- **Remove** `proxy/config/feature_map.yaml`, `proxy/src/infra/feature_map.rs`, and
  the `feature_map` field from `state.rs`/`lib.rs` wiring. Update any tests.
- **`domain/context.rs`**: add `pub site: Option<String>` to `EvaluationContext`;
  include `"site"` in `to_input_value()` projection.
- **`domain/processors/site_match.rs`** (new): `kind()="site_match"`; config
  `{ site: String }`; `evaluate` → `Branch::Yes` if `ctx.site.as_deref() ==
  Some(cfg.site)` else `Branch::No`. Register one line in `default_registry()`.
- **`eval.rs`** (`/__rre/eval`): accept an optional `site` in the test request
  context; thread into `EvaluationContext.site` so the Test panel can simulate it.

## 6. Frontend

- **`lib/api/sites.ts`**: zod `SiteRead`/`SiteCreate`/`SiteUpdate`;
  `listSites({page,page_size,q})`, `getSite`, `createSite`, `updateSite`,
  `deleteSite`, `searchSites(q)` (wraps list with `q`, small page_size).
- **Sites admin** at `app/products/sites/page.tsx` + `components/sites/SitesListClient.tsx`
  (+ create/edit modal) mirroring `FeaturesListClient`. Card shows
  `source_proto://source_host:source_port → dest_proto://dest_host:dest_port`.
  Add a **"Sites"** nav link next to Features.
- **`site_select` control**: add `"site_select"` to the `NodeFieldControl` zod enum
  in `lib/api/nodeTypes.ts`; add a case in `GenericNodeForm.tsx` rendering a
  searchable combobox (debounce 300ms, single-select, case-insensitive) that calls
  `searchSites` and stores the selected **slug** in the processor config. Show the
  selected site's name/slug as the current value. `manifest.ts` `fieldDisplayValue`
  handles `site_select` (display the stored slug).
- The `site_match` node then appears automatically in the palette + config drawer
  via the manifest.

## 7. Seed / de-dn

- `bin/seed_demo.rs`: seed a demo Site idempotently
  (`slug=demo-localhost`, `name="Demo (localhost:9000)"`, source `http localhost
  9000`, dest `http demo-upstream 8081`; dest host/port overridable via env
  `DEMO_UPSTREAM_HOST`/`DEMO_UPSTREAM_PORT` for native dev). Keep the demo rule
  feature.
- Rename feature ids `dn-article`→`demo-article`, `dn-json-article`→
  `demo-json-article`; grep-replace across `backend/`, `proxy/`, `frontend/`
  (tests, e2e, seed). Remove dn references from docs where they describe routing.

## 8. Out of scope (YAGNI)

- No HTTP 302 redirects; no per-site feature association (features stay global);
  no dynamic per-site OS port binding; no multi-select site picker; no auth/TLS
  termination changes.

## 9. Verification

- `make backend-check` / `make proxy-check` / `make frontend-check` all green
  (cargo prefixed with `PATH="$HOME/.cargo/bin:$PATH"`; backend tests may need
  `DOCKER_HOST` for testcontainers).
- New unit tests: site_repository CRUD + conflict; site_service validation;
  site_match processor (yes/no/none); site_map host normalization + match/fallback;
  frontend sites api + site_select render + SitesListClient.
- Manual: `make up` → demo Site routes `localhost:9000` → demo-upstream; paywall/
  regwall still applied; create a 2nd Site in UI; add a site-match node, pick a
  site, verify yes/no branch in the Test panel.

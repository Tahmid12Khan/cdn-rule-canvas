# Golden fixtures for `rre_core::apply`

These are the **cross-host parity contract**. `rre-core` is consumed by two
hosts — the Axum proxy in this repo (and its Test panel) and the Fastly Compute
Wasm guest in `dngroup-fastly` — and a rule has to mean the same thing in both.
Each file here pins one request through `rre_core::apply`: the exact output
body, the `changed` flag, and every `FeatureReport`.

`tests/apply.rs` builds its bundles as typed Rust values, so it breaks at
compile time on a rename but cannot see the WIRE format. These fixtures are
committed JSON parsed by the same `Deserialize` impls the bundle exporter
writes for, so a backend/proxy/edge schema skew fails here first.

Bodies are compared as **exact strings** — no whitespace normalisation. Drift in
`lol_html` injection, the `ammonia` allow-list, mustache escaping, or
`serde_json` key ordering is precisely what a golden exists to catch. JSON
output is currently key-sorted because `serde_json` is built without
`preserve_order`; enabling that feature anywhere in the graph would reorder
every JSON body and is a parity break, not a cosmetic one.

## What each file pins

| File | Pins |
| --- | --- |
| `01_json_article_trim_and_flag.json` | A JSON feature that matches and rewrites the body; two expression nodes applied in trace order. |
| `02_html_apply_component.json` | An HTML feature rendering a pre-resolved Component: mustache substitution, ammonia stripping a `<script>`, and the `lol_html` `replace` placement with its idempotency marker. |
| `03_applicability_skip_html.json` | `SkipReason::Applicability` — the version gate rejects the page before evaluation. |
| `04_no_match_device_type.json` | `SkipReason::NoMatch` — the decision's `no` branch dead-ends, so no expression node is reached. |
| `05a_identity_logged_out.json` / `05b_identity_has_product.json` | Identity routing. The SAME bundle, both branches pinned: no product → stripped + paywalled, product held → untouched + granted. |
| `06_bad_body_json.json` | `SkipReason::BadBody` — a truncated body returns byte-identical and never panics. |
| `07_chained_features_json.json` | Bundle-order chaining: the second feature's decision reads a field the first feature wrote, so it evaluates the RUNNING body, not the upstream one. |

## Adding a case

1. Copy the closest existing file. The runner (`tests/golden.rs`) uses
   `deny_unknown_fields`, so a typo'd key fails loudly rather than defaulting.
2. Fill in `description`, `body_kind`, `facts`, `bundle`, `body`.
3. Leave `expected.body` wrong on purpose and run
   `cargo test --manifest-path rre-core/Cargo.toml --test golden`. The failure
   prints the actual output.
4. **Read that output and satisfy yourself it is CORRECT**, not merely what the
   code happens to do today. Then paste it in. Pinning a bug turns it into a
   contract.

`facts.identity` is the RESOLVED identity: hosts run `identity::resolve` over
their own cookie/header names before calling `apply`, so a fixture states the
result rather than re-deriving it.

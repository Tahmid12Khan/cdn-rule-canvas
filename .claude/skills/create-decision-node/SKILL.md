---
name: create-decision-node
description: >
  Add a new decision-node type (a canvas processor that branches the rule graph)
  to the RRE app built on zen. Use this WHENEVER the user wants a new decision /
  condition / branch node, a new "processor", a new palette chip that routes the
  rule graph, or asks to "add a node type", "support deciding on X", "branch on
  cookies/headers/geo/path/time", or extend the visual rule builder with a new
  yes/no test. Also use when extending a decision beyond yes/no into multiple
  switch branches. Covers the full stack: proxy processor (Rust), the rule_graph
  schema mirror (backend + proxy), and the frontend palette/form. Reach for this
  even if the user only mentions the proxy or only the UI — a decision node is
  useless unless every layer agrees.
---

# Create a decision node

A **decision node** is a diamond on the rule-builder canvas that tests one thing
about the request/response and routes the graph down a branch. At eval time each
decision node becomes a zen **CustomNode** (runs your processor) feeding a zen
**SwitchNode** (routes on the processor's `branch` output). Adding one is a
trait impl + a few schema edits — **no zen engine changes**.

There are two shapes:

- **Binary (yes / no)** — the supported fast path. 99% of decision nodes. The
  translator already wires a 2-statement switch (`:yes` / `:no`) for every
  decision, so you only add a processor and register it. Follow **Part A**.
- **Multi-branch switch (3+ outcomes)** — e.g. routing on a value into
  `low` / `mid` / `high`. The `Branch` enum is hard-wired to `Yes`/`No` across
  every layer today, so this is a cross-cutting change. Follow
  `references/multi-branch-switch.md` AFTER you understand Part A.

Read `CONTRACTS.md` §6 (rule_graph schema) and §8 (proxy domain) first —
`CONTRACTS.md` wins over any `tasks/*.md` on conflict.

---

## Architecture (why each file exists)

```
canvas Decision{processor} ──translator──▶ CustomNode(kind) ─▶ SwitchNode(:yes/:no) ─▶ branch edges
        │                                       │
   ProcessorConfig                       ProcessorRegistry.get(kind).evaluate(config, ctx)
   (serde tag="type",                          │
    snake_case)                          returns ProcessorOutcome{ branch: Yes|No }
```

- The canvas `ProcessorConfig` discriminator `type` is **snake_case**
  (`meta_tags`, `device_type`). The JDM `CustomNode.kind` and the registry key
  are **camelCase** (`metaTags`, `deviceType`). `ProcessorConfig::kind_key()`
  bridges the two. Single-word types are identical in both (`cookie` → `cookie`).
- The translator (`proxy/src/domain/translator.rs`) is **processor-agnostic** —
  it reads `kind_key()` + `to_config_value()` and emits the fixed yes/no switch.
  A new binary processor needs **zero** translator edits.
- Processors are **pure + sync, read-only** over `EvaluationContext`. Never
  panic, never do I/O, never mutate. On bad config return `ProcessorError`; on a
  benign miss fail **open** to `Branch::No` with a `tracing::warn!`.

---

## Part A — binary (yes/no) decision node

Worked example below: a **Cookie** decision — "does request cookie `X` exist /
equal `Y`?". It reads `ctx.request_cookies`, which already exists, so it needs no
context plumbing. Substitute your own kind throughout. Legend: `cookie` = your
snake type, `Cookie` = PascalCase variant, `cookie` = camel kind key.

Touch these files, in order. Backend + proxy schema mirrors must stay **verbatim
identical** (the proxy `graph.rs` is a copy of the backend `rule_graph.rs`).

### 1. Backend schema — `backend/src/schemas/rule_graph.rs`

Add the variant to `ProcessorConfig` and any new operator/value enums. This is
the canonical §6 schema (note `ToSchema` derive — backend types carry it).

```rust
// inside pub enum ProcessorConfig { ... }
    /// Match against a request cookie.
    Cookie {
        /// Cookie name to read.
        cookie_name: String,
        /// How to compare.
        operator: CookieOperator,
        /// Operand (unused for `exists`).
        #[serde(default)]
        value: Option<String>,
    },
```

```rust
/// Comparison operator for the cookie processor.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CookieOperator {
    Equals,
    Exists,
}
```

Structural validation in `rule_graph_service` is processor-agnostic — a binary
processor needs no new validation rule.

### 2. Proxy schema mirror — `proxy/src/domain/graph.rs`

Paste the **same** variant + enums (drop the `ToSchema` derive — proxy types
don't carry it). Then add the `kind_key()` arm:

```rust
    pub fn kind_key(&self) -> &'static str {
        match self {
            ProcessorConfig::MetaTags { .. } => "metaTags",
            ProcessorConfig::DeviceType { .. } => "deviceType",
            ProcessorConfig::Cookie { .. } => "cookie", // camelCase if multi-word
        }
    }
```

`to_config_value()` needs no change — it serializes whatever variant is present.

### 3. Processor impl — `proxy/src/domain/processors/cookie.rs` (new file)

Mirror `device_type.rs` / `meta_tags.rs`. Pull fields from the JSON `config`
defensively (never `unwrap`), return `ProcessorOutcome { branch }`.

```rust
//! `CookieProcessor` (kind = "cookie"). Reads `ctx.request_cookies` and applies
//! the configured operator. A missing cookie fails open to `Branch::No`.

use serde_json::Value;

use crate::domain::context::EvaluationContext;

use super::{Branch, CanvasProcessor, ProcessorError, ProcessorOutcome};

#[derive(Debug)]
pub struct CookieProcessor;

impl CanvasProcessor for CookieProcessor {
    fn kind(&self) -> &'static str {
        "cookie"
    }

    fn evaluate(
        &self,
        config: &Value,
        ctx: &EvaluationContext,
    ) -> Result<ProcessorOutcome, ProcessorError> {
        let operator = config
            .get("operator")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing operator".to_string()))?;
        let cookie_name = config
            .get("cookie_name")
            .and_then(Value::as_str)
            .ok_or_else(|| ProcessorError::Config("missing cookie_name".to_string()))?;
        let want = config.get("value").and_then(Value::as_str);

        let present = ctx.request_cookies.get(cookie_name);

        let branch = match operator {
            "exists" => {
                if present.is_some() {
                    Branch::Yes
                } else {
                    tracing::warn!(cookie_name, "cookie_miss");
                    Branch::No
                }
            }
            "equals" => match (present, want) {
                (Some(c), Some(w)) if c == w => Branch::Yes,
                _ => Branch::No,
            },
            other => return Err(ProcessorError::Config(format!("unknown operator: {other}"))),
        };

        Ok(ProcessorOutcome { branch })
    }
}
```

**If your decision needs data not already on `EvaluationContext`** (e.g. geoIP,
time-of-day, a parsed query param), add a `Send + Sync` field to BOTH
`EvaluationContext` and the `EvaluationContextParts` carrier in `context.rs`, and
populate it in `from_request` / `into_context`. Do NOT hold `scraper::Html` or
any `!Send` type on the context — that breaks zen's `with_adapter`. The cookie
example needs none of this.

### 4. Register — `proxy/src/domain/processors/mod.rs`

```rust
pub mod cookie;        // add near the other `pub mod` lines
// ...
pub fn default_registry() -> ProcessorRegistry {
    let mut registry = ProcessorRegistry::new();
    registry.register(Arc::new(meta_tags::MetaTagsProcessor));
    registry.register(Arc::new(device_type::DeviceTypeProcessor));
    registry.register(Arc::new(cookie::CookieProcessor)); // <-- one line
    registry
}
```

### 5. Processor test — `proxy/tests/cookie_processor.rs` (new file)

Mirror `proxy/tests/device_type_processor.rs`: cover each operator, a match, a
miss (fail-open `No`), and an unknown operator (`Err`).

```rust
use std::collections::HashMap;

use http::HeaderMap;
use rre_proxy::domain::context::EvaluationContextParts;
use rre_proxy::domain::processors::{cookie::CookieProcessor, Branch, CanvasProcessor};
use serde_json::json;

fn ctx_with_cookie(name: &str, val: &str) -> rre_proxy::domain::context::EvaluationContext {
    let mut cookies = HashMap::new();
    cookies.insert(name.to_string(), val.to_string());
    EvaluationContextParts::from_request(
        &HeaderMap::new(),
        "/",
        &cookies,
        "<html></html>".to_string(),
    )
    .into_context()
}

#[test]
fn cookie_exists_matches() {
    let p = CookieProcessor;
    let ctx = ctx_with_cookie("sid", "abc");
    let cfg = json!({ "type": "cookie", "cookie_name": "sid", "operator": "exists" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::Yes);
}

#[test]
fn missing_cookie_fails_open_to_no() {
    let p = CookieProcessor;
    let ctx = ctx_with_cookie("sid", "abc");
    let cfg = json!({ "type": "cookie", "cookie_name": "other", "operator": "exists" });
    assert_eq!(p.evaluate(&cfg, &ctx).unwrap().branch, Branch::No);
}

#[test]
fn unknown_operator_errors() {
    let p = CookieProcessor;
    let ctx = ctx_with_cookie("sid", "abc");
    let cfg = json!({ "type": "cookie", "cookie_name": "sid", "operator": "regex" });
    assert!(p.evaluate(&cfg, &ctx).is_err());
}
```

### 6. Frontend type — `frontend/src/lib/canvas/types.ts`

Add to the `ProcessorConfig` union (and any operator/value literal types). Keep
field names + snake_case identical to the Rust serde.

```ts
export type CookieOperator = "equals" | "exists";

export type ProcessorConfig =
  | { type: "meta_tags"; tag_name: string; operator: MetaTagsOperator; value?: string | null }
  | { type: "device_type"; operator: DeviceOperator; value: DeviceValue }
  | { type: "cookie"; cookie_name: string; operator: CookieOperator; value?: string | null };
```

### 7. Frontend zod schema — `frontend/src/lib/canvas/processorSchemas.ts`

Add a schema and include it in the `processorSchema` union.

```ts
export const cookieOperator = z.enum(["equals", "exists"]);

export const cookieSchema = z
  .object({
    type: z.literal("cookie"),
    cookie_name: z.string().trim().min(1, "Enter the cookie name"),
    operator: cookieOperator,
    value: z.string().nullish(),
  })
  .superRefine((val, ctx) => {
    if (val.operator === "equals" && (!val.value || !val.value.trim())) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: "Enter a value, or switch the operator to 'exists'",
        path: ["value"],
      });
    }
  });

export const processorSchema = z.union([metaTagsSchema, deviceTypeSchema, cookieSchema]);
```

### 8. Palette chip + default — `frontend/src/lib/canvas/nodeTemplates.ts`

Add a `DEFAULT_*` config (intentionally incomplete so validation nudges the user
to open the config drawer) and an **enabled** chip in the right category.

```ts
export const DEFAULT_COOKIE: Extract<ProcessorConfig, { type: "cookie" }> = {
  type: "cookie",
  cookie_name: "",
  operator: "exists",
  value: "",
};

// add a chip to a category's `chips: [...]`
{
  id: "session:cookie",
  label: "Cookie",
  enabled: true,
  payload: { kind: "decision", processor: DEFAULT_COOKIE },
},
```

### 9. Config drawer form — `frontend/src/components/canvas/config/NodeConfigDrawer.tsx`

Add a render branch keyed on the new `type`, mirroring the existing
`initial?.type === "meta_tags"` / `"device_type"` blocks:

```tsx
{isDecision && initial?.type === "cookie" && (
  /* fields: cookie_name (text), operator (select), value (text, hidden when exists) */
)}
```

---

## Verify (run these — don't claim done without green output)

```bash
make proxy-check      # fmt + clippy + cargo test for the proxy crate
make backend-check    # schema mirror must still compile + serde round-trip
make frontend-check   # lint + typecheck + vitest
```

Targeted while iterating (cargo isn't on PATH in non-interactive shells):

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path proxy/Cargo.toml cookie
cd frontend && npm run typecheck && npm run test
```

End-to-end smoke (rules apply at the proxy): `make up`, then drop the new
decision node on the canvas at `/products/features`, publish, and hit
`http://localhost:9000/article.html` with a request that should match.

### Success checklist

- [ ] Backend + proxy `ProcessorConfig` variants are byte-for-byte identical (minus `ToSchema`).
- [ ] `kind_key()` arm added; camelCase if the snake type is multi-word.
- [ ] Processor registered in `default_registry()`.
- [ ] Processor never panics; bad config → `Err`, benign miss → `Branch::No` + warn.
- [ ] Frontend type, zod union, palette chip, and drawer form all include the new `type`.
- [ ] `make proxy-check && make backend-check && make frontend-check` all green.

---

## Multi-branch switch (3+ outcomes)

Going beyond yes/no touches the `Branch` enum in `processors/mod.rs`,
`graph.rs` (edge branch), the translator's hard-coded `:yes`/`:no` statements,
the `branch_unique` validation rule, and the frontend `Branch` type + edge
handles. Read **`references/multi-branch-switch.md`** for the full touch-point
map before starting — it is a deliberate, cross-cutting change, not a copy-paste.

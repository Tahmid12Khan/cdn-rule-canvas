# ZEN Engine Playground

A small web UI for authoring and testing [ZEN](https://github.com/gorules/zen) rules. The frontend lets you edit a JDM decision graph and a JSON context, then sends both to a Rust [axum](https://github.com/tokio-rs/axum) backend that wraps `zen-engine` and returns the evaluation result.

## Run

From the workspace root:

```sh
source "$HOME/.cargo/env"
cargo run -p zen-playground
```

Then open http://127.0.0.1:3000

## Time Gate custom rule

The Time Gate rule takes a picked datetime (`pickedTime`) and returns `isBefore`, which tells you whether the current time is before the picked time. It uses the ZEN expression:

```
date(pickedTime) > date('now')
```

It also returns a human-readable `message` plus the `nowEpoch` and `pickedEpoch` values for reference.

## Example rules

The `rules/` directory contains ready-to-load example graphs:

- `time-gate.json` — Compares a picked datetime against the current time and reports whether now is before it.
- `adult-check.json` — Returns whether `customer.age >= 18` and an `adult`/`minor` category.
- `shipping-fee.json` — A first-hit decision table that picks a shipping fee from `customer.country` and `cart.total`.
- `flow-traffic-light.json` — A switch-node flowchart that maps an input `color` to a Stop/Slow/Go action.
- `kitchen-sink.json` — A graph that exercises every node type (see "All node types example" below).

## Flowchart editor

Open http://127.0.0.1:3000/flow.html for a visual node + edge editor built with [React Flow](https://reactflow.dev). You drag nodes onto the canvas, connect their ports, edit each node's contents, then Evaluate a JSON input. The executed path is highlighted using the engine trace.

Switch nodes branch: each statement (condition) is its own output handle, and the statement with an empty condition is the else branch.

The seed graph is a traffic light: an input `color` flows into a switch that routes to Stop, Slow, or Go, then on to the response. Try a context of `{ "color": "red" }` versus `{ "color": "green" }` to see different branches light up.

## All node types example

`rules/kitchen-sink.json` is a single graph that uses every JDM node type: an **input** node, a **function** node (JS), an **expression** node, a **switch** node, a **decision table** node, and an **output** node. Selecting **"Kitchen Sink (all node types)"** in the editor's Example dropdown loads it.

The flow is:

```
Request (input)
  -> Enrich (function, JS)        derives `browser` + `isMobile` from `userAgent`
  -> Signals (expression)         maps browser/isMobile/total/isCheckout/isToday
  -> Route by browser (switch)    first-hit on `browser`
       chrome  -> Chrome pricing (decision table)   channel + discount by checkout/total
       firefox -> Firefox tag (expression)          channel='firefox', discount=5
       else    -> Other tag (expression)            channel='other', discount=0
  -> Response (output)
```

The function node normalizes the raw `userAgent` string into a `browser` signal (`firefox`, `chrome`, or `other`) plus an `isMobile` flag. The Signals expression node then projects the fields the rest of the graph needs, including an `isToday` check built from the `requestedAt` timestamp. The switch routes by browser using first-hit policy, where the branch with an empty condition is the else branch. The Chrome branch goes through a first-hit decision table that sets `channel` and `discount` based on whether `isCheckout` is true and whether `total > 1000`; the Firefox and Other branches are plain expression nodes.

Things to try in the JSON context:

- `userAgent` with **Chrome** (e.g. `"Mozilla/5.0 ... Chrome/120 Safari/537"`) routes through the Chrome pricing table.
- `userAgent` with **Firefox** (e.g. `"Mozilla/5.0 ... Firefox/121"`) routes to the Firefox tag.
- `userAgent` with **Safari** (e.g. `"Mozilla/5.0 ... Version/17 Safari/605"`) has neither `Firefox` nor `Chrome`, so it falls to the Other branch.
- A `url` containing `"checkout"` makes `isCheckout` true, hitting the checkout rows of the Chrome table.
- `cart.total` greater than `1000` combined with a checkout `url` hits the `chrome-vip` row (discount 15).

Example Chrome-VIP context:

```json
{
  "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36",
  "url": "https://shop.example.com/checkout",
  "cart": { "total": 1500 },
  "requestedAt": "2024-01-01T12:00:00Z"
}
```

## Preset nodes

The editor offers preset nodes you can drop onto the canvas. Each preset is grouped below with the exact, verified ZEN expression it uses.

**Date/Time** (field: `timestamp`):

| Preset | Expression |
| --- | --- |
| `beforeNow` | `date(timestamp) < date('now')` |
| `afterNow` | `date(timestamp) > date('now')` |
| `isToday` | `year(date(timestamp))==year(date('now')) and monthOfYear(date(timestamp))==monthOfYear(date('now')) and dayOfMonth(date(timestamp))==dayOfMonth(date('now'))` |
| `isWeekend` | `dayOfWeek(date(timestamp)) > 5` |
| `expiresWithin7d` | `date(expiresAt) > date('now') and date(expiresAt) < date('now') + duration('7d')` |

**Browser** (field: `userAgent`):

| Preset | Expression |
| --- | --- |
| `isChrome` | `contains(userAgent,'Chrome') and not contains(userAgent,'Edg')` |
| `isFirefox` | `contains(userAgent,'Firefox')` |
| `isSafari` | `contains(userAgent,'Safari') and not contains(userAgent,'Chrome')` |
| `isEdge` | `contains(userAgent,'Edg')` |
| `isMobile` | `contains(userAgent,'Mobile')` |
| Browser switch | branches `chrome` / `firefox` / `safari` / `else` |

**URL** (field: `url`):

| Preset | Expression |
| --- | --- |
| `url_contains` | `contains(url,'checkout')` |
| `url_startsWith` | `startsWith(url,'https')` |
| `url_matches` | `matches(url,'^https://[^/]+\.example\.com')` |

**Number**:

| Preset | Expression |
| --- | --- |
| `isHighValue` | `cart.total > 1000` |
| `inRange` | `amount >= 100 and amount <= 500` |

> Note: the date helpers (`date`, `year`, `monthOfYear`, `dayOfMonth`, `dayOfWeek`, `duration`) are **functions**, not methods. Method-style calls like `date(x).isToday()` do **not** work in expression nodes — use the function form shown above (e.g. `year(date(x))==year(date('now'))`).

## API

`POST /api/evaluate` accepts a JDM decision graph and an evaluation context:

```json
{ "jdm": ..., "context": ... }
```

and responds with:

```json
{ "ok": true, "result": ... }
```

The request also accepts an optional `"trace": true`, in which case the response includes a per-node `"trace"` map describing what each node produced during evaluation.

### curl example

Evaluate the Time Gate rule with a future `pickedTime`:

```sh
curl -s http://127.0.0.1:3000/api/evaluate \
  -H 'Content-Type: application/json' \
  -d "{
    \"jdm\": $(cat rules/time-gate.json),
    \"context\": { \"pickedTime\": \"2030-01-01T00:00:00Z\" }
  }"
```

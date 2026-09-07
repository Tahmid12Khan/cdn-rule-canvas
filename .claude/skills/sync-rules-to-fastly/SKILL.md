---
name: sync-rules-to-fastly
description: Use when publishing RRE rules to the Fastly Compute edge, or after changing rre-core — exports the rule bundle and re-vendors the engine into the dngroup-fastly repo, then verifies the edge still builds.
---

# Sync rules to the Fastly edge

Publishes this repo's rules to the Fastly Compute service in
`../dngroup-fastly` and refreshes the rule engine it vendors.

## When to use

- A rule was published in RRE and should now run at the POP.
- Anything under `rre-core/` changed (processors, appliers, the bundle format).
- The edge's tests fail with a type or schema mismatch against `rre-core`.

## What it does, and what it does not

The edge makes **no call to this backend**, at request time or at startup. The
`rre_export` flow compiles bundles into the Wasm binary with `include_str!`, so
deploying the Fastly service IS the publish step. This skill runs offline
against the local database and writes files into the other repo.

Two artifacts always move together:

| Artifact | Destination | Why |
|---|---|---|
| Exported bundle | `compute/intrafish-edge/rules/<site>.json` | the rules themselves |
| `rre-core` + zen `core/*` | `compute/intrafish-edge/vendor/` | the engine that reads them |

Syncing only one is the failure this skill exists to prevent: a bundle written
by a newer exporter deserializes cleanly into older structs and silently means
something different.

## Steps

1. **Confirm which site and environment.** Ask if the user did not say. Default
   is `--site intrafish --env production`. Getting this wrong publishes staging
   rules to real readers.

2. **Make sure the database is reachable.** The exporter reads it directly.
   `make dev` (or `docker compose ... up -d postgres`) plus `DATABASE_URL`.

3. **Run the script** from the repo root:

   ```bash
   DATABASE_URL=postgres://... scripts/sync-rules-to-fastly.sh --site intrafish --env production
   ```

   Useful flags: `--fastly-repo DIR` if the checkout is not `../dngroup-fastly`,
   `--skip-export` to re-vendor the engine only, `--skip-vendor` to re-export
   rules only. Prefer running both.

4. **Read the bundle diff before committing.** `git -C ../dngroup-fastly diff --stat`
   then look at the rules file. This is a rule change that will run on real
   readers, and nothing downstream reviews it. Show the user which features
   appear, disappear, or change version, and confirm before committing.

5. **Adding a NEW site is one manual edit.** `include_str!` needs a literal
   path, so a new `rules/<site>.json` also needs a line in `BUNDLES` in
   `compute/intrafish-edge/src/flows/rre_export/mod.rs`.

6. **Commit in the Fastly repo,** then tell the user to deploy. Do not deploy
   for them.

## Verification is part of the sync

The script runs the edge's full gate (`fmt --check`, `clippy -D warnings`,
`cargo test`, `cargo build --target wasm32-wasip1`). A failure means the sync
is **not finished** — the bundle and the vendored engine disagree. Fix it here
rather than reporting a successful sync with a broken build.

## Before pointing real readers at an RRE flow

The Zephr flow truncates the article teaser on every HTML response. The RRE
flows do not, unless an `html` feature in the bundle says so. Until such a
feature is published, HTML through an RRE flow leaks the full article body.
Check the bundle for one, or keep the A/B on the JSON content endpoint. See
`compute/intrafish-edge/rules/README.md`.

## Never hand-edit

Neither `rules/*.json` nor `vendor/` is a file to edit by hand. An edit that
parses but is wrong is applied to readers with no review of the rule it
encodes, and `vendor/RRE_CORE_SOURCE` would then name a commit that does not
describe what is there.

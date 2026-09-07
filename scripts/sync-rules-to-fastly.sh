#!/usr/bin/env bash
#
# Sync RRE rules and the shared rule engine into the Fastly Compute service.
#
# Two things travel together and MUST stay in step:
#   1. the exported rule bundles  -> compute/intrafish-edge/rules/
#   2. the rre-core + zen engine  -> compute/intrafish-edge/vendor/
# A bundle exported by a newer rre-core than the edge vendors can deserialize
# into the old structs and mean something different, so this script always
# refreshes both and stamps the source commit.
#
# Nothing here runs at request time. The edge NEVER calls this backend: a
# deploy of the Fastly service is the publish step.
#
# Usage:
#   scripts/sync-rules-to-fastly.sh [--fastly-repo DIR] [--site SLUG] [--env ENV]
#                                   [--skip-export] [--skip-vendor]
#
# --env is `live` or `staging`; --site is the site slug as the RRE database
# records it, which is not always the hostname.
#
# Requires DATABASE_URL unless --skip-export is given.

set -euo pipefail

ZEN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FASTLY_REPO="${FASTLY_REPO:-$(cd "$ZEN_ROOT/.." && pwd)/dngroup-fastly}"
SITE="intrafish"
ENVIRONMENT="live"
SKIP_EXPORT=0
SKIP_VENDOR=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fastly-repo) FASTLY_REPO="$2"; shift 2 ;;
    --site)        SITE="$2";        shift 2 ;;
    --env)         ENVIRONMENT="$2"; shift 2 ;;
    --skip-export) SKIP_EXPORT=1;    shift ;;
    --skip-vendor) SKIP_VENDOR=1;    shift ;;
    -h|--help)     sed -n '2,22p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

export PATH="$HOME/.cargo/bin:$PATH"

EDGE="$FASTLY_REPO/compute/intrafish-edge"
[[ -d "$EDGE" ]] || { echo "not a Fastly Compute checkout: $EDGE" >&2; exit 1; }

say() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

# --- 1. Export the bundle -----------------------------------------------------
# Runs against the RRE database on this machine and writes a file. It is the
# only step that touches the database, and it is offline with respect to the
# edge: no endpoint is exposed to Fastly and Fastly calls nothing.
if [[ "$SKIP_EXPORT" == "0" ]]; then
  say "Exporting $SITE ($ENVIRONMENT)"
  [[ -n "${DATABASE_URL:-}" ]] || {
    echo "DATABASE_URL is not set (or pass --skip-export)" >&2; exit 1; }
  # `--out`, not a `>` redirect: the shell truncates the target the moment it
  # opens it, so a failed export would leave an empty rules file that still
  # compiles and silently applies nothing. The exporter writes via a temp file
  # and renames, so the previous good bundle survives a failure intact.
  cargo run --quiet --manifest-path "$ZEN_ROOT/backend/Cargo.toml" \
      --bin export_bundle -- \
      --site "$SITE" --env "$ENVIRONMENT" --out "$EDGE/rules/$SITE.json"
  python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$EDGE/rules/$SITE.json"
  echo "wrote rules/$SITE.json"

  # `include_str!` needs a literal path, so a bundle nobody listed is dead
  # weight: exported, committed, deployed, and never consulted. That failure is
  # invisible at the POP — the flow just reports `no_bundle` — so it is caught
  # here instead.
  if ! grep -q "rules/$SITE.json" "$EDGE/src/flows/rre_export/mod.rs"; then
    echo "WARNING: rules/$SITE.json is NOT in the BUNDLES list in" >&2
    echo "         src/flows/rre_export/mod.rs, so the rre_export flow will" >&2
    echo "         never load it. Add the include_str! line before deploying." >&2
  fi
else
  say "Skipping export (--skip-export)"
fi

# --- 2. Refresh the vendored engine ------------------------------------------
# rsync with --delete: a file deleted upstream must disappear here too, or the
# edge keeps compiling a processor the authoring UI no longer offers.
if [[ "$SKIP_VENDOR" == "0" ]]; then
  say "Vendoring rre-core + zen core"
  # `tests/` is excluded on purpose. The vendored copy exists to be compiled
  # into Wasm; the golden parity suite belongs to the source repo, which is
  # where it is actually run, and vendoring it would ship fixtures that no
  # command here executes.
  rsync -a --delete --delete-excluded \
      --exclude 'target/' --exclude 'Cargo.lock' --exclude 'tests/' \
      "$ZEN_ROOT/rre-core/" "$EDGE/vendor/rre-core/"
  rsync -a --delete --exclude 'target/' \
      "$ZEN_ROOT/core/" "$EDGE/vendor/core/"

  # The vendored crates inherit `workspace = true` dependencies, so they need a
  # workspace root next to them. zen's own root serves, with rre-core added as
  # a member and rre-core's own `[workspace]` table stripped so it joins.
  python3 - "$ZEN_ROOT" "$EDGE" <<'PY'
import re, sys, pathlib
zen, edge = (pathlib.Path(p) for p in sys.argv[1:3])

ROOT_BANNER = '''# VENDORED from the RRE repo's workspace root. Present so the vendored
# `core/*` crates can inherit `workspace.dependencies`; `rre-core` is a member
# here rather than its own workspace root for the same reason.
#
# Do not hand-edit — regenerate with the `sync-rules-to-fastly` skill.
'''
CRATE_BANNER = '''# VENDORED copy. Unlike the source, this is a MEMBER of vendor/Cargo.toml's
# workspace (the `[workspace]` table was stripped) so the `core/*` crates it
# depends on can inherit their workspace dependencies.
#
# Do not hand-edit — regenerate with the `sync-rules-to-fastly` skill.
'''

root = (zen / "Cargo.toml").read_text()
if '"rre-core"' not in root:
    root = re.sub(r'(members\s*=\s*\[)', r'\1\n    "rre-core",', root, count=1)
(edge / "vendor" / "Cargo.toml").write_text(ROOT_BANNER + root)

manifest = edge / "vendor" / "rre-core" / "Cargo.toml"
text = manifest.read_text()
# Strip the source's own leading comment block and its standalone `[workspace]`
# table: here rre-core is a member of vendor/Cargo.toml, not a workspace root.
text = re.sub(r'\A(?:#[^\n]*\n)+', '', text)
text = re.sub(r'(?m)^\[workspace\]\s*\n', '', text)
manifest.write_text(CRATE_BANNER + text)
PY

  # The stamp names a commit, so an uncommitted change makes it a lie: the
  # vendored bytes would not be what that commit contains.
  if ! git -C "$ZEN_ROOT" diff --quiet -- rre-core core ||
     ! git -C "$ZEN_ROOT" diff --cached --quiet -- rre-core core; then
    echo "WARNING: rre-core/ or core/ has uncommitted changes." >&2
    echo "         RRE_CORE_SOURCE will name a commit that is NOT what was" >&2
    echo "         vendored. Commit here first, then re-run." >&2
  fi
  git -C "$ZEN_ROOT" rev-parse HEAD > "$EDGE/vendor/RRE_CORE_SOURCE"
  echo "vendored from $(cat "$EDGE/vendor/RRE_CORE_SOURCE")"
else
  say "Skipping vendor refresh (--skip-vendor)"
fi

# --- 3. Prove it still builds -------------------------------------------------
# The bundle and the engine only agree if the edge's own tests pass against
# them. Failing here means the sync is not done, not that the sync is finished
# and something else is broken.
say "Building and testing the edge"
( cd "$EDGE" && cargo fmt --check && cargo clippy --all-targets -- -D warnings \
                && cargo test && cargo build --target wasm32-wasip1 )

say "Done"
echo "Changed in $FASTLY_REPO:"
[[ "$SKIP_EXPORT" == "0" ]] && echo "  compute/intrafish-edge/rules/$SITE.json"
[[ "$SKIP_VENDOR" == "0" ]] && echo "  compute/intrafish-edge/vendor/"
cat <<'EOF'

Review the bundle diff before committing — it encodes rules that will run on
real readers. Then commit in the Fastly repo and deploy; the deploy IS the
publish step.
EOF

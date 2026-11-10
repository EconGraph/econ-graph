#!/usr/bin/env bash
# Show that the dependency audit gates fail on a known-vulnerable dependency:
# frontend/scripts/check-npm-audit.mjs (npm), cargo deny and cargo audit (Rust).
#
# Builds two throwaway projects in a temporary directory: an npm lockfile with
# minimist 1.2.5 (GHSA-xvch-5gv4-984h, Critical) and a Cargo lockfile with
# smallvec 1.6.0 (RUSTSEC-2021-0003). Nothing is installed or built; both
# audits read the lockfile only. The fixtures are generated here, not committed,
# so the repository's own dependency scanners never see them.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }

mkdir -p "$WORK/npm"
cat >"$WORK/npm/package.json" <<'JSON'
{ "name": "audit-gate-fixture", "private": true, "dependencies": { "minimist": "1.2.5" } }
JSON
(cd "$WORK/npm" && npm install --package-lock-only --ignore-scripts --no-audit --no-fund >/dev/null)
echo '[]' >"$WORK/allowlist.json"
if node "$REPO/frontend/scripts/check-npm-audit.mjs" "$WORK/npm" --allowlist "$WORK/allowlist.json" >"$WORK/npm.log" 2>&1; then
    cat "$WORK/npm.log"; fail "check-npm-audit.mjs passed a lockfile with minimist 1.2.5"
fi
grep -q 'GHSA-xvch-5gv4-984h' "$WORK/npm.log" || { cat "$WORK/npm.log"; fail "npm gate failed for another reason"; }
echo "ok: npm audit gate fails on minimist 1.2.5 (GHSA-xvch-5gv4-984h)"

mkdir -p "$WORK/cargo/src"
: >"$WORK/cargo/src/lib.rs"
cat >"$WORK/cargo/Cargo.toml" <<'TOML'
[package]
name = "audit-gate-fixture"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
smallvec = "=1.6.0"
TOML
(cd "$WORK/cargo" && cargo generate-lockfile -q)
# The same configuration the backend gate uses, so a broad ignore there shows up here.
if cargo deny --manifest-path "$WORK/cargo/Cargo.toml" --config "$REPO/backend/deny.toml" check advisories >"$WORK/cargo.log" 2>&1; then
    cat "$WORK/cargo.log"; fail "cargo deny passed a lockfile with smallvec 1.6.0"
fi
grep -q 'RUSTSEC-2021-0003' "$WORK/cargo.log" || { cat "$WORK/cargo.log"; fail "cargo deny failed for another reason"; }
echo "ok: cargo deny fails on smallvec 1.6.0 (RUSTSEC-2021-0003)"

if [ "${SKIP_CARGO_AUDIT:-}" = 1 ]; then
    echo "skipped: cargo audit (SKIP_CARGO_AUDIT=1)"
    exit 0
fi
# From backend/, so cargo audit reads backend/.cargo/audit.toml, the ignore list the gate uses.
if (cd "$REPO/backend" && cargo audit --file "$WORK/cargo/Cargo.lock") >"$WORK/audit.log" 2>&1; then
    cat "$WORK/audit.log"; fail "cargo audit passed a lockfile with smallvec 1.6.0"
fi
grep -q 'RUSTSEC-2021-0003' "$WORK/audit.log" || { cat "$WORK/audit.log"; fail "cargo audit failed for another reason"; }
echo "ok: cargo audit fails on smallvec 1.6.0 (RUSTSEC-2021-0003)"

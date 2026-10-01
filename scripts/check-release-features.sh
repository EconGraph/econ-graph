#!/usr/bin/env bash
# REL-3 "nothing fake" check (exit criterion 4), backend half: unfinished or dev-only backend
# code must not ship in a release build by default.
#
# `cargo build --release` (no --features) is what `backend/Dockerfile` runs, so whatever ends
# up in cargo's actual resolved feature set for each crate (not just its own `default = [...]`
# line) is what a release image gets: a feature can also be turned on transitively (another
# local feature enables it) or by a different workspace crate's dependency declaration
# unconditionally requesting it. This is a static, metadata-only check (no compile) for two
# known risks:
#   - econ-graph-sec-crawler's `xbrl-parser` feature (SEC-2): the XBRL parser, DTS download and
#     financial ratio calculator are unfinished. Always a cargo feature; must stay off.
#   - econ-graph-crawler / econ-graph-crawler-worker's dev-only static-catalog adapters
#     (DATA-1/FLAGS-4): ten hardcoded sources that can't fetch live data. Currently a cargo
#     feature, `static-catalogs`, that must stay off; FLAGS-4 replaces it with a build flag,
#     `static_catalogs`, gated by `cfg(flag_static_catalogs)` the same way `mcp` is (see
#     backend/crates/econ-graph-flags-build). This script checks whichever mechanism is
#     present, so it keeps working whichever of this PR and FLAGS-4 merges second.
# The Docker build arg that can turn the cargo feature on must also default to empty.
#
# A full-build confirmation that each feature is actually compiled out (not just absent from
# cargo's resolved feature set / off in the release flag profile) already runs as "Check the
# release flag profile" and "Crawler CLI in release mode registers only the live sources" in
# ci-core.yml; this check catches a Cargo.toml or flag-value regression before those slower
# jobs would.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

fail=0

# `cargo metadata` without --no-deps resolves the whole workspace's default build (same
# features cargo would activate for a plain `cargo build --release`, no --features), so this
# reflects a feature turned on transitively within a crate or by another crate's dependency
# declaration, not just a crate's own top-level `default = [...]` line.
metadata="$(cargo metadata --manifest-path backend/Cargo.toml --locked --format-version=1)"

resolved_features() {
  local crate="$1" ids count
  ids="$(echo "$metadata" | jq -r --arg name "$crate" '[.packages[] | select(.name == $name) | .id]')"
  count="$(echo "$ids" | jq 'length')"
  if [[ "$count" -ne 1 ]]; then
    echo "::error::expected exactly one workspace package named \"$crate\" in cargo metadata, found $count. Has it been renamed or split?" >&2
    return 1
  fi
  local id
  id="$(echo "$ids" | jq -r '.[0]')"
  echo "$metadata" | jq -r --arg id "$id" '.resolve.nodes[] | select(.id == $id) | .features[]'
}

check_cargo_feature_off_by_default() {
  local crate="$1" feature="$2"
  local active
  if ! active="$(resolved_features "$crate")"; then
    fail=1
    return
  fi
  if echo "$active" | grep -qx "$feature"; then
    echo "::error::$crate resolves with feature \"$feature\" active in a plain release build (cargo build --release, no --features) — directly, transitively through another feature, or forced on by another workspace crate's dependency declaration. See backend/crates/$crate/Cargo.toml and its dependents."
    fail=1
  else
    echo "ok: $crate does not resolve with feature \"$feature\" active"
  fi
}

check_build_flag_off_in_release() {
  local flag="$1" flags_file="backend/flags/release.json"
  if [[ ! -f "$flags_file" ]]; then
    echo "::error::$flags_file is missing; cannot check flag \"$flag\""
    fail=1
    return
  fi
  local entry kind value
  entry="$(jq -r --arg key "$flag" '.flags[$key] // "null"' "$flags_file")"
  if [[ "$entry" == "null" ]]; then
    echo "::error::$flags_file has no \"$flag\" flag to check (expected since its cargo feature is gone)."
    fail=1
    return
  fi
  kind="$(echo "$entry" | jq -r '.kind')"
  value="$(echo "$entry" | jq -r '.value')"
  if [[ "$kind" != "build" ]]; then
    echo "::error::flag \"$flag\" in $flags_file is kind \"$kind\", expected \"build\" (a non-build flag can't compile code out)."
    fail=1
  elif [[ "$value" != "false" ]]; then
    echo "::error::flag \"$flag\" in $flags_file is on in the release profile, so a release build compiles its code in."
    fail=1
  else
    echo "ok: build flag \"$flag\" is off in the release profile ($flags_file)"
  fi
}

check_cargo_feature_off_by_default econ-graph-sec-crawler xbrl-parser

# static-catalogs: check the cargo feature on whichever of econ-graph-crawler /
# econ-graph-crawler-worker still declares it, or the static_catalogs build flag once FLAGS-4
# has removed the feature from both. (Presence is a plain Cargo.toml lookup, not resolution:
# we need to know whether the feature exists at all before asking whether it's active.)
static_catalogs_crates_with_feature=()
for crate in econ-graph-crawler econ-graph-crawler-worker; do
  has_feature="$(echo "$metadata" | jq -r --arg name "$crate" '.packages[] | select(.name == $name) | (.features | has("static-catalogs"))')"
  if [[ "$has_feature" == "true" ]]; then
    static_catalogs_crates_with_feature+=("$crate")
    check_cargo_feature_off_by_default "$crate" static-catalogs
  fi
done
if [[ ${#static_catalogs_crates_with_feature[@]} -eq 0 ]]; then
  check_build_flag_off_in_release static_catalogs
fi

dockerfile="backend/Dockerfile"
if ! grep -qE '^ARG CARGO_FEATURES=""$' "$dockerfile"; then
  echo "::error::$dockerfile's CARGO_FEATURES build arg no longer defaults to empty, so a plain docker build could enable xbrl-parser or static-catalogs in a release image without anyone passing --build-arg."
  fail=1
else
  echo "ok: $dockerfile's CARGO_FEATURES build arg defaults to empty"
fi

exit $fail

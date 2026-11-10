#!/usr/bin/env bash
# The stub is a quoted heredoc and runs on its own.
# shellcheck disable=SC2016
#
# Tests for scripts/release/verify-image-provenance.sh against a stub gh that
# logs its arguments; nothing reaches GitHub or a registry:
#
#   - every image is verified by digest with --repo EconGraph/econ-graph, the
#     signer regex, GitHub's OIDC issuer and --deny-self-hosted-runners;
#   - a failed verification prints gh's output and fails, and later images
#     are not verified;
#   - a tag, a short digest or no argument is refused without calling gh;
#   - the signer regex accepts release.yml on main, release/* and rc tags, and
#     refuses other branches, final or malformed tags, other workflows, other
#     repositories, and look-alike names (checked with grep -E, which reads
#     this regex the same way as gh's Go regexp).
#
# Usage: scripts/release/test-verify-image-provenance.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
verify="${SCRIPT_DIR}/verify-image-provenance.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
failed=0

cat >"$WORK/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$STUB_LOG"
case "$3" in
    *"${STUB_FAIL_ON:-never-matches}"*)
        echo "Error: no attestations found for subject"
        exit 1 ;;
esac
echo "Loaded 1 attestation from GitHub API"
STUB
chmod +x "$WORK/gh"
export GH="$WORK/gh" STUB_LOG="$WORK/gh.log"

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1"
    [ -z "${2:-}" ] || printf '     %s\n' "$2"
    failed=1
}
run() {
    : >"$STUB_LOG"
    rc=0
    out="$("$verify" "$@" 2>&1)" || rc=$?
}

hex() { local s=""; for _ in $(seq 64); do s+="$1"; done; printf '%s' "$s"; }
backend="ghcr.io/econgraph/econ-graph-backend@sha256:$(hex a)"
frontend="ghcr.io/econgraph/econ-graph-frontend@sha256:$(hex b)"
regex="$("$verify" --print-signer-regex)"

run "$backend" "$frontend"
want_backend="attestation verify oci://${backend} --repo EconGraph/econ-graph --cert-identity-regex ${regex} --cert-oidc-issuer https://token.actions.githubusercontent.com --deny-self-hosted-runners"
if [ "$rc" -eq 0 ] && [ "$(wc -l <"$STUB_LOG" | tr -d ' ')" = 2 ] &&
    [ "$(sed -n 1p "$STUB_LOG")" = "$want_backend" ] &&
    grep -qF "attestation verify oci://${frontend} " "$STUB_LOG" &&
    grep -qF "${frontend}: signed provenance from release.yml" <<<"$out"; then
    ok "verifies every image by digest with the release identity"
else
    fail "verify two images (exit $rc)" "$out"$'\n'"$(cat "$STUB_LOG")"
fi

export STUB_FAIL_ON=econ-graph-backend
run "$backend" "$frontend"
if [ "$rc" -ne 0 ] && grep -qF "no attestations found" <<<"$out" &&
    grep -qF "${backend} has no valid build provenance" <<<"$out" &&
    ! grep -qF "econ-graph-frontend" "$STUB_LOG"; then
    ok "a failed verification fails and stops"
else
    fail "failed verification (exit $rc)" "$out"
fi
unset STUB_FAIL_ON

for bad in "ghcr.io/econgraph/econ-graph-backend:sha-$(hex c | cut -c1-40)" \
    "ghcr.io/econgraph/econ-graph-backend@sha256:abc" "ghcr.io/econgraph/econ-graph-backend:v4.2.0@sha256:"; do
    run "$bad"
    if [ "$rc" -ne 0 ] && grep -qF "verify by digest, never by a tag" <<<"$out" && [ ! -s "$STUB_LOG" ]; then
        ok "refuses ${bad##*/} without calling gh"
    else
        fail "refuses ${bad##*/} (exit $rc)" "$out"
    fi
done
run
if [ "$rc" -ne 0 ] && grep -qF "usage:" <<<"$out"; then ok "no argument is a usage error"; else fail "no argument (exit $rc)" "$out"; fi

wf="https://github.com/EconGraph/econ-graph/.github/workflows"
for san in "$wf/release.yml@refs/heads/main" "$wf/release.yml@refs/heads/release/v4.2" \
    "$wf/release.yml@refs/tags/v4.2.0-rc.1" \
    "$wf/release.yml@refs/tags/v10.20.30-rc.12"; do
    if grep -qE "$regex" <<<"$san"; then ok "signer regex accepts ${san#"$wf"/}"; else fail "signer regex refuses ${san}"; fi
done
for san in "$wf/release.yml@refs/heads/feature/x" "$wf/release.yml@refs/heads/mainline" \
    "$wf/release.yml@refs/heads/main/x" "$wf/release.yml@refs/heads/release-x" \
    "$wf/release.yml@refs/heads/release/" "$wf/release.yml@refs/heads/release/hotfix/x" "$wf/release.yml@refs/tags/v4.2.0" \
    "$wf/release.yml@refs/tags/v4.2.0-rc.1x" "$wf/release.yml@refs/tags/train-5" \
    "$wf/release.yml@refs/pull/12/merge" "$wf/ci-core.yml@refs/heads/main" \
    "$wf/releaseXyml@refs/heads/main" "$wf/release.yml.bak@refs/heads/main" \
    "https://github.com/someone/econ-graph/.github/workflows/release.yml@refs/heads/main" \
    "https://github.com/EconGraph/econ-graph-fork/.github/workflows/release.yml@refs/heads/main" \
    "https://github.com/EconGraph/econ-graph/.github/workflows/release.yml@refs/heads/main extra"; do
    if grep -qE "$regex" <<<"$san"; then fail "signer regex accepts ${san}"; else ok "signer regex refuses ${san#https://github.com/}"; fi
done

exit "$failed"

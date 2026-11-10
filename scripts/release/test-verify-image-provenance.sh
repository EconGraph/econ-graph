#!/usr/bin/env bash
# The stub is a quoted heredoc and runs on its own.
# shellcheck disable=SC2016
#
# Tests for scripts/release/verify-image-provenance.sh against a stub gh that
# logs its arguments; nothing reaches GitHub or a registry:
#
#   - every image is verified by digest with --repo EconGraph/econ-graph, the
#     signer regex, GitHub's OIDC issuer, --source-digest <commit> and
#     --deny-self-hosted-runners;
#   - a missing attestation or one that fails the policy (another signer,
#     another commit) exits 1 with "no valid build provenance", and later
#     images are not verified;
#   - gh not logged in, or an API, network or registry error, exits 3 with
#     "could not check" / "not logged in", never "no valid build provenance";
#   - a tag, a short digest, a missing or short --commit, or no image is a
#     usage error (exit 2) without calling gh;
#   - the signer regex accepts release.yml on main, release/<name> and rc
#     tags, and refuses other branches, final or malformed tags, other
#     workflows, other repositories, and look-alike names (checked with
#     grep -E, which reads this regex the same way as gh's Go regexp).
#
# Usage: scripts/release/test-verify-image-provenance.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
verify="${SCRIPT_DIR}/verify-image-provenance.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
failed=0

# STUB_AUTH_RC: exit status of `gh auth status`. STUB_FAIL_ON: an image name
# whose verification fails the way STUB_FAIL_AS says (missing, policy, api,
# unauthorized).
cat >"$WORK/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$STUB_LOG"
if [ "$1 $2" = "auth status" ]; then
    [ "${STUB_AUTH_RC:-0}" = 0 ] || echo "You are not logged into any GitHub hosts. To log in, run: gh auth login"
    exit "${STUB_AUTH_RC:-0}"
fi
case "$3" in
    *"${STUB_FAIL_ON:-never-matches}"*)
        case "${STUB_FAIL_AS:-missing}" in
            missing) echo "Error: failed to fetch attestations from EconGraph/econ-graph: HTTP 404: Not Found" ;;
            policy) printf '%s\n' "Verification failed" "Error: expected SourceRepositoryDigest to be abc, got def" ;;
            api) echo "Error: failed to fetch attestations from EconGraph/econ-graph: HTTP 503: Service Unavailable" ;;
            unauthorized) echo "Error: failed to fetch attestations from EconGraph/econ-graph: HTTP 401: Bad credentials" ;;
        esac
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
verifies() { grep -c '^attestation verify ' "$STUB_LOG" | tr -d ' '; }

hex() { local s=""; for _ in $(seq 64); do s+="$1"; done; printf '%s' "$s"; }
commit="$(hex d | cut -c1-40)"
backend="ghcr.io/econgraph/econ-graph-backend@sha256:$(hex a)"
frontend="ghcr.io/econgraph/econ-graph-frontend@sha256:$(hex b)"
regex="$("$verify" --print-signer-regex)"

run --commit "$commit" "$backend" "$frontend"
want_backend="attestation verify oci://${backend} --repo EconGraph/econ-graph --cert-identity-regex ${regex} --cert-oidc-issuer https://token.actions.githubusercontent.com --source-digest ${commit} --deny-self-hosted-runners"
if [ "$rc" -eq 0 ] && [ "$(verifies)" = 2 ] && grep -qxF "auth status" "$STUB_LOG" &&
    grep -qxF -- "$want_backend" "$STUB_LOG" &&
    grep -qF "attestation verify oci://${frontend} " "$STUB_LOG" &&
    grep -qF "${frontend}: signed provenance from release.yml for ${commit}" <<<"$out"; then
    ok "verifies every image by digest with the release identity and the commit"
else
    fail "verify two images (exit $rc)" "$out"$'\n'"$(cat "$STUB_LOG")"
fi

# expect NAME RC MESSAGE NOT_MESSAGE ARGS...: exit RC, prints MESSAGE, not NOT_MESSAGE.
expect() {
    local name="$1" want_rc="$2" message="$3" not_message="$4"
    shift 4
    run "$@"
    if [ "$rc" -eq "$want_rc" ] && grep -qF -- "$message" <<<"$out" &&
        { [ -z "$not_message" ] || ! grep -qF -- "$not_message" <<<"$out"; }; then
        ok "$name"
    else
        fail "$name (exit $rc, want $want_rc)" "$out"
    fi
}

bad="has no valid build provenance"
export STUB_FAIL_ON=econ-graph-backend
for mode in missing policy; do
    export STUB_FAIL_AS=$mode
    expect "a ${mode} attestation exits 1" 1 "${backend} ${bad} from release.yml on main, release/<name> or an rc tag for commit ${commit}" \
        "could not check" --commit "$commit" "$backend" "$frontend"
    [ "$(verifies)" = 1 ] || fail "a ${mode} attestation stops before the next image" "$(cat "$STUB_LOG")"
done
for mode in api unauthorized; do
    export STUB_FAIL_AS=$mode
    expect "an ${mode} error exits 3, not 'no provenance'" 3 "could not check the provenance of ${backend}" \
        "$bad" --commit "$commit" "$backend"
done
unset STUB_FAIL_ON STUB_FAIL_AS

export STUB_AUTH_RC=1
expect "gh not logged in exits 3 before verifying" 3 "gh is not logged in" "$bad" --commit "$commit" "$backend"
[ "$(verifies)" = 0 ] || fail "verified despite gh not being logged in"
unset STUB_AUTH_RC
GH="$WORK/no-such-gh" expect "no gh exits 3" 3 "needs gh on PATH" "$bad" --commit "$commit" "$backend"

for bad_ref in "ghcr.io/econgraph/econ-graph-backend:sha-${commit}" \
    "ghcr.io/econgraph/econ-graph-backend@sha256:abc" "ghcr.io/econgraph/econ-graph-backend:v4.2.0@sha256:"; do
    run --commit "$commit" "$bad_ref"
    if [ "$rc" -eq 2 ] && grep -qF "verify by digest, never by a tag" <<<"$out" && [ ! -s "$STUB_LOG" ]; then
        ok "refuses ${bad_ref##*/} without calling gh"
    else
        fail "refuses ${bad_ref##*/} (exit $rc)" "$out"
    fi
done
for args in "" "$backend" "--commit $commit" "--commit abc123 $backend" "--commit"; do
    # shellcheck disable=SC2086 # split on purpose
    run $args
    if [ "$rc" -eq 2 ] && grep -qF "usage:" <<<"$out" && [ ! -s "$STUB_LOG" ]; then
        ok "usage error for '${args:0:20}'"
    else
        fail "usage error for '${args}' (exit $rc)" "$out"
    fi
done

wf="https://github.com/EconGraph/econ-graph/.github/workflows"
for san in "$wf/release.yml@refs/heads/main" "$wf/release.yml@refs/heads/release/v4.2" \
    "$wf/release.yml@refs/tags/v4.2.0-rc.1" "$wf/release.yml@refs/tags/v10.20.30-rc.12"; do
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

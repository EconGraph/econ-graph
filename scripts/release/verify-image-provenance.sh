#!/usr/bin/env bash
#
# Refuse an image that release.yml did not build, from a reviewed ref, out of
# the expected commit.
#
# Usage: scripts/release/verify-image-provenance.sh --commit SHA IMAGE@sha256:DIGEST...
#
# For each image, by digest, `gh attestation verify` must find a build
# provenance attestation that:
#   - is signed by Sigstore with a GitHub Actions OIDC certificate (keyless:
#     no key to leak or rotate);
#   - was made by .github/workflows/release.yml in EconGraph/econ-graph, run
#     from refs/heads/main, a refs/heads/release/<name> branch or a
#     vX.Y.Z-rc.N tag (RELEASE_SIGNER_REGEX below); a run of a copy of the
#     workflow on another branch, another workflow or another repository fails;
#   - names SHA as its source commit (--source-digest). release.yml builds
#     exactly the commit its run is for, so this is the commit in the image;
#   - was made on a GitHub-hosted runner.
# The certificate fields are set by GitHub from the run's OIDC token, so the
# workflow itself cannot forge them.
#
# release.yml runs this before adopting an image that already has its sha
# tag, before giving a new digest its sha tag, and before retagging the
# rc-built images as the release; scripts/deploy/load-release-images.sh runs
# it before pulling anything.
#
# Exit status:
#   0  every image has valid provenance;
#   1  an image has none, or none that passes (missing attestation, another
#      signer or ref, another commit): look at the image, do not use it;
#   2  usage error;
#   3  the check could not run (gh missing or not logged in, an API, network
#      or registry error): nothing is known about the image; fix that and
#      re-run.
#
# Needs gh (logged in, or GH_TOKEN set) and read access to the registry (an
# existing `docker login ghcr.io`). GH overrides the gh binary, for tests.

set -euo pipefail

RELEASE_REPO="EconGraph/econ-graph"
# The certificate's SubjectAlternativeName: the signing workflow and the ref
# it ran from. Anchored at both ends. release/<name> has no slash, like the
# release ruleset's refs/heads/release/*.
RELEASE_SIGNER_REGEX='^https://github\.com/EconGraph/econ-graph/\.github/workflows/release\.yml@refs/(heads/main|heads/release/[^/[:space:]]+|tags/v[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+)$'
GH="${GH:-gh}"

usage() {
    echo "verify-image-provenance: usage: $0 --commit SHA IMAGE@sha256:DIGEST..." >&2
    exit 2
}
no_provenance() {
    echo "verify-image-provenance: $*" >&2
    exit 1
}
cannot_check() {
    echo "verify-image-provenance: $*" >&2
    exit 3
}

if [ "${1:-}" = --print-signer-regex ]; then
    printf '%s\n' "$RELEASE_SIGNER_REGEX"
    exit 0
fi
[ "${1:-}" = --commit ] && [ $# -ge 3 ] || usage
commit="$2"
shift 2
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || usage
for ref in "$@"; do
    [[ "$ref" =~ ^[^@[:space:]]+@sha256:[0-9a-f]{64}$ ]] || {
        echo "verify-image-provenance: '${ref}' is not IMAGE@sha256:<64 hex>; verify by digest, never by a tag" >&2
        exit 2
    }
done

command -v "$GH" >/dev/null 2>&1 || cannot_check "needs gh on PATH; the provenance check did not run"
if ! auth_out="$("$GH" auth status 2>&1)"; then
    printf '%s\n' "$auth_out" >&2
    cannot_check "gh is not logged in (run gh auth login, or set GH_TOKEN); the provenance check did not run"
fi

for ref in "$@"; do
    if out="$("$GH" attestation verify "oci://${ref}" \
        --repo "$RELEASE_REPO" \
        --cert-identity-regex "$RELEASE_SIGNER_REGEX" \
        --cert-oidc-issuer https://token.actions.githubusercontent.com \
        --source-digest "$commit" \
        --deny-self-hosted-runners 2>&1)"; then
        echo "verify-image-provenance: ${ref}: signed provenance from release.yml for ${commit}"
        continue
    fi
    printf '%s\n' "$out" >&2
    # What gh prints when it looked and the image failed: no attestation for
    # the digest (the attestations API's 404), or attestations that don't pass
    # the policy ("Verification failed", "expected <field> to be ..."). Anything
    # else (401/403, 5xx, rate limits, network, a registry 404 for the image
    # itself) means the check failed, which says nothing about the image.
    if grep -qiE 'no attestations found|fetch(ing)? attestations[^[:cntrl:]]*HTTP 404|verification failed|expected [[:alnum:]_.]+ to (be|match)' <<<"$out"; then
        no_provenance "${ref} has no valid build provenance from release.yml on main, release/<name> or an rc tag for commit ${commit}; refusing it"
    fi
    cannot_check "could not check the provenance of ${ref} (see gh's output above); nothing is known about the image; fix the cause and re-run"
done

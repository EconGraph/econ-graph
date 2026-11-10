#!/usr/bin/env bash
#
# Refuse an image that release.yml did not build from a reviewed ref.
#
# Usage: scripts/release/verify-image-provenance.sh IMAGE@sha256:DIGEST...
#
# For each image, by digest, `gh attestation verify` must find a build
# provenance attestation that:
#   - is signed by Sigstore with a GitHub Actions OIDC certificate (keyless:
#     no key to leak or rotate);
#   - was made by .github/workflows/release.yml in EconGraph/econ-graph, run
#     from refs/heads/main, a refs/heads/release/* branch or a
#     vX.Y.Z-rc.N tag (RELEASE_SIGNER_REGEX below); a run of a copy of the
#     workflow on another branch, another workflow or another repository fails;
#   - was made on a GitHub-hosted runner.
# The certificate fields are set by GitHub from the run's OIDC token, so the
# workflow itself cannot forge them. Which commit the image holds is checked
# separately: release.yml sets org.opencontainers.image.revision, inside the
# digest, and scripts/deploy/load-release-images.sh compares it to the tag's
# commit.
#
# release.yml runs this before adopting an image that already has its sha
# tag, and before retagging the rc-built images as the release;
# scripts/deploy/load-release-images.sh runs it before pulling anything.
#
# Needs gh (logged in, or GH_TOKEN set) and read access to the registry (an
# existing `docker login ghcr.io`). GH overrides the gh binary, for tests.

set -euo pipefail

RELEASE_REPO="EconGraph/econ-graph"
# The certificate's SubjectAlternativeName: the signing workflow and the ref
# it ran from. Anchored at both ends.
RELEASE_SIGNER_REGEX='^https://github\.com/EconGraph/econ-graph/\.github/workflows/release\.yml@refs/(heads/main|heads/release/[^/[:space:]]+|tags/v[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+)$'
GH="${GH:-gh}"

die() {
    echo "verify-image-provenance: $*" >&2
    exit 1
}

if [ "${1:-}" = --print-signer-regex ]; then
    printf '%s\n' "$RELEASE_SIGNER_REGEX"
    exit 0
fi
[ $# -ge 1 ] || die "usage: $0 IMAGE@sha256:DIGEST..."
command -v "$GH" >/dev/null 2>&1 || die "needs gh on PATH"

for ref in "$@"; do
    [[ "$ref" =~ ^[^@[:space:]]+@sha256:[0-9a-f]{64}$ ]] ||
        die "'${ref}' is not IMAGE@sha256:<64 hex>; verify by digest, never by a tag"
    if ! out="$("$GH" attestation verify "oci://${ref}" \
        --repo "$RELEASE_REPO" \
        --cert-identity-regex "$RELEASE_SIGNER_REGEX" \
        --cert-oidc-issuer https://token.actions.githubusercontent.com \
        --deny-self-hosted-runners 2>&1)"; then
        printf '%s\n' "$out" >&2
        die "${ref} has no valid build provenance from release.yml on main, release/* or an rc tag; refusing it"
    fi
    echo "verify-image-provenance: ${ref}: signed provenance from release.yml"
done

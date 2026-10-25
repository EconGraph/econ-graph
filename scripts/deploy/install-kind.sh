#!/bin/bash
#
# Install the kind CLI the deploy scripts use (KIND_VERSION in
# scripts/deploy/lib/kind-version.sh) for the current user: the official
# release binary, checked against the SHA-256 pinned below, at
#
#   ${XDG_DATA_HOME:-$HOME/.local/share}/econgraph/bin/kind-<version>
#
# with a `kind` symlink next to it. Run it once per user and again after
# KIND_VERSION changes. It needs no sudo, refuses to run as root, and leaves
# any other kind (/usr/bin/kind from a distribution package, say) alone; the
# deploy scripts run this copy whatever is on PATH.
#
# Idempotent: when the binary is already installed and its checksum matches,
# nothing is downloaded or changed.
#
# Usage: scripts/deploy/install-kind.sh [--dry-run]
#   --dry-run  print what would be downloaded and installed; change nothing.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-version.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-version.sh"

# SHA-256 of the release binaries, as published with the release:
#   https://github.com/kubernetes-sigs/kind/releases/download/v0.33.0/kind-linux-amd64.sha256sum
#   https://github.com/kubernetes-sigs/kind/releases/download/v0.33.0/kind-linux-arm64.sha256sum
declare -A KIND_SHA256=(
    [v0.33.0/linux-amd64]="aee6151561422756b764a4ae28e7f44cda5af5a9eead3cc9985112b1de8d8e0d"
    [v0.33.0/linux-arm64]="20022bee6cfcd5086cb7234d218e3454e6090022f2a8f55d1fa7fcf42c3867a2"
)
KIND_RELEASES="https://github.com/kubernetes-sigs/kind/releases/download"

DRY_RUN=false
usage="usage: $0 [--dry-run]"
for arg in "$@"; do
    case "$arg" in
        --dry-run | -n) DRY_RUN=true ;;
        -h | --help) echo "$usage"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done

die() {
    echo "error: $*" >&2
    exit 1
}

[ "$(id -u)" -ne 0 ] ||
    die "run this as the user who runs the deploy scripts, not as root; it installs into that user's ${KIND_BIN_DIR}"

case "$(uname -s)" in
    Linux) os=linux ;;
    *) die "unsupported OS $(uname -s); this installs the Linux release of kind" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=amd64 ;;
    aarch64 | arm64) arch=arm64 ;;
    *) die "unsupported CPU architecture $(uname -m)" ;;
esac
platform="${os}-${arch}"
expected="${KIND_SHA256[${KIND_VERSION}/${platform}]:-}"
[ -n "$expected" ] || die "no checksum pinned for kind ${KIND_VERSION} ${platform} in $0"
url="${KIND_RELEASES}/${KIND_VERSION}/kind-${platform}"
link="${KIND_BIN_DIR}/kind"
target="$(basename "${KIND_PINNED_BIN}")"

sha256_of() { sha256sum "$1" | cut -d' ' -f1; }

link_ok() { [ -L "$link" ] && [ "$(readlink "$link")" = "$target" ]; }

# Already installed and intact?
if [ -f "${KIND_PINNED_BIN}" ] && [ -x "${KIND_PINNED_BIN}" ] &&
    [ "$(sha256_of "${KIND_PINNED_BIN}")" = "$expected" ]; then
    if link_ok; then
        echo "kind ${KIND_VERSION} is already installed at ${KIND_PINNED_BIN} (checksum matches); nothing to do"
        exit 0
    fi
    if [ "$DRY_RUN" = true ]; then
        echo "kind ${KIND_VERSION} is already installed at ${KIND_PINNED_BIN} (checksum matches)"
        echo "+ ln -sfn ${target} ${link}"
        exit 0
    fi
    ln -sfn "$target" "$link"
    echo "kind ${KIND_VERSION} is already installed at ${KIND_PINNED_BIN}; ${link} now points to it"
    exit 0
fi

if [ -e "${KIND_PINNED_BIN}" ]; then
    echo "${KIND_PINNED_BIN} exists but is not kind ${KIND_VERSION} ${platform} (checksum differs, or not executable); replacing it"
fi

if [ "$DRY_RUN" = true ]; then
    echo "+ mkdir -p ${KIND_BIN_DIR}"
    echo "+ curl ${url}"
    echo "+ check its SHA-256 is ${expected}"
    echo "+ install it as ${KIND_PINNED_BIN}"
    echo "+ ln -sfn ${target} ${link}"
    echo "Dry run: nothing was changed"
    exit 0
fi

mkdir -p "${KIND_BIN_DIR}"
# Downloaded next to its final path, so the rename below is atomic.
tmp="$(mktemp "${KIND_BIN_DIR}/.kind-download.XXXXXX")"
trap 'rm -f "$tmp"' EXIT

echo "Downloading ${url}"
curl -fsSL --proto '=https' --tlsv1.2 --retry 3 -o "$tmp" "$url" ||
    die "downloading ${url} failed; nothing was installed"
actual="$(sha256_of "$tmp")"
[ "$actual" = "$expected" ] ||
    die "checksum mismatch for ${url}: got ${actual}, expected ${expected}; nothing was installed"

chmod 0755 "$tmp"
mv -f "$tmp" "${KIND_PINNED_BIN}"
trap - EXIT
ln -sfn "$target" "$link"
echo "Installed kind ${KIND_VERSION} at ${KIND_PINNED_BIN} (SHA-256 ${expected})"
echo "The deploy scripts (scripts/deploy/lib/kind-env.sh) use it from there."

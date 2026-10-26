#!/bin/bash
#
# Check scripts/deploy/install-kind.sh and the kind resolver in
# scripts/deploy/lib/kind-env.sh, offline and without root:
#
#   - --dry-run downloads and writes nothing;
#   - an install puts kind-<version> and the `kind` symlink in place, and a
#     second run downloads nothing (as does a run that only has to restore
#     the symlink);
#   - a download whose SHA-256 differs from the pinned one is refused and
#     nothing is installed;
#   - root is refused;
#   - with the pinned kind missing, the resolver refuses everything but a
#     read-only `kind get`, which falls back to a kind on PATH with a warning;
#   - require_kind_version accepts the pinned kind and refuses an old one.
#
#   - each supported platform (Linux and macOS, amd64 and arm64) has its own
#     release URL and pinned checksum;
#   - a caller's own kind() (deploy.sh --dry-run's stub) is kept and used.
#
# curl, sha256sum, id and uname are stubs on PATH: curl "downloads" a fake
# kind, sha256sum reports FAKE_SHA256 when it is set, id and uname report root
# or another platform when asked to. Usage:
# scripts/deploy/test-install-kind.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INSTALL="${PROJECT_ROOT}/scripts/deploy/install-kind.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# shellcheck source=scripts/deploy/lib/kind-version.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-version.sh"

REAL_SHA256SUM="$(type -P sha256sum)"
STUBS="${WORK}/stubs"
mkdir -p "$STUBS" "${WORK}/old-kind"
export CURL_LOG="${WORK}/curl.log" FAKE_RELEASE="${WORK}/fake-kind" REAL_SHA256SUM
: >"$CURL_LOG"

# fake_kind FILE VERSION: a "kind" that only answers `version`.
fake_kind() {
    printf '#!/bin/sh\necho "kind %s go1.26 linux/amd64 $*"\n' "$2" >"$1"
    chmod 0755 "$1"
}
fake_kind "$FAKE_RELEASE" "${KIND_VERSION}"
fake_kind "${WORK}/old-kind/kind" v0.30.0

cat >"${STUBS}/curl" <<'EOF'
#!/bin/bash
out="" url=""
while [ $# -gt 0 ]; do
    case "$1" in
        -o) out="$2"; shift 2 ;;
        -*) shift ;;
        *) url="$1"; shift ;;
    esac
done
echo "$url" >>"$CURL_LOG"
cp "$FAKE_RELEASE" "$out"
EOF
cat >"${STUBS}/sha256sum" <<'EOF'
#!/bin/bash
if [ -n "${FAKE_SHA256:-}" ]; then
    echo "${FAKE_SHA256}  $1"
else
    exec "$REAL_SHA256SUM" "$@"
fi
EOF
cat >"${STUBS}/id" <<'EOF'
#!/bin/bash
if [ "${FAKE_ROOT:-}" = 1 ] && [ "${1:-}" = -u ]; then echo 0; else exec "$REAL_ID" "$@"; fi
EOF
cat >"${STUBS}/uname" <<'EOF'
#!/bin/bash
case "${1:-}" in
    -s) [ -n "${FAKE_UNAME_S:-}" ] && { echo "$FAKE_UNAME_S"; exit 0; } ;;
    -m) [ -n "${FAKE_UNAME_M:-}" ] && { echo "$FAKE_UNAME_M"; exit 0; } ;;
esac
exec "$REAL_UNAME" "$@"
EOF
REAL_ID="$(type -P id)" REAL_UNAME="$(type -P uname)"
export REAL_ID REAL_UNAME
chmod 0755 "${STUBS}"/*
export PATH="${STUBS}:${PATH}"

failed=0
pass() { echo "ok   $1"; }
fail() {
    echo "FAIL $1"
    [ -n "${2:-}" ] && printf '     %s\n' "$2"
    failed=1
}
downloads() { wc -l <"$CURL_LOG" | tr -d ' '; }

# run_install DATA_HOME ARGS...: run install-kind.sh with XDG_DATA_HOME=DATA_HOME;
# sets OUT and RC.
run_install() {
    local data="$1"
    shift
    OUT="$(XDG_DATA_HOME="$data" "$INSTALL" "$@" 2>&1)"
    RC=$?
}

# --- install-kind.sh --------------------------------------------------------------

data="${WORK}/data"
bin="${data}/econgraph/bin"

run_install "$data" --dry-run
pinned_sha="$(sed -n 's/^+ check its SHA-256 is \([0-9a-f]\{64\}\)$/\1/p' <<<"$OUT")"
if [ "$RC" -eq 0 ] && [ -n "$pinned_sha" ] && [ ! -e "$data" ] && [ "$(downloads)" = 0 ]; then
    pass "--dry-run prints the plan and changes nothing"
else
    fail "--dry-run prints the plan and changes nothing (exit $RC)" "$OUT"
fi
export FAKE_SHA256="$pinned_sha"

run_install "$data"
if [ "$RC" -eq 0 ] && [ -x "${bin}/kind-${KIND_VERSION}" ] &&
    [ "$(readlink "${bin}/kind")" = "kind-${KIND_VERSION}" ] && [ "$(downloads)" = 1 ] &&
    grep -qx "https://github.com/kubernetes-sigs/kind/releases/download/${KIND_VERSION}/kind-linux-$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')" "$CURL_LOG"; then
    pass "installs kind-${KIND_VERSION} and the kind symlink"
else
    fail "installs kind-${KIND_VERSION} and the kind symlink (exit $RC)" "$OUT"
fi

run_install "$data"
if [ "$RC" -eq 0 ] && grep -q "nothing to do" <<<"$OUT" && [ "$(downloads)" = 1 ]; then
    pass "a second run downloads nothing"
else
    fail "a second run downloads nothing (exit $RC)" "$OUT"
fi

run_install "$data" --dry-run
if [ "$RC" -eq 0 ] && grep -q "nothing to do" <<<"$OUT" && [ "$(downloads)" = 1 ]; then
    pass "--dry-run on an installed kind says there is nothing to do"
else
    fail "--dry-run on an installed kind says there is nothing to do (exit $RC)" "$OUT"
fi

rm "${bin}/kind"
run_install "$data"
if [ "$RC" -eq 0 ] && [ "$(readlink "${bin}/kind")" = "kind-${KIND_VERSION}" ] && [ "$(downloads)" = 1 ]; then
    pass "a missing symlink is restored without a download"
else
    fail "a missing symlink is restored without a download (exit $RC)" "$OUT"
fi

data2="${WORK}/data-mismatch"
FAKE_SHA256="$(printf '0%.0s' {1..64})" run_install "$data2"
if [ "$RC" -ne 0 ] && grep -q "checksum mismatch" <<<"$OUT" &&
    [ ! -e "${data2}/econgraph/bin/kind-${KIND_VERSION}" ] && [ ! -e "${data2}/econgraph/bin/kind" ] &&
    [ -z "$(find "${data2}" -name '.kind-download.*')" ]; then
    pass "a download with the wrong checksum is refused and nothing is installed"
else
    fail "a download with the wrong checksum is refused and nothing is installed (exit $RC)" "$OUT"
fi

data3="${WORK}/data-root"
FAKE_ROOT=1 run_install "$data3"
if [ "$RC" -ne 0 ] && grep -q "not as root" <<<"$OUT" && [ ! -e "$data3" ]; then
    pass "refuses to run as root"
else
    fail "refuses to run as root (exit $RC)" "$OUT"
fi
unset FAKE_SHA256

seen=""
for platform in Linux:x86_64:linux-amd64 Linux:aarch64:linux-arm64 Darwin:x86_64:darwin-amd64 Darwin:arm64:darwin-arm64; do
    IFS=: read -r os machine name <<<"$platform"
    FAKE_UNAME_S="$os" FAKE_UNAME_M="$machine" run_install "${WORK}/data-${name}" --dry-run
    sha="$(sed -n 's/^+ check its SHA-256 is \([0-9a-f]\{64\}\)$/\1/p' <<<"$OUT")"
    if [ "$RC" -eq 0 ] && [ -n "$sha" ] && ! grep -qw "$sha" <<<"$seen" &&
        grep -qx "+ curl https://github.com/kubernetes-sigs/kind/releases/download/${KIND_VERSION}/kind-${name}" <<<"$OUT"; then
        pass "${os} ${machine} gets kind-${name} with its own pinned checksum"
    else
        fail "${os} ${machine} gets kind-${name} with its own pinned checksum (exit $RC)" "$OUT"
    fi
    seen+=" $sha"
done

FAKE_UNAME_S=FreeBSD run_install "${WORK}/data-freebsd" --dry-run
if [ "$RC" -ne 0 ] && grep -q "unsupported OS FreeBSD" <<<"$OUT"; then
    pass "refuses an unsupported OS"
else
    fail "refuses an unsupported OS (exit $RC)" "$OUT"
fi

# --- the resolver in kind-env.sh ----------------------------------------------------

# A PATH with only the tools kind-env.sh needs, so no kind the host (or CI
# runner) has installed is found.
SYSBIN="${WORK}/sysbin"
mkdir -p "$SYSBIN"
for tool in bash dirname awk sort head grep sed cat; do
    ln -s "$(type -P "$tool")" "${SYSBIN}/${tool}"
done

# in_kind_env DATA_HOME PATH_PREFIX COMMAND: run COMMAND with kind-env.sh
# sourced, XDG_DATA_HOME=DATA_HOME and PATH_PREFIX (may be empty) first on a
# PATH that otherwise has no kind; sets OUT and RC.
in_kind_env() {
    local data="$1" prefix="$2" cmd="$3"
    OUT="$(env -u KIND_FALLBACK_WARNED XDG_DATA_HOME="$data" PATH="${prefix:+${prefix}:}${SYSBIN}" \
        KIND_CLUSTER=econ-graph ECONGRAPH_SCRATCH_STORAGE= bash -c "
            source '${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh'
            ${cmd}" 2>&1)"
    RC=$?
}

empty="${WORK}/data-empty"
in_kind_env "$empty" "" 'kind load docker-image x --name econ-graph'
if [ "$RC" -ne 0 ] && grep -q "run scripts/deploy/install-kind.sh" <<<"$OUT" && ! grep -q "is not used" <<<"$OUT"; then
    pass "without the pinned kind, kind load fails and says to run install-kind.sh"
else
    fail "without the pinned kind, kind load fails and says to run install-kind.sh (exit $RC)" "$OUT"
fi

in_kind_env "$empty" "${WORK}/old-kind" 'kind load docker-image x --name econ-graph'
if [ "$RC" -ne 0 ] && grep -q "run scripts/deploy/install-kind.sh" <<<"$OUT" &&
    grep -q "${WORK}/old-kind/kind is not used" <<<"$OUT" && ! grep -q "v0.30.0" <<<"$OUT"; then
    pass "without the pinned kind, kind load does not fall back to another kind"
else
    fail "without the pinned kind, kind load does not fall back to another kind (exit $RC)" "$OUT"
fi

in_kind_env "$empty" "${WORK}/old-kind" 'kind get clusters'
if [ "$RC" -eq 0 ] && grep -q "kind v0.30.0" <<<"$OUT" &&
    grep -q "using ${WORK}/old-kind/kind for the read-only 'kind get'" <<<"$OUT"; then
    pass "without the pinned kind, kind get falls back with a warning"
else
    fail "without the pinned kind, kind get falls back with a warning (exit $RC)" "$OUT"
fi

in_kind_env "$empty" "${WORK}/old-kind" 'require_kind_version'
if [ "$RC" -ne 0 ] && grep -q "run scripts/deploy/install-kind.sh" <<<"$OUT"; then
    pass "without the pinned kind, require_kind_version fails"
else
    fail "without the pinned kind, require_kind_version fails (exit $RC)" "$OUT"
fi

in_kind_env "$data" "${WORK}/old-kind" 'kind version && require_kind_version && echo version-ok'
if [ "$RC" -eq 0 ] && grep -q "kind ${KIND_VERSION} " <<<"$OUT" && grep -qx "version-ok" <<<"$OUT" &&
    ! grep -q "v0.30.0" <<<"$OUT"; then
    pass "with the pinned kind installed, it is used ahead of the one on PATH"
else
    fail "with the pinned kind installed, it is used ahead of the one on PATH (exit $RC)" "$OUT"
fi

old="${WORK}/data-old"
mkdir -p "${old}/econgraph/bin"
fake_kind "${old}/econgraph/bin/kind-${KIND_VERSION}" v0.30.0
in_kind_env "$old" "" 'require_kind_version'
if [ "$RC" -ne 0 ] && grep -q "older than ${KIND_MIN_VERSION}" <<<"$OUT"; then
    pass "require_kind_version refuses a pinned binary that reports an older kind"
else
    fail "require_kind_version refuses a pinned binary that reports an older kind (exit $RC)" "$OUT"
fi

# shellcheck disable=SC2016 # expanded by the inner shell
in_kind_env "$empty" "" 'set -e
    for pair in v0.33.0:v0.33.0 v0.33.1:v0.33.0 v0.34.0:v0.33.0 v1.0.0:v0.33.0 v0.100.0:v0.33.0 v0.34.0-alpha:v0.33.0; do
        version_at_least "${pair%%:*}" "${pair#*:}" || echo "wrongly older: $pair"
    done
    for pair in v0.30.0:v0.33.0 v0.32.9:v0.33.0 v0.9.0:v0.33.0 garbage:v0.33.0; do
        ! version_at_least "${pair%%:*}" "${pair#*:}" || echo "wrongly new enough: $pair"
    done
    echo compared'
if [ "$RC" -eq 0 ] && [ "$OUT" = "compared" ]; then
    pass "version_at_least compares versions numerically"
else
    fail "version_at_least compares versions numerically (exit $RC)" "$OUT"
fi

broken="${WORK}/data-broken"
mkdir -p "${broken}/econgraph/bin"
printf '#!/bin/sh\nexit 1\n' >"${broken}/econgraph/bin/kind-${KIND_VERSION}"
chmod 0755 "${broken}/econgraph/bin/kind-${KIND_VERSION}"
in_kind_env "$broken" "" 'set -e -o pipefail; require_kind_version'
if [ "$RC" -ne 0 ] && grep -q "does not run; reinstall it with scripts/deploy/install-kind.sh" <<<"$OUT"; then
    pass "under set -e -o pipefail, a pinned kind that fails still gets the reinstall message"
else
    fail "under set -e -o pipefail, a pinned kind that fails still gets the reinstall message (exit $RC)" "$OUT"
fi

# As deploy.sh --dry-run: kind() is defined before kind-env.sh is sourced.
OUT="$(env XDG_DATA_HOME="$empty" PATH="${SYSBIN}" KIND_CLUSTER=econ-graph ECONGRAPH_SCRATCH_STORAGE= bash -c "
    kind() { echo econ-graph; }
    source '${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh'
    kind_cluster_exists && echo cluster-found" 2>&1)"
RC=$?
if [ "$RC" -eq 0 ] && grep -qx "cluster-found" <<<"$OUT"; then
    pass "a caller's own kind() is kept and kind_cluster_exists uses it"
else
    fail "a caller's own kind() is kept and kind_cluster_exists uses it (exit $RC)" "$OUT"
fi

if [ "$failed" -ne 0 ]; then
    echo "install-kind checks failed"
    exit 1
fi
echo "install-kind checks passed"

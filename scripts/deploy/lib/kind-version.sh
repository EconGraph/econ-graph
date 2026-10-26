# shellcheck shell=bash
# shellcheck disable=SC2034 # the settings below are used by the scripts that source this
#
# The kind CLI the deploy scripts run, and where it lives. Sourced by
# scripts/deploy/install-kind.sh, which installs it, and by
# scripts/deploy/lib/kind-env.sh, which runs it. Source it; don't run it.
#
# Nodes running Kubernetes 1.36 or newer need kind v0.33.0 or newer: older
# kind creates the cluster but cannot `kind load` images into it ("unknown
# containerd config version: 4"). The kind that a distribution package puts on
# PATH may be older, so the scripts use their own copy, installed per user
# without sudo. Changing KIND_VERSION means updating the checksums in
# install-kind.sh too.

KIND_VERSION="v0.33.0"
# The oldest kind the scripts accept (require_kind_version in kind-env.sh).
KIND_MIN_VERSION="v0.33.0"
KIND_BIN_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/econgraph/bin"
KIND_PINNED_BIN="${KIND_BIN_DIR}/kind-${KIND_VERSION}"

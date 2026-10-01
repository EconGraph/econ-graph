#!/bin/bash
# Prints the CHANGELOG.md section for one version (e.g. "4.0.0"): everything
# between its "## [<version>] - ..." heading and the next "## [" heading.
# Used by scripts/release/tag.sh and the Release Images workflow (to fill in
# the drafted GitHub release's body).
#
# Usage: scripts/release/changelog-section.sh <version>

set -euo pipefail

version="${1:?usage: scripts/release/changelog-section.sh <version>}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
changelog="${root}/CHANGELOG.md"

section="$(awk -v ver="$version" '
  BEGIN { gsub(/\./, "\\.", ver) }
  /^## \[/ {
    if (found) exit
    found = ($0 ~ ("\\[" ver "\\]")) ? 1 : 0
    next
  }
  found { print }
' "$changelog")"

if [ -z "$(printf '%s' "$section" | tr -d '[:space:]')" ]; then
  echo "changelog-section.sh: no CHANGELOG.md section found for version ${version}" >&2
  exit 1
fi

printf '%s\n' "$section"

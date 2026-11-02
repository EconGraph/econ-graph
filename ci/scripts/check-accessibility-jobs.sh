#!/usr/bin/env bash
# Validate the jobs selected by the reusable/manual accessibility workflow.
# A selected audit must succeed; an expected skip is allowed only for a mode
# that does not request that audit. The common gate rejects failure/cancellation
# in any other dependency too. NEEDS is GitHub's toJSON(needs).
set -euo pipefail

TEST_TYPE="${TEST_TYPE:-all}"
EVENT="${EVENT:-}"
required=""
case "$TEST_TYPE" in
    all) required="accessibility-static-analysis accessibility-runtime-tests accessibility-browser-tests" ;;
    static) required="accessibility-static-analysis" ;;
    runtime) required="accessibility-runtime-tests accessibility-browser-tests" ;;
    manual) ;;
    *) echo "::error::unknown accessibility test type: $TEST_TYPE"; exit 1 ;;
esac
if [[ "$EVENT" == schedule || ( "$EVENT" == workflow_dispatch && ( "$TEST_TYPE" == all || "$TEST_TYPE" == manual ) ) ]]; then
    required="$required accessibility-manual-tests"
elif [[ "$TEST_TYPE" == manual ]]; then
    echo "::error::manual-only accessibility checks require workflow_dispatch"
    exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Enforce success for selected audits on every event, using the common gate's
# strict mode. This does not dispatch a workflow or contact GitHub.
EVENT=merge_group MUST_RUN="$required" "$script_dir/check-required-jobs.sh"

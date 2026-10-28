#!/usr/bin/env bash
#
# Tests for ci/scripts/check-required-jobs.sh, the result job of the required
# workflows: which job results pass a pull request and a merge group.
#
# Usage: ci/scripts/test-check-required-jobs.sh (from anywhere; needs jq)

set -uo pipefail

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check-required-jobs.sh"
failed=0

# check NAME WANT EVENT MUST_RUN NEEDS: the script exits 0 (WANT=pass) or not (fail).
check() {
    local name="$1" want="$2" rc=0 out
    out="$(NEEDS="$5" EVENT="$3" MUST_RUN="$4" "$script" 2>&1)" || rc=$?
    if { [ "$want" = pass ] && [ "$rc" -eq 0 ]; } || { [ "$want" = fail ] && [ "$rc" -ne 0 ]; }; then
        echo "ok   $name"
    else
        echo "FAIL $name: exit $rc, expected $want"
        printf '     %s\n' "$out"
        failed=1
    fi
}

ok='{"changes":{"result":"success"},"build":{"result":"success"},"test":{"result":"success"}}'
skipped='{"changes":{"result":"success"},"build":{"result":"skipped"},"test":{"result":"skipped"}}'
failure='{"changes":{"result":"success"},"build":{"result":"failure"},"test":{"result":"skipped"}}'
cancelled='{"changes":{"result":"success"},"build":{"result":"success"},"test":{"result":"cancelled"}}'

check "pull request, all passed" pass pull_request "changes build test" "$ok"
check "pull request, filtered jobs skipped" pass pull_request "changes build test" "$skipped"
check "pull request, a job failed" fail pull_request "changes build test" "$failure"
check "pull request, a job cancelled" fail pull_request "changes build test" "$cancelled"
check "merge group, all passed" pass merge_group "changes build test" "$ok"
check "merge group, a required job skipped" fail merge_group "changes build test" "$skipped"
check "merge group, a job failed" fail merge_group "changes build test" "$failure"
check "merge group, a required job missing from needs" fail merge_group "changes build test lint" "$ok"
check "merge group, a skipped job not in MUST_RUN" pass merge_group "changes" "$skipped"
check "push, filtered jobs skipped" pass push "changes build test" "$skipped"
if NEEDS='' "$script" >/dev/null 2>&1; then
    echo "FAIL no NEEDS: passed"
    failed=1
else
    echo "ok   no NEEDS fails"
fi

exit "$failed"

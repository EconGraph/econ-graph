#!/usr/bin/env bash
#
# The result job of a workflow whose one check branch protection requires
# (docs/development/MERGE_QUEUE.md). Reads the results of the jobs it needs and
# fails when the workflow must not count as passed:
#
#   - any job failed or was cancelled;
#   - in a merge group (EVENT=merge_group), any job named in MUST_RUN did not
#     succeed: nothing is path-filtered there, so a skip means it never ran.
#
# On a pull request a skipped job passes: path filtering skips the jobs a
# change can't affect.
#
# Environment: NEEDS (the workflow's `toJSON(needs)`), EVENT
# (github.event_name), MUST_RUN (space-separated job ids, may be empty).
# Needs jq.

set -euo pipefail

: "${NEEDS:?NEEDS must be the JSON of the needs context}"
EVENT="${EVENT:-}"
MUST_RUN="${MUST_RUN:-}"

# A non-empty object of jobs, or nothing passes: malformed JSON, null, a list
# or {} would otherwise read as "no job failed".
if ! jq -e 'type == "object" and length > 0' <<<"$NEEDS" >/dev/null 2>&1; then
    echo "::error::NEEDS is not a non-empty JSON object of job results: ${NEEDS:0:200}"
    exit 1
fi

bad=()
while IFS=$'\t' read -r job result; do
    [ -n "$job" ] || continue
    echo "${job}: ${result}"
    case "$result" in
        failure | cancelled) bad+=("${job}: ${result}") ;;
    esac
done < <(jq -r 'to_entries[] | [.key, .value.result] | @tsv' <<<"$NEEDS")

if [ "$EVENT" = merge_group ]; then
    for job in $MUST_RUN; do
        result="$(jq -r --arg j "$job" '.[$j].result // "missing"' <<<"$NEEDS")"
        if [ "$result" != success ]; then
            bad+=("${job}: ${result} (a merge group runs every job of the suite)")
        fi
    done
fi

if [ "${#bad[@]}" -gt 0 ]; then
    echo "::error::required jobs did not pass"
    printf '  %s\n' "${bad[@]}"
    exit 1
fi
echo "all required jobs passed"

#!/usr/bin/env bash
#
# Tests for scripts/github/apply-rulesets.sh against a stub `gh` that keeps
# rulesets as files and logs every call; nothing reaches GitHub:
#
#   - a dry run writes nothing;
#   - the first apply gives the rulesets of docs/development/MERGE_QUEUE.md and
#     keeps every live field, rule and required check;
#   - a re-run is a no-op ("already applied");
#   - the release ruleset has no merge queue, and is created once, then
#     updated in place;
#   - it refuses without admin rights, when gh is logged out, on a missing or
#     unexpected main ruleset, on duplicate release rulesets, and without a
#     terminal or --yes;
#   - it fails when the re-read rulesets don't match;
#   - a write GitHub rejects prints the ruleset and GitHub's error body and
#     exits non-zero.
#
# Usage: scripts/github/test-apply-rulesets.sh (from anywhere; needs jq)

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="${APPLY_RULESETS:-${SCRIPT_DIR}/apply-rulesets.sh}"
rules="${SCRIPT_DIR}/merge-queue-rules.json"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
failed=0

# --- Stub gh -----------------------------------------------------------------
#
# State in $STUB_STATE: rulesets/<id>.json, calls.log (every call), writes.log
# (PUT, POST, PATCH, DELETE). STUB_ADMIN (true/false), STUB_AUTH_RC,
# STUB_DROP_ON_WRITE (a rule type the stub silently drops when it stores a
# write), and STUB_REJECT (a method, PUT or POST, answered with GitHub's
# HTTP 422 validation error instead of being stored).
mkdir -p "$WORK/bin"
cat >"$WORK/bin/gh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
state="$STUB_STATE"
echo "$*" >>"$state/calls.log"
if [ "${1:-}" = auth ]; then exit "${STUB_AUTH_RC:-0}"; fi
[ "${1:-}" = api ] || { echo "stub gh: unexpected: $*" >&2; exit 64; }
shift
method=GET path="" input=""
while [ $# -gt 0 ]; do
    case "$1" in
        -X | --method) method="$2"; shift 2 ;;
        --input) input="$2"; shift 2 ;;
        --paginate) shift ;;
        -*) echo "stub gh: unexpected flag $1" >&2; exit 64 ;;
        *) path="$1"; shift ;;
    esac
done
repo="repos/EconGraph/econ-graph"
[ "$method" = GET ] || echo "$method $path" >>"$state/writes.log"
if [ "$method" = "${STUB_REJECT:-}" ]; then
    # What gh does on an error response: the body on stdout, a summary on stderr.
    jq -n '{message: "Validation Failed",
            errors: [{resource: "Repository", code: "custom", field: "rules", message: "Merge queue cannot be used with wildcard branch patterns"}],
            documentation_url: "https://docs.github.com/rest/repos/rules#create-a-repository-ruleset",
            status: "422"}'
    echo "gh: Validation Failed (HTTP 422)" >&2
    exit 1
fi
store() { # store ID FILE: what GitHub keeps from a write (read-only fields stay)
    local old="$state/rulesets/$1.json"
    [ -f "$old" ] || echo '{}' >"$old"
    jq --argjson id "$1" --arg drop "${STUB_DROP_ON_WRITE:-}" --slurpfile old "$old" \
        '$old[0] + . + {id: $id, source_type: "Repository", source: "EconGraph/econ-graph"}
         | .rules |= map(select(.type != $drop))' "$2" >"$state/new.json"
    mv "$state/new.json" "$old"
}
case "$method $path" in
    "GET $repo") jq -n --argjson a "${STUB_ADMIN:-true}" '{full_name: "EconGraph/econ-graph", permissions: {admin: $a}}' ;;
    "GET $repo/rulesets?includes_parents=false")
        # Two pages, like --paginate prints them.
        files=("$state"/rulesets/*.json)
        if [ -e "${files[0]}" ]; then
            jq -s '[.[] | {id, name, target, source_type, enforcement}] | .[:1]' "${files[@]}"
            jq -s '[.[] | {id, name, target, source_type, enforcement}] | .[1:]' "${files[@]}"
        else
            echo '[]'
        fi ;;
    "GET $repo/rulesets/"*)
        f="$state/rulesets/${path##*/}.json"
        [ -f "$f" ] || { echo '{"message":"Not Found","status":"404"}'; echo "gh: Not Found (HTTP 404)" >&2; exit 1; }
        cat "$f" ;;
    "PUT $repo/rulesets/"*)
        id="${path##*/}"
        [ -f "$state/rulesets/$id.json" ] || { echo "gh: Not Found (HTTP 404)" >&2; exit 1; }
        store "$id" "$input"; cat "$state/rulesets/$id.json" ;;
    "POST $repo/rulesets")
        id=$(( $(cat "$state/next-id" 2>/dev/null || echo 9000001) ))
        echo $(( id + 1 )) >"$state/next-id"
        store "$id" "$input"; cat "$state/rulesets/$id.json" ;;
    *) echo "stub gh: unexpected $method $path" >&2; exit 64 ;;
esac
STUB
chmod +x "$WORK/bin/gh" || exit 1
export PATH="$WORK/bin:$PATH"
# Never fall through to a real, logged-in gh: the --yes cases write rulesets.
if [ "$(command -v gh)" != "$WORK/bin/gh" ]; then
    echo "FAIL the stub gh is not the gh on PATH" >&2
    exit 1
fi

# The live main ruleset as GitHub returns it (read-only fields included).
live_main='{
  "id": 8429203, "name": "main only by pr", "target": "branch",
  "source_type": "Repository", "source": "EconGraph/econ-graph", "enforcement": "active",
  "conditions": {"ref_name": {"exclude": [], "include": ["~DEFAULT_BRANCH"]}},
  "rules": [
    {"type": "non_fast_forward"},
    {"type": "pull_request", "parameters": {
      "required_approving_review_count": 1, "dismiss_stale_reviews_on_push": false,
      "required_reviewers": [], "require_code_owner_review": false,
      "dismissal_restriction": {"enabled": false, "allowed_actors": []},
      "require_last_push_approval": false, "required_review_thread_resolution": false,
      "require_extra_approval_for_unattributed_changes": true,
      "allowed_merge_methods": ["merge", "squash", "rebase"]}}
  ],
  "node_id": "RRS_x", "created_at": "2025-09-25T23:31:54.759-07:00",
  "updated_at": "2025-09-26T18:24:36.345-07:00",
  "bypass_actors": [{"actor_id": null, "actor_type": "OrganizationAdmin", "bypass_mode": "always"}],
  "current_user_can_bypass": "always"
}'

# reset [MAIN_JSON]: a fresh state with only the main ruleset.
reset() {
    export STUB_STATE="$WORK/state"
    rm -rf "$STUB_STATE"
    mkdir -p "$STUB_STATE/rulesets"
    : >"$STUB_STATE/calls.log"
    : >"$STUB_STATE/writes.log"
    printf '%s\n' "${1:-$live_main}" >"$STUB_STATE/rulesets/8429203.json"
    unset STUB_ADMIN STUB_AUTH_RC STUB_DROP_ON_WRITE STUB_REJECT
}

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1"
    [ -z "${2:-}" ] || printf '     %s\n' "$2"
    failed=1
}

# run ARGS...: run the script with stdin not a terminal; sets $out and $rc.
run() {
    rc=0
    out="$("$script" "$@" </dev/null 2>&1)" || rc=$?
}

writes() { wc -l <"$STUB_STATE/writes.log" | tr -d ' '; }
release_files() { grep -l '"release branches by pr"' "$STUB_STATE"/rulesets/*.json 2>/dev/null; }

# expect_refusal NAME MESSAGE ARGS...: exits non-zero, prints MESSAGE, writes nothing.
expect_refusal() {
    local name="$1" message="$2"
    shift 2
    run "$@"
    if [ "$rc" -ne 0 ] && grep -qF -- "$message" <<<"$out" && [ "$(writes)" = 0 ]; then
        ok "$name"
    else
        fail "$name (exit $rc, $(writes) writes)" "$out"
    fi
}

# What MERGE_QUEUE.md describes, computed from the live main ruleset before
# any release ruleset exists: the reference for the first apply. The release
# ruleset has no merge queue and no Lint workflows check.
jq 'map(select(.type != "merge_queue"))
    | (.[] | select(.type == "required_status_checks") | .parameters.required_status_checks) |=
      map(select(.context != "Lint workflows"))' "$rules" >"$WORK/release-new-rules.json"
jq --slurpfile new "$rules" '{name, target, enforcement, conditions, bypass_actors,
      rules: ([.rules[] | select(.type as $t | [$new[0][].type] | index($t) | not)] + $new[0])}' \
    <<<"$live_main" >"$WORK/doc-main.json"
jq --slurpfile new "$WORK/release-new-rules.json" '{name: "release branches by pr", target, enforcement,
      bypass_actors, conditions: {ref_name: {include: ["refs/heads/release/*"], exclude: []}},
      rules: ([.rules[] | select(.type == "non_fast_forward" or .type == "pull_request")] + $new[0])}' \
    <<<"$live_main" >"$WORK/doc-release.json"

# same_ruleset NAME EXPECTED_FILE ACTUAL_FILE: the writable fields are equal,
# whatever the order of rules and checks.
same_ruleset() {
    local norm='{name, target, enforcement, conditions, bypass_actors,
        rules: (.rules | sort_by(.type) | map(if .type == "required_status_checks"
          then .parameters.required_status_checks |= sort_by(.context) else . end))}'
    if diff <(jq -S "$norm" "$2") <(jq -S "$norm" "$3") >"$WORK/diff" 2>&1; then
        ok "$1"
    else
        fail "$1" "$(cat "$WORK/diff")"
    fi
}

# --- Dry run -------------------------------------------------------------------

reset
run --dry-run
if [ "$rc" -eq 0 ] && [ "$(writes)" = 0 ] && grep -qF 'Dry run: nothing was changed.' <<<"$out" &&
    grep -qF '+      "type": "merge_queue"' <<<"$out" && grep -qF '== new ruleset "release branches by pr": changes' <<<"$out"; then
    ok "dry run prints both diffs and writes nothing"
else
    fail "dry run (exit $rc, $(writes) writes)" "$out"
fi
if grep -qvE '^(auth status|api repos/EconGraph/econ-graph(/rulesets(/[0-9]+|\?includes_parents=false))?|api --paginate repos/EconGraph/econ-graph/rulesets\?includes_parents=false)$' \
    "$STUB_STATE/calls.log"; then
    fail "dry run makes only reads" "$(cat "$STUB_STATE/calls.log")"
else
    ok "dry run makes only reads"
fi
if cmp -s <(printf '%s\n' "$live_main") "$STUB_STATE/rulesets/8429203.json" && [ -z "$(release_files)" ]; then
    ok "dry run leaves the rulesets as they were"
else
    fail "dry run changed a ruleset"
fi

# --- First apply ---------------------------------------------------------------

reset
run --yes
if [ "$rc" -eq 0 ] && grep -qF 'Verified: both rulesets match.' <<<"$out" &&
    [ "$(grep -c '^PUT repos/EconGraph/econ-graph/rulesets/8429203$' "$STUB_STATE/writes.log")" = 1 ] &&
    [ "$(grep -c '^POST repos/EconGraph/econ-graph/rulesets$' "$STUB_STATE/writes.log")" = 1 ] &&
    [ "$(writes)" = 2 ]; then
    ok "first apply: PUT main, POST release, verified"
else
    fail "first apply (exit $rc)" "$out"$'\n'"$(cat "$STUB_STATE/writes.log")"
fi
same_ruleset "first apply: main is MERGE_QUEUE.md's ruleset" "$WORK/doc-main.json" "$STUB_STATE/rulesets/8429203.json"
release_file="$(release_files)"
if [ "$(wc -w <<<"$release_file")" = 1 ]; then
    same_ruleset "first apply: release is MERGE_QUEUE.md's ruleset" "$WORK/doc-release.json" "$release_file"
else
    fail "first apply: one release ruleset" "$release_file"
fi
# GitHub rejects a merge queue on a wildcard branch pattern; main keeps its own.
if jq -e '[.rules[].type] | (index("merge_queue") | not) and index("deletion")
          and index("required_status_checks") and index("pull_request") and index("non_fast_forward")' \
    "$release_file" >/dev/null &&
    jq -e '[.rules[].type] | index("merge_queue")' "$STUB_STATE/rulesets/8429203.json" >/dev/null; then
    ok "first apply: release has no merge_queue rule, main has one"
else
    fail "first apply: merge_queue rules" "$(jq -c '.rules | map(.type)' "$release_file")"
fi
# Every live field of main is still there, unchanged.
if jq -e --argjson live "$live_main" '
    . as $now
    | ($live | del(.updated_at)) as $l
    | all($l | keys[]; . as $k | $k == "rules" or $now[$k] == $l[$k])
    and all($l.rules[]; . as $r | any($now.rules[]; . == $r))' \
    "$STUB_STATE/rulesets/8429203.json" >/dev/null; then
    ok "first apply keeps every live field and rule of main"
else
    fail "first apply dropped or changed a live field of main" "$(jq -c . "$STUB_STATE/rulesets/8429203.json")"
fi

# --- Re-run --------------------------------------------------------------------

: >"$STUB_STATE/writes.log"
run --yes
if [ "$rc" -eq 0 ] && [ "$(writes)" = 0 ] && grep -qF 'already applied; nothing to do' <<<"$out"; then
    ok "re-run is a no-op"
else
    fail "re-run (exit $rc, $(writes) writes)" "$out"
fi
run --dry-run
if [ "$rc" -eq 0 ] && [ "$(writes)" = 0 ] && grep -qF 'already applied; nothing to do' <<<"$out"; then
    ok "dry run after apply says already applied"
else
    fail "dry run after apply (exit $rc)" "$out"
fi

# --- Release ruleset updated in place -----------------------------------------

release_id="$(jq -r .id "$release_file")"
jq '.rules |= map(if .type == "required_status_checks"
                  then .parameters.strict_required_status_checks_policy = true else . end
                  | select(.type != "deletion"))
    | .rules += [{"type": "required_signatures"}]
    | .conditions.ref_name.include += ["refs/heads/hotfix/*"]' "$release_file" >"$WORK/r.json"
cp "$WORK/r.json" "$release_file"
: >"$STUB_STATE/writes.log"
run --yes
if [ "$rc" -eq 0 ] && [ "$(cat "$STUB_STATE/writes.log")" = "PUT repos/EconGraph/econ-graph/rulesets/${release_id}" ] &&
    [ "$(release_files | wc -l)" = 1 ]; then
    ok "a changed release ruleset is updated in place, not duplicated"
else
    fail "release update (exit $rc)" "$out"$'\n'"$(cat "$STUB_STATE/writes.log")"
fi
if jq -e '
    (.rules | map(.type) | index("required_signatures") and index("deletion"))
    and (.rules | map(.type) | index("merge_queue") | not)
    and (.rules[] | select(.type == "required_status_checks")
         | .parameters.strict_required_status_checks_policy == false)
    and (.conditions.ref_name.include == ["refs/heads/release/*", "refs/heads/hotfix/*"])' \
    "$release_file" >/dev/null; then
    ok "release update keeps its own rules and refs and restores the managed values"
else
    fail "release update result" "$(jq -c . "$release_file")"
fi

# --- Live main with more than the script manages ------------------------------

extended_main="$(jq '
    .rules += [{"type": "required_signatures"},
               {"type": "required_status_checks", "parameters": {
                  "strict_required_status_checks_policy": true, "future_check_setting": "kept",
                  "required_status_checks": [{"context": "Security Audit", "integration_id": 15368},
                                             {"context": "Core CI Result", "integration_id": 15368}]}},
               {"type": "merge_queue", "parameters": {"merge_method": "MERGE", "future_queue_setting": 3}}]
    | (.rules[] | select(.type == "pull_request") | .parameters) += {"automatic_copilot_code_review_enabled": true}
    | .bypass_actors += [{"actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "pull_request"}]
    | .conditions.ref_name.exclude = ["refs/heads/legacy"]' <<<"$live_main")"
reset "$extended_main"
run --yes
main_file="$STUB_STATE/rulesets/8429203.json"
if [ "$rc" -eq 0 ] && jq -e '
    (.rules | map(.type) | sort) == ["deletion", "merge_queue", "non_fast_forward", "pull_request",
                                     "required_signatures", "required_status_checks"]
    and (.rules[] | select(.type == "required_status_checks") | .parameters
         | (.required_status_checks | map(.context) | sort) == ["Core CI Result", "Feature Flags Check",
             "Lint workflows", "Markdown lint and links", "Release E2E Result", "Security Audit"]
           and .strict_required_status_checks_policy == false and .do_not_enforce_on_create == false
           and .future_check_setting == "kept")
    and (.rules[] | select(.type == "merge_queue") | .parameters
         | .merge_method == "SQUASH" and .future_queue_setting == 3 and .max_entries_to_build == 5)
    and (.rules[] | select(.type == "pull_request") | .parameters
         | .automatic_copilot_code_review_enabled == true
           and .require_extra_approval_for_unattributed_changes == true
           and .dismissal_restriction == {"enabled": false, "allowed_actors": []})
    and (.bypass_actors | length) == 2
    and .conditions.ref_name.exclude == ["refs/heads/legacy"]' "$main_file" >/dev/null; then
    ok "unmanaged rules, parameters, checks, actors and conditions of main are kept"
else
    fail "merge with an extended main (exit $rc)" "$out"$'\n'"$(jq -c . "$main_file")"
fi
# The release ruleset copies main's pull_request rule and bypass actors.
if jq -e '(.rules[] | select(.type == "pull_request") | .parameters.automatic_copilot_code_review_enabled)
          and (.bypass_actors | length) == 2
          and (.rules | map(.type) | index("required_signatures") | not)' "$(release_files)" >/dev/null; then
    ok "release copies main's pull_request rule and bypass actors, not its other rules"
else
    fail "release from an extended main" "$(jq -c . "$(release_files)")"
fi

# --- Refusals ------------------------------------------------------------------

reset
export STUB_ADMIN=false
expect_refusal "refuses without admin rights" "no admin rights" --yes
expect_refusal "refuses without admin rights, in a dry run too" "no admin rights" --dry-run

reset
export STUB_AUTH_RC=1
expect_refusal "refuses when gh is logged out" "not logged in" --yes

reset
rm "$STUB_STATE/rulesets/8429203.json"
expect_refusal "refuses when the main ruleset is missing" "8429203 is missing" --yes

reset "$(jq '.name = "main"' <<<"$live_main")"
expect_refusal "refuses a renamed main ruleset" 'name is "main"' --yes

reset "$(jq '.id = 1234' <<<"$live_main")"
expect_refusal "refuses a main ruleset with another id" "id is 1234" --yes

reset "$(jq '.target = "tag"' <<<"$live_main")"
expect_refusal "refuses a main ruleset for tags" 'target is "tag"' --yes

reset "$(jq '.conditions.ref_name.include = ["refs/heads/dev"]' <<<"$live_main")"
expect_refusal "refuses a main ruleset off the default branch" "does not cover the default branch" --yes

reset "$(jq '.rules |= map(select(.type != "pull_request"))' <<<"$live_main")"
expect_refusal "refuses a main ruleset without its pull_request rule" "lacks the pull_request" --yes

reset "$(jq 'del(.bypass_actors)' <<<"$live_main")"
expect_refusal "refuses when bypass_actors can't be read" "bypass_actors is not readable" --yes

reset
for id in 501 502; do
    jq -n --argjson id "$id" '{id: $id, name: "release branches by pr", target: "branch", enforcement: "active",
        conditions: {ref_name: {include: ["refs/heads/release/*"], exclude: []}}, bypass_actors: [], rules: []}' \
        >"$STUB_STATE/rulesets/$id.json"
done
expect_refusal "refuses two release rulesets of the same name" "2 rulesets are named" --yes

reset
expect_refusal "refuses to apply without a terminal or --yes" "confirmation needs a terminal"

reset
expect_refusal "rejects an unknown argument" "unknown argument" --dryrun

# --- Verification --------------------------------------------------------------

reset
export STUB_DROP_ON_WRITE=merge_queue
run --yes
if [ "$rc" -ne 0 ] && grep -qF 'does not match what was applied' <<<"$out"; then
    ok "fails when the re-read ruleset doesn't match"
else
    fail "verification (exit $rc)" "$out"
fi

# --- Rejected writes -----------------------------------------------------------

reset
export STUB_REJECT=POST
run --yes
if [ "$rc" -ne 0 ] &&
    grep -qF 'GitHub rejected the write of new ruleset "release branches by pr" (POST repos/EconGraph/econ-graph/rulesets)' <<<"$out" &&
    grep -qF 'gh: Validation Failed (HTTP 422)' <<<"$out" &&
    grep -qF 'message: Validation Failed' <<<"$out" &&
    grep -qF "error: Merge queue cannot be used with wildcard branch patterns" <<<"$out" &&
    grep -qF 'Ruleset 8429203 "main only by pr" was already updated' <<<"$out" &&
    ! grep -qF 'Created ruleset' <<<"$out" && ! grep -qF 'Verified' <<<"$out"; then
    ok "a rejected release POST prints GitHub's error, says main was written, exits non-zero"
else
    fail "rejected release POST (exit $rc)" "$out"
fi

reset
export STUB_REJECT=PUT
run --yes
if [ "$rc" -ne 0 ] &&
    grep -qF 'GitHub rejected the write of ruleset 8429203 "main only by pr"' <<<"$out" &&
    grep -qF 'message: Validation Failed' <<<"$out" && grep -qF 'Nothing was written.' <<<"$out" &&
    [ "$(cat "$STUB_STATE/writes.log")" = "PUT repos/EconGraph/econ-graph/rulesets/8429203" ]; then
    ok "a rejected main PUT prints GitHub's error and stops before the release ruleset"
else
    fail "rejected main PUT (exit $rc)" "$out"$'\n'"$(cat "$STUB_STATE/writes.log")"
fi

exit "$failed"

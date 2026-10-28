#!/usr/bin/env bash
#
# Apply the merge-queue branch protection of docs/development/MERGE_QUEUE.md
# to EconGraph/econ-graph:
#
#   - extend the ruleset `main only by pr` (id 8429203) in place with the rules
#     in merge-queue-rules.json (deletion protection, the required checks and
#     the merge queue);
#   - create, or update, the ruleset `release branches by pr` for
#     refs/heads/release/*: main's live `non_fast_forward` and `pull_request`
#     rules and `bypass_actors`, plus the same rules without the
#     `Lint workflows` check.
#
# Usage: scripts/github/apply-rulesets.sh [--dry-run] [--yes]
#
#   --dry-run  read the live rulesets, print the before/after diff of each,
#              and stop: nothing is written.
#   --yes      apply without asking. Without it the script asks on a terminal,
#              and refuses when stdin is not one.
#
# The merge is additive, by rule type: a live rule of a type the script doesn't
# manage is kept as it is; a live rule of a type it does manage keeps every
# parameter, with the script's values set over it, and a live required check
# the script doesn't list stays required. Nothing live is removed, so removing
# a rule or a check is done by hand. The release ruleset is found by name and
# target, so a re-run updates it instead of creating another. When the live
# rulesets already match, the script says "already applied" and writes nothing.
#
# It refuses before writing anything when `gh` is not logged in, the account
# has no admin rights on the repository, or the main ruleset is missing or not
# the expected one (id, name, target, source, branch condition, rules). After
# writing it re-reads both rulesets and fails unless they match.
#
# Needs gh (logged in as a repository admin), jq and diff. Reads and writes
# only through `gh api`; temporary files go under $TMPDIR and are removed.

set -euo pipefail

REPO="EconGraph/econ-graph"
MAIN_ID=8429203
MAIN_NAME="main only by pr"
RELEASE_NAME="release branches by pr"
RELEASE_REF="refs/heads/release/*"
# release/v4.0 has no Actionlint workflow; a required check that nothing
# reports keeps every PR waiting.
RELEASE_SKIPPED_CHECK="Lint workflows"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULES_FILE="${SCRIPT_DIR}/merge-queue-rules.json"

usage() {
    sed -n '/^# Usage:/,/^#   --yes/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    echo "             (see the header of ${BASH_SOURCE[0]})"
}

dry_run=false
assume_yes=false
for arg in "$@"; do
    case "$arg" in
        --dry-run) dry_run=true ;;
        --yes) assume_yes=true ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "apply-rulesets: unknown argument: $arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

die() {
    echo "apply-rulesets: $*" >&2
    exit 1
}

for tool in gh jq diff; do
    command -v "$tool" >/dev/null 2>&1 || die "needs $tool on PATH"
done
[ -r "$RULES_FILE" ] || die "missing $RULES_FILE"
jq -e 'type == "array" and length > 0 and all(.[]; type == "object" and (.type | type) == "string")' \
    "$RULES_FILE" >/dev/null || die "$RULES_FILE is not a list of rules"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# jq definitions shared by every filter below.
# shellcheck disable=SC2016 # jq variables, not shell ones
JQ_LIB='
# The fields a ruleset PUT or POST takes.
def writable: {name, target, enforcement, conditions, bypass_actors, rules};

# Overlay rule $o onto the live rule of the same type: every live parameter
# stays, $o sets its own; required checks are a union by context, live first.
def merge_rule($o):
  if ($o | has("parameters")) | not then .
  elif .type == "required_status_checks" then
    ((.parameters.required_status_checks // []) as $live
     | .parameters = (((.parameters // {}) * ($o.parameters | del(.required_status_checks)))
         + {required_status_checks: ($live + [($o.parameters.required_status_checks // [])[]
             | . as $c | select(any($live[]; .context == $c.context) | not)])}))
  else .parameters = ((.parameters // {}) * $o.parameters)
  end;

# Add the rules $new to the rule list: merged into a live rule of the same
# type, appended otherwise. No live rule is removed.
def merge_rules($new):
  reduce $new[] as $o ((. // []);
    if any(.[]; .type == $o.type) then map(if .type == $o.type then merge_rule($o) else . end)
    else . + [$o] end);

# Add the bypass actors $add that are not already there (by type and id).
def union_actors($add):
  (. // []) as $b
  | $b + [$add[] | . as $a | select(any($b[]; .actor_type == $a.actor_type and .actor_id == $a.actor_id) | not)];

# Order-independent form, for diffs and comparisons.
def norm:
  .rules |= ((. // []) | sort_by(.type) | map(
      if .type == "required_status_checks"
      then .parameters.required_status_checks |= ((. // []) | sort_by(.context)) else . end))
  | .bypass_actors |= ((. // []) | sort_by(.actor_type, .actor_id))
  | .conditions.ref_name.include |= ((. // []) | sort)
  | .conditions.ref_name.exclude |= ((. // []) | sort);

# True when everything in . is in $a (GitHub may add default parameters).
def subset($a):
  . as $d
  | if ($d | type) == "object" then
      ($a | type) == "object" and all($d | keys[]; . as $k | ($a | has($k)) and ($d[$k] | subset($a[$k])))
    elif ($d | type) == "array" then
      ($a | type) == "array" and ($d | length) == ($a | length)
      and all(range(0; $d | length); . as $i | $d[$i] | subset($a[$i]))
    else $d == $a end;
'

# --- Preconditions -----------------------------------------------------------

gh auth status >/dev/null 2>&1 || die "gh is not logged in (run: gh auth login)"
gh api "repos/${REPO}" >"$work/repo.json" 2>"$work/err" || die "can't read ${REPO}: $(cat "$work/err")"
jq -e '.permissions.admin == true' "$work/repo.json" >/dev/null ||
    die "the gh account has no admin rights on ${REPO}; rulesets need a repository admin"

gh api "repos/${REPO}/rulesets/${MAIN_ID}" >"$work/main-live.json" 2>"$work/err" ||
    die "the main ruleset ${MAIN_ID} is missing or unreadable: $(cat "$work/err")"

main_problem="$(jq -r --argjson id "$MAIN_ID" --arg name "$MAIN_NAME" --arg repo "$REPO" '
  [ (select(.id != $id) | "id is \(.id), not \($id)"),
    (select(.name != $name) | "name is \(.name | tojson), not \($name | tojson)"),
    (select(.target != "branch") | "target is \(.target | tojson), not \"branch\""),
    (select(.source_type != "Repository" or .source != $repo)
       | "source is \(.source | tojson) (\(.source_type)), not this repository"),
    (select((.conditions.ref_name.include // []) | index("~DEFAULT_BRANCH") | not)
       | "it does not cover the default branch"),
    (select((.bypass_actors | type) != "array") | "bypass_actors is not readable"),
    (select([.rules[]?.type] | (index("pull_request") and index("non_fast_forward")) | not)
       | "it lacks the pull_request or non_fast_forward rule")
  ] | join("; ")' "$work/main-live.json")"
[ -z "$main_problem" ] ||
    die "the live main ruleset is not the expected one ($main_problem); check docs/development/MERGE_QUEUE.md and the repository settings"

gh api --paginate "repos/${REPO}/rulesets?includes_parents=false" >"$work/list-pages.json" 2>"$work/err" ||
    die "can't list the rulesets of ${REPO}: $(cat "$work/err")"
jq -s 'map(if type == "array" then . else [.] end) | add // []' "$work/list-pages.json" >"$work/list.json"
mapfile -t release_ids < <(jq -r --arg name "$RELEASE_NAME" --argjson main "$MAIN_ID" \
    '.[] | select(.name == $name and .target == "branch" and .id != $main) | .id' "$work/list.json")
[ "${#release_ids[@]}" -le 1 ] ||
    die "${#release_ids[@]} rulesets are named \"${RELEASE_NAME}\" (ids ${release_ids[*]}); keep one and re-run"
release_id="${release_ids[0]:-}"
if [ -n "$release_id" ]; then
    gh api "repos/${REPO}/rulesets/${release_id}" >"$work/release-live.json" 2>"$work/err" ||
        die "can't read the release ruleset ${release_id}: $(cat "$work/err")"
else
    echo null >"$work/release-live.json"
fi

# --- Desired rulesets --------------------------------------------------------

jq --arg skip "$RELEASE_SKIPPED_CHECK" '
  (.[] | select(.type == "required_status_checks") | .parameters.required_status_checks) |=
    map(select(.context != $skip))' "$RULES_FILE" >"$work/release-new-rules.json"

jq "$JQ_LIB"' writable | .rules |= merge_rules($new[0])' \
    --slurpfile new "$RULES_FILE" "$work/main-live.json" >"$work/main-desired.json"

jq -n "$JQ_LIB"'
  $main[0] as $m
  | ($release[0] | if . == null then
       {name: $name, target: $m.target, enforcement: $m.enforcement,
        conditions: {ref_name: {include: [], exclude: []}}, bypass_actors: [], rules: []}
     else writable end)
  | .conditions.ref_name.include |= ((. // []) | if index($ref) then . else . + [$ref] end)
  | .conditions.ref_name.exclude |= (. // [])
  | .bypass_actors |= union_actors($m.bypass_actors)
  | .rules |= merge_rules([$m.rules[] | select(.type == "non_fast_forward" or .type == "pull_request")]
                          + $new[0])' \
    --arg name "$RELEASE_NAME" --arg ref "$RELEASE_REF" \
    --slurpfile main "$work/main-live.json" --slurpfile release "$work/release-live.json" \
    --slurpfile new "$work/release-new-rules.json" >"$work/release-desired.json"

# --- Diff --------------------------------------------------------------------

# show_diff LABEL LIVE DESIRED: print the diff; return 0 when they differ.
show_diff() {
    local label="$1" live="$2" desired="$3"
    if jq -e '. == null' "$live" >/dev/null; then
        : >"$work/a.json"
    else
        jq -S "$JQ_LIB"' writable | norm' "$live" >"$work/a.json"
    fi
    jq -S "$JQ_LIB"' norm' "$desired" >"$work/b.json"
    if cmp -s "$work/a.json" "$work/b.json"; then
        echo "== ${label}: already applied"
        return 1
    fi
    echo "== ${label}: changes"
    diff -u --label "${label} (live)" --label "${label} (after)" "$work/a.json" "$work/b.json" || true
    return 0
}

main_changes=false
release_changes=false
show_diff "ruleset ${MAIN_ID} \"${MAIN_NAME}\"" "$work/main-live.json" "$work/main-desired.json" &&
    main_changes=true
echo
if [ -n "$release_id" ]; then
    release_label="ruleset ${release_id} \"${RELEASE_NAME}\""
else
    release_label="new ruleset \"${RELEASE_NAME}\""
fi
show_diff "$release_label" "$work/release-live.json" "$work/release-desired.json" &&
    release_changes=true
echo

if ! $main_changes && ! $release_changes; then
    echo "Both rulesets are already applied; nothing to do."
    exit 0
fi
if $dry_run; then
    echo "Dry run: nothing was changed."
    exit 0
fi

# --- Apply -------------------------------------------------------------------

if ! $assume_yes; then
    [ -t 0 ] || die "confirmation needs a terminal; re-run at one, or pass --yes"
    read -r -p "Apply these changes to ${REPO}? [y/N] " answer
    case "$answer" in
        y | Y | yes | YES) ;;
        *) die "not applied" ;;
    esac
fi

if $main_changes; then
    gh api -X PUT "repos/${REPO}/rulesets/${MAIN_ID}" --input "$work/main-desired.json" >/dev/null
    echo "Updated ruleset ${MAIN_ID} \"${MAIN_NAME}\"."
fi
if $release_changes; then
    if [ -n "$release_id" ]; then
        gh api -X PUT "repos/${REPO}/rulesets/${release_id}" --input "$work/release-desired.json" >/dev/null
        echo "Updated ruleset ${release_id} \"${RELEASE_NAME}\"."
    else
        gh api -X POST "repos/${REPO}/rulesets" --input "$work/release-desired.json" >"$work/created.json"
        release_id="$(jq -r '.id' "$work/created.json")"
        [[ "$release_id" =~ ^[0-9]+$ ]] || die "the create returned no ruleset id"
        echo "Created ruleset ${release_id} \"${RELEASE_NAME}\"."
    fi
fi

# --- Verify ------------------------------------------------------------------

# verify ID DESIRED: the live ruleset ID has everything in DESIRED.
verify() {
    local id="$1" desired="$2"
    gh api "repos/${REPO}/rulesets/${id}" >"$work/check.json" ||
        die "can't re-read ruleset ${id}"
    if ! jq -e "$JQ_LIB"' ($desired[0] | norm) | subset($live[0] | writable | norm)' -n \
        --slurpfile desired "$desired" --slurpfile live "$work/check.json" >/dev/null; then
        diff -u --label "ruleset ${id} (expected)" --label "ruleset ${id} (live)" \
            <(jq -S "$JQ_LIB"' norm' "$desired") <(jq -S "$JQ_LIB"' writable | norm' "$work/check.json") >&2 || true
        die "ruleset ${id} does not match what was applied"
    fi
}
verify "$MAIN_ID" "$work/main-desired.json"
verify "$release_id" "$work/release-desired.json"
echo "Verified: both rulesets match."

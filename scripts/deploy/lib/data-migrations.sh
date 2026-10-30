# shellcheck shell=bash
#
# Stop the crawler worker before pending data migrations (ECO-802). Sourced by
# scripts/deploy/deploy.sh, which runs stop_crawler_for_pending_migrations just before the
# crawler-migrate Job.
#
# The old crawler-worker keeps writing to the data database until deploy.sh applies the
# new crawler-worker.yaml, after the crawler-migrate Job. A migration that rewrites rows
# or takes table locks while it writes may fail, or race it. So when any data migration
# is pending, the worker is stopped first, by scripts/deploy/precheck-db-migration.sh
# --stop-crawler: the same step release-deploy.sh runs, with its checks (no backup or
# crawler-migrate Job running, no long transaction on the data database). deploy.sh then
# brings the worker back, on the new image, when it applies crawler-worker.yaml after the
# Job; if the Job fails, the worker stays stopped. With nothing pending, the worker is left
# running and nothing changes.
#
# Pending means what `crawler migrate --dry-run` lists, run in a Job like crawler-migrate's
# (the same image, Secret and network; k8s/manifests/crawler-migrate-job.yaml) that
# changes nothing.
#
# Uses: kubectl (deploy.sh's, on its pinned context), python3 with PyYAML, PROJECT_ROOT,
# DRY_RUN (true: only say what would run).

DATA_MIGRATION_CHECK_JOB="crawler-migrate-check"

# The check Job: crawler-migrate-job.yaml renamed and relabelled, running
# `crawler migrate --dry-run`, kept an hour after it finishes.
render_data_migration_check_job() {
    python3 - "$PROJECT_ROOT/k8s/manifests/crawler-migrate-job.yaml" "$DATA_MIGRATION_CHECK_JOB" <<'PY'
import sys
import yaml

path, name = sys.argv[1], sys.argv[2]
with open(path) as f:
    job = yaml.safe_load(f)
job["metadata"]["name"] = name
job["metadata"]["labels"]["app"] = name
job["spec"]["ttlSecondsAfterFinished"] = 3600
template = job["spec"]["template"]
template["metadata"]["labels"]["app"] = name
container = template["spec"]["containers"][0]
assert container["command"] == ["/app/crawler", "migrate"], container["command"]
container["command"] = ["/app/crawler", "migrate", "--dry-run"]
yaml.safe_dump(job, sys.stdout, sort_keys=False)
PY
}

# pending_data_migrations: the versions the check Job lists, one per line (none: nothing
# pending). Fails, saying why, when the Job can't be run or doesn't complete.
pending_data_migrations() {
    local manifest status="" logs
    manifest="$(render_data_migration_check_job)" || {
        echo "❌ could not render the ${DATA_MIGRATION_CHECK_JOB} Job from k8s/manifests/crawler-migrate-job.yaml" >&2
        return 1
    }
    # A Job cannot be re-run in place, so the previous one goes first.
    kubectl -n econ-graph delete job "$DATA_MIGRATION_CHECK_JOB" --ignore-not-found >/dev/null || return 1
    kubectl apply -f - <<<"$manifest" >/dev/null || return 1
    # Until Complete or Failed: about 10 minutes at most (the image is local, and a dry
    # run only reads the migrations table).
    for _ in $(seq 1 "${DATA_MIGRATION_CHECK_LOOKS:-120}"); do
        status="$(kubectl -n econ-graph get job "$DATA_MIGRATION_CHECK_JOB" \
            -o jsonpath='{.status.conditions[?(@.status=="True")].type}' 2>/dev/null || true)"
        case "$status" in *Complete* | *Failed*) break ;; esac
        sleep "${DATA_MIGRATION_CHECK_POLL:-5}"
    done
    logs="$(kubectl -n econ-graph logs "job/${DATA_MIGRATION_CHECK_JOB}" -c crawler-migrate 2>&1)" || logs=""
    case "$status" in
        *Complete*) ;;
        *)
            echo "❌ ${DATA_MIGRATION_CHECK_JOB} (crawler migrate --dry-run) did not complete (status: ${status:-none}):" >&2
            printf '%s\n' "$logs" | sed 's/^/   /' >&2
            return 1
            ;;
    esac
    # The versions are 14-digit lines; anything else in the log is logging.
    grep -E '^[0-9]{14}$' <<<"$logs" || true
}

# stop_crawler_for_pending_migrations: with any data migration pending, stop crawler-worker
# (precheck-db-migration.sh --stop-crawler) before the crawler-migrate Job; with none, leave
# it running. Fails when the check or the stop does, so the migrations don't run.
stop_crawler_for_pending_migrations() {
    local pending count
    if [ "${DRY_RUN:-false}" = true ]; then
        echo "[dry-run] would run crawler migrate --dry-run (Job ${DATA_MIGRATION_CHECK_JOB}) and, if any data" \
            "migration is pending, scripts/deploy/precheck-db-migration.sh --stop-crawler before crawler-migrate"
        return 0
    fi
    echo "🔎 Checking for pending data migrations (crawler migrate --dry-run)..."
    pending="$(pending_data_migrations)" || return 1
    if [ -z "$pending" ]; then
        echo "✅ No data migrations pending; crawler-worker keeps running"
        return 0
    fi
    count="$(wc -l <<<"$pending")"
    echo "🗄️  ${count} data migration(s) pending: $(tr '\n' ' ' <<<"$pending")"
    echo "   Stopping crawler-worker first (precheck-db-migration.sh --stop-crawler); it comes back"
    echo "   on the new image once crawler-migrate completes."
    if ! "$PROJECT_ROOT/scripts/deploy/precheck-db-migration.sh" --stop-crawler; then
        echo "❌ The precheck refused (above), so the data migrations were not applied." >&2
        return 1
    fi
}

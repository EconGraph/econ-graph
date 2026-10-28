# Crawler coverage and freshness runbook

The release checks require that each source has data for at least 95% of its series, and
that scheduled refresh keeps the series fresh (exit criteria 2 and 3 in
[ECO-5](https://linear.app/econgraph/issue/ECO-5)). This page covers the command
that reports both, the metrics and alerts behind them, and how QA forces each alert.

## `crawler coverage`

The `crawler` CLI ships in the crawler-worker image, and it reads the pod's
`DATA_DATABASE_URL`:

```sh
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage --json
# Exit criterion 2: exits non-zero, after printing the table, if any listed source is under 95%.
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage --fail-under 95
```

It prints one row for each registered source that supports scheduled series fetches,
provided its `data_sources` row is enabled or missing. A missing row reports zero discovered
series and fails the threshold check. The scheduler enqueues work; workers fetch it.
Static catalogs and SEC are excluded, as are sources in `FETCH_UNIMPLEMENTED`, which is
empty. A passing `--fail-under 95` says nothing about excluded sources, so check that every
release source appears in the table. See the [scheduler's source eligibility rules](../../backend/crates/econ-graph-crawler/src/scheduler.rs).

| Column | Meaning |
|---|---|
| `DISCOVERED` | Distinct series ids from catalog discovery (`series_metadata`) and fetched series (`economic_series`), active only |
| `WITH DATA` | Fetched series with at least one data point (`end_date` set) |
| `COVERAGE` | `WITH DATA / DISCOVERED`; `-` when nothing is discovered, which counts as below target |
| `OVERDUE` | Series with data whose last successful crawl is missing or at least two refresh intervals old; ten intervals for `not_found` series |
| `oldest success` | The earliest last successful crawl among the source's series |

Refresh intervals depend on frequency: 1 day for daily series, 7 days for weekly and monthly,
14 days for quarterly, 30 days for annual, and 7 days for anything else (see
[scheduler interval definitions](../../backend/crates/econ-graph-crawler/src/scheduler.rs)). Census counts only the ids its
adapter can fetch.

Coverage excludes series confirmed absent (`crawl_status = 'not_found'`) that never had
data. The worker deactivates catalog-only rows when their first fetch returns NotFound.
Historical series that later return NotFound still count because their data remains useful;
they become overdue after ten intervals. See the [coverage calculation](../../backend/crates/econ-graph-crawler/src/coverage.rs).

## Metrics and alerts

crawler-worker serves the same numbers on `:9102/metrics`, refreshed every 5 minutes
(`CRAWLER_COVERAGE_METRICS_INTERVAL_SECS`):
`crawler_coverage_series_discovered`, `crawler_coverage_series_with_data`,
`crawler_coverage_ratio`, `crawler_coverage_series_overdue` and
`crawler_coverage_oldest_success_timestamp_seconds`, all labelled by `source`, and
`crawler_coverage_last_refresh_timestamp_seconds`, when they were last refreshed. The queue
poller, which runs every 30 seconds, adds `crawler_last_failure_timestamp_seconds{source}`.

The alerts are in the [crawler rules ConfigMap](../../k8s/monitoring/prometheus-rules-crawler.yaml),
group `crawler.coverage`. Prometheus mounts it at `/etc/prometheus/rules`;
[`restart-k8s-rollout.sh`](../../scripts/deploy/restart-k8s-rollout.sh) applies it.
No Alertmanager is deployed, so firing alerts are visible in Prometheus without notifications: open `/alerts` on the Prometheus service
(`kubectl -n econ-graph port-forward svc/prometheus-service 9090`) or query
`ALERTS{alertname="CrawlerCrawlFailed"}`.

| Alert | Fires when |
|---|---|
| `CrawlerCoverageLow` | A source's coverage has been under 95% for 6 hours |
| `CrawlerSourceNotRefreshed` | A source has had overdue series for 30 minutes |
| `CrawlerCrawlFailed` | A crawl job of the source failed for good within the last hour |
| `CrawlerCoverageStale` | The worker is running, and the coverage gauges have remained more than 15 minutes out of date for 30 minutes |

`CrawlerCoverageStale` also detects an absent refresh gauge while the worker has an
available replica. It concerns metric collection; `CrawlerSourceNotRefreshed` concerns
source data freshness.

Coverage uses `last_crawled_at`, the last successful crawl, which a failed attempt leaves
unchanged. For an existing series with an ordinary failure, the scheduler waits two refresh
intervals after the later of that success and the latest failed attempt before retrying.
For `not_found` series it waits ten intervals. Coverage can therefore mark a series overdue
before its retry is eligible. Repeated ordinary failures keep the source freshness alert
firing until the series succeeds or is deactivated. Inspect failed series and deactivate
one only after confirming it is gone for good:

```sql
SELECT es.id, es.external_id, es.crawl_error_message
FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id
WHERE ds.name = 'Federal Reserve Economic Data (FRED)' AND es.crawl_status = 'failed';
-- then, for a series that is gone for good:
UPDATE economic_series SET is_active = FALSE WHERE id = '<id>';
```

To check the rules and run their unit tests locally, with promtool and PyYAML installed:
[`k8s/monitoring/check-prometheus-rules.sh`](../../k8s/monitoring/check-prometheus-rules.sh).
CI runs it in the "Prometheus Rules" job. The [coverage rule tests](../../k8s/monitoring/tests/crawler-coverage.test.yaml)
include stale and absent refresh gauges; these test rule evaluation with synthetic metrics,
whereas the procedures below exercise the deployed worker and Prometheus.

## Forcing the alerts (QA)

Use a QA cluster for these experiments. Record the worker's scheduler and coverage interval
settings before changing them, and restore those settings after each experiment. The examples
assume the defaults: scheduler enabled and coverage refreshed every 300 seconds. A GitOps sync
can revert temporary changes; keep the chosen settings in effect for the full pending period.
`kubectl set env` recreates the worker pod, so wait for each rollout before continuing.

`CrawlerCoverageStale` and `CrawlerMetricsMissing` also require kube-state-metrics, installed
from the [monitoring manifests](../../k8s/monitoring) by
[`scripts/deploy/deploy.sh`](../../scripts/deploy/deploy.sh). If the cluster also has the
old operator-based `PrometheusRule` (`kubectl -n econ-graph get prometheusrule crawler-alerts`),
remove that obsolete resource to avoid duplicate alerts.

All SQL on this page targets the **data plane database**, which contains the series and
crawl tables. The worker image has no `psql`. For the examples below, run a local `psql`
through a port-forward to `svc/data-postgres-service`, and configure its connection using
the database, user and credentials corresponding to the worker's `DATA_DATABASE_URL`.
Export `PGHOST=127.0.0.1`, `PGPORT` with the forwarded local port, `PGDATABASE`, and `PGUSER`,
and supply authentication through a protected password file or password prompt. These
settings are inherited by every subsequent `psql -c` command; a port-forward alone does not
configure the connection. Alternatively, run the SQL in `psql` inside the data Postgres pod
(`postgresql` StatefulSet), using its database and user. See [database settings](../../k8s/README.md#databases).

### Stale coverage gauges

Increase only the coverage metrics interval, leaving the worker and its metrics endpoint
running. The [worker configuration](../../backend/crates/econ-graph-crawler-worker/src/main.rs)
accepts this interval; the [gauge loop](../../backend/crates/econ-graph-crawler-worker/src/metrics.rs)
refreshes immediately at startup and then waits for the interval:

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_COVERAGE_METRICS_INTERVAL_SECS=7200
kubectl -n econ-graph rollout status deploy/crawler-worker
```

Confirm in Prometheus that `kube_deployment_status_replicas_available{namespace="econ-graph",
deployment="crawler-worker"}` is at least 1, and that
`crawler_coverage_last_refresh_timestamp_seconds` has a recent value that stays unchanged
across scrapes. Ensure no other worker instance is publishing fresh coverage gauges, because
the rule uses the maximum refresh timestamp across instances. If the initial refresh fails, investigate the worker log before timing the
test. Keep the pod running without restarts: a restart resets the experiment with another
immediate refresh. Once the timestamp is more than 900 seconds old, `CrawlerCoverageStale`
becomes pending. It fires after that condition has held for 30 minutes, subject to scrape
and evaluation timing. Queue metrics should continue updating throughout.

Restore the recorded interval (300 seconds for the default) and wait for the rollout:

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_COVERAGE_METRICS_INTERVAL_SECS=300
kubectl -n econ-graph rollout status deploy/crawler-worker
```

The startup refresh advances the timestamp, and the alert clears after Prometheus observes
it. Confirm that the timestamp continues advancing at the restored interval. This experiment
exercises stale gauges while the worker remains available after rollout; the local rule tests
also cover a refresh gauge that is entirely absent.

### Crawl failure

Enqueue a series id that doesn't exist. FRED answers it with HTTP 400 "The series does not
exist", and the worker fails the job without retrying:

```sh
kubectl -n econ-graph exec deploy/crawler-worker -- \
  /app/crawler enqueue --source FRED --series ECONGRAPH_QA_FORCED_FAILURE
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler status   # FAILED/24H goes up
```

Once the job reaches terminal failure, the next queue metrics poll (every 30 seconds)
updates `crawler_last_failure_timestamp_seconds{source="FRED"}`. `CrawlerCrawlFailed` fires
when Prometheus next scrapes and evaluates the rule. It clears an hour after the failure
if no further FRED jobs fail. Without a FRED key the job fails as well, because of the key
error.

### Stale source

Disable the scheduler and wait for its rollout. Let already queued work finish, then select
an active FRED series with data, without an active queue job, whose status is not `not_found`.
Record its id and original `last_crawled_at` so the experiment can be undone. Backdate that
series using the recorded id:

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_SCHEDULER=false
kubectl -n econ-graph rollout status deploy/crawler-worker
```

```sql
SELECT es.id, es.external_id, es.last_crawled_at
FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id
WHERE ds.name = 'Federal Reserve Economic Data (FRED)' AND es.is_active
  AND es.end_date IS NOT NULL AND es.crawl_status IS DISTINCT FROM 'not_found'
  AND NOT EXISTS (SELECT 1 FROM crawl_queue q WHERE q.source = 'FRED'
    AND q.series_id = es.external_id AND q.status IN ('pending', 'processing', 'retrying'))
LIMIT 1;
```

```sh
psql -c "UPDATE economic_series SET last_crawled_at = NOW() - INTERVAL '90 days'
  WHERE id = '<recorded-id>'"
```

Do not enqueue that series during the experiment. `crawler coverage` immediately shows
an additional overdue series for FRED if the selected series was fresh.
`CrawlerSourceNotRefreshed` fires after the next gauge refresh reports overdue series
continuously for 30 minutes. Restore the recorded timestamp, then restore the scheduler:

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_SCHEDULER=true
kubectl -n econ-graph rollout status deploy/crawler-worker
```

Confirm that the overdue count returns to its baseline after the next gauge refresh. For a
series outside a `legacy` dataset with no recent failed attempt delaying its retry, a running
scheduler can enqueue the backdated series on its next tick (normally within 5 minutes).
If the fetch succeeds before the alert's pending period elapses, the alert does not fire,
which demonstrates that refresh works.

### Low coverage

Check `crawler coverage` first: a source already under 95% fires the alert after six hours
continuously below target without synthetic rows. Otherwise disable the scheduler and wait
for its rollout before inserting rows, using the same commands as the stale-source test.
The scheduler normally enqueues active catalog-only rows; a worker fetching a synthetic id
can receive NotFound and deactivate the row, removing it from the coverage denominator.

Reserve a fresh prefix for this experiment. The examples use `QA_UNFETCHED_ECO627_`.
Run this preflight query and proceed only when all counts are zero; otherwise choose an
unused prefix and replace it consistently in the queries. This rules out already queued
jobs, which disabling the scheduler does not cancel. Keep other discovery jobs and manual
producers from creating or enqueueing these synthetic ids during the test.

```sql
SELECT
  (SELECT COUNT(*) FROM series_metadata
    WHERE external_id LIKE 'QA\_UNFETCHED\_ECO627\_%') AS metadata_rows,
  (SELECT COUNT(*) FROM economic_series
    WHERE external_id LIKE 'QA\_UNFETCHED\_ECO627\_%') AS fetched_rows,
  (SELECT COUNT(*) FROM crawl_queue
    WHERE source = 'FRED' AND series_id LIKE 'QA\_UNFETCHED\_ECO627\_%') AS queue_rows;
```

Insert `N` catalog rows, where `N` is a tenth of FRED's `DISCOVERED` rounded up, with a
minimum of 1000. Adding this many unfetched rows takes even 100% coverage below 95%:

```sh
psql -c "INSERT INTO series_metadata (source_id, external_id, title, is_active)
  SELECT ds.id, 'QA_UNFETCHED_ECO627_' || g, 'QA', TRUE
  FROM data_sources ds, generate_series(1, 1000) g
  WHERE ds.name = 'Federal Reserve Economic Data (FRED)'
  ON CONFLICT (source_id, external_id) DO NOTHING"
```

Replace `1000` with `N`. `crawler coverage --fail-under 95` must now fail for FRED.
Keep the scheduler disabled and verify that the rows remain active and unfetched, with no
queue jobs for the reserved prefix. `CrawlerCoverageLow` fires once the refreshed gauge
has stayed below 95% for six hours. Other completed fetches can raise coverage during that
period; check the ratio and increase `N` if necessary, restarting the pending period if the
ratio reaches 95%.

Delete the synthetic rows **before** restoring the scheduler so it cannot enqueue them:

```sql
DELETE FROM series_metadata sm USING data_sources ds
WHERE sm.source_id = ds.id AND ds.name = 'Federal Reserve Economic Data (FRED)'
  AND sm.external_id LIKE 'QA\_UNFETCHED\_ECO627\_%';
```

Restore the recorded scheduler setting and wait for its rollout. Check that FRED's coverage
returns to its baseline after the next gauge refresh and that the alert clears if coverage
is at least 95%.

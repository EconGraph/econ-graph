# Crawler coverage and freshness runbook

Release 1 exit criteria 2 and 3 say that each source has data for at least 95% of its
series, and that scheduled refresh keeps the series fresh. This page covers the command
that reports both, the metrics and alerts behind them, and how QA forces each alert.

## `crawler coverage`

The `crawler` CLI ships in the crawler-worker image, and it reads the pod's
`DATABASE_URL`:

```sh
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage --json
# Exit criterion 2: exits non-zero, after printing the table, if any listed source is under 95%.
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage --fail-under 95
```

It prints one row for each source that the refresh scheduler refreshes and whose
`data_sources` row is enabled. Sources the scheduler doesn't fetch yet (those in
`FETCH_UNIMPLEMENTED` in `scheduler.rs`, today World Bank and BEA, and the static
catalogs) are not listed and not measured: a passing `--fail-under 95` says nothing about
them, so check that every release source appears in the table.

| Column | Meaning |
|---|---|
| `DISCOVERED` | Distinct series ids from catalog discovery (`series_metadata`) and fetched series (`economic_series`), active only |
| `WITH DATA` | Fetched series with at least one data point (`end_date` set) |
| `COVERAGE` | `WITH DATA / DISCOVERED`; `-` when nothing is discovered, which counts as below target |
| `OVERDUE` | Series with data whose last successful crawl is older than twice their refresh interval |
| `oldest success` | The earliest last successful crawl among the source's series |

Refresh intervals depend on frequency: 1 day for daily series, 7 days for weekly and monthly,
14 days for quarterly, 30 days for annual, and 7 days for anything else (see
`backend/crates/econ-graph-crawler/src/scheduler.rs`). Census counts only the ids its
adapter can fetch.

## Metrics and alerts

crawler-worker serves the same numbers on `:9102/metrics`, refreshed every 5 minutes
(`CRAWLER_COVERAGE_METRICS_INTERVAL_SECS`):
`crawler_coverage_series_discovered`, `crawler_coverage_series_with_data`,
`crawler_coverage_ratio`, `crawler_coverage_series_overdue` and
`crawler_coverage_oldest_success_timestamp_seconds`, all labelled by `source`, and
`crawler_coverage_last_refresh_timestamp_seconds`, when they were last refreshed. The queue
poller, which runs every 30 seconds, adds `crawler_last_failure_timestamp_seconds{source}`.

The alerts are in `k8s/monitoring/prometheus-rules-crawler.yaml` (a ConfigMap that
Prometheus mounts at `/etc/prometheus/rules`; `scripts/deploy/restart-k8s-rollout.sh`
applies it), group `crawler.coverage`. No Alertmanager is deployed yet, so a firing alert is
seen in Prometheus, not notified: open `/alerts` on the Prometheus service
(`kubectl -n econ-graph port-forward svc/prometheus-service 9090`) or query
`ALERTS{alertname="CrawlerCrawlFailed"}`.

| Alert | Fires when |
|---|---|
| `CrawlerCoverageLow` | A source's coverage has been under 95% for 6 hours |
| `CrawlerSourceNotRefreshed` | A source has had overdue series for 30 minutes |
| `CrawlerCrawlFailed` | A crawl job of the source failed for good within the last hour |
| `CrawlerCoverageStale` | The worker runs but the coverage gauges haven't been refreshed for 15 minutes, for 30 minutes |

Until the scheduler also fetches series that discovery found but nobody enqueued (release 1
item DATA-13), those series count as discovered without data. `CrawlerCoverageLow` then
fires even though nothing regressed; `crawler coverage` shows the gap.

A series is overdue as soon as it misses twice its interval. The scheduler retries a failed
series only after twice its interval, so a single failed refresh makes it overdue, and
`CrawlerSourceNotRefreshed` fires, until a retry succeeds. A series that fails every time
(for example a discontinued id) keeps it firing. Find those series and fix or deactivate
them:

```sql
SELECT es.external_id, es.crawl_error_message
FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id
WHERE ds.name = 'Federal Reserve Economic Data (FRED)' AND es.crawl_status = 'failed';
-- then, for a series that is gone for good:
UPDATE economic_series SET is_active = FALSE WHERE id = '<id>';
```

To check the rules and run their unit tests locally, with promtool and PyYAML installed:
`k8s/monitoring/check-prometheus-rules.sh`. CI runs it in the "Prometheus Rules" job.

## Forcing the alerts (QA)

The coverage stale alert and `CrawlerMetricsMissing` also need kube-state-metrics
(`k8s/monitoring/kube-state-metrics-*.yaml`, applied by `deploy.sh`). If the cluster ran the
old operator-based `PrometheusRule` (`kubectl -n econ-graph get prometheusrule crawler-alerts`),
delete it, or its alerts fire twice.

The SQL below runs against the application database. The worker image has no `psql`, so run
it from the Postgres pod (`kubectl -n econ-graph exec -it <postgres pod> -- psql -U <user>
<database>`) or through `kubectl port-forward` with the `DATABASE_URL` from the
crawler-worker's Secret.

### Crawl failure

Enqueue a series id that doesn't exist. FRED answers it with HTTP 400 "The series does not
exist", and the worker fails the job without retrying:

```sh
kubectl -n econ-graph exec deploy/crawler-worker -- \
  /app/crawler enqueue --source FRED --series ECONGRAPH_QA_FORCED_FAILURE
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler status   # FAILED/24H goes up
```

Within about a minute, `crawler_last_failure_timestamp_seconds{source="FRED"}` moves to
now and `CrawlerCrawlFailed` fires for FRED. It resolves by itself an hour later. Without a
FRED key the job fails as well, because of the key error.

### Stale source

Stop the scheduler, then backdate one series:

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_SCHEDULER=false
psql -c "UPDATE economic_series SET last_crawled_at = NOW() - INTERVAL '90 days'
  WHERE id = (SELECT es.id FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id
              WHERE ds.name = 'Federal Reserve Economic Data (FRED)' AND es.is_active
                AND es.end_date IS NOT NULL LIMIT 1)"
```

`crawler coverage` shows `OVERDUE` 1 for FRED at once. `CrawlerSourceNotRefreshed` fires
30 minutes after the next gauge refresh. `kubectl set env` rolls the pod, and a GitOps sync
may revert it, so undo it explicitly with `CRAWLER_SCHEDULER=true`: the next tick re-crawls
the series and the alert clears.

If the scheduler keeps running, it re-crawls the backdated series on its next tick (within
5 minutes). The alert then never fires, which shows that refresh works.

### Low coverage

FRED may already be under 95% (see the DATA-13 note above); check `crawler coverage` first.
Otherwise insert catalog rows that are never fetched: `N` = a tenth of FRED's `DISCOVERED`
from `crawler coverage` (at least 1000), which takes a source at 100% below 95%:

```sh
psql -c "INSERT INTO series_metadata (source_id, external_id, title, is_active)
  SELECT ds.id, 'QA_UNFETCHED_' || g, 'QA', TRUE
  FROM data_sources ds, generate_series(1, 1000) g
  WHERE ds.name = 'Federal Reserve Economic Data (FRED)'
  ON CONFLICT (source_id, external_id) DO NOTHING"
```

(replace `1000` with `N`).

`crawler coverage --fail-under 95` then fails for FRED, and `CrawlerCoverageLow` fires 6
hours later. Delete the rows afterwards:
`DELETE FROM series_metadata WHERE external_id LIKE 'QA\_UNFETCHED\_%'`.

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

It prints one row for each source that the refresh scheduler refreshes and whose
`data_sources` row is enabled. Sources the scheduler doesn't fetch yet (those in
`FETCH_UNIMPLEMENTED` in `scheduler.rs`, today World Bank and BEA, and the static
catalogs) are not listed and not measured: a passing `--fail-under 95` says nothing about
them, so check that every release source appears in the table.

| Column | Meaning |
|---|---|
| `DISCOVERED` | Distinct current-publication series from catalog discovery and fetched series |
| `WITH DATA` | Current-publication series with at least one data point (`end_date` set) |
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

A series is overdue as soon as it misses twice its interval. Investigate transient failures
rather than deactivating stored series. Publication state is separate from fetch health:
retired series keep their observations and remain reachable by link. Non-active publication
states are excluded from automatic fetch scheduling and live coverage/freshness denominators;
complete catalog discovery can reactivate a series that returns upstream.

## Catalog selection and retained series

BEA and FRED discovery selects from current upstream catalogs using declarative rules in
`backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml` and `fred.toml`.
The worker image includes these files under `$CRAWLER_DATA_DIR/catalog_selection/`.
Rules select BEA dataset/table families and frequencies, or FRED release/category catalogs
with optional title, id and frequency predicates. Small explicit overrides are supported.
Deploy the updated data schema before the frontend that selects lifecycle fields.
Change rules and restart the worker to pick them up. Titles come from the source; historical
dimension labels stay additive and are never used as the current upstream catalog.

Selection limits bound expansion before observation jobs are enqueued. A rejected selection
raises `CrawlerCatalogGuardRejected`; inspect the upstream catalog and worker warning before
increasing limits. Do not blindly raise a limit to silence an alert.

One atomic data migration creates `series_lifecycle` (including fetch evidence),
`catalog_lifecycle_scopes`, and `catalog_reference_snapshots`. Apply it with `crawler migrate`
before deploying code that reads lifecycle state. A complete successful
catalog under unchanged selection rules supplies absence evidence. Failed or partial catalogs
supply none, and a rule change resets the comparison scope. Three consecutive successful
absences confirm a missing series. A snapshot that suddenly loses more than 20% of the
previously present scope, or an empty scope, is guarded rather than treated as removal.
On first adoption, retained BEA histories that match the configured rules establish a
baseline without counting absence. Three subsequent complete snapshots are required for
a missing badge. Histories outside those rules are not enrolled or retired by that scope.
Confirmed fetch-level NotFound responses use a separate consecutive-failure counter, reset
by a successful fetch. Successful persistence shares the source lock with missing-result
writes so normal fetches and forced refetches cannot race; upstream HTTP stays concurrent.
Fetch success never erases catalog absence evidence: a delisted
series can still serve historical observations.

| Status | Public behavior |
|---|---|
| `active` | Normal listing and ranking |
| `discontinued` | History remains public with a publication badge |
| `missing_from_catalog` | History remains public, badged and ranked below current series |
| `retired` | Confirmed missing upstream; stored history stays public |
| `restricted` | Hidden from public series and observation reads; retained internally |
| `superseded` | History stays separate, with a human-confirmed successor link |

Data Sources shows lifecycle counts. Series pages distinguish proposed successors from
confirmed successors. A proposed match is a review hint: compare source definitions, units
and base periods before confirming it against the data database:

```sh
crawler confirm-successor --source FRED --series OLD_ID --successor NEW_ID
```

This command records the reviewed link. It never merges histories, moves annotations or
deletes either series. Keeping values separate avoids joining numbers with different bases.

The worker publishes `crawler_series_lifecycle{source,status}` counts and
`crawler_series_lifecycle_transitions_total{source,status}` transition counters.
`CrawlerSeriesPublicationChanged` is informational and points operators to source counts
and successor candidates. `crawler_catalog_guard_rejections_total{source,reason}` distinguishes
`volume` and `mass_disappearance`. Counts refresh from the database on the coverage polling
interval. Event counters are process-local: an increase requires scraping around the event.
Alert rules do not send notifications without Alertmanager.

To check the rules and run their unit tests locally, with promtool and PyYAML installed:
`k8s/monitoring/check-prometheus-rules.sh`. CI runs it in the "Prometheus Rules" job.

## Forcing the alerts (QA)

The coverage stale alert and `CrawlerMetricsMissing` also need kube-state-metrics
(`k8s/monitoring/kube-state-metrics-*.yaml`, applied by `deploy.sh`). If the cluster ran the
old operator-based `PrometheusRule` (`kubectl -n econ-graph get prometheusrule crawler-alerts`),
delete it, or its alerts fire twice.

The SQL below runs against the application database. The worker image has no `psql`, so run
it from the Postgres pod (`kubectl -n econ-graph exec -it <postgres pod> -- psql -U <user>
<database>`) or through `kubectl port-forward` with the `DATA_DATABASE_URL` from the
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

Check `crawler coverage` first: a source already under 95% fires the alert without this
step. Otherwise insert catalog rows that are never fetched: `N` = a tenth of FRED's `DISCOVERED`
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

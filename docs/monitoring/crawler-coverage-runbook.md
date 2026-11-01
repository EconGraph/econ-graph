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
| `DISCOVERED` | Distinct current-publication series from catalog discovery and fetched series |
| `WITH DATA` | Current-publication series with at least one data point (`end_date` set) |
| `COVERAGE` | `WITH DATA / DISCOVERED`; `-` when nothing is discovered, which counts as below target |
| `OVERDUE` | Series with data whose last successful crawl is missing or at least two refresh intervals old; ten intervals for `not_found` or `absent` series |
| `oldest success` | The earliest last successful crawl among the source's series |

Refresh intervals depend on frequency: 1 day for daily series, 7 days for weekly and monthly,
14 days for quarterly, 30 days for annual, and 7 days for anything else (see
[scheduler interval definitions](../../backend/crates/econ-graph-crawler/src/scheduler.rs)). Census counts only the ids its
adapter can fetch.

Coverage excludes series that never had data when their fetch status is `not_found`
or `absent`. For sources without catalog lifecycle tracking, the worker deactivates
catalog-only rows on their first NotFound. FRED and BEA track lifecycle instead: fetch misses
do not count toward retirement while the current accepted catalog lists the series; otherwise
three consecutive NotFound responses or six consecutive omissions retire it. Tracked
catalog-only rows remain in coverage until their publication state becomes non-active. Historical series
with NotFound or absent fetch status still count while their publication state is active;
they become overdue after ten intervals. See the [coverage calculation](../../backend/crates/econ-graph-crawler/src/coverage.rs).

## Metrics and alerts

crawler-worker serves the same numbers on `:9102/metrics`, refreshed every 5 minutes
(`CRAWLER_COVERAGE_METRICS_INTERVAL_SECS`):
`crawler_coverage_series_discovered`, `crawler_coverage_series_with_data`,
`crawler_coverage_ratio`, `crawler_coverage_series_overdue` and
`crawler_coverage_oldest_success_timestamp_seconds`, all labelled by `source`, and
`crawler_coverage_last_refresh_timestamp_seconds`, when they were last refreshed. The queue
poller, which runs every 30 seconds, adds `crawler_last_failure_timestamp_seconds{source}`.
The worker also exports `crawler_scheduler_enabled`: 1 when automatic scheduling is on,
0 when it was explicitly disabled. The value describes startup configuration, not whether
scheduler ticks are succeeding. It defaults to 1 until startup configuration explicitly
sets the mode to 0.

The alerts are in the [crawler rules ConfigMap](../../k8s/monitoring/prometheus-rules-crawler.yaml),
group `crawler.coverage`. Prometheus mounts it at `/etc/prometheus/rules`;
[`restart-k8s-rollout.sh`](../../scripts/deploy/restart-k8s-rollout.sh) applies it.
No Alertmanager is deployed, so firing alerts are visible in Prometheus without notifications: open `/alerts` on the Prometheus service
(`kubectl -n econ-graph port-forward svc/prometheus-service 9090`) or query
`ALERTS{alertname="CrawlerCrawlFailed"}`.

| Alert | Fires when |
|---|---|
| `CrawlerSchedulerDisabled` | Every discovered primary worker reports scheduling disabled with a successful scrape (info, no delay) |
| `CrawlerCoverageLow` | A source's coverage has been under 95% for 6 hours |
| `CrawlerSourceNotRefreshed` | A source has had overdue series for 30 minutes and automatic scheduling is not explicitly disabled |
| `CrawlerCrawlFailed` | A crawl job of the source failed for good within the last hour |
| `CrawlerCoverageStale` | The worker is running, and the coverage gauges have remained more than 15 minutes out of date for 30 minutes |

`CrawlerCoverageStale` also detects an absent refresh gauge while the worker has an
available replica. It concerns metric collection; `CrawlerSourceNotRefreshed` concerns
source data freshness.

A series is overdue as soon as it misses twice its interval (ten intervals for `not_found` or `absent`). Investigate transient failures
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

Every discovery publishes `crawler_catalog_rule_matches{source,rule}`, the number of catalog
entries each rule matched; `rule` is the rule's position in its file and the catalog it walks
(`6:Regional:SAGDP2`, `0:release:53`). A rule that matches nothing logs a warning and raises
`CrawlerCatalogRuleMatchesNothing`, even when the other rules keep discovery succeeding. That
usually means upstream renamed the table or release: the rule's previously selected series
may start counting as absent, so update the rule. Each worker also publishes
`crawler_catalog_rule_configured{source,rule}` for the rules it loaded at startup, and the
alert fires only while a worker still reports the rule as configured; after a rule is edited
and the workers restart, the old rule's label stops alerting within 15 minutes.

One atomic data migration creates `series_lifecycle` (including fetch evidence),
`catalog_lifecycle_scopes`, and `catalog_reference_snapshots`. Apply it with `crawler migrate`
before deploying code that reads lifecycle state. A complete successful
catalog under unchanged selection rules supplies absence evidence. Failed or partial catalogs
supply none, and a rule change resets the comparison scope. Three consecutive successful
absences confirm a missing series. A snapshot that drops more than 20% of the series listed
by the last accepted catalog of the same rules, or an empty snapshot, is guarded rather than
treated as removal (see [Reviewing a guarded catalog](#reviewing-a-guarded-catalog)).
The guard compares each snapshot only with the previous accepted one, so a catalog that
shrinks by less than 20% per refresh is never guarded, however far it shrinks in total.
This is deliberate: it lets normal gradual retirements through without operator review, and
the three-absence rule still applies to every series.
On first adoption, retained BEA histories that match the configured rules establish a
baseline without counting absence. Members already absent at that baseline were never
listed by an accepted catalog, so they do not count toward the 20%; three subsequent
complete snapshots are required for their missing badge. Histories outside those rules are
not enrolled or retired by that scope.
Absence counts only for series the whole walked catalog no longer lists. A series that
upstream still lists but the rules no longer select (for example a FRED series retitled out
of a `title_contains` rule, or a BEA table retitled out of a title rule) keeps its
publication state and stays scheduled. The worker logs a warning, publishes
`crawler_catalog_unselected_series{source}`, and `CrawlerCatalogSeriesUnselected` (info)
suggests reviewing the rules. That gauge changes only when a complete catalog snapshot is
reconciled; between discoveries it keeps the last snapshot's count. A FRED series that
moves out of the walked releases and categories altogether is no longer listed anywhere the
walk looks, so it becomes `missing_from_catalog` after three catalogs and stops updating,
even though FRED still publishes it; add it as an `include` override to keep it current.

The BEA rule `prefix = "SAGDP2"` selects every Regional table whose name starts with
`SAGDP2` except those in `exclude`: today `SAGDP2`, and `SAGDP2N` while BEA lists the table
under that name. `SAGDP2S`, the discontinued SIC-basis series, is excluded.
Confirmed fetch-level NotFound responses use a separate consecutive-failure counter, reset
by a successful fetch. Only definite answers count: a FRED 400 "series does not exist" or an
HTTP 404. Three in a row retire a series from fetching. An upstream incident that answers
every request with 404 can fetch-retire many series at once; that heals itself, because
fetch-retired series are probed every 12 refresh intervals and the first successful fetch
reactivates each one. A BEA GetData response that omits a
requested line or area is weaker evidence (`crawl_status` `absent`) with its own counter:
six in a row retire the series from fetching. That includes an empty response, which is what
BEA returns when every requested Regional area is gone or a NIPA table is no longer published
at that frequency. Both kinds back off ten refresh intervals between attempts, so an omitted
line takes at least as long to retire as a NotFound, and a table that is briefly empty during
a BEA update cannot retire anything. Neither NotFound nor omissions are counted for a series
the current accepted catalog lists outright: the catalog alone decides its publication
state, and its failed fetches still raise the ordinary crawl-failure alerts. Omissions do
count for lines under tables the rules no longer select and for legacy BEA rows outside the
rules. Only an outright listing or a successful fetch clears a fetch retirement; a series
that is merely still published outside the rules stays retired. Any return to active clears
both fetch counters. A FRED `/series`
response with no series is a transient failure and counts as neither. The
scheduler still probes a fetch-retired series once every 12 refresh intervals (a series with
no stored history: at most once per queue retention), and a successful probe makes it active
again. A complete catalog that lists the series again also makes it active. Per-series writers (fetch success,
NotFound, restriction) serialize only on the same series, so normal fetches and forced
refetches of one series cannot race while different series persist concurrently; a complete
catalog reconciliation briefly excludes them for its source. Fetch success never erases
catalog absence evidence: a delisted series can still serve historical observations.

| Status | Public behavior |
|---|---|
| `active` | Normal listing and ranking |
| `discontinued` | History remains public with a publication badge |
| `missing_from_catalog` | History remains public, badged and ranked below current series |
| `retired` | Confirmed missing upstream; stored history stays public |
| `restricted` | Hidden from public series and observation reads; retained internally |
| `superseded` | History stays separate, with a human-confirmed successor link |

`restricted` is sticky. Once a catalog or a fetch reports a legal restriction, no later
catalog, rule change, successful fetch or accepted guarded catalog clears it; the series stays
hidden even if upstream lifts the restriction. Lifting it is a reviewed manual change in the
data database. First confirm upstream that the restriction is gone (otherwise the next
catalog or fetch restricts it again), then run this in one transaction with `psql` against
the data database, setting the source's `data_sources.name` and the series id:

```sql
\set source 'Federal Reserve Economic Data (FRED)'
\set series 'SERIES_ID'
BEGIN;
-- Lifts only a row that is restricted now, and only reactivates rows for that row. Raises,
-- and so rolls back, unless exactly one lifecycle row changed.
CREATE TEMP TABLE lifted ON COMMIT DROP AS
WITH changed AS (
  UPDATE series_lifecycle l SET status = 'active', since = NULL
    FROM data_sources ds
   WHERE ds.id = l.source_id AND ds.name = :'source' AND l.external_id = :'series'
     AND l.status = 'restricted'
  RETURNING l.source_id, l.external_id)
SELECT * FROM changed;
DO $$ BEGIN
  IF (SELECT count(*) FROM lifted) <> 1 THEN
    RAISE EXCEPTION 'expected exactly one restricted series, found %', (SELECT count(*) FROM lifted);
  END IF;
END $$;
UPDATE economic_series es SET is_active = true, crawl_status = 'success'
  FROM lifted WHERE es.source_id = lifted.source_id AND es.external_id = lifted.external_id;
UPDATE series_metadata sm SET is_active = true
  FROM lifted WHERE sm.source_id = lifted.source_id AND sm.external_id = lifted.external_id;
COMMIT;
```

The series' observations were retained while it was restricted; the next scheduled fetch
brings it current.

Data Sources shows lifecycle counts. Series pages distinguish proposed successors from
confirmed successors. A proposed match is a review hint: compare source definitions, units
and base periods before confirming it against the data database:

```sh
crawler confirm-successor --source FRED --series OLD_ID --successor NEW_ID
```

This command records the reviewed link. It never merges histories, moves annotations or
deletes either series. Keeping values separate avoids joining numbers with different bases.
It cannot be undone with a command, and `superseded` takes precedence over everything but a
restriction: a later catalog that lists the old id again does not reactivate it. Confirm only
after the review above.

### Before deploying catalog selection

The first deployment of catalog-driven discovery, and any later change to the selection
files, needs two checks against the data database and the live APIs:

1. **FRED volume.** The GDP release walk is capped at `max_series` in
   `catalog_selection/fred.toml` (500). Count what the rule selects today, and record the
   count in the release ticket. The script reads every page until it has the catalog's whole
   `count`, matches frequencies the way the rule does (`Quarterly` also matches
   `Quarterly, End of Period`), and passes the API key in a curl config file rather than on
   the command line:

   ```sh
   set -euo pipefail
   dir=$(mktemp -d); trap 'rm -rf "$dir"' EXIT
   ( umask 077; printf 'get\ndata-urlencode = "api_key=%s"\n' "$FRED_API_KEY" > "$dir/key.cfg" )
   offset=0
   while :; do
     curl -sfK "$dir/key.cfg" -o "$dir/page$offset.json" \
       "https://api.stlouisfed.org/fred/release/series?release_id=53&file_type=json&limit=1000&offset=$offset"
     count=$(jq .count "$dir/page$offset.json")
     offset=$((offset + 1000))
     [ "$offset" -lt "$count" ] || break
   done
   jq -s '[.[].seriess[] | select((.title | ascii_downcase | contains("gross domestic product"))
       and (.frequency as $q | any("Quarterly", "Annual"; . as $f | $q == $f or ($q | startswith($f + ",")))))]
     | length' "$dir"/page*.json
   ```

   Add the `include` overrides. If the total exceeds `max_series`, every FRED discovery
   fails the volume guard; raise `max_series` in the deployed `catalog_selection/fred.toml`
   (it does not change the selection scope) and restart the workers. Each newly selected
   series then fetches its full vintage history, in windows of up to 1,000 vintage dates.
2. **BEA rows that will stay inactive.** Rows an earlier unlisted-series sweep deactivated
   stay inactive, and unscheduled, until a catalog lists them outright. Count them before
   deploying and record the number:

   ```sql
   -- Table prefixes from the shipped catalog_selection/bea.toml rules.
   SELECT count(*) FROM economic_series es JOIN data_sources ds ON ds.id = es.source_id
    WHERE ds.name = 'Bureau of Economic Analysis (BEA)' AND NOT es.is_active
      AND (es.external_id ~ '^bea_nipa/(T10101|T10105|T10106|T20100|T20600|T20804)'
        OR (es.external_id LIKE 'bea_regional/SAGDP2%'
            AND es.external_id NOT LIKE 'bea_regional/SAGDP2S.%'));
   ```

### Reviewing a guarded catalog

A guarded snapshot changes no absence evidence; the worker keeps the newest one for review
and warns `catalog mass-disappearance guard`. Every later snapshot is compared with the last
accepted catalog, so a real upstream removal of more than 20% stays guarded until an operator
accepts it. Against the data database, list the series the snapshot would count as absent:

```sh
crawler accept-guarded-catalog --source BEA
```

The output starts with the time the snapshot was guarded. Check the listed ids upstream (a
table rename, a removed release, or an incomplete response). If the snapshot is right, apply
it, passing that time exactly as printed:

```sh
crawler accept-guarded-catalog --source BEA --accept --guarded-at 2026-10-30T18:02:15.123456+00:00
```

If a newer snapshot was guarded after your review, the command refuses and nothing changes;
review the new one instead.

Accepting records one absence for each newly missing series it listed, exactly as an
unguarded catalog would,
and makes the snapshot the new comparison baseline. Two further complete catalogs are still
needed before those series show as missing. Nothing is deleted. If the snapshot was
incomplete, do not accept it; the next complete catalog that passes the guard replaces it.

While a guarded snapshot waits, `crawler_catalog_guarded_snapshot_pending{source}` is 1 (from
the database, refreshed after each catalog and on the coverage polling interval) and
`CrawlerCatalogGuardedSnapshotPending` stays firing, so a pending review is not forgotten
after the one-off `CrawlerCatalogGuardRejected` alert clears.

The worker publishes `crawler_series_lifecycle{source,status}` counts and
`crawler_series_lifecycle_transitions_total{source,status}` transition counters.
`CrawlerSeriesPublicationChanged` is informational and points operators to source counts
and successor candidates. `crawler_catalog_guard_rejections_total{source,reason}` distinguishes
`volume` and `mass_disappearance`. Counts refresh from the database on the coverage polling
interval. Event counters are process-local: an increase requires scraping around the event.
Alert rules do not send notifications without Alertmanager.

To check the rules and run their unit tests locally, with promtool and PyYAML installed:
[`k8s/monitoring/check-prometheus-rules.sh`](../../k8s/monitoring/check-prometheus-rules.sh).
CI runs it in the "Prometheus Rules" job. The [coverage rule tests](../../k8s/monitoring/tests/crawler-coverage.test.yaml)
include stale and absent refresh gauges; these test rule evaluation with synthetic metrics,
whereas the procedures below exercise the deployed worker and Prometheus.

## Holding a crawl static and running a deliberate refresh

A persistent primary can hold a completed crawl static with
`CRAWLER_SCHEDULER=false`. Record this setting in the deployment configuration so the
next deploy preserves it; for an operator-controlled temporary change:

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_SCHEDULER=false
kubectl -n econ-graph rollout status deploy/crawler-worker
```

Disabling the scheduler stops automatic enqueueing. It does not cancel queued work:
the worker continues draining the queue, and its queue and coverage pollers keep running.
Confirm `crawler status` has no pending, processing or retrying jobs before considering
the crawl static. Startup reference-data refreshes still run; this setting is not a
read-only database mode.

On the primary (`kubernetes_namespace="econ-graph"`), `CrawlerNoTraffic` and
`CrawlerSourceNotRefreshed` are suppressed only when every discovered worker scrape
target reports `crawler_scheduler_enabled=0`: idle traffic and aging observations are
expected. An enabled worker, an old image without the mode metric, or a failed scrape
prevents suppression of idle/freshness warnings while measurements are available.
If a failed scrape makes those measurements stale, `CrawlerMetricsMissing` and
`CrawlerCoverageStale` report the missing telemetry instead; absent data cannot establish
no traffic or overdue series. Branch schedulers do not affect the primary's mode or these
two alerts. `CrawlerNoCompletions` stays active when jobs are queued, because manual work must
still complete. `CrawlerCoverageStale`, low coverage, job failures and worker/scrape
failures also remain active: the independent metric pollers must stay healthy.
`CrawlerSchedulerDisabled` remains visible at info level while every discovered primary
worker reports static mode and has a successful scrape. Missing mode metrics or a failed
scrape prevent this all-disabled indication, just as they prevent suppression above. All crawler alerts select the primary namespace (`econ-graph`); branch
worker traffic, coverage and failures do not affect them.
No blanket Alertmanager silence is required.

A deliberate refresh uses a selected source and series on the converted data plane,
a tagged release image, and a verified host backup. For example, two existing FRED
series can be refreshed without enabling the automatic scheduler:

```sh
kubectl -n econ-graph exec deploy/crawler-worker -- \
  /app/crawler enqueue --source FRED --series GDP,UNRATE
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler status
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler coverage --fail-under 95
```

The refresh is complete when those jobs have completed; coverage is a separate check,
not proof of an empty queue. `crawler discover --source FRED` refreshes the source catalog
but is not itself a full crawl of every series. A full-source refresh uses a selected set
of series or a deliberate temporary scheduler run. Both paths preserve the data database and existing series rows: prior series IDs
remain present and match the stable-ID function. The static deployment retains a verified
backup of the completed crawl, with `CrawlerSchedulerDisabled` showing its mode and
the failure and telemetry alerts showing its health. The rule tests simulate alert
behavior; they do not verify a live refresh or backup.

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
a current-publication FRED series with data, without an active queue job, whose status
is not `not_found`.
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
  AND NOT EXISTS (SELECT 1 FROM series_lifecycle sl WHERE sl.source_id = es.source_id
    AND sl.external_id = es.external_id AND sl.status <> 'active')
  AND NOT EXISTS (SELECT 1 FROM crawl_queue q WHERE q.source = 'FRED'
    AND q.series_id = es.external_id AND q.status IN ('pending', 'processing', 'retrying'))
LIMIT 1;
```

```sh
psql -c "UPDATE economic_series SET last_crawled_at = NOW() - INTERVAL '90 days'
  WHERE id = '<recorded-id>'"
```

`crawler coverage` shows an increased `OVERDUE` count for FRED after the edit, but
`CrawlerSourceNotRefreshed` stays quiet while the scheduler is explicitly disabled.
The promtool tests verify that overdue data alerts after 30 minutes when scheduling is
enabled or its mode metric is missing. Restore the original scheduler setting after QA;
`kubectl set env` rolls the pod, and a GitOps sync may revert it. Enabling the scheduler
can enqueue a current, non-legacy series on its next tick (normally within 5 minutes),
unless a recent failed attempt delays its retry. A successful fetch can repair the fixture
before an alert fires.
Restore the recorded timestamp after the experiment and confirm the overdue count
returns to its baseline after the next gauge refresh.

```sh
kubectl -n econ-graph set env deploy/crawler-worker CRAWLER_SCHEDULER=true
kubectl -n econ-graph rollout status deploy/crawler-worker
```

Use the recorded scheduler value rather than `true` if scheduling was disabled before QA.
Confirm that the overdue count returns to its baseline after the next gauge refresh.

### Low coverage

Check `crawler coverage` first: a source already under 95% fires the alert after six hours
continuously below target without synthetic rows. Otherwise disable the scheduler and wait
for its rollout before inserting rows, using the same commands as the stale-source test.
The scheduler normally enqueues active catalog-only rows. These synthetic FRED ids are not
listed by an accepted catalog, so three consecutive NotFound fetches can retire them through
lifecycle tracking, removing them from the coverage denominator. A first NotFound alone does
not deactivate a FRED catalog-only row. Keep the fixtures unscheduled throughout the test.

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

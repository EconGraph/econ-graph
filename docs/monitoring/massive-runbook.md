# Massive runbook

Massive, a market data vendor, sells the daily stock bars that the crawler stores in the
`stock_price_daily` Iceberg dataset ([ECO-871](https://linear.app/econgraph/issue/ECO-871)).
The data sits behind `catalog.massive_market_data:read`, a role nobody holds. This page covers
the API key, the Massive alerts, and how to force each alert in a branch environment.

## API key

The crawler worker reads the key from `MASSIVE_API_KEY`. Its value comes from the
`massive-api-key` entry of Secret `crawler-api-keys` in namespace `econ-graph`, which is
optional. It is a paid key: each deployment needs its own subscription, and the key is never
committed in plain text.

Without the key the worker skips the source entirely: no Massive discovery, refreshes,
backfill or coverage gauges, and none of the Massive alerts below can fire. At startup it logs
one warning, "source skipped: its API key is not set ...", with fields `source=MASSIVE` and
`env=MASSIVE_API_KEY`. With the key, the worker registers the source on its next start, and the
scheduler enqueues the daily refreshes.

### Adding or rotating the key

On the Talos platform (Flux, `platform/`), `crawler-api-keys` is a SOPS-encrypted input; see
[Secret inputs](../../platform/README.md#secret-inputs).

1. In the deployment input workspace, open the encrypted Secret with
   `sops platform/secrets/crawler-api-keys.sops.yaml`. If it doesn't exist yet, write a
   `Secret` named `crawler-api-keys` in namespace `econ-graph`, encrypt it with
   `sops --encrypt`, and list it in `platform/secrets/kustomization.yaml`.
2. Set `stringData.massive-api-key` to the key, which the Massive dashboard shows for the
   account, and save. sops re-encrypts on save. Never put the key on a command line.
3. Commit and push the ciphertext. Flux's `platform-secrets` Kustomization applies it.
4. Environment variables are read at pod start, so restart the worker:
   `kubectl -n econ-graph rollout restart deployment/crawler-worker`.

On the kind cluster, run `scripts/deploy/set-crawler-keys.sh` in your own terminal. It prompts
for `MASSIVE_API_KEY` with hidden input, writes `massive-api-key`, and restarts the worker.

Afterwards, check that the worker log has no "source skipped" warning for MASSIVE, and that
`crawler_coverage_series_discovered{source="MASSIVE"}` appears in Prometheus. Removing the key
and restarting the worker turns the source off again. Stored data is kept.

## Alerts

The Massive alerts are in group `crawler.massive` of
[rules-crawler.yaml](../../platform/observability/rules-crawler.yaml). All of them match only
`namespace="econ-graph"`, so a branch environment can neither fire nor mute them. The
per-source crawler alerts (`CrawlerSourceFailing`, `CrawlerCrawlFailed`, `CrawlerCoverageLow`,
`CrawlerSourceNotRefreshed`, and the error-rate and rate-limit alerts with
`crawler_type="massive"`) cover Massive like any other source; see the
[crawler coverage runbook](crawler-coverage-runbook.md).

| Alert | Fires when |
|---|---|
| `MassiveApiKeyRejected` | Massive answered 401, or a "not entitled" 403, in the last 15 minutes |
| `MassiveDataStale` | `crawler_source_observation_lag_trading_days{source="MASSIVE"}` has been 1 or more for 6 hours, or the source runs but exports no lag gauge |
| `MassiveBackfillFailed` | A backfill run ended with outcome `failure`, or a `massive-backfill-*` Job failed, in the last day |

### MassiveApiKeyRejected

The key was revoked or rotated, or the subscription lapsed or was downgraded. A 403 for a date
outside the plan's history window is not counted. Check the subscription on the Massive
dashboard. If the key changed, install the new one as above. A lapsed subscription needs
renewing. Until then, every Massive job fails and `MassiveDataStale` follows.

### MassiveDataStale

The lag gauge counts the trading days after the newest stored day whose refresh slot (11:30
America/New_York on the next calendar day) has passed, on the NYSE calendar. Weekends,
holidays and unscheduled closures have no slot, so they don't raise it, and the rule needs no
headroom for them. The worker recomputes the gauge from storage at startup and on its coverage
interval. Until data is stored (none in the last 45 days, or no table yet) the lag is 260, its
cap, so the alert fires 6 hours after the key is first installed unless the backfill has
stored recent bars by then. It clears once the backfill and the daily refresh have caught up.

Check `MassiveApiKeyRejected` and the per-source alerts for MASSIVE first. Then look for
failed jobs with `SELECT series_id, kind, error_message FROM crawl_queue WHERE source =
'MASSIVE' AND status = 'failed' ORDER BY updated_at DESC LIMIT 20`, and for Massive errors in
the crawler-worker log. If the lag gauge is missing while coverage gauges exist, the worker's
freshness computation is failing. `CrawlerCoverageStale` shows whether the coverage loop
itself is stuck.

### MassiveBackfillFailed

The backfill runs as a Job created from the suspended CronJob `massive-backfill`
([ECO-214](https://linear.app/econgraph/issue/ECO-214)), one run per pod. PodMonitor
`econgraph-massive-backfill` scrapes the pod's `crawler_backfill_*` metrics while it runs and
for 120 seconds after. The alert fires for a day after a run reports outcome `failure`, or
after a Job named `massive-backfill-*` fails without reporting (a crash, OOM or eviction, seen
through kube-state-metrics). Name each Job with that prefix, for example
`kubectl -n econ-graph create job --from=cronjob/massive-backfill massive-backfill-$(date +%Y%m%d%H%M)`.

`crawler_backfill_days_remaining{source="MASSIVE"}` shows how much history is left to load.
Find the error with `kubectl -n econ-graph logs job/<name>`. Common causes are the key (see
above), the vendor's rate limits, and disk space on the host object store (the disk alerts
from ECO-867). Fix the cause and create a new Job as above.

A 429 from Massive on a paid plan is a vendor fault, not a sign that the crawler is too fast.
Check Massive's status page before lowering the source's rate.

## Forcing the alerts (QA)

Use a branch environment with its own Massive key, and its own Prometheus or rules copy whose
namespace matcher names the branch namespace. The production rules ignore branch namespaces.

- `MassiveApiKeyRejected`: set `massive-api-key` to a wrong value and restart the worker. The
  next Massive request is rejected, and the alert fires within a minute. Restore the key.
- `MassiveDataStale`: with the source running, scale the worker to 0 replicas past a trading
  day's slot, then back to 1 with the scheduler disabled (`CRAWLER_SCHEDULER=false`). The lag
  gauge reports 1, and the alert fires 6 hours later. Re-enable the scheduler to clear it.
- `MassiveBackfillFailed`: create a backfill Job while the key is wrong. The run fails, and
  the alert fires once its final metrics are scraped.

The rule tests in
[crawler-massive.test.yaml](../../platform/observability/tests/crawler-massive.test.yaml) cover
each alert, branch series, normal and holiday weekends, and the per-source alerts for MASSIVE.

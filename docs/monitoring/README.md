# EconGraph Grafana Dashboards

Use this guide to select, import, and troubleshoot the standalone dashboards in
[`grafana-dashboards`](../../grafana-dashboards/). The
[Terraform monitoring module](../../terraform/modules/monitoring/main.tf) defines a
separate set of dashboards inline; it does not load these files.

The catalog below describes each standalone panel's actual query. Several panel
titles describe signals their queries do not measure. Both these files and the
Terraform definitions also contain older crawler metric names. Check the query,
exported series, and labels before interpreting a panel; an empty panel does not
establish that a queue is empty or healthy.

## Standalone Dashboard Catalog

Panel titles and IDs below identify the checked-in panels, including titles that
do not match their measurements. These descriptions do not imply that every
required exporter or series is available in an installation.

### EconGraph Platform Overview

[`econgraph-overview.json`](../../grafana-dashboards/econgraph-overview.json)
refreshes every 30 seconds and opens at the last 1 hour. It primarily queries
Kubernetes pod and resource metrics.

| ID | Panel title | Query measurement |
| --- | --- | --- |
| 1 | System Status | Count of `kube_pod_status_phase` series with `phase="Running"` in `econ-graph`. The query does not filter the gauge value to 1, so this is not a count of running pods or an overall health check. |
| 2 | API Request Rate | Unscoped container network byte rates, memory usage, and CPU usage rate; no HTTP request counts. |
| 3 | Response Time Percentiles | Container memory for burstable and best-effort cgroups, plus burstable CPU usage rate; no latency percentiles. |
| 4 | Active Pods | Counts of Running-phase series in `econ-graph`, split by backend/frontend/crawler pod-name patterns. Like System Status, these counts do not filter gauge values to 1. |
| 5 | Database Connections | PostgreSQL pod Running-phase series count and requested pod memory in `econ-graph`; no database connection counts or limits. |
| 6 | Memory Usage | Requested memory for backend, frontend, crawler, and PostgreSQL pods in `econ-graph`; no measured memory consumption. |
| 7 | CPU Usage | Requested CPU for backend, frontend, crawler, and PostgreSQL pods in `econ-graph`, multiplied by 100; no measured CPU utilization percentage. |
| 8 | Network I/O | Unscoped sums of container receive/transmit byte rates. |

### Database Statistics

[`database-statistics.json`](../../grafana-dashboards/database-statistics.json)
refreshes every 1 minute and opens at the last 6 hours. Its queries depend on
PostgreSQL exporter metrics, including optional table, statement, and WAL series.

| ID | Panel title | Query measurement |
| --- | --- | --- |
| 1 | Database Size | `pg_database_size_bytes` for `econ_graph`. |
| 2 | Total Records | Sum of cumulative insert, update, and delete statistics across exported user tables; not a current row count. |
| 3 | Connection Pool Status | `pg_stat_activity_count` for `econ_graph`; no pool limit or saturation comparison. |
| 4 | Database Performance | Buffer hit percentage for `econ_graph`, computed from block hits and reads. |
| 5 | Table Sizes | Per-table cumulative insert/update/delete totals and `pg_relation_size`; the operation totals are not row counts. |
| 6 | Query Performance | Exported mean and maximum statement times, converted from seconds to milliseconds. |
| 7 | Database Activity | Commit, rollback, insert, update, and delete rates for `econ_graph`. |
| 8 | I/O Statistics | Block read/hit rates for `econ_graph`, multiplied by 8192 to estimate bytes per second assuming 8 KiB blocks. |
| 9 | Lock Statistics | Exported `pg_locks_count` series. |
| 10 | WAL Statistics | Rates of exported WAL written/flushed byte counters. |
| 11 | Time Series Data Growth | Cumulative insertion statistics for `economic_series`, `data_points`, and `crawl_queue`; not current table sizes or net row growth. |

Queries without a database selector can include other databases exposed by the
data source. Confirm exporter names and enabled collectors before using them.

### Crawler Status

[`crawler-status.json`](../../grafana-dashboards/crawler-status.json) refreshes every
15 seconds and opens at the last 2 hours. Most queue and HTTP queries use older
metric families; see [Current worker metrics](#current-worker-metrics) before
adapting them.

| ID | Panel title | Query measurement |
| --- | --- | --- |
| 1 | Active Crawler Instances | Sum of successful scrapes for `job="econgraph-crawler"`; not worker readiness or loop health. |
| 2 | Queue Items Pending | Sum of legacy `econgraph_queue_items{status="pending"}`. |
| 3 | Processing Rate | Sum of legacy processed-counter rates over 5 minutes. |
| 4 | Error Rate | Legacy crawl-error rate divided by processed rate; this ratio is not necessarily a fraction of failed job attempts. |
| 5 | Queue Processing Rate | Legacy processed and failed counter rates, retaining their exported labels. |
| 6 | Queue Status Distribution | Legacy pending, processing, completed, failed, and retrying queue gauges. |
| 7 | Data Source Crawl Status | Legacy last-crawl timestamp, processed count increase over 24 hours, and a percentage derived from processed/failed counter increases. |
| 8 | Crawler Worker Health | CPU usage rate times 100 for pods matching `econgraph-crawler-.*`; no health or readiness signal. |
| 9 | Queue Processing Time Distribution | Legacy duration-bucket increases over 5 minutes, summed by `le`. |
| 10 | API Rate Limiting Status | Legacy `econgraph_api_requests_total` rates times 60 for `fred` and `bls`; no configured API limit comparison. |
| 11 | Data Freshness by Source | Hours since the legacy last-crawl timestamp; not the age of every series in a source. |
| 12 | Error Analysis by Source | Legacy crawl-error counter rates with their exported labels. |
| 13 | Queue Backlog Trend | Legacy pending and retrying queue gauges summed by source. |

## Installation and Setup

### Terraform provisioning

Follow the [Terraform setup guide](../../terraform/README.md#quick-start) for
prerequisites, configuration, planning, and deployment. Its monitoring module
configures a Helm-provided **EconGraph Overview** and ConfigMaps for **EconGraph
System Metrics**, **EconGraph Database Statistics**, and **EconGraph Crawler Status**.
These are separate inline definitions, so editing a standalone JSON file does not change
Terraform's dashboards.

The module configures Grafana at `https://grafana.<domain>`, where `domain` is the
Terraform input. Provisioning configuration alone does not establish that Grafana
has loaded a dashboard or that its queries have matching series. Inspect the
loaded dashboards and data source in your installation.

### Importing standalone files

The three catalogued files wrap the Grafana dashboard model in a top-level
`dashboard` property. From the repository root, extract the inner object into
one import file per dashboard:

```bash
mkdir -p /tmp/econgraph-dashboard-import
for name in econgraph-overview database-statistics crawler-status; do
  jq -e '.dashboard | select(type == "object")' \
    "grafana-dashboards/$name.json" \
    > "/tmp/econgraph-dashboard-import/$name.json" || exit 1
done
```

Sign in to Grafana using your installation's account. Open **Dashboards → Import**
and upload one extracted JSON file at a time, or paste its contents. Review the
title, folder, and UID before importing so that an existing dashboard is not
unintentionally replaced. Check the panel data source bindings against your
Prometheus data source; select or update the binding where necessary. Repeat for
the other files, then inspect each panel's query and matching series.

The file-upload and JSON-model import paths are present in
[Grafana 10.2's import implementation](https://github.com/grafana/grafana/blob/v10.2.0/public/app/features/manage-dashboards/DashboardImportPage.tsx).
Menu wording can differ in other versions.

## Metrics and Data Sources

### Current worker metrics

The [worker metrics endpoint](../../backend/crates/econ-graph-crawler-worker/src/metrics.rs)
serves `/metrics` on port 9102 by default. Its queue families, defined in
[crawler metrics](../../backend/crates/econ-graph-metrics/src/crawler.rs), are:

- `crawler_queue_items{source,status}`: queue counts for pending, processing, and retrying jobs.
- `crawler_jobs_total{source,kind,outcome}`: finished job attempts, with outcomes completed, retrying, and failed.
- `crawler_queue_failed_24h{source}`: jobs that became failed during the last 24 hours.
- `crawler_last_success_timestamp_seconds{source}`: latest completed job timestamp; absent for a source with no successful job.

HTTP-level crawler families include `econgraph_crawler_requests_total`,
`econgraph_crawler_errors_total`, and `econgraph_crawler_rate_limit_hits_total`.
Requests have `crawler_type`, `source`, `endpoint`, and `status` labels; errors
have `crawler_type`, `source`, and `error_type`; rate-limit hits have `crawler_type`
and `source`. Inspect actual source label values rather than assuming the
standalone dashboard's lowercase `fred`/`bls` selectors match them.

Both the standalone crawler file and Terraform definitions query older names
such as `econgraph_queue_items`, `econgraph_queue_items_processed_total`, and
`econgraph_last_crawl_timestamp`. The standalone file also uses
`econgraph_api_requests_total`. Adapting these queries requires choosing the
correct measurement and labels, not just replacing names. In particular, the
current queue gauge has no completed or failed status: a completed-attempt
counter and a failed-in-24-hours gauge are not equivalent to those legacy queue
states. Check both asset sets before relying on their results.

For source coverage measurements and their alerts, see the
[crawler coverage runbook](crawler-coverage-runbook.md).

### Backend and exporter metrics

The [backend metric definitions](../../backend/crates/econ-graph-backend/src/metrics.rs)
include `http_requests_total`, `http_request_duration_seconds`, and
`graphql_query_duration_seconds`. Histograms expose `_bucket` series for
percentiles. Their definitions do not establish coverage of every route: the
[active GraphQL route](../../backend/crates/econ-graph-backend/src/main.rs) executes
the schema directly, bypassing the handler that calls `record_graphql_query`.
The HTTP health/root handlers also record a duration of zero, so their latency
samples do not measure request processing time.

The database dashboard queries PostgreSQL exporter families such as
`pg_database_size_bytes`, `pg_stat_activity_count`, `pg_stat_database_*`,
`pg_stat_user_tables_*`, and `pg_locks_count`. Container resource usage comes from
cAdvisor-style metrics; pod status and resource requests come from kube-state-metrics.
These exporters and their collectors must supply the series that a query uses.

### Instrumentation labels and scrape metadata

Instrumentation defines labels for each metric family, such as queue `status`
and job `outcome`; they are not universal labels to add to every metric.
Prometheus supplies target metadata such as `job` and `instance`, and generates
`up` to report scrape success. `up` does not establish application readiness.

Pod and namespace label names depend on scrape configuration. The
[standalone Prometheus configuration](../../k8s/monitoring/prometheus-config.yaml)
uses `kubernetes_pod_name` and `kubernetes_namespace` for discovered pod targets;
other queries assume `pod` or `namespace`. Inspect the returned label sets and
target configuration before choosing selectors or changing instrumentation.

## Alerting Rules

The [Terraform monitoring module](../../terraform/modules/monitoring/main.tf)
defines these four alerts:

| Alert | Condition | Condition must hold for |
| --- | --- | --- |
| `EconGraphBackendDown` | `up{job="econgraph-backend"} == 0` | 5 minutes |
| `EconGraphHighResponseTime` | HTTP 95th percentile from the backend job's duration buckets exceeds 1 second | 10 minutes |
| `EconGraphQueueBacklog` | Sum of legacy `econgraph_queue_items{status="pending"}` exceeds 1000 | 15 minutes |
| `EconGraphDatabaseConnectionsHigh` | Sum of `pg_stat_activity_count{datname="econ_graph"}` exceeds 80 | 5 minutes |

This module does not define `EconGraphDatabaseDown`, `EconGraphQueueStuck`,
`EconGraphHighErrorRate`, or `EconGraphDataStale`. Other deployment paths have
separate rule definitions; inspect the live Prometheus rule list for loaded
coverage. The backlog rule requires migration for the current worker. Confirm
the scrape jobs, metric producers, and matching series before relying on any rule;
a missing `up` series does not satisfy `up == 0`.

## Customization

Edit the source for the provisioning path you use: the standalone JSON's
`dashboard` object or the Terraform module's inline definitions. Add panels to
that object's `panels` array with a unique panel `id` and an appropriate
`gridPos`. For example, after checking that ID 99 is unused:

```json
{
  "id": 99,
  "title": "Custom Metric",
  "type": "graph",
  "gridPos": { "h": 8, "w": 12, "x": 0, "y": 0 },
  "targets": [
    {
      "expr": "your_custom_metric",
      "legendFormat": "Custom Legend",
      "refId": "A"
    }
  ]
}
```

Replace the metric placeholder and bind the panel to your Prometheus data source.
Set units and a title that describe the query's measurement.

To open at the last 6 hours and refresh every 30 seconds, set these properties
on the dashboard object:

```json
{
  "time": {
    "from": "now-6h",
    "to": "now"
  },
  "refresh": "30s"
}
```

A query variable can expose deployment labels when those labels exist:

```json
{
  "templating": {
    "list": [
      {
        "name": "environment",
        "type": "query",
        "query": "label_values(up, environment)",
        "multi": true,
        "includeAll": true
      }
    ]
  }
}
```

Bind the variable to the Prometheus data source. Adding it does not filter panels:
reference it in their selectors, for example `up{environment=~"$environment"}`.
Merge it into the existing variable list instead of replacing unrelated variables.

## Troubleshooting

For missing data, check the data source, scrape target, exact metric name, and
labels in that order. A successful JSON import does not verify metric availability.
For import errors, validate JSON and check Grafana/plugin compatibility and data
source bindings. Reduce refresh frequency or narrow selectors for slow queries.

### Accessing Prometheus

For the [standalone Kubernetes service](../../k8s/monitoring/prometheus-service.yaml)
in namespace `econ-graph`, the access command is:

```bash
kubectl -n econ-graph port-forward svc/prometheus-service 9090:9090
```

Open `http://localhost:9090`. This command applies to that manifest, not the
Terraform Helm release. For Terraform, identify the Prometheus service and
namespace from the installed release and forward its service port 9090. The
module's configured URL alone does not establish a running service's name or
namespace.

### Inspecting queries and labels

In Prometheus's table view, start with an unaggregated selector:

```promql
crawler_queue_items
```

Inspect each returned series' `source`, `status`, and scrape labels. Aggregating
by metric name would discard the labels needed to diagnose a selector. Then
narrow to a known status or calculate completed-attempt rates:

```promql
crawler_queue_items{status="pending"}
```

```promql
sum by (source) (rate(crawler_jobs_total{outcome="completed"}[5m]))
```

These examples select all matching targets in the data source. Add the observed
job/namespace labels to isolate an installation and avoid summing duplicate
scrapes. For a histogram percentile, apply `rate()` to counter buckets before
summing, and retain `le` for `histogram_quantile()`.

## Maintenance and Recovery

Keep changes in the source that provisions the dashboard. Recheck queries when
metric names, labels, exporters, or worker semantics change, and tune alert
thresholds against the measurements they actually use.

For the standalone files, the repository is the recovery source: check out the
desired revision, extract its inner dashboard objects as described above, and
import them individually. For Terraform-managed dashboards, restore the desired
inline definitions through your normal Terraform review and deployment process.

To preserve edits made in Grafana, open each dashboard's **Share → Export** tab
and choose **Save to file**, keeping one JSON file per dashboard. Grafana 10.2's
[export implementation](https://github.com/grafana/grafana/blob/v10.2.0/public/app/features/dashboard/components/ShareModal/ShareExport.tsx)
serializes a dashboard object as JSON. Validate each saved file with
`jq -e 'type == "object" and has("panels")' dashboard.json` and restore it using
the same Import screen. Review destination folder, UID, and data source bindings
before importing. Store reviewed changes back in the appropriate source definition
so a later provisioning run does not replace them. Dashboard JSON does not back
up data sources, credentials, alert rules, or the rest of Grafana's state.

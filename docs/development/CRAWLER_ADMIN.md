# Crawler operator interface

The admin frontend polls the persistent crawler queue. It uses the configured
`VITE_GRAPHQL_ENDPOINT` (default `/graphql`), includes credentials and forwards
the existing `auth_token` bearer token. Authentication and permissions are
enforced by the server for every operation. An authorization failure replaces
displayed operator data with its error; there is no unauthenticated fallback.

| Operation | Permission | Behavior |
| --- | --- | --- |
| `crawlerStatus` | `admin.system:read` | Existing consistent queue snapshot, including active claim holders and source counts |
| `queueStatistics` | `admin.system:read` | Existing persisted queue aggregates |
| `crawlerJobs(limit, offset, source, status)` | `admin.system:read` | Latest queue records ordered by update time and UUID, with exact source/status filters; limit 1–200, offset 0–100000 |
| `crawlerSources` | `admin.system:read` | Adapters in this build and their stored scheduler configuration |
| `triggerCrawl(input)` | `admin.crawlers:manage` | Existing asynchronous catalog/fetch enqueue; active duplicate jobs are skipped |
| `setCrawlerSourceEnabled(source, enabled)` | `admin.crawlers:manage` | Audited update to that source's stored `is_enabled` setting |

The dashboard polls every 15 seconds; execution history polls every 10 seconds
and can be paused. There is no subscription transport. Queue history displays
the last persisted state, retry count, timestamps and worker error for each job;
it does not claim to retain every retry or stream raw process output. Error
messages omit upstream URLs and common credential assignments. Detailed process
logs remain in the deployment's logging system. No random CPU, memory or
throughput values are displayed.

`is_enabled` controls future automatic scheduler work. Disabling a source does
not stop running workers, cancel existing jobs, or prohibit an explicit
operator enqueue. Adapters are compiled into the worker; source CRUD cannot
create a new implementation. A missing stored source configuration is shown as
uninitialized, and the mutation refuses to fabricate it. Initialize the crawler
through its existing migrate/startup procedure.

Worker count, timeout and rate-limit policy remain deployment settings. The UI
therefore offers no start/stop/restart, backup or maintenance-mode buttons.
These are not operations implemented by the queue worker. SEC management UI is
also excluded: company search and SEC trigger/import GraphQL resolvers are not
present in this baseline.

The authenticated control backend needs its app database for audit records and
its data database for crawler state. An app-only schema returns
`DATA_SERVED_REMOTELY`; no resolver silently bypasses that boundary or delegates
an operator mutation to the public data endpoint. A strict query-only data
service must continue rejecting these admin roots and all mutations. Configure
the admin frontend to use the control backend, with its supported token audience.

Validation uses the production schema with explicit caller roles and disposable
Postgres. `crawler_admin` covers role denial, filtered/bounded history, redaction,
source scheduling, audit outcomes, distinct app/data pools and app-only refusal.
Frontend tests check authenticated requests, recorded counts/errors and the
absence of invented process controls. The existing production browser smoke
test still runs against the built admin app.

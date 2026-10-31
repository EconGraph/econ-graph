# QA scenario suite

The QA scenario suite runs the release-candidate checklist against a deployed site and reports
each scenario as pass, fail or skip, with evidence. The same suite runs against a lab
deployment, against the release end-to-end stack in CI, and read-only against production
after a deploy.
It complements the [deployed release QA guide](v4.0.0-qa.md), which covers the checks that
need a person or a long observation period.

## Run it

```bash
QA_LAB_TARGET_OK=lab-1.example scripts/qa/run-scenarios.sh --target https://lab-1.example --profile lab
scripts/qa/run-scenarios.sh --target https://www.econgraph.com --profile production --json out.json
QA_LAB_TARGET_OK=lab-1.example scripts/qa/run-scenarios.sh --target https://lab-1.example --profile lab \
    --only anonymous-api,mcp-reads
scripts/qa/run-scenarios.sh --list
```

- `--target` is the site's base URL, the origin a visitor opens.
- `--profile` is `lab`, `ci` or `production`, as described in [Profiles](#profiles).
- `--only` runs the named scenarios. Naming one that the profile does not run is an error, and
  the runner runs nothing.
- `--require-pass` names scenarios that must pass: one that skips fails the run. CI uses it so
  that a scenario quietly turning into a skip is caught.
- `--json` writes the report described in [Results](#results).

The runner exits 0 when no scenario failed and every `--require-pass` scenario passed, 1
otherwise, and 2 for a usage error (including two scenarios declaring one name) or a refusal. A
skipped scenario does not otherwise fail the run; the summary and the report give its reason.
Within a passing scenario, a check that could not run (for example `catalog-roles`' redeploy
without `QA_REDEPLOY_COMMAND`) is recorded as skipped, and the scenario's reason lists it.

Each scenario runs in its own process group. One that overruns its `qa-timeout` fails. Its
group gets SIGTERM, so a Python scenario stops the commands it started and runs its cleanup
(the annotation round trip deletes what it created, and `catalog-roles` restores the
anonymous reader's roles). After `QA_TIMEOUT_GRACE_SECONDS` (default 60), whatever remains is
killed. Stopping the runner with SIGTERM or Ctrl-C does the same to the running scenario, and
the runner then starts no further scenario and exits 1. A scenario's own waits during cleanup
use at most half the grace period, and a command it starts during cleanup with `qa.run` gets
only what is left of the grace period, less a margin (a fifth of it, at most 5 seconds), so it
ends before the scenario is killed; with nothing left, it isn't started.

`qa.run` starts each command in its own session, outside the scenario's process group, and a
command can leave it with `setsid`. So once a scenario has ended, however it ends, the runner
also kills every process still carrying the scenario's `QA_RUN_MARKER` environment variable,
after giving them a second to end on their own. The scenario then fails, and its reason names
each one (`left running, now killed: psql (4242)`) after the scenario's own reason. A process
the runner can't see this way is neither killed nor reported:

- anything on macOS, which has no `/proc`;
- a process that clears or rewrites its environment: `env -i`, `sudo` with its default
  environment reset, and programs that rewrite their process title, such as browsers,
  PostgreSQL and nginx;
- a process the runner's user can't read: setuid or non-dumpable ones, such as `ssh-agent`.

If such a process still holds the scenario's output open, the runner stops waiting for the
output 10 seconds after the scenario ends and reports what it has.

Killing a local `kubectl exec` or `docker exec` client doesn't stop the command it started
remotely. So a scenario, and any command it runs through a hook such as `QA_REDEPLOY_COMMAND`,
must not leave anything running, including helpers that persist on purpose: an ssh
ControlMaster (ControlPersist), `kubectl port-forward` or a gpg-agent.

## A QA environment for a release candidate

`scripts/qa/deploy-rc-to-qa.sh` stands up a scratch kind cluster running a release
candidate's published images on a copy of production's data, for the lab profile and
manual QA:

```bash
scripts/qa/deploy-rc-to-qa.sh --tag vX.Y.Z-rc.N --dump-dir ~/econgraph-backups/<timestamp>
```

It works from any checkout: it runs the candidate's own deploy scripts from a worktree of
the tag (from v4.1). The dump directory is a verified backup (`counts.env`, the dumps and
`secrets/`), read and never written. Everything else is in `~/econgraph-qa/NAME/`
(`ECONGRAPH_QA_ROOT` overrides the root; never under `/tmp`), and the cluster `NAME`
(default `qa-<tag without dots>`) is created by the script and marked as its own. It
refuses the production cluster names, a cluster with a production marker or one it did not
create, and a dump inside the QA root.

The site is at `http://localhost:PORT`, on 127.0.0.1 only, with sign-in on that address.
crawler-worker runs at 0 replicas and production's crawler API keys are not copied. The QA
accounts `qa-a`, `qa-b` and `qa-admin` (realm role `admin`) get random passwords in
`~/econgraph-qa/NAME/accounts.env` and `accounts/<user>.password` (mode 600), for
`QA_USERNAME` and `QA_PASSWORD_FILE`. The script prints the URL, the accounts file and
`TARGET_URL` and `TARGET_HOST` for the QA prompt; run the scenarios with
`QA_LAB_TARGET_OK=localhost --target http://localhost:PORT --profile lab`.

Rerun it to finish or refresh an environment. `--teardown` deletes the cluster and its
directory, never the dump; `--dry-run` only checks and prints the plan. The script's usage
header is the full reference.

## Profiles

Each scenario declares in its header whether it changes state (`qa-mutates: yes` or `no`) and
which profiles it runs in (`qa-profiles`). A scenario whose `qa-mutates` line is missing or
unclear counts as changing state.

| Profile | Runs | Targets |
|---|---|---|
| `lab` | every scenario | only hosts named in `QA_LAB_TARGET_OK`, never `econgraph.com`, `www.econgraph.com`, or a host in `QA_PRODUCTION_HOSTS` |
| `ci` | every scenario except the lab-only ones | the same |
| `production` | the scenarios that change nothing, plus `annotation-round-trip` as a dedicated QA account | any |

The lab and ci profiles run state-changing scenarios, so their target must be named: on the
production host, `http://localhost` or the host's own address reaches production as surely as
its public name. `QA_LAB_TARGET_OK` is a comma-separated list of host names; the release e2e
stack's spec sets it to its own host.

The production exception is fixed in the runner (`PRODUCTION_MUTATING_EXCEPTIONS` in
`scripts/qa/run_scenarios.py`): the name `annotation-round-trip` and the one file that may carry
it, `scripts/qa/scenarios/annotation-round-trip.py`. A scenario cannot grant it to itself, and a
file elsewhere declaring the same name gets no exception; the command line only runs the
scenarios in `scripts/qa/scenarios/`. The exception applies only when `QA_USERNAME` names a
`qa-*` account and its password is configured. Without one, the production profile skips the
round trip.

## Credentials

Credentials come from the environment or from files the environment names, never from
command-line arguments. The runner removes secret values from everything it prints and writes:
the value of every variable whose name contains `PASSWORD`, `SECRET`, `TOKEN` or `CREDENTIAL`,
the content of such a variable's `_FILE`, JSON Web Tokens, bearer tokens and the passwords in
database URLs.

| Variable | Used by |
|---|---|
| `QA_USERNAME`, `QA_PASSWORD` or `QA_PASSWORD_FILE` | the sign-in scenarios: a realm user of the `econ-graph` realm. In production, a `qa-*` account |
| `QA_OIDC_ISSUER` | the realm's issuer; default `<target>/idp/realms/econ-graph` |
| `QA_OIDC_REDIRECT_URI` | a redirect URI registered on `econ-graph-web`; default `<target>/auth/callback` |
| `QA_KEYCLOAK_URL`, `QA_KEYCLOAK_ADMIN_USERNAME`, `QA_KEYCLOAK_ADMIN_PASSWORD` or `_FILE` | `catalog-roles`: Keycloak's admin API, which the public route does not serve |

Sign-in uses the web client's own flow, an authorization code with PKCE submitted through
Keycloak's sign-in form. It therefore works where the password grant is off, as in every
deployed realm.

## Scenarios

| Scenario | Changes state | Profiles | What it checks |
|---|---|---|---|
| `public-site` | no | all | Runs [`scripts/deploy/check-public-site.sh`](../../scripts/deploy/check-public-site.sh) against the target's host: routing, canonical-host redirects, the issuer, the sign-in page (which must refuse a `http://localhost` redirect URI), both public CORS origins and refused-origin preflights, forwarded-header spoofing, and admin, metrics and playground not served. Each of its lines is a check. It reaches the host through DNS, as a visitor does; `QA_PUBLIC_SITE_ROUTE=gateway` sends it to the gateway on the runner's host instead (`LOCAL_INGRESS_IP` and the ports `check-public-site.sh` reads) |
| `anonymous-api` | no | all | Without a token, through `/graphql`: a search, the series, its observations and the world map's cross-section. The same search and observations through `/data/graphql` must match |
| `anonymous-browser` | no | all | Signed out, in Chromium: the home page without script errors, a search, a series page and its chart, the CSV download against the page's newest point, and the world map's colored countries and value table |
| `annotation-round-trip` | yes | all, production only as a `qa-*` account | Signs in, creates a private annotation, retries the create with the same idempotency key and gets the same annotation, checks that it is stored once and invisible to an anonymous reader, then deletes it. Anything it created is deleted even when a check fails |
| `mcp-reads` | no | all | As the QA account: `/mcp` refuses an anonymous caller, lists its tools, and reads a series through `search_economic_series`, `get_series_metadata` and `get_series_data`. Skipped where nothing answers MCP: release builds compile `/mcp` out and the gateway routes no `/mcp` path |
| `catalog-roles` | yes | lab, ci | Removes one catalog role (`QA_CATALOG_ROLE`, default `catalog.bls:read`) from the anonymous reader as the [Keycloak README](../../config/keycloak/README.md) describes, waits until an anonymous read of that catalog's series is refused, runs `QA_REDEPLOY_COMMAND` and checks that the restriction survived, then restores the original mappings and waits until the series is readable again. Skipped on a realm without catalog roles. Without `QA_REDEPLOY_COMMAND`, the redeploy and persistence checks are recorded as skipped |
| `backup-restore` | yes | lab | Backs up the lab database (`QA_BACKUP_SOURCE_DATABASE_URL`), restores it into a new database whose name contains `scratch` (`QA_SCRATCH_DATABASE_URL`), and compares every table's owner and row count. Database passwords reach `psql`, `pg_dump` and `pg_restore` as `PGPASSWORD`, never on their command lines. `QA_BACKUP_COMMAND` and `QA_RESTORE_COMMAND` replace the default `pg_dump` and `pg_restore` with the lab's own, and `QA_EXPECTED_OWNER` names the role every table must belong to |
| `conversion-checks` | no | lab | Judges the report of a data conversion ([ECO-426](https://linear.app/econgraph/issue/ECO-426)), read from `QA_CONVERSION_REPORT` or printed by `QA_CONVERSION_CHECK_COMMAND`: every table's row count and checksum unchanged, and every annotation still on its original series. The report must list its annotations, as `[]` when there are none |
| `crawl-sample` | yes | lab | Runs `QA_CRAWL_SAMPLE_COMMAND` once per source in `QA_CRAWL_SOURCES`, one source at a time, `QA_CRAWL_INTERVAL_SECONDS` apart; each run must report one series fetched with observations |
| `branch-isolation` | yes | lab | v4.1.0 exit criterion 1, against a branch from `deploy-branch.sh` (`QA_BRANCH_NAMESPACE`; the target is its `http://<slug>.localhost`). The namespace reads the primary data plane and has no data Postgres of its own. The series an anonymous `/data/graphql` search finds there are rows of the primary data database. The same search through the branch's own backend (`/graphql`) must return the same series, since the branch's `/data/graphql` route goes to the primary's backend. Signed in to the branch's realm, it creates a private annotation. That annotation and its user must be in the branch's app database, and the annotation must be in neither production's nor any other branch's. Another branch whose app database can't be read is recorded as skipped. The annotation is deleted afterwards, even when the create's response was lost |
| `data-survives-redeploy` | yes | lab | v4.1.0 exit criterion 2. It reads every data-database table's row count and checksum, a sum of a hash of each whole row, so changed values count too. It then runs `QA_DATA_REDEPLOY_COMMAND` (the redeploy or app-database recreate under test). After that it waits for the data database's StatefulSet to roll out and requires every count and checksum to be unchanged. Run it in a quiet window with the crawler in static mode. Before and after each reading, every running `crawler-worker` pod must have its scheduler off (`CRAWLER_SCHEDULER=false`, or `--scheduler false` in its args) and be ready, and `crawl_queue` must have no pending, processing, retrying or locked job. When that is not so, the scenario ends inconclusive: a skip whose reason starts `inconclusive:`, neither a pass nor a failure, saying what to wait for. `crawl_queue` is not compared: the worker purges its old finished jobs even in static mode. A worker restarted by the redeploy writes its build's dataset definitions and any changed reference data at startup, so when only `datasets` and `reference_file_cache` changed after such a restart, the result is inconclusive too. A change to any other table fails. The data database is only read |
| `gateway-routes-only` | no | lab, production | v4.1.0 exit criterion 5. No Ingress or IngressClass is left, and nothing of ingress-nginx's: no namespace, deployment, daemonset, service or pod, and no ClusterRole, ClusterRoleBinding or admission webhook configuration. There are no route kinds other than HTTPRoute. There is at least one HTTPRoute, and every one of them is accepted, with its references resolved, by each of its Gateways |

`QA_SEARCH_QUERY` (default `GDP`) is the search the read scenarios use. Each scenario's header
documents its inputs and outputs in full.

### Main address and sign-in checks

The production target is the main address, `https://www.econgraph.com` by default:
`PUBLIC_HOST=econgraph.com` is the base domain and `MAIN_HOST` selects the main host.
With the apex as the main host, use the apex target and the checker's
`--main-host econgraph.com`. Once the switch to www is finalized, run the `public-site`
scenario with `MAIN_HOST_PERMANENT=www.econgraph.com`, so it expects 301 redirects
rather than 302s. See [Main public address](../deployment/MAIN_HOST.md) for
the operator commands. These browser and token checks supplement the `public-site`
scenario: a passing routing probe does not show that sign-in completes.

| Check | Expected result |
| --- | --- |
| Open HTTP on each public name and HTTPS on the apex, with a page path and query | HTTP first gets Cloudflare's 301 to HTTPS on the same name (Always Use HTTPS); HTTPS on the apex then redirects to the same path and query on `https://www.econgraph.com`: 302 until the switch is finalized, 301 after; the destination serves content without another canonical-host redirect |
| Open HTTPS www directly, in both a fresh and an existing browser profile | App loads without a redirect loop; the canonical link uses HTTPS www and the path, without query/fragment. Clear cached permanent redirects if an existing profile retains the opposite redirect |
| Start sign-in from a page reached through each name | Both flows reach the same www realm, complete the callback on www, and return to the intended page |
| Reload, renew a token, and sign out after signing in | Session restore/renewal and logout use www callbacks; no apex issuer or callback is introduced |
| Complete a sign-in already in progress when the host changes | Old state either completes safely on the main host or offers a fresh sign-in; no loop or unrecoverable callback error |
| Inspect discovery and a newly issued token | Issuer is exactly `https://www.econgraph.com/idp/realms/econ-graph`, matching backend validation and the frontend's resolved relative issuer |
| Use a token from before the issuer change in an isolated QA deployment | Backend rejects the old issuer; a newly issued token succeeds |
| Browse signed out during the switch in an isolated QA deployment | Anonymous catalog data keeps loading, apart from at most the backend's own rollout |
| Preflight from both public origins | Both origins are allowed whichever is the main host; `http://localhost` stays allowed, and `http://localhost:3000` and arbitrary origins get the backend's 403 with no allow-origin header |
| Preview/apply the switch, finalize and apex rollback twice in a stubbed test or isolated QA deployment | Dry run makes no changes; reruns converge; finalize turns the redirects to www into 301s; rollback restores the apex issuer and the inverse redirect matrix while retaining both public CORS origins and TLS names |

State-changing switch, token and rollback tests run against an isolated QA deployment
or the targeted tests with `kubectl` and `curl` stubbed, never against production;
never glob-run `scripts/deploy/test-*.sh`.

### The cluster scenarios

`branch-isolation`, `data-survives-redeploy` and `gateway-routes-only` read the kind cluster. Each
skips unless `QA_KUBE_CONTEXT` names its context explicitly, e.g. `kind-econ-graph`; the current
context is never used. They run the kind cluster's own kubectl, chosen by
[`scripts/qa/lib/kind-kubectl.sh`](../../scripts/qa/lib/kind-kubectl.sh) through
`scripts/deploy/lib/kind-env.sh`, never a kubectl from `PATH`. They read databases with `psql` in
the Postgres pods: the data database is `econ_graph` in `postgresql-0`, and each app database is
`econ_graph_app` in its namespace's `app-postgres-0`. The primary namespace is `QA_DATA_NAMESPACE`
(default `econ-graph`).

```bash
# Exit criterion 5, read-only:
QA_KUBE_CONTEXT=kind-econ-graph scripts/qa/run-scenarios.sh --target https://www.econgraph.com \
    --profile production --only gateway-routes-only

# Exit criterion 1, against a deployed branch, as a user of the branch's realm:
QA_LAB_TARGET_OK=my-branch.localhost QA_KUBE_CONTEXT=kind-econ-graph \
    QA_BRANCH_NAMESPACE=<the branch's namespace> QA_USERNAME=<user> QA_PASSWORD_FILE=<file> \
    scripts/qa/run-scenarios.sh --target http://my-branch.localhost --profile lab --only branch-isolation

# Exit criterion 2, in a quiet window with the crawler in static mode
# (docs/monitoring/crawler-coverage-runbook.md):
QA_LAB_TARGET_OK=my-branch.localhost QA_KUBE_CONTEXT=kind-econ-graph \
    QA_DATA_REDEPLOY_COMMAND='scripts/deploy/deploy-branch.sh up my-branch --context kind-econ-graph ...' \
    scripts/qa/run-scenarios.sh --target http://my-branch.localhost --profile lab --only data-survives-redeploy
```

`scripts/deploy/deploy-branch.sh list --context kind-econ-graph` shows the branch namespaces.
For criterion 2, run `data-survives-redeploy` once per redeploy the criterion names: production,
UAT, a branch, an app-database recreate and a kind cluster recreate. The helper checks that
the cluster answers before each scenario uses it, and picks the kubectl again after the
redeploy.

The conversion report has this shape:

```json
{
  "tables": [
    {"name": "economic_series",
     "before": {"rows": 812, "checksum": "md5:..."},
     "after": {"rows": 812, "checksum": "md5:..."}}
  ],
  "annotations": [
    {"id": "...", "series_id_before": "...", "series_id_after": "..."}
  ]
}
```

The crawl hook prints, as its last line, one JSON object for the source in `QA_CRAWL_SOURCE`:

```json
{"source": "FRED", "series": "GDP", "ok": true, "observations": 312, "detail": "..."}
```

## Results

The summary lists each scenario's status, duration and reason, and each failed check beneath a
failed scenario. The JSON report holds the same per scenario:

```json
{
  "target": "https://lab-1.example",
  "profile": "lab",
  "started_at": "...",
  "finished_at": "...",
  "counts": {"pass": 5, "fail": 0, "skip": 4},
  "scenarios": [
    {"name": "annotation-round-trip", "mutates": true, "status": "pass", "reason": null,
     "duration_seconds": 0.4, "exit_code": 0,
     "evidence": [{"check": "idempotent retry", "ok": true, "detail": "..."}],
     "output_tail": ["..."]}
  ]
}
```

## In CI

The [Release E2E workflow](../../.github/workflows/release-e2e.yml) has a second matrix leg,
`QA scenarios (ci profile)`. It sets up the same stack as the release suite and first runs the
runner's unit tests (`python3 -m unittest discover -s scripts/qa/tests`). These tests prove the
profile guards and run each scenario against a stub site. The cluster scenarios get a stub
kubectl through `QA_KIND_KUBECTL_HELPER`, which only the tests set. The leg then runs
`RELEASE_E2E_CONFIG=playwright.qa-stack.config.ts scripts/release-e2e.sh`. That command builds
and seeds the stack as the release suite does and starts the same backend and frontend
([`playwright.qa-stack.config.ts`](../../frontend/playwright.qa-stack.config.ts)). It then runs
the suite with the `ci` profile, signed in as the dev realm's `grace`, with
`--require-pass annotation-round-trip,anonymous-api,anonymous-browser`: those three must pass
on this stack, not skip. The leg uploads the JSON report as the `qa-scenarios-report`
artifact.

On that stack, `public-site` skips because the target is plain http. `mcp-reads` skips because
release builds compile `/mcp` out, and the lab-only scenarios skip as well. Run the same command
locally from the repository root, after stopping the development stack's Keycloak, which binds
the same port.

## Add a scenario

Add an executable to `scripts/qa/scenarios/` with the header lines `qa-scenario`, `qa-mutates`,
`qa-profiles`, `qa-description` and optionally `qa-timeout` (seconds, default 600). The runner
sets `QA_TARGET`, `QA_PROFILE`, `QA_WORK_DIR` and `QA_EVIDENCE_FILE`. A scenario appends one JSON
line per check (`{"check": ..., "ok": ..., "detail": ...}`) to the evidence file. It exits 0
when everything passed, 77 to skip, after appending `{"skip": "<reason>"}`, and anything else to
fail. Python scenarios use the helpers in `scripts/qa/lib/qa.py`. A browser scenario runs a spec
under `frontend/tests/qa/browser/` through
[`playwright.qa.config.ts`](../../frontend/playwright.qa.config.ts).

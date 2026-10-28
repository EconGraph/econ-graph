# QA scenario suite

The QA scenario suite runs the release-candidate checklist against a deployed site and reports
each scenario as pass, fail or skip, with evidence. The same suite runs against a lab
deployment, against the release end-to-end stack in CI, and read-only against production
after a deploy.
It complements the [deployed release QA guide](v4.0.0-qa.md), which covers the checks that
need a person or a long observation period.

## Run it

```bash
scripts/qa/run-scenarios.sh --target https://lab-1.example --profile lab
scripts/qa/run-scenarios.sh --target https://econgraph.com --profile production --json out.json
scripts/qa/run-scenarios.sh --target https://lab-1.example --profile lab --only anonymous-api,mcp-reads
scripts/qa/run-scenarios.sh --list
```

- `--target` is the site's base URL, the origin a visitor opens.
- `--profile` is `lab`, `ci` or `production`, as described in [Profiles](#profiles).
- `--only` runs the named scenarios. Naming one that the profile refuses is an error, and the
  runner runs nothing.
- `--json` writes the report described in [Results](#results).

The runner exits 0 when no scenario failed, 1 when one did, and 2 for a usage error or a
refusal. A skipped scenario does not fail the run; the summary and the report give its reason.

Each scenario runs in its own process group. One that overruns its `qa-timeout` fails. Its
group gets SIGTERM, so a Python scenario stops the commands it started and runs its cleanup
(the annotation round trip deletes what it created, and `catalog-roles` restores the
anonymous reader's roles). After `QA_TIMEOUT_GRACE_SECONDS` (default 60), whatever remains is
killed.

## Profiles

Each scenario declares in its header whether it changes state (`qa-mutates: yes` or `no`) and
which profiles it runs in (`qa-profiles`). A scenario whose `qa-mutates` line is missing or
unclear counts as changing state.

| Profile | Runs | Refuses |
|---|---|---|
| `lab` | every scenario | `econgraph.com` and any host in `QA_PRODUCTION_HOSTS` as the target |
| `ci` | every scenario except the lab-only ones | the same hosts |
| `production` | the scenarios that change nothing, plus `annotation-round-trip` as a dedicated QA account | every other state-changing scenario |

The production exception is fixed in the runner (`PRODUCTION_MUTATING_EXCEPTIONS` in
`scripts/qa/run_scenarios.py`); a scenario cannot grant it to itself. It applies only when
`QA_USERNAME` names a `qa-*` account and its password is configured. Without one, the
production profile skips the round trip.

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
| `public-site` | no | all | Runs [`scripts/deploy/check-public-site.sh`](../../scripts/deploy/check-public-site.sh) against the target's host: routing, the issuer, the sign-in page, CORS, forwarded-header spoofing, and admin, metrics and playground not served. Each of its lines is a check. It reaches the host through DNS, as a visitor does; `QA_PUBLIC_SITE_ROUTE=gateway` sends it to the gateway on the runner's host instead (`LOCAL_INGRESS_IP` and the ports `check-public-site.sh` reads) |
| `anonymous-api` | no | all | Without a token, through `/graphql`: a search, the series, its observations and the world map's cross-section. The same search and observations through `/data/graphql` must match |
| `anonymous-browser` | no | all | Signed out, in Chromium: the home page without script errors, a search, a series page and its chart, the CSV download against the page's newest point, and the world map's colored countries and value table |
| `annotation-round-trip` | yes | all, production only as a `qa-*` account | Signs in, creates a private annotation, retries the create with the same idempotency key and gets the same annotation, checks that it is stored once and invisible to an anonymous reader, then deletes it. Anything it created is deleted even when a check fails |
| `mcp-reads` | no | all | As the QA account: `/mcp` refuses an anonymous caller, lists its tools, and reads a series through `search_economic_series`, `get_series_metadata` and `get_series_data`. Skipped where nothing answers MCP: release builds compile `/mcp` out and the gateway routes no `/mcp` path |
| `catalog-roles` | yes | lab, ci | Removes one catalog role (`QA_CATALOG_ROLE`, default `catalog.fred:read`) from the anonymous reader as the [Keycloak README](../../config/keycloak/README.md) describes, waits until an anonymous read of that catalog's series is refused, runs `QA_REDEPLOY_COMMAND` and checks that the restriction survived, then restores the original mappings and waits until the series is readable again. Skipped on a realm without catalog roles. Without `QA_REDEPLOY_COMMAND`, the redeploy check reports that persistence was not checked |
| `backup-restore` | yes | lab | Backs up the lab database (`QA_BACKUP_SOURCE_DATABASE_URL`), restores it into a new database whose name contains `scratch` (`QA_SCRATCH_DATABASE_URL`), and compares every table's owner and row count. `QA_BACKUP_COMMAND` and `QA_RESTORE_COMMAND` replace the default `pg_dump` and `pg_restore` with the lab's own, and `QA_EXPECTED_OWNER` names the role every table must belong to |
| `conversion-checks` | no | lab | Judges the report of a data conversion ([ECO-426](https://linear.app/econgraph/issue/ECO-426)), read from `QA_CONVERSION_REPORT` or printed by `QA_CONVERSION_CHECK_COMMAND`: every table's row count and checksum unchanged, and every annotation still on its original series. The report must list its annotations, as `[]` when there are none |
| `crawl-sample` | yes | lab | Runs `QA_CRAWL_SAMPLE_COMMAND` once per source in `QA_CRAWL_SOURCES`, one source at a time, `QA_CRAWL_INTERVAL_SECONDS` apart; each run must report one series fetched with observations |

`QA_SEARCH_QUERY` (default `GDP`) is the search the read scenarios use. Each scenario's header
documents its inputs and outputs in full.

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
profile guards and run each scenario against a stub site. The leg then runs
`RELEASE_E2E_CONFIG=playwright.qa-stack.config.ts scripts/release-e2e.sh`. That command builds
and seeds the stack as the release suite does and starts the same backend and frontend
([`playwright.qa-stack.config.ts`](../../frontend/playwright.qa-stack.config.ts)). It then runs
the suite with the `ci` profile, signed in as the dev realm's `grace`. The leg uploads the JSON
report as the `qa-scenarios-report` artifact.

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

# Roadmap: release trains

Status: proposed (2026-09-26). Forks 1 to 3 are decided; fork 4 is open for Joe.

The topic roadmaps are organized by feature line: auth, admin UI, federation and global
analysis. This doc cuts across them. It says what goes into the next release, what is
deliberately left out, and the rough order of the releases after it. Each topic roadmap
links back here from its own "Release trains" section.

## How trains work

- **A train is a scope, not a date.** It leaves when its exit criteria pass. Nobody can
  estimate a revived project's pace yet, so dates would be guesses.
- **Each train is a tag on its release branch** (below). Every package is at `0.1.0` (`backend/Cargo.toml`,
  `frontend/package.json`, `admin-frontend/package.json`), but the repo already carries
  old demo tags from `v0.1` up to `v3.7.3`, including `v0.4.0` to `v0.7.0`. So the first
  train is `v4.0.0`, above every existing tag, and each later train bumps the minor
  version. The old tags stay: deleting published tags would break anyone who fetched them.
  Each release also pushes a `train-N` tag, which the feature flag file's `remove_by`
  check reads ([feature-flags.md](./feature-flags.md)).
- **Work keeps landing as small PRs on `main`.** The only branches cut from it are the release branches for QA.
  Unfinished work merges only behind build-time flags that compile it out of release
  builds. Runtime flags are for alpha and beta features that work end to end, never for
  mock data ([feature-flags.md](./feature-flags.md), #195).
- **Unfinished screens don't ship (Joe, 2026-09-26).** A screen that isn't finished is
  either not merged or compiled out of release builds. A feature that belongs to a later
  train is not deleted from an earlier one to get there. The global tabs are
  compiled out of every profile until the PRs that replace them delete them, and so are the
  financial components. Joe prefers compiling out to deleting unless code is known bad
  (2026-09-26), so code is deleted now only when it is wrong: the fake correlations,
  `imf.rs`, mock data and hard-coded results such as the
  financial demo page's own inline fixtures, and `/analysis` with `ProfessionalChart` and
  `ChartCollaboration`, which run on mock data and a made-up chart id.
- **A train can shrink but not grow.** If an item threatens the exit criteria, it moves
  to the next train. New ideas go to a later train, not the current one.

- **QA runs on a release branch (Joe, 2026-09-27).** Work stays on `main` until a train
  reaches QA. At that point `release/vX.Y` is cut from `main`, and the QA cycle, its fixes and the tag all happen on
  that branch. `main` reopens for experimental work and the next train, and the areas
  start scoping the next cut there, each scope reviewed by a Fable agent. Fixes land on
  the branch first and are forward-ported to `main`. See the
  [release process](../development/RELEASE_PROCESS.md).

## The gap the topic roadmaps don't cover

Each topic roadmap is sound on its own. Together they are almost all infrastructure:
Keycloak, a role catalog, two database planes, Iceberg, Arrow Flight and an admin app.
None of them covers the product's core loop, which is to find a series, open it and
read the chart. That loop mostly shows fake numbers today (checked on `main` at `140fadf`):

| Page | Route | What it shows |
|---|---|---|
| Dashboard | `/` | Four indicator cards with hard-coded values (`'$27.36T'`, `'3.7%'` in `frontend/src/pages/Dashboard.tsx:66-98`) |
| Explore | `/explore` | **Real** search results from `searchSeries` |
| Series detail | `/series/:id` | Mock points from `generateMockDataPoints`, after a simulated one-second delay (`frontend/src/pages/SeriesDetail.tsx:55-231`). It never calls the backend's `series` or `seriesData` |
| Professional analysis | `/analysis/:id?` | `SAMPLE_SERIES`, `MOCK_COLLABORATORS` and mock annotations (`frontend/src/pages/ProfessionalAnalysis.tsx:47-227`) |
| Global analysis | `/global` | Sample data, and the backend is unreachable and partly fake (see [global-analysis.md](./global-analysis.md)) |
| Data sources | `/sources` | Real |

The backend already serves what the series page needs: `series`, `seriesData` and
`apply_data_transformation` in `backend/crates/econ-graph-graphql/src/graphql/query.rs`. The crawler
already refreshes series on a schedule (`backend/crates/econ-graph-crawler-worker/src/main.rs`). The gap
is mostly frontend wiring plus a fairly short list of fixes.

So the first release is **broad real data behind pages that work**, and the
infrastructure it carries is limited to what those pages need. Joe chose to include
accounts on Keycloak (fork 1), so sign-in is done once, the right way, rather than built
in-house and migrated later.

## Train 1 (`v4.0.0`): broad real data, nothing broken

**Goal.** Joe set the bar for "usable" on 2026-09-26: wide swaths of data working, and no
feature that is obviously broken. So any visitor can browse US and international
macro data, open any series that search returns and read a correct, current chart, and
see the latest value of any one indicator at a time for every country the source reports it for, on a map. A signed-in user can
also annotate a series chart, privately or publicly. Every control a visitor can reach does what it
says. Anything that doesn't work yet is compiled out of the release build.

### Data in scope

"Wide swaths" means more than the three sources that fetch today. Train 1 targets
roughly the US and international headline data that FRED users expect.

| Source | Today | Train 1 work | Rough size |
|---|---|---|---|
| FRED | Works | Keep | Thousands of series (FRED's catalog is far larger; discovery decides) |
| BLS | Works, hard-coded list of series | Keep. Widen the list to the main CPI, CES and LAUS series | Hundreds |
| Census BDS | Works, national and per state (#175) | Keep | About 1,000 |
| FHFA HPI | Fetch aimed at an endpoint that probably never existed | Rebuild on the published master CSV ([fhfa.md](../data-sources/fhfa.md)) | Hundreds |
| BEA | Discovery only, with made-up series ids | Real NIPA table and line ids, and `fetch_series` ([bea.md](../data-sources/bea.md)) | Hundreds |
| World Bank WDI | Discovery only | `fetch_series` for a curated set of about 50 indicators (global analysis decision 8) | About 11,000 |

IMF and the other SDMX sources wait for train 3 (fork 3). The ten static catalogs (BOC, BOE, BOJ, ECB, ILO,
OECD, RBA, SNB, UN Stats, WTO in `static_catalogs.rs`) are hard-coded lists that always
fail to fetch. They are useful in a development environment and only meant for one (Joe,
2026-09-26), so train 1 keeps them for development and leaves them out of release and
production builds, and search in those builds never returns a series that can't have
data. `imf.rs` is deleted. Sources come back as real adapters through [data-sources.md](./data-sources.md).

**Datasets metadata moves into train 1.** The new adapters (World Bank, BEA) are
where country, indicator and table dimensions first appear. If they land before the
`datasets` table, they encode dimensions in external ids the way
`CENSUS_BDS_ESTAB_state_06` does, and train 3 rewrites them. The `datasets` table and the
`economic_series` dataset and dimension columns are Postgres-only (federation phase 3's
first PR, also global analysis phase 3), so they go first in train 1. Observations still
go to `data_points`, one value per point, with the dataset's default measure as the value.

### Features in scope

The State column tracks each item's PRs. Every PR updates its item's row when it opens, changes scope or merges (Joe, 2026-09-26). Numbers are labels, not order. Item 20 (datasets metadata) goes first. Items 21 to 24 are the data source rows above that need work (FRED and Census BDS already fetch), and items 25 and 26 back exit criteria 2 and 3.

| # | Item | Where it comes from | State |
|---|---|---|---|
| 1 | Security hygiene: JWT secret fail-closed, non-UUID `sub` rejected, CORS allowlist, `user(userId)` restricted, token required on `/mcp` | Auth phase 0: #180 (merged), #181, #182, #183, #185 | #180, #181, #182, #185 merged; #183 open |
| 2 | Rotate the leaked Google and Facebook credentials and move them into a Kubernetes Secret, along with the other plaintext credentials in `k8s/manifests` | Security section of the [index](./README.md) | Waiting on Joe to rotate |
| 3 | The collaboration API takes the acting user from the verified token and never from the request, for reads as well as writes: today `annotationsForSeries` takes a `userId` argument (`backend/crates/econ-graph-graphql/src/graphql/query.rs:266-270`), so any caller can read another user's private annotations | [analysis-workspace.md](./analysis-workspace.md) phase 2; #194 | #194 open |
| 4 | Attach the existing GraphQL depth and complexity limits (`backend/crates/econ-graph-graphql/src/security/*`) to the schema, and turn off `/playground` in deployed builds | Security section of the index | #221 draft |
| 5 | `latestRevisionOnly` and `asOf` done in SQL | #184 (federation phase 0) | #184 open |
| 6 | Ranked search with a weighted tsvector and pg_trgm. With tens of thousands of series, `ILIKE` with a constant rank stops being usable | #165 | #165 draft |
| 7 | Series detail page on `series` and `seriesData`, with the existing transformations. Delete the mock generators | New | #204, #209 drafts |
| 8 | Dashboard cards read the latest values of a fixed list of series (for example FRED `GDP`, `UNRATE`, `CPIAUCSL`, `FEDFUNDS`) | New | #212 draft |
| 9 | Explore page: drop the random "search time" (`Math.random` in `frontend/src/pages/SeriesExplorer.tsx:255`) and the made-up fallback dates, and filter by the enabled sources | New | #201 draft |
| 10 | World map on real data: country reference data, `crossSection`, the map on `crossSection` with each value's date in the tooltip, `world-atlas` bundled. The comparison, correlation and events tabs are flagged off until train 3 | Global analysis phases 1, 2, 5 and 6 (its phase 3 is item 20 and phase 4 is item 24) | #205 draft; more to come |
| 11 | CSV download of a series' data on the series page. Today's only export button, in `frontend/src/components/charts/ProfessionalChart.tsx` (`exportChart`), is empty | New | In progress |
| 12 | Fine-grained role catalog and `authorize()`, replacing `require_admin` and the three disagreeing role vocabularies | Auth phase 1 | In progress |
| 13 | Keycloak as the identity provider: realm as code, Google as an identity provider, the backend verifying Keycloak tokens by JWKS and reading `roles` from them, in-house login and JWT issuance retired. Staff use the Keycloak admin console | Auth phase 2 | In progress |
| 14 | Sign-in in the main frontend through Keycloak (authorization code with PKCE), then private and public annotations and comments on the series page, on the existing backend (`annotationsForSeries`, `commentsForAnnotation`). This comes after the collaboration API takes the acting user from the token instead of the request: today `shareChart` reads `ownerUserId` from its input (`backend/crates/econ-graph-graphql/src/graphql/mutation.rs:123`). Sharing waits for train 2, because there is no charts table for a share to point at | Auth phase 2; [analysis-workspace.md](./analysis-workspace.md) phases 2 and 3 (#191) | #211 draft; more to come |
| 15 | The ten static catalogs (`static_catalogs.rs`) become development-only: kept for development environments and left out of release and production builds. `imf.rs` is deleted | [data-sources.md](./data-sources.md) decision 1 (#192) | In progress |
| 16 | Release flags at build time: a flag file in the repo with metadata and a CI check, frontend flags folded in by Vite so flagged-off code is absent from the release bundle, and backend startup flags (for example `/mcp`) | [feature-flags.md](./feature-flags.md) phase 1 (#195) | #198 draft |
| 17 | Flag off what can't work yet. The "Coming Soon" global tabs sit behind build-time flags that are off in every profile until the PRs that replace them delete them. `/mcp` is flagged off in release builds. The financial components and the XBRL parser are compiled out. Deleted as known bad: `/analysis` (`ProfessionalAnalysis`, a second series view on mock data) with `ProfessionalChart` and `ChartCollaboration`, the financial demo page's own inline fixtures (the mock GraphQL query documents that `components/financial` imports stay with those compiled-out components until SEC phase 4), the fake correlation code, and on the SEC side the Arelle path, the large-object placeholder, the hard-coded CIK list and the stubs that report success with nothing done | Global analysis phase 1; [feature-flags.md](./feature-flags.md) fork 1; [sec-financial-data.md](./sec-financial-data.md) phase 0 | #199 draft; more to come |
| 18 | Committed `schema.graphql` with a backend test that fails when it drifts, and the main frontend's operations validated against it | Admin UI phase 1, applied to the main frontend first | #208 draft |
| 19 | Release mechanics: version bump, release notes, deploy from the tag | New | QA plan in `docs/release/v4.0.0-qa.md` (PR #238); version and tag workflow not started |
| 20 | Datasets metadata: the `datasets` table and the `economic_series` dataset and dimension columns. Goes first: items 10 and 22 to 24 build on it | Federation phase 3's first PR; global analysis phase 3 | #220 draft |
| 21 | BLS: widen the hard-coded series list to the main CPI, CES and LAUS series | [data-sources.md](./data-sources.md) | In progress |
| 22 | FHFA HPI rebuilt on the published master CSV | [fhfa.md](../data-sources/fhfa.md) | In progress |
| 23 | BEA: real NIPA table and line ids, and `fetch_series` | [bea.md](../data-sources/bea.md) | In progress |
| 24 | World Bank WDI: `fetch_series` for the curated set of about 50 indicators | Global analysis phase 4 | In progress |
| 25 | Coverage: a report of series with and without data per source (exit criterion 2), and search hides series with no data points | New | In progress |
| 26 | Scheduled refresh for every enabled source with an alert on crawl failure (exit criterion 3). The scheduler exists; the alert and per-source schedules are what's new | New | #210 draft |
| 27 | Batch fetch in the crawler adapter contract and worker: `batch_key`, `fetch_batch`, a `max_batch` policy and batched claiming, so a source can serve many series per request. Items 21 to 24 build on it (BLS batches 50 series per request, or 25 without a key) | [data-sources.md](./data-sources.md) (#192, moved from train 3 into train 1) | #213 open |

### Explicitly out

- **Organizations, teams and plans** (auth phases 3 and 4). Every signed-in user has
  the same roles. Billing stays deferred.
- **Facebook and email-and-password sign-in.** Google alone through Keycloak is enough
  to start. The others are Keycloak configuration and can follow any time.
- **Saved charts, workspaces and sharing.** There is no charts table, and the series
  page makes up a chart id the backend rejects, so sharing waits for saved charts in
  train 2.
- **MCP.** Decided in fork 2 below: `/mcp` is flagged off in release builds, and MCP with OAuth comes in train 2.
- **The admin app.** It can't talk to the backend today (see [admin-ui.md](./admin-ui.md)).
  Operators use Grafana (`grafana-dashboards/`), the crawler alerts and `kubectl`.
- **Country comparison, correlations and events.** These are global analysis phases 7,
  8 and 10, in train 3.
- **Crawl-date vintages for most sources.** Train 1 records the vintages that sources
  publish: FRED's ALFRED `realtime_start` and the World Bank's `lastupdated`. The other
  adapters keep setting `revision_date = date` until train 3 stamps crawl-date vintages.
- **All other federation work.** Train 1 takes only #184 and the datasets metadata (the
  `datasets` table and the `economic_series` dataset and dimension columns). Out: the `TimeSeriesStore`
  trait, the plane split, Iceberg and Flight. Observations stay in `data_points` for now.
  There is no production data, so the phase 5 cutover drops them and nothing is migrated.
- **SEC financial data in the UI.** No GraphQL API exists for it yet. The financial
  components are compiled out, and the demo page's own inline fixtures are deleted. Company pages
  get a train of their own, train 4 (Joe, 2026-09-26). [sec-financial-data.md](./sec-financial-data.md)
  (#193) owns the phases. Its phase 0 (compile out, delete what is known bad) and phase 1
  (ingest) change nothing a user sees, so they can merge during train 1.

Anything flagged off or deleted gets a roadmap for how to build it properly (Joe, 2026-09-26). The global analysis tabs are covered by [global-analysis.md](./global-analysis.md),
MCP by [auth-plans-permissions.md](./auth-plans-permissions.md) and the admin app by
[admin-ui.md](./admin-ui.md). Three new roadmaps cover the rest:

- [analysis-workspace.md](./analysis-workspace.md) (#191): `/analysis`, multi-series charts, saved charts and chart export.
- [sec-financial-data.md](./sec-financial-data.md) (#193): the financial components and the
  SEC API. It replaces [`SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md`](../development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md).
- [data-sources.md](./data-sources.md) (#192): the development-only static catalogs, the generic SDMX adapter, source order, and keys and rate limits by source.

### Exit criteria

1. CI is green on the tagged commit.
2. **Coverage.** Every enabled source has fetched data for at least 95% of the series it
   discovered. A check lists the rest, and search hides any series with no data points.
3. **Freshness.** The crawler has refreshed every enabled source on its schedule for
   seven days in a row with no manual step, and a crawl failure fires an alert in
   Prometheus. Routing alerts to a person (Alertmanager and a receiver) waits until there is an on-call target.
4. **Nothing fake.** With release flags, no reachable route imports sample or mock data. A
   test builds with release flags and fails if a reachable page imports `generateMock*`,
   `SAMPLE_*`, `mock*` or `sampleCountryData`, or renders "Coming Soon".
5. **Nothing broken.** A Playwright suite runs against the deployed build and exercises
   every page and control. At minimum it:
   - searches for a series from each source, opens it and sees a chart whose latest point
     is within the source's expected cadence;
   - applies each transformation;
   - downloads the CSV;
   - loads the map for three WDI indicators and sees a value for every country the World
     Bank reports one for (a sanity floor: at least 150 countries);
   - signs in through Keycloak and annotates a series publicly and privately, and a second
     account sees the public annotation and not the private one (live: as two QA-only
     realm-local test users; the real Google sign-in is checked by hand, see the QA plan).
6. The security items 1, 3 and 4 are merged, item 2's rotation is done, and no endpoint accepts a token Keycloak didn't issue.
7. `v4.0.0` and `train-1` are tagged on `release/v4.0`, with release notes that list the sources and features.

Until the final QA phase, CI runs the Playwright suite against a local stack seeded from
recorded fixtures, with no live API keys. Criteria 2, 3 and 5 against live sources, and
criterion 6 against the live Keycloak, are checked in a final QA phase that deploys with real API keys and runs the full end-to-end
tests there (Joe, 2026-09-26). That deployment runs for at least seven days before the tag, for criterion 3. The tag comes after that phase passes. Where it deploys is still open (open question 2), and the release pauses before the QA deploy until Joe decides. QA runs on the `release/v4.0` branch, cut from `main` when train 1's items are in.

### Forks for train 1

**Fork 1: accounts in train 1? Decided (Joe, 2026-09-26): Keycloak now.** The options
were no accounts, today's in-house login, or auth phases 1 and 2 in train 1. Joe chose
the third. The cost is a second large body of work beside the data work, so the two
proceed in parallel and either can hold the train.

**Fork 2: MCP in train 1? Decided (2026-09-26, on the recommendation, with Joe's go-ahead for release 1): no, MCP comes in train 2.** With Keycloak in train 1, MCP OAuth (the MCP part of auth
phase 6) builds on it directly.

| Option | For | Against |
|---|---|---|
| A. MCP with OAuth in train 1 | Claude and ChatGPT users can connect to the broad data from day one. Keycloak is already there | Protected Resource Metadata, dynamic client registration and a consent screen add to an already large train |
| **B. MCP not routed in train 1; OAuth in train 2 (chosen)** | Keeps train 1 to what the website needs | No MCP at launch |

**Fork 3: how wide is "wide"? Decided (Joe, 2026-09-26): no SDMX in train 1.** The
sources above already cover a lot of useful data without SDMX. FRED alone republishes
many international series from the IMF, OECD and others, and the World Bank has its own
JSON API. So the generic SDMX adapter, with the IMF, ECB, OECD, Eurostat and ILO on it,
starts in train 3. A source in the table that isn't ready when everything else passes
moves to the next train rather than holding the release.

## Later trains

The order below is a proposal. Each train names the topic roadmap phases it contains, and
the topic roadmaps own the detail.

| Train | Theme | Contains | Depends on |
|---|---|---|---|
| 2 (`v4.1.0`) | Machine access and staff | MCP with OAuth and client-credentials machine access (auth phase 6). Multi-series charts on `/chart` and saved charts with sharing, CSV and PNG export ([analysis-workspace.md](./analysis-workspace.md) phases 4 and 5). Admin UI phases 0 to 2 (cleanup, one client, Keycloak login, routing), deployed for staff. Runtime flags, `preview:access` and kill switches ([feature-flags.md](./feature-flags.md) phase 2) | Train 1 |
| 3 (`v4.2.0`) | Cross-country analysis and more sources | Federation phase 1 (`TimeSeriesStore` trait). The remaining adapters stamp crawl-date vintages, so `asOf` has history for every source. Global analysis phases 7 to 10 (country comparison, correlations on request, legacy tables dropped, events). A generic SDMX adapter, with the IMF first, then the ECB, OECD, Eurostat and ILO ([data-sources.md](./data-sources.md)). US state maps for Census BDS. Frequency alignment and formula series ([analysis-workspace.md](./analysis-workspace.md) phase 6) | Train 1. Can run in parallel with train 2 |
| 4 (`v4.3.0`) | Company pages | [sec-financial-data.md](./sec-financial-data.md) phases 1 to 6 on SEC's `companyfacts` JSON (phase 1, ingest, may land earlier since users can't see it): every company that files XBRL, standard concepts, the company API, the company page (filings, a chart of concepts over time, statement tables with a period picker, CSV download), ratios and peers, notes on company charts, statement lines and facts, and MCP tools for company data. Joe wants this train fully useful, so it ships only when all of that works, rather than as a thin slice. Share prices (market cap, P/E, a price chart) are out of this train and deferred (Joe, 2026-09-26), since they need a licensed end-of-day price source. The compiled-out `components/financial` code comes back piece by piece as the page uses it | Train 2 (MCP OAuth for the MCP tools, sharing for notes). Runs in parallel with train 3 |
| 5 (`v4.4.0`) | Company segments and filings as reported | [sec-financial-data.md](./sec-financial-data.md) phases 7 to 9: raw filings in object storage, our own XBRL parser, then segment and geographic breakdowns, company-specific concepts and statements laid out as filed. This is the headline capability for company data, the part SEC's JSON can't give and that makes the platform powerful (Joe, 2026-09-26), not an add-on. Phase 7 changes nothing users see, so it can merge during train 4. SEC's `companyfacts` JSON keeps being crawled after the parser lands, as a quality check on the parser's facts | Train 4 |
| 6 (`v4.5.0`) | Data plane split | Federation phase 2 (two databases, subgraphs and a gateway, a staging data plane, the `dev-readonly` client). Admin UI phase 4 (crawler admin on the post-#157 crawler) | Trains 2 and 3. Can run in parallel with trains 4 and 5 |
| 7 (`v4.6.0`) | Iceberg storage | Federation phases 3 to 5 (Iceberg tables, transactional commits, cutover, drop `data_points`), then phase 6 (Arrow Flight) | Train 6 |
| Unscheduled | Teams and plans | Auth phases 3 (organizations), 4 (plans and limits) and 7 (staff tooling), admin UI phase 3 | Train 2, and a customer who needs it |
| Unscheduled | Product analytics and experiments | [feature-flags.md](./feature-flags.md) phases 3 and 4 | A public deployment, then enough weekly users (about 6,500 per arm for a 10% relative lift) |
| Unscheduled | Billing, enterprise SSO | Auth phases 5 and 8 | Teams and plans |

### Where this challenges the topic roadmaps

- **Split development may be early at train 6.** Federation's main purpose is letting
  feature work run on a fresh user database against already-crawled data. Before a
  second developer or a large crawl exists, a published snapshot of the crawled tables
  (a `pg_dump` of the data tables, restored locally) gives most of that benefit at a small
  fraction of phase 2's cost. It also hands out a file, not credentials, which fits Joe's
  rule that development code never holds staging credentials. See fork 4.
- **Teams and plans come after data.** The auth roadmap orders organizations and plans
  (phases 3 and 4) straight after Keycloak. Billing is deferred and nobody is paying yet,
  so a larger, trusted dataset (trains 3 to 7) is worth more than a plan ceiling. Auth
  phase 1's role catalog still lands early, because every later phase builds on it.
- **Admin UI waits a train after Keycloak.** Its login and roles depend on auth phase
  2, and in train 1 staff can use the Keycloak admin console. Its phase 1 schema check
  lands in train 1, on the main frontend first, because that is where the mock data hid.
- **The roadmap index's "register `GlobalAnalysisQuery`" step is dropped.** This follows
  [global-analysis.md](./global-analysis.md): train 1 deletes the fake correlations and
  replaces the query with `crossSection`.
- **Datasets metadata lands before its roadmap planned it.** Federation places it in
  phase 3, and global analysis asks for it early. Train 1 puts it first, because train 1's
  new adapters are where dimensions appear.
- **`ProfessionalAnalysis` may not need to exist.** It is a second series view, entirely
  on mock data. Train 1 deletes it and puts annotations on the series page instead. A
  separate analysis page comes back only when there is a specific job for it.

**Fork 4: what is train 6?** It follows trains 2 and 3 and can run beside trains 4 and 5. Under options B and C, train 7 (Iceberg) depends on trains 2 and 3 instead of train 6.

| Option | For | Against |
|---|---|---|
| **A. Data plane split (train 6 as above) (recommended: release 1 already runs many parallel Claude threads, each wanting a fresh app database against shared crawled data)** | It is what federation is for, and Iceberg builds on it | A gateway and two databases to run for a small team |
| B. A data snapshot for development, and go straight to Iceberg | Much less to operate. Iceberg's scale work starts sooner | The split still has to happen before production has real users |
| C. Teams and plans | Needed before charging anyone | Nobody to charge yet |

## Open questions for Joe

1. Fork 4 above: what train 6 is.
2. **Where does `v4.0.0` run?** Pending (Joe, 2026-09-27: he has plans). This is a planned
   pause: the release stops before the QA deploy until he decides, and all work up to that
   step continues. The [QA plan](../release/v4.0.0-qa.md) lists what any target needs.
3. **Which series go on the dashboard?** The proposal is FRED `GDP`, `UNRATE`,
   `CPIAUCSL` and `FEDFUNDS`, which match today's hard-coded cards.
4. **API keys and network access.** FRED, BEA and Census need API keys, and the cloud
   environment's network policy blocks the source APIs. So adapters can't be checked
   against live responses from a Claude session until the hosts are allowed.
5. **GitHub milestones.** Should each train get a milestone that its PRs are added to, or
   is this doc enough?

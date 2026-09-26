# Roadmap: feature flags and experiments

Status: proposed (2026-09-26), for Joe to decide the forks marked below.

The release plan ([releases.md](./releases.md), #190) raised the question of whether a
release has to delete unfinished features, or can keep them in the code behind flags that
are off in release builds. This doc works out what flags are for in EconGraph, which tool
to use, how flags relate to the fine-grained roles in
[auth-plans-permissions.md](./auth-plans-permissions.md), and when experiments (A/B tests)
make sense. It places each phase on the release trains.

## Where we are today

Checked on `main` at `c92bdaf`:

- **There is no flag system.** Nothing in the backend, frontend or admin frontend reads a
  flag. `docs/technical/CRATE_SPLIT_PLAN.md` mentions feature flags only as Cargo
  features (lines 256 and 463), not as product switches.
- **Frontend configuration is baked in at build time.** `frontend/src/vite-env.d.ts`
  declares five `VITE_*` variables (API URLs and OAuth client ids), and
  `frontend/Dockerfile` passes them as build arguments. So the frontend image is already
  built once per environment.
- **The backend reads environment variables** (`ENVIRONMENT`, `DATABASE_URL`,
  `CORS_ALLOWED_ORIGINS` and so on) and has no notion of a per-feature switch. `/mcp` is
  routed unconditionally in `backend/crates/econ-graph-backend/src/main.rs` (lines
  371-383).
- **Some switches already exist as data, and should stay that way.** `data_sources` has
  `is_visible` and `is_enabled`, which turn a whole source on or off for the crawler and
  the UI. `users` has `notifications_enabled` and `collaboration_enabled`, which are user
  preferences.
- **There is no product analytics.** The frontend sends no usage events anywhere, so no
  experiment could be measured today.
- **Entitlements were once modelled as permissions.** Stale PR #149 had tier features such
  as `export:watermark-free`. The auth roadmap already puts these in fine-grained roles
  that Keycloak composes into plans.

## Strategy (Joe, 2026-09-26)

- **An unfinished screen is not shipped.** Either it isn't merged yet, or it is compiled
  out of release builds by a build-time flag. Existing code is compiled out rather than
  deleted, unless it is known to be wrong (mock data, hard-coded results). A runtime
  flag never hides unfinished work, because runtime-off code is still in the bundle or
  binary, one flip away from users.
- **Runtime flags are for functionality that does something, at alpha or beta quality.**
  It can be rough, incomplete or slow, but a user who sees it can use it end to end and
  get a real result, never mock data or a "Coming Soon" panel. The flag decides who sees
  it (staff for alpha; beta testers or a percentage for beta) or turns it off if it
  misbehaves.
- **For features that need the backend, the backend decides once.** It evaluates the flag,
  hides the GraphQL fields for users who don't have it (async-graphql's `visible`) and
  rejects calls to them, and returns the same value in the flag response the frontend
  reads. The UI never disagrees with the API, and the web frontend doesn't probe the
  schema to detect features.
- **Clients released separately from the backend degrade gracefully, where that makes
  sense.** The web frontend deploys with the backend, so the two rarely disagree. A mobile
  app (none exists today), MCP clients and scripts against the API run whatever version
  their user has, against whatever backend they reach. GraphQL already covers an older
  client against a newer backend: the schema only changes additively, and fields are
  deprecated long before they are removed, so there are no API versions. What GraphQL
  doesn't cover is a client newer than the backend, or a feature that is deployed but off.
  For those, the client reads the same flag response the web frontend uses and hides an
  optional feature whose flag is off or missing from the response, rather than failing.
  There is no separate capabilities API or version number. Operations that use an optional
  feature are kept separate from core ones, because GraphQL rejects a whole operation that
  names a field the backend doesn't serve. Core features, such as search and series data,
  stay required: a client doesn't degrade around them. This also lets a client ship a
  finished screen before launch: it stays hidden until the backend turns the feature's
  flag on, after the screen has been tested against the real API with `preview:access` and
  the announcement is ready.

## What a flag is for, and what it is not

Teams tend to use one flag tool for many different jobs. Only the first four below belong
in a flag system here.

| Kind | Example | Lifetime | Where it lives in EconGraph |
|---|---|---|---|
| **Build-time flag** (unfinished work) | The world map on `crossSection`, merged in slices before it's finished | Until the feature ships, then deleted | Flag file, folded in at build time. Never evaluated at runtime |
| **Preview flag** (alpha or beta: does something, still rough) | The finished world map, shown first to `preview:access` holders, then to everyone | Until the feature is generally available, then deleted | Flags, evaluated at runtime |
| **Ops flag, or kill switch** | Turn off `/mcp` or CSV download if it misbehaves, without a deploy | Long-lived, few in number | Flags (this doc) |
| **Experiment** | Two layouts of the series page, split 50/50 | Until the test concludes, then deleted | Flags for assignment, plus analytics (phase 4) |
| **Entitlement** | Pro plan users get watermark-free export | Permanent | **Fine-grained roles**, never flags |
| Source switch | Disable the IMF adapter while its API migrates | As needed | `data_sources.is_enabled`, never flags |
| Environment config | API URL, OAuth client id | Permanent | Environment variables, never flags |

Three rules follow from this:

1. **A flag never grants access.** Code behind a flag still calls `authorize()` with the
   fine-grained role it needs. Flags decide whether a feature exists in this build or for
   this person. Roles decide whether the person is allowed to use it. Because a flag is
   never a security boundary, the backend can tell any client its flag values, and an
   anonymous visitor's `targetingKey` can come from the browser.
2. **A permanent flag is a role or a setting in disguise.** Commercial flag tools market
   "entitlement flags". Here, anything that should differ by plan goes into a
   fine-grained role, so the plan composites in Keycloak remain the one place that says
   what a plan includes. A build or preview flag that is still around after its feature has
   shipped gets deleted. An ops flag that is never flipped gets deleted too.
3. **Previewing alpha and beta features is one role, not one role per feature.** Staff and
   beta testers need to try features before everyone gets them. Rather than a fine-grained
   role for each feature, the role catalog gets one role, `preview:access`, which Keycloak
   grants through the staff composites and a `beta-tester` composite. Flags target that
   role. The backend can also check it directly for a server-side preview.

## Unfinished code: compiled out, off everywhere

The release thread proposed keeping `/analysis`, the global comparison tabs and the
financial pages in the code, behind flags that are off in release builds and on in
development builds. As proposed, that had three problems, which the decision below resolves:

- **It fails train 1's own bar.** Joe's definition of usable says anything that doesn't
  work is removed from the build, and exit criterion 4 fails if a routed page imports mock
  data. A runtime flag leaves the code in the bundle. Only a build-time flag that the
  bundler or compiler removes meets that bar (see phase 1).
- **Most of that code is going to be replaced, not finished.** The topic roadmaps rebuild
  these features on different foundations. `/analysis` is replaced by the series page and
  `/chart` ([analysis-workspace.md](./analysis-workspace.md), #191). The comparison and
  correlation tabs are rebuilt on `crossSection`, and the correlation code returns a
  hard-coded 0.75 ([global-analysis.md](./global-analysis.md), #188). The financial
  components query operations the backend doesn't serve, and #193 recommends building the
  company page fresh on the series chart ([sec-financial-data.md](./sec-financial-data.md)).
  Flagged code keeps compiling against every type change on `main`, so each flag needs
  an owner and a `remove_by` train, or it stays forever.
- **"On in development" means developing against fake data.** Every developer and every
  Claude thread would see pages full of sample numbers, which is how the mock data hid in
  the first place.

Whether or not train 1 is deployed, the build-time flags land in train 1: exit criterion
4 checks the release build, and parked code has to be off before work on later trains
merges to `main`.

**Decision (Joe, 2026-09-26):** compile code out rather than delete it, unless it is
known to be wrong. So unfinished or unscheduled code goes behind a build-time flag that
is off in both the release and development profiles, and the PR that replaces or
finishes it removes the flag. Code that is known to be wrong is deleted: mock-data
generators and sample data on routed paths, hard-coded results such as the 0.75
correlation, and `/analysis` with its chart and mocked collaboration (#191). Build-time
flags also cover **new** work that lands on `main` in small PRs before it's ready,
which is what the release plan's "a PR that belongs to a later train can merge early if
it hides behind a flag" already says.

| Item | Train 1 treatment |
|---|---|
| `/analysis` (`ProfessionalAnalysis`), `ProfessionalChart`, `ChartCollaboration` | Delete: known wrong. Fake data only, the second series is plotted by array index (`ProfessionalChart.tsx:223,246`), and collaboration is mocked. Owned by #191, which deletes these and keeps only the trading indicators, behind a build-time flag |
| Global comparison, correlation and events tabs (less the fake correlation below) | Build-time flag, off everywhere, until the train 3 PRs rebuild them on `crossSection` (#188) |
| Fake correlation code (hard-coded 0.75, `GlobalEconomicNetworkMap.tsx:199`) | Delete: known wrong |
| `FinancialComponentsDemo.tsx` and `components/financial` | Build-time flag, off everywhere, `remove_by: train 4` (company pages, per [releases.md](./releases.md)). The SEC financial data thread (#193) says which components are kept and which are known wrong and deleted |
| `/mcp` | Build-time flag `mcp`, compiled out of the release backend, `remove_by: train 2`, if fork 2 of the release plan keeps MCP out of train 1. Once MCP ships it becomes an ops flag |
| World map on `crossSection`, annotations on the series page, datasets-backed adapters | Build-time flags while each lands in slices during train 1, deleted when train 1 ships |

## Tools compared

Criteria: free to use, open source preferred, Rust and React support, what it costs to
operate before a live environment exists (release plan open question 2), and how well it
fits a team where every change is a reviewed PR. Paid-only services (LaunchDarkly, Statsig,
Optimizely) are left out. Versions and licenses were checked on 2026-09-26 against
crates.io and each project's `LICENSE` on its default branch. Every option is free
when self-hosted, so the real cost is what it takes to run on Kubernetes: Unleash,
Flagsmith and GrowthBook each ship a Helm chart for a server plus its database, while
flagd needs no service at all when the backend evaluates in-process from a mounted
ConfigMap (the recommended setup below). Where a separate evaluator is wanted, flagd is
one stateless container (a plain Deployment, or a sidecar injected by the OpenFeature
Operator) reading the same ConfigMap.

| Option | License | What runs | Rust | Browser | Experiments | Notes |
|---|---|---|---|---|---|---|
| A. No tool: build-time constants and env vars | n/a | Nothing | Hand-rolled | Vite `define` | None | Covers build-time flags only. No runtime switch, no targeting |
| **B. OpenFeature API with flagd, flags as JSON in the repo** | Apache-2.0 (CNCF) | Nothing: a library in the backend, evaluating in-process from a file. The backend also answers the browser's flag requests | `open-feature` 0.3.0 and `open-feature-flagd` 0.2.2, with RPC, OFREP, in-process and file modes | OpenFeature web and React SDKs with the OFREP provider | Assignment only (fractional splits by consistent hash) | No database and no UI: a flag change is a PR. flagd reloads the file on change, so a ConfigMap edit applies without a restart |
| C. GO Feature Flag | MIT | A relay proxy | Through OpenFeature (OFREP) | OpenFeature | Assignment, and it exports evaluation events | Much like B, with its own flag format. Stronger at exporting evaluations, weaker on Rust (no native provider) |
| D. Flipt v2 | Server under the Fair Core License (turns MIT after two years), clients MIT | A server that reads flags from a Git repo | Through OFREP | Through OFREP | Assignment only | Git-native with a UI, which is attractive, but the server is not open source under an OSI license today |
| E. Unleash | **AGPL-3.0** for the server (per its `LICENSE` today), with paid Enterprise features | A server and Postgres | `unleash-api-client` 0.17.1 | React SDK | Variants, no analysis | Mature and has a UI. One more service and database to run and secure, and SSO and change requests are paid |
| F. Flagsmith | BSD-3-Clause | A server and Postgres | `flagsmith` 3.1.2 | React SDK | Multivariate flags, analysis is thin | Same operating cost as E, with a friendlier license |
| G. GrowthBook | MIT, with enterprise directories under a paid license | A server and **MongoDB** for its own data | `growthbook-rust` 0.2.2 | React SDK | **Best of the free options**: Bayesian and frequentist analysis against our own Postgres. CUPED and sequential testing are paid | The strongest experiment tool, but its own database is MongoDB, which nothing else here uses |
| H. PostHog Cloud | MIT core (self-hosting is heavy: ClickHouse, Kafka) | Nothing, it's a service | `posthog-rs` 0.27.0 | JS SDK | Yes, with analytics built in | Free up to 1M events and 1M flag requests a month. Usage data about our users goes to a third party |

**Recommendation: B, OpenFeature with flagd, flags checked into the repo.**

- It costs nothing to run for the backend, which evaluates in-process from a file. That
  matters while there is no live environment and no one to run a flag server.
- Every flag change is a reviewed PR with history, which is how this project already works.
  A flag UI is mainly for people who don't open PRs, and nobody here needs that yet.
- OpenFeature is the vendor-neutral API. Code calls OpenFeature, so moving to Unleash,
  Flagsmith or GrowthBook later is a provider change at startup, not a rewrite.
- flagd's flag format has targeting rules (JsonLogic) and fractional splits by consistent
  hashing, which is enough for staff previews, percentage rollouts and experiment
  assignment.

Reasons to revisit: someone who doesn't open PRs needs to flip flags, or an ops flag has to
change faster than a ConfigMap update reaches the pods. Then F (Flagsmith, BSD) is the
first choice, because its license is the least restrictive of the options with a UI.

**Where this challenges the obvious choice.** Unleash is the usual open source default, but
its server is now AGPL-3.0, and it adds a server and a database before there is anything
to deploy them to. GrowthBook is the best experiment tool, but experiments aren't the
first need (see below), and it brings MongoDB.

## Experiments: not yet

An A/B test needs enough people to detect a difference. To detect a lift from 20% to 22% in
something like "opened a second series" (a 10% relative lift), at the usual 5%
significance and 80% power, each arm needs about 6,500 users:

    n per arm = (1.96 + 0.84)² × (0.20 × 0.80 + 0.22 × 0.78) / 0.02² ≈ 6,500

EconGraph has no deployed users yet, and no analytics to measure anything with. Until
weekly users reach that order of magnitude, an experiment runs for months and still
can't tell a real effect from noise. Before that point, plain usage analytics (which pages
people open, which searches return nothing, where they give up) answers more questions
than any A/B test.

So the experiment work is two unscheduled phases with explicit triggers: analytics once
there is a public deployment, experiments once traffic makes a test feasible. Assignment
reuses flagd's fractional splits from phase 2, so no second flag system is needed.

## Phased plan

### Phase 1: build-time flags (train 1)

- One flag file, `config/flags/flags.flagd.json`, in flagd's format, is the one source of
  truth. Each flag's `defaultVariant` is its **release** value: off for `build` and
  `preview` flags, on for `ops` kill switches. A flag that is off keeps `state: ENABLED`
  with its off variant as the default: the generator drops `DISABLED` flags, so code that
  names one would fail the typecheck. `config/flags/dev.flagd.json` lists only
  the flags whose development value differs. Flag keys are lowercase `snake_case`: the
  key `world_map` becomes `__FLAGS__.world_map` in the frontend and `cfg(flag_world_map)`
  in the backend.
- Each flag carries metadata: `kind` (`build`, `preview`, `ops` or `experiment`), `owner`
  (the roadmap doc), `stage` (`alpha` or `beta`) for preview flags, and `remove_by` (the
  train, or `unscheduled`) for build and preview flags. A CI check fails when a flag
  lacks them, when the dev file names a flag the main file lacks, when a flag's
  `remove_by` train has shipped and the flag still exists, or when code reads a
  `build` flag at runtime. A shipped train is marked by a `train-N` tag, pushed with the
  release. Version tags can't mark it, because the repo already has old tags from
  `v0.1` to `v3.7.3`.
- **One merge.** A single script, `scripts/flags-merge`, merges the dev file over the main
  file by key and writes the resolved values for each profile to committed files inside
  each app: `frontend/flags/{release,dev}.json` and `backend/flags/{release,dev}.json`
  (and `admin-frontend/flags/` from train 2).
  Both images are built with `./frontend` and `./backend` as the Docker context, so they
  can't see `config/flags/`. CI runs the script with `--check` and fails if the generated
  files are stale. Nothing else merges or reads `config/flags/`.
- **Frontend.** `vite.config.ts` reads `frontend/flags/<profile>.json` for `FLAGS_PROFILE`
  (unset means `release` for `vite build`, and `dev` for the dev server and tests) and
  passes each `build` flag to `define` as its own literal, for example
  `'__FLAGS__.world_map': 'false'`, typed from the generated file so an unknown name fails
  the typecheck. The lazy import itself sits
  behind the constant, `const WorldMap = __FLAGS__.world_map ? lazy(() =>
  import('./pages/WorldMap')) : null`, because a module-level `lazy()` call isn't pure and
  would still emit the page's chunk.
- **Check.** A canary flag guards a module with a unique marker, and CI checks that the
  release build has no marker in any file in `dist/`. The canary is `kind: build`,
  `remove_by: unscheduled`, owned by this doc, and is the one permanent build flag. Exit
  criterion 4 of train 1 (no routed page imports mock data) also runs on the release
  build's output.
- **Backend.** `build` flags are compiled out too. A `cargo::rustc-cfg` applies only to
  the crate whose build script emits it, so each crate with flagged code gets a two-line
  `build.rs` that calls a small shared build-dependency, `econ-graph-flags-build`. The
  helper reads `backend/flags/<profile>.json` for `FLAGS_PROFILE`, emits
  `cargo::rustc-cfg=flag_<key>` for each `build` flag that is on and
  `cargo::rustc-check-cfg=cfg(flag_<key>)` for every one, and emits
  `cargo::rerun-if-env-changed=FLAGS_PROFILE` and `cargo::rerun-if-changed` for both
  `backend/flags/release.json` and `backend/flags/dev.json`. With no variable and no
  config set, the profile is `release`. Local development sets `dev` in
  `backend/.cargo/config.toml`, and `backend/Dockerfile` sets `ENV FLAGS_PROFILE=release`
  before building, so an image never picks up the development setting. Code uses
  `#[cfg(flag_mcp)]` on the `/mcp` filter. A flagged GraphQL area gets two cfg-gated
  definitions of the root, `#[cfg(flag_x)] #[derive(MergedObject)] struct Query(Core, X);`
  and `#[cfg(not(flag_x))] #[derive(MergedObject)] struct Query(Core);`. CI also builds
  with `FLAGS_PROFILE=release`, and the committed `schema.graphql` check (train 1 item 17)
  runs on that build, which is the backend's counterpart to the frontend canary.
  OpenFeature isn't needed until phase 2.
- **Admin app.** The same Vite `define` mechanism, with its own generated file. The admin
  app isn't deployed in train 1 (see [admin-ui.md](./admin-ui.md)), so it needs no flags
  until train 2.
- **Cleanup in the same train:** put unfinished and replaced code behind build-time flags
  that are off in every profile, and delete only what is known wrong (see the table
  above).

**The minimum train 1 needs:** the flag file with its metadata check and generator, the
Vite `define` for the main frontend, and, if MCP stays out of train 1, the backend build
helper for `/mcp`. That is two or three small PRs, no new service and no new runtime
dependency.

**Release 1 PRs:** the flag file, check and generator (#198), Vite build-time flags (#215,
stacked on #198), and the backend build helper with the `/mcp` gate (#222, stacked on
#198). Each PR updates this section when it lands or changes the plan.

### Phase 2: runtime flags and previews (train 2)

Needs Keycloak (train 1) and a deployed environment.

- `preview:access` joins the fine-grained role catalog. Keycloak grants it through the staff
  composites and a new `beta-tester` composite.
- **Backend** evaluates per request, in-process from the flag file mounted from a
  ConfigMap. The evaluation context is built from the verified token: `targetingKey` is
  the `sub`, plus `org` and `roles`. A preview flag's rule is "on if `roles` contains
  `preview:access`".
- **Moving parts.** `backend/flags/release.flagd.json` becomes a ConfigMap at deploy time
  and is mounted into the backend pod as a file (a whole-volume mount, not `subPath`, so
  the kubelet keeps it current). `open-feature-flagd` is a library compiled into the
  backend, registered in `main.rs` in file mode with the path from `FLAGS_FILE`, passed as
  its source configuration. It is built with `default-features = false` and only the
  `in-process` feature, which includes file mode and drops the REST client but still pulls
  in `tonic` and `prost`. It holds the flags in memory, evaluates with local function
  calls and reloads when the file changes, so a merged flag change reaches running pods
  within a minute or two, with no restart and no daemon.
- **Browser** gets runtime flags over OFREP (the OpenFeature Remote Evaluation Protocol)
  from the **backend**: a small handler at `/ofrep/v1/evaluate/flags` evaluates every flag
  with the same in-process provider and returns them in one response. The generated
  `.flagd.json` files contain no `build` flags, so neither the provider nor the handler
  ever sees them. The context comes from the verified token, not from
  what the browser sends. Anonymous visitors send a random id kept in local storage as
  `targetingKey`, which is acceptable because flags never grant access (rule 1). The React
  SDK's OFREP provider caches the response, its hooks read from the cache, and it
  re-fetches when the app updates the context on sign-in. Running flagd as its own
  Deployment is the fallback if a second service ever needs the same flags.
- **Kill switches** for the features with the most risk of load or abuse: `/mcp`,
  `crossSection`, CSV download. Crawler sources keep using `data_sources.is_enabled`.
- From phase 2 on, `scripts/flags-merge` writes two files per profile for the backend:
  `backend/flags/<profile>.json` (resolved values for the build helper) and
  `backend/flags/<profile>.flagd.json` (the full flagd file, without `build` flags, for
  runtime). Local development points `FLAGS_FILE` at `backend/flags/dev.flagd.json`. No
  extra container. `FLAGS_FILE` is read at runtime and is separate from the build-time
  `FLAGS_PROFILE`: a build flag compiled out by the profile stays out whatever the file
  says.

### Phase 3: product analytics (unscheduled; start with the first public deployment)

- Decide first-party or hosted (fork 3). Either way: page views, searches with no results,
  series opened, transformations applied and downloads, keyed by the same
  `targetingKey` as flags, with no series-level data sent anywhere.
- The privacy policy page (`/privacy`) is updated in the same PR.

### Phase 4: experiments (unscheduled; start when weekly users make a test feasible)

- Assignment by flagd's fractional split on `targetingKey`.
- An OpenFeature hook records an exposure event the first time a user is evaluated into an
  experiment.
- Analysis with the tool chosen in fork 3. Each experiment flag is `kind: experiment`, with
  a hypothesis, a primary metric and a stop date in its metadata.

## Forks for Joe

**Fork 1: flag the unfinished features, or delete them? Decided (Joe, 2026-09-26): compile
out, and delete only what is known wrong.** The options were flagging everything
unfinished with flags on in development, parking replaced code behind flags until its
replacement lands (the middle path in [releases.md](./releases.md)), deleting code with
no scheduled replacement, or deleting everything a roadmap replaces. The decision keeps
the middle path and extends it to unscheduled code. Flags for parked code are off in
both the release and development profiles, so nobody develops against fake data, and
each names its `remove_by` train, or `unscheduled` with the roadmap that owns it. The
cost is that parked code keeps compiling against every change on `main` until it is
finished or replaced.

**Fork 2: which flag tool?**

| Option | For | Against |
|---|---|---|
| **A. OpenFeature with flagd, flags in the repo (recommended)** | Free, Apache-2.0, nothing to operate in train 1, changes reviewed as PRs, provider can be swapped later | No UI, and flipping a runtime flag means editing a ConfigMap |
| B. Flagsmith (BSD) self-hosted | UI, audit log, Rust and React SDKs | A server and database to run before there is a deployment |
| C. Unleash self-hosted | The most widely used open source option | Server is AGPL-3.0 and needs its own database. SSO is paid |
| D. Build-time constants only | Nothing to add | Dead end: no runtime switch, no targeting, no experiments |

**Fork 3: analytics and experiment analysis, when the time comes?**

| Option | For | Against |
|---|---|---|
| A. PostHog Cloud free tier for analytics and analysis, with flagd still doing assignment | Nothing to run. Analytics and experiment results in one place | Our users' behavior goes to a third party. Needs a consent banner in some regions |
| **B. First-party events in our own database, with GrowthBook self-hosted for analysis (recommended, if Joe prefers open source and owning the data)** | All data stays with us. GrowthBook's free statistics are good | GrowthBook needs MongoDB, and we store and query the events ourselves |
| C. Decide when phase 3 starts | Traffic and a deployment will make the trade-off clearer | Nothing is lost by waiting, since phases 3 and 4 have no train yet |

Fork 1 is decided. Fork 2 affects train 1. Fork 3 can wait.

## Release trains

| Train | Flag work |
|---|---|
| 1 | Phase 1: the flag file and metadata check, build-time flags in the frontend and the backend, compiling unfinished code out behind flags off in every profile, and deleting what is known wrong |
| 2 | Phase 2: `preview:access`, per-request flags in the backend, OFREP served by the backend for the browser, kill switches |
| Unscheduled | Phase 3 (analytics) with the first public deployment. Phase 4 (experiments) once traffic allows |

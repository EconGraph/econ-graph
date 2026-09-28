# Release end-to-end suite

The Playwright project `release` (`frontend/playwright.release.config.ts`) runs
the frontend release build against the real backend on Postgres 18. The database
is seeded from recorded fixtures through the crawler adapters, so nothing
touches the network and no API keys are needed. This suite is the release exit
check (release 1 exit criterion 5). The other specs under `tests/e2e/` belong to
the older configs, which skip this directory.

## Run it

From the repo root:

```bash
scripts/release-e2e.sh                        # everything
scripts/release-e2e.sh smoke.spec.ts          # one file (extra args go to `playwright test`)
scripts/release-e2e.sh --headed series-ui/    # one area, with a visible browser
```

The script does five things:

1. It starts a fresh Keycloak container from `docker-compose.release-e2e.yml` on
   port 8081, importing the auth area's dev realm (`config/keycloak/`). Always,
   even when `RELEASE_E2E_DATABASE_URL` is set: CI has no service-container
   equivalent for it, since the realm and users files come from the checkout
   under test.
2. It starts a fresh Postgres 18 container from `docker-compose.release-e2e.yml`
   on port 5439, with data in tmpfs. It skips this when
   `RELEASE_E2E_DATABASE_URL` is set, which CI points at a service container.
   The script and the Playwright config ignore `DATABASE_URL`, so neither
   migrates or seeds your dev database.
3. It builds `econ-graph-backend` and `seed-fixtures` in one cargo invocation.
4. It runs `seed-fixtures`, which applies migrations and loads the series listed
   in `backend/crates/econ-graph-crawler/tests/fixtures/e2e-seed.json`.
5. It runs Playwright. Playwright starts the backend on port 18080, pointed at
   Keycloak through `OIDC_ISSUER`/`OIDC_AUDIENCE`. It also builds the frontend
   with `FLAG_PROFILE=release` and `VITE_OIDC_ISSUER` set, and serves it with
   `vite preview` on port 18081, which proxies `/graphql` and `/api` to the
   backend. The auth REST calls go straight to the backend through
   `VITE_API_URL`.

Locally, Playwright reuses a backend or frontend that is already listening on
those ports. Stop them, or you'll test a stale build.

Environment overrides:

- `RELEASE_BACKEND_PORT` changes the backend port. `RELEASE_FRONTEND_PORT`
  changes the frontend port.
- `RELEASE_BACKEND_BIN` points at a different backend binary. The script
  respects `CARGO_TARGET_DIR`.
- `RELEASE_CHROMIUM_PATH` sets a Chromium executable, for machines whose
  preinstalled browser doesn't match this Playwright version. For example, use
  `/opt/pw-browsers/chromium` in Claude Code cloud containers.

CI runs the suite in `.github/workflows/release-e2e.yml`, in Chromium only, on
every PR that touches `frontend/` or `backend/`.

## Against a deployed build (REL-QA check 3)

Set `RELEASE_BASE_URL` to the deployed frontend's origin and run Playwright
directly, not through the script:

```bash
cd frontend
RELEASE_BASE_URL=https://qa.example \
RELEASE_OIDC_ISSUER=https://auth.qa.example/realms/econ-graph \
RELEASE_QA_USER1_PASSWORD=... RELEASE_QA_USER2_PASSWORD=... \
  npx playwright test --config=playwright.release.config.ts
```

In this mode (`env.ts`):

- Playwright starts no backend or frontend and needs no database URL.
- Specs that compare with fixture values check live data instead: the journey
  checks that GDP's newest point is within FRED's publication cadence
  (`liveMaxAgeDays` in `fixtures.ts`) and that each transformation shows a
  number. The per-source cadence check for the other train 1 sources (check 3)
  comes with REL-4's six-source sweep.
- The signed-in specs use the two QA-only realm users, `qa-alice` and `qa-bob`
  (`config/keycloak/qa/`). Their passwords come from `RELEASE_QA_USER1_PASSWORD`
  and `RELEASE_QA_USER2_PASSWORD`; `RELEASE_QA_USER<n>`, `_NAME` and `_EMAIL`
  override the username, display name and email.
- Specs run one at a time, since two users are shared by every signed-in spec.
- `RELEASE_SKIP_AUTH=1` drops the `auth/` folder and ends the journey before it
  signs in, for a QA run while sign-in is broken. The config refuses to start
  without the issuer and passwords unless it is set.

## What the suite covers

- `smoke.spec.ts`: the home page loads, the seeded series are reachable through
  the frontend's `/graphql` proxy, and explore search finds one.
- `journey.spec.ts`: release 1 exit criterion 5's cross-area journey in one
  browser session. Search for GDP, open it, check the chart's newest point,
  apply every transformation and check the value it shows, then sign in and
  annotate. The CSV download step joins it with UI-6 (#223).
- `crawl/`: every visible control with a control's ARIA role (link, button, tab,
  checkbox, switch, combobox) on every route in `src/App.tsx` is clicked, signed
  out, starting from a freshly loaded page. The click must change the visible
  text, ARIA state, open overlays or layout, get a successful response to a
  request, or start a download or popup. A navigation within the site must land
  on a route the app serves, keep the query parameters it went with, and render
  something. Links off the site are only checked for an http(s) or mailto
  address, not followed. Controls inside menus and dialogs, signed-in controls,
  and clickable elements with no role are out of scope (give the last a role). A
  control that fails fails the crawl unless `crawl/allowlist.ts` lists it with a
  reason, normally the issue that fixes it. Delete an entry when its control is
  fixed; the crawl reports entries that excused nothing as
  `stale allowlist entry` annotations. A new route in `App.tsx` fails the crawl
  until `ROUTE_URLS` in `crawl/every-control.spec.ts` says which URLs to visit
  for it.
- `auth/`: sign-in and annotation privacy (the auth area's spec).

## Add specs for your area

Put them in `tests/e2e/release/<area>/`, for example
`series-ui/series-page.spec.ts`.

- **Assert on seeded data.** Use the constants in `fixtures.ts`, not literals
  copied from a fixture file, so a fixture change shows up in one place.
- **Need more data?** Add the recorded response under
  `backend/crates/econ-graph-crawler/tests/fixtures/<source>/`. Then add an
  entry to `e2e-seed.json` with the source, the external id, and each request
  the adapter makes as `method`, `path`, optional `query` and `fixture`. Add the
  series to `fixtures.ts`. The seed fails on any request the manifest doesn't
  cover, so a stale fixture shows up there, not as an empty page.
- **Need a signed-in user?** Use `signInOnKeycloak` from `auth/helpers.ts`,
  which drives Keycloak's login page, with a user from `USERS` in `env.ts`, and
  `uniqueTitle` for rows you create. Never sign the same dev user in from two
  specs at once: the realm's brute-force detection can then lock that user out
  (for a minute, by default). alice and bob belong to the auth area's serial
  group in `auth/sign-in.spec.ts` and dave to the journey; an area that needs a
  signed-in user adds its own dev user to `config/keycloak/dev/` and `USERS`, or
  its test joins that file.
- **Need a seeded series' id?** `seededSeriesId(request, SEEDED.x)` from
  `auth/helpers.ts` finds it by search, so it works in deployed mode too.
- **Keep specs independent.** They run in parallel against one shared database.
  A spec that writes, such as an annotation, creates its own rows and must not
  depend on rows another spec creates (the auth group's serial tests are the one
  exception).
- **Blocked by unmerged work?** Write the spec anyway and mark it `test.fixme`
  with a reason that names the blocking PR. Change it to `test` when the blocker
  merges.
- **Live QA.** Each spec should name the live QA check it stands in for, so the
  same specs can run against the deployed build in the final QA phase. Where a
  spec asserts a fixture value, branch on `DEPLOYED` from `env.ts` and assert
  what holds live instead.

## Not in the stack yet

- **Release flags.** `FLAG_PROFILE=release` has no effect until the build-time
  flag switch lands. The frontend build already sets it.
- **Test-only code in the backend binary.** The e2e backend is a debug build
  that shares one cargo invocation with `seed-fixtures`. That means it links the
  crawler's `testkit` feature. It is not the deployed image.

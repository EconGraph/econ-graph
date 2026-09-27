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

The script does four things:

1. It starts a fresh Postgres 18 container from `docker-compose.release-e2e.yml`
   on port 5439, with data in tmpfs. It skips this when
   `RELEASE_E2E_DATABASE_URL` is set, which CI points at a service container.
   The script and the Playwright config ignore `DATABASE_URL`, so neither
   migrates or seeds your dev database.
2. It builds `econ-graph-backend` and `seed-fixtures` in one cargo invocation.
3. It runs `seed-fixtures`, which applies migrations and loads the series listed
   in `backend/crates/econ-graph-crawler/tests/fixtures/e2e-seed.json`.
4. It runs Playwright. Playwright starts the backend on port 18080. It also
   builds the frontend with `FLAG_PROFILE=release` and serves it with
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
- **Keep specs independent.** They run in parallel against one shared database.
  A spec that writes, such as an annotation, creates its own rows and must not
  depend on rows another spec creates.
- **Blocked by unmerged work?** Write the spec anyway and mark it `test.fixme`
  with a reason that names the blocking PR. Change it to `test` when the blocker
  merges.
- **Live QA.** Each spec should name the live QA check it stands in for, so the
  same specs can run against the deployed build in the final QA phase.

## Not in the stack yet

- **Keycloak.** Auth specs are skipped until the auth area's dev realm (AUTH-3)
  and the backend's OIDC verification (AUTH-5) land. Then the stack starts
  Keycloak on port 8081 with that realm and its seeded users, and the frontend
  and backend are pointed at its issuer.
- **Release flags.** `FLAG_PROFILE=release` has no effect until the build-time
  flag switch lands. The frontend build already sets it.
- **Search.** `/explore` search is broken on main, in both the backend and the
  page. The smoke spec's search test is `fixme` until #165 and the series-ui
  explore fix (UI-9) merge.
- **Test-only code in the backend binary.** The e2e backend is a debug build
  that shares one cargo invocation with `seed-fixtures`. That means it links the
  crawler's `testkit` feature. It is not the deployed image.

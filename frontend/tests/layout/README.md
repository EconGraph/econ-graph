# Public and admin layout regression checks

Tracking:
[ECO-265](https://linear.app/econgraph/issue/ECO-265/add-maintained-playwright-visuallayout-regression-checks-to-ci).

From the repository root:

```sh
npm ci --prefix frontend
npm ci --prefix admin-frontend
cd frontend
npx playwright install --with-deps chromium
npm run test:layout
```

The dedicated config builds the actual public release entrypoint and admin
entrypoint, then owns two isolated Vite preview servers on ports 18181/18182. No
live backend, authentication account, crawler, or feature-flag change is
required. The release E2E suite remains responsible for actual backend
integration. The admin build here is `vite build`; the separate typecheck CI
remains responsible for TypeScript diagnostics. `LAYOUT_CHROMIUM_PATH`
optionally selects an already installed Chromium for local restricted
environments; CI installs the revision from the frontend lockfile.

## Contracts

| Surface           | Desktop 1440 × 1000                                                                                                                            | Mobile 390 × 844                                                                        |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Public navigation | Target containment, title/description separation, 44px targets, real Tab focus and painted outline                                             | Same contracts in the temporary drawer                                                  |
| Public dashboard  | Indicator row alignment/equal height, text containment, panel overflow, separated quick actions, page overflow                                 | Stacked indicator cards, text containment, panel overflow, separated actions            |
| Admin crawler     | Card width/alignment, label/value separation, text containment, panel/page overflow; action spacing, tab containment and real Tab focus ripple | Stacked status/queue cards, label/value separation, text containment and panel overflow |

These are browser-rendered geometry and computed-style assertions, not
screenshot capture presented as a test. Screenshots and traces are failure
diagnostics; there are no pixel baselines to silently create or update. The 1px
geometric allowance is for subpixel box rounding, not a screenshot mismatch
threshold. Tests use zero retries.

Fixtures intercept only named GraphQL reads, reject unexpected API/external
traffic, and fail on page JavaScript errors. They use stable records, a fixed
date, UTC, en-US, light mode and reduced motion. Four locally bundled Latin
Roboto fonts are loaded before app rendering; live Google Fonts stylesheets are
replaced with empty CSS. The font assets are unchanged from
`@fontsource/roboto@5.2.5` (npm tarball SHA-1
`b2d869075277e2cba31694951a2d355a8965d763`); their OFL license is alongside
them. No application typography/layout rules are replaced. Emoji may use system
fallback; these contracts do not compare emoji pixels.

`.github/workflows/visual-layout.yml` runs on frontend/admin/flag/workflow
pull-request changes (including stacked PRs), installs both lockfiles and
Chromium, builds both apps, then runs this command. Failed runs retain HTML
reports, screenshots and traces for 14 days. Locally open
`playwright-report-layout/index.html` or use
`npx playwright show-report playwright-report-layout`.

## Scope and known defects

This is representative coverage, not a claim that every route or mobile shell is
responsive. On the initial main-based audit, a 390px mobile viewport exposed
admin crawler heading/actions extending the document to 720px and clipped
centered tabs; the public dashboard extended to 398px. The separate
responsive-shell fix will add whole-page mobile containment assertions. The
admin action/tab test is explicitly desktop-only until that fix; mobile card
assertions still run. No blanket CSS masks, production style edits or wider
mobile viewport hide those defects here.

## Proving sensitivity

Temporarily set both sidebar title and description to `display: inline` in
`src/components/layout/Sidebar.tsx`, then run:

```sh
npm run test:layout -- --project=desktop --grep 'public navigation'
```

The text-separation assertion must fail. Restore the source and rerun the full
suite. Never commit fault injections or update expectations merely to make a
regression pass.

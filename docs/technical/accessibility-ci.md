# Accessibility CI

In either `frontend` or `admin-frontend`, run:

```sh
npm ci
npm run lint:accessibility
npm run test:accessibility
```

The static audit reuses the project's flat ESLint parser, plugins, settings and
file matching, runs only accessibility rules, and enables the complete jsx-a11y
recommended ruleset. `--no-inline-config` prevents existing inline lint disables
from hiding violations. Admin ESLint and `@eslint/js` use v9 because jsx-a11y
6.10.2 declares support through ESLint v9; no peer-dependency bypass is used.

The dedicated Vitest configurations inherit the existing React transform,
jsdom environment, aliases and public frontend build flags. Their setup replaces
unit-test mocks that substitute UI and routing. The accessibility directory is
excluded from the regular unit suite so these tests always use their dedicated
setup. `passWithNoTests: false` makes a missing test suite fail.

Public tests mount the real App with MemoryRouter and QueryClientProvider; App
supplies authentication and theme providers. They audit About, Privacy and the
not-found route, including the shared header/sidebar. Admin tests mount the real
App and theme provider, wait for the crawler dashboard to load, and stub only
GraphQL fetch responses. App supplies its real Apollo and React Query providers.
Neither suite mocks UI components or filters/disables axe rules.

Reports in each project's ignored `accessibility-results/` directory include
ESLint JSON, Vitest JSON/JUnit, full axe results (including incomplete checks),
and CI installation/audit logs. CI uploads them even after failure. Independent
matrix entries ensure a failure in one frontend does not cancel the other's
audit. The workflow summary uses actual job and step outcomes, distinguishes
skipped checks from passes, and fails when a dependency job fails or is cancelled.
The manual artifact is a checklist, not evidence of completed testing.

## Focused financial browser regressions

In `frontend`, run:

```sh
npx playwright install chromium
npm run test:accessibility:browser
```

These five Playwright/Chromium checks mount the real FinancialExport,
FinancialMobile and FinancialDashboard components with production CSS and theme
providers. Only data is supplied by the fixture (including a seeded QueryClient);
no UI components or styles are mocked. The test-only HTML entry is outside the
production routes and Vite build entries. This is focused component/browser
coverage, not a full application end-to-end audit.

Checks cover export choices at 390px and 1280px, mobile filing rows at 390px,
and desktop dashboard filing rows in light and dark themes. Geometry assertions verify stacked text,
full-width rows, icon alignment and the responsive export grid. Computed styles
check the native-button reset and selection indication. Real Tab/Shift+Tab,
Space and Enter exercise keyboard focus, export selection and dashboard filing
selection. A targeted contrast assertion catches dark-theme hover text regressions. A 2px focus outline with an offset is required. The accessibility
workflow runs this suite alongside the unchanged static and axe audits and
preserves failure screenshots/traces; a failed browser job fails the summary.

For environments with an existing Chromium installation, set
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` to its executable path. Local validation used
Chrome for Testing 153.0.8010.12 from the official Google storage host after the
Playwright CDN returned an unavailable HTML page instead of an archive.

## Findings exposed by the repaired checks

Validation against main `1dbb3ce` exposed the defects below. The follow-up
application fixes replace clickable containers with native buttons, remove login
autofocus, correct heading semantics while retaining typography, and add admin
landmarks and progress labels. Both static audits and all four App axe tests now
pass with the original rules and assertions:

- Public static audit: six errors across `FinancialDashboard.tsx`,
  `FinancialExport.tsx`, and `FinancialMobile.tsx`: clickable non-interactive
  elements lack keyboard handling and appropriate interaction semantics.
- Admin static audit: `pages/auth/LoginPage.tsx` uses `autoFocus`.
- Public runtime: About and Privacy skip heading levels (`heading-order`).
  The not-found route passes.
- Admin runtime: the queue progress bar lacks an accessible name
  (`aria-progressbar-name`), headings skip levels (`heading-order`), and page
  content lacks landmark containment (`region`).

## Failure propagation validation

A temporary test in each accessibility directory rendered
`<main><button aria-label="Probe action" /></main>` and called the same
`auditPage` helper as the app tests. Running
`npm run test:accessibility -- -t 'deliberate violation probe'` exited 0.
Removing the button's accessible name produced `button-name` and exit 1 in both
projects. The temporary tests and deliberate violations were removed afterward.
Temporarily removing the app test files also produced exit 1 for empty suites.

For the financial browser suite, temporarily emptying the production
`financial-selection.css` made all four tests fail with the browser default
`appearance: auto` instead of the required button reset. Restoring the stylesheet
returned the suite to four passes. A subsequent dark-theme hover case reproduced
the reviewer finding at about 1.05:1 contrast before the correction; translucent
hover/selection surfaces restored readable inherited text. All five final browser
cases pass. The deliberate regressions were not committed.

## Coverage limits

This is a focused audit, not an exhaustive route/state audit. Data-heavy public
routes, admin configuration/log tabs and authentication states need additional
maintained cases. jsdom does not implement full browser layout or canvas behavior;
axe reports `color-contrast` as incomplete here. Keep browser-based contrast,
keyboard, focus, zoom and screen-reader testing alongside these checks. Full axe
reports retain incomplete findings rather than claiming complete WCAG compliance.

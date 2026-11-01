# Accessibility CI

In either `frontend` or `admin-frontend`, run:

```sh
npm ci
npm run lint:accessibility
npm run test:accessibility
```

For the frontend's real-browser financial selection checks:

```sh
cd frontend
npx playwright install chromium
npm run test:accessibility:browser
```

The static audit reuses the project's flat ESLint parser, plugins, settings and
file matching, runs only accessibility rules, and enables the complete jsx-a11y
recommended ruleset. `--no-inline-config` prevents inline lint disables from
hiding violations. Admin ESLint and `@eslint/js` use v9 because jsx-a11y 6.10.2
declares support through ESLint v9; no peer-dependency bypass is used.

The dedicated Vitest configurations inherit the existing React transform,
jsdom environment, aliases and public frontend build flags. Their setup replaces
unit-test mocks that substitute UI and routing. The accessibility directory is
excluded from the regular unit suite so these tests use their dedicated setup.
`passWithNoTests: false` makes a missing test suite fail.

## Current application coverage

Public tests mount the real App with its routing, authentication and theme
providers, plus a fresh React Query client. They audit:

- About, Privacy and the not-found route, including the shared header/sidebar;
- the dashboard and Sources page with loaded, empty, failed-request and pending
  responses, so a loading shell cannot stand in for the loaded data UI.

The public suite has 11 cases. The admin case mounts the real App and theme,
waits for the current crawler overview and Queue counts table, and audits its
navigation and dashboard. Transport fixtures return the current GraphQL/API
contracts; UI components and axe rules are not replaced or disabled.

The frontend Chromium suite checks the financial selection components' production
styles, layout, visible keyboard focus, selection and keyboard activation. These
components remain behind a disabled release flag; maintaining their semantics
does not enable the financial demo or establish release acceptance for it.

## Required merge gate

Core CI calls `.github/workflows/accessibility-tests.yml` as a reusable workflow.
It runs static and jsdom audits independently for both frontends, plus the
frontend Chromium checks. Its caller follows Core CI's existing filters:
frontend/shared changes run the audits; documentation-only and backend-only PRs
may skip them. Protected-branch pushes and merge groups run them without path
filtering. There is no second PR-triggered accessibility workflow duplicating
the same audit matrices.

The reusable workflow's summary requires every selected automated job to succeed.
Failure, cancellation, a missing result or a skipped selected audit fails it.
Core CI includes the reusable job in both `ci-result.needs` and its merge-group
`MUST_RUN` list. A failing accessibility workflow therefore fails the existing
required `Core CI Result`; a merge group cannot pass by skipping it. This uses
the existing branch rule and does not require a separate accessibility status
check to be configured in GitHub.

The same workflow supports manual `all`, `static`, `runtime` and `manual` modes,
and its weekly schedule. Runtime mode includes the Chromium checks. The manual
artifact is a checklist, not evidence of completed testing. Manual-only runs do
not claim to have executed automated audits.

Reports in each project's ignored `accessibility-results/` directory include
ESLint JSON, Vitest JSON/JUnit, full axe results (including incomplete checks),
and installation/audit logs. Browser failure artifacts are preserved too. CI
uploads reports after failure, and matrix fail-fast is disabled so one frontend
does not cancel the other's audit. The summary reports actual job and step
outcomes and distinguishes skipped checks from passes.

## Defects confirmed against current main

Using the repaired harness with main `b32da9f4` reproduced About's skipped heading
levels (`heading-order`) and the current admin application's missing main
landmark (`region`). Applying the heading and landmark fixes made those audits
pass. Privacy and the not-found page already passed that control; older findings
about Privacy or the replaced admin dashboard are not current defects.

The maintained data-page cases also check the current dashboard's repeated source
notices and the Sources page's summary counts. Source notices use labels specific
to each series, and numeric summary values do not create misleading heading
levels. Native buttons and pressed state preserve keyboard selection in the
retained financial components; production CSS and browser checks keep those
semantic fixes from introducing layout or focus regressions.

## Failure propagation validation

`ci/scripts/test_accessibility_ci.py`, run by the required workflow lint job,
executes the accessibility and Core CI result scripts with failed, cancelled,
skipped, missing and empty job results. It checks manual selection modes, normal
PR filtering, and the merge group's requirement that the audits run. It also
checks that the reusable caller and summary dependencies connect those results
to `Core CI Result`. These are offline contracts; they do not claim that a hosted
run executed the UI suites.

The UI tests retain complete axe results and assert no violations. A passing
workflow must also discover and execute its selected suites; empty Vitest suites
fail rather than passing as an audit.

## Coverage limits

This is focused regression coverage, not an exhaustive route/state audit.
Other data-heavy public routes, admin tabs, authenticated account states and
assistive-technology journeys need maintained cases. jsdom does not implement
full browser layout or canvas behavior; axe reports `color-contrast` as incomplete
there. The bounded Chromium component checks do not replace full-page contrast,
zoom, keyboard or screen-reader testing. Full axe reports retain incomplete
findings rather than claiming complete WCAG compliance.

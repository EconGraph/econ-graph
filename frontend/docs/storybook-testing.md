# Storybook testing workflow

Use Storybook to inspect frontend components with fixture data and compare their
exported variants. Browser inspection, automated rendering checks, and screenshot
regression tests establish different things; this setup provides no screenshot
baselines or image comparisons.

## View and build stories

From the repository root, use the Node version in [`.nvmrc`](../../.nvmrc) and install
the locked frontend dependencies:

```bash
cd frontend
npm ci
npm run storybook
```

Open `http://localhost:6006`. Useful starting points are:

| Sidebar component | Exported stories |
| --- | --- |
| `Financial/FinancialDashboard` | `Default`, `BeginnerUser`, `AdvancedUser` |
| `Financial/FinancialStatementViewer` | `Default`, `BeginnerUser`, `AdvancedUser`, `ExpertUser` |

These variants change component args such as `userType` and educational features.
They do not select loading or error responses. Inspect or edit the `args` in the
[dashboard stories](../src/components/financial/FinancialDashboard.stories.tsx) or
[statement viewer stories](../src/components/financial/FinancialStatementViewer.stories.tsx)
to reproduce a particular prop combination. For responsive inspection, open the
story canvas in its own browser tab and resize it or use browser device emulation.

[`.storybook/main.ts`](../.storybook/main.ts) registers only the onboarding and
links addons. The controls and viewport parameters in `preview.tsx` do not enable
those addons, so this workflow does not depend on a Controls panel or Viewport
toolbar.

To check static bundling separately from the development server, run this from
`frontend/`:

```bash
npm run build-storybook
```

## Understand the shared preview

[`.storybook/preview.tsx`](../.storybook/preview.tsx) wraps stories with Material UI
styled-engine and theme providers, date localization using `AdapterDateFns`,
`CssBaseline`, and React Query's `QueryClientProvider`. Add a story decorator only
for context the component needs beyond those global providers. There is no global
error boundary; supply one in a story if its behavior is part of the example.

One module-level `QueryClient` is shared across stories and renders. It disables
query retries and refetching on window focus, treats data as fresh for five
minutes, and retains inactive queries for ten minutes. Switching stories does not
clear cached data. When changing mock responses, invalidate or clear the affected
queries, or reload the preview, so an earlier cached result does not hide the
change.

## Choose fixture responses

The preview calls [`setupSimpleMSW`](../src/test-utils/mocks/simpleServer.ts).
Despite its name, this helper replaces `global.fetch`; it does not start an MSW
service worker or reuse the regular tests' MSW server. Requests whose URL contains
`graphql` are handled using the JSON request body's `operationName` (or a name
extracted from its query), variables, and the current mock scenario. Other
requests pass through to the original fetch. The preview's `msw.handlers`
parameter does not select responses: no MSW addon or loader consumes it.

The helper maps GraphQL operation names to fixture keys. For example,
`GetFinancialStatement` maps to `get_financial_statement`. The
[response loader](../src/test-utils/mocks/graphql-response-loader.ts) statically
imports JSON from
[`graphql-responses/`](../src/test-utils/mocks/graphql-responses/) and selects the
entry in `RESPONSE_MAP` for that key and scenario, such as
[`get_financial_statement/error.json`](../src/test-utils/mocks/graphql-responses/get_financial_statement/error.json).
Adding a JSON file alone does not register it: add its import and map entry too.
A new GraphQL operation also needs an entry in `simpleServer.ts`'s `operationMap`.

The default scenario is `MockScenarios.SUCCESS`. To create a response variant,
call `setMockScenario(MockScenarios.ERROR)` before the component requests data,
and reset it to `MockScenarios.SUCCESS` when leaving that variant. The scenario is
module state; the preview does not reset it between stories. Check `RESPONSE_MAP`
or `getAvailableScenarios()` before choosing a scenario, because operations
support different subsets. A `loading` fixture returns immediately with fixture
data; it does not delay the request or keep a fetch pending.

A request's `statementId` overrides the global scenario when it is
`error-statement-id`, `empty-statement-id`, `loading-statement-id`, or
`not-found-statement-id`. These select `error`, `empty`, `loading`, and `not_found`
respectively, but still require that operation/scenario pair in `RESPONSE_MAP`.
An unknown GraphQL operation returns `data: null` and an `Unknown operation`
error. A missing fixture key or scenario returns `data: null` with a GraphQL error
whose code is `MOCK_NOT_FOUND`; it does not fall back to the success fixture.

## Run browser checks

The installed `@storybook/test-runner` package exposes the `test-storybook`
executable. With Storybook running in another terminal and Playwright's Chromium
installed, invoke it directly from `frontend/`:

```bash
./node_modules/.bin/test-storybook --config-dir=.storybook --url http://127.0.0.1:6006 --disable-telemetry
```

Use `--help` to inspect supported options, `--browsers firefox chromium` to select
browsers with installed binaries, or `--listTests` to list test files. Even
`--listTests` first requires a running Storybook. The `test-storybook` npm scripts
currently name `@storybook/test-runner` as an executable; use the direct command
above instead. The runner has no Playwright Test `--ui` or `--headed` mode.

The runner uses Jest with Playwright. Its hook configuration is
`.storybook/test-runner.ts` (or `.js`), and Jest overrides use
`test-runner-jest.config.*`. The checked-in
[`.storybook/test-runner.config.ts`](../.storybook/test-runner.config.ts) is not
loaded by this runner; its Playwright-shaped browser projects, retries, and
reporters do not apply. See the [runner configuration reference](https://github.com/storybookjs/test-runner#ejecting-configuration)
for the supported contract.

The runner checks that selected stories render and executes their `play`
functions, if present. The active stories have no `play` assertions, and this
repository has no Storybook screenshot comparison hook or baseline setup. A
rendering pass therefore does not establish interaction or visual regression
coverage. CLI help and configuration loading can be checked without a browser;
a successful story run requires the running preview and browser binaries.

## Troubleshooting

- **A variant shows old data:** check the shared React Query cache and the current
  global scenario before changing fixtures. Reloading the preview resets both.
- **A mock returns `MOCK_NOT_FOUND`:** read the logged operation/scenario pair,
  then check the operation mapping, static import, and `RESPONSE_MAP` entry. A
  statement-ID override can select a scenario that the operation does not support.
- **A browser import mentions `fs` or `path`:** trace the import back to the story
  or mock and use browser-compatible code. The fixture loader uses static JSON
  imports; externalizing a Node module does not provide its filesystem API in a
  browser. Check aliases and the `global: 'globalThis'` definition in `main.ts`
  when diagnosing resolution or global-variable errors.
- **JSX fails to parse:** use `.tsx` for TypeScript containing JSX, as the global
  preview does.
- **The runner executable is missing:** run `npm ci` in `frontend/`; the dependency
  is already declared and locked. Use the direct binary above rather than adding
  another dependency.
- **The runner cannot reach Storybook:** confirm the development server is
  listening on port 6006 and that `--url` matches it. Building static files alone
  does not serve them.

## CI coverage

The workflows do not build or test Storybook. Core CI's frontend jobs run
TypeScript checks, Vitest coverage, and release-bundle checks; its dependent
frontend integration job runs `test:coverage` again. These checks do not replace
local story inspection or a Storybook runner invocation.

Files under `frontend/`, including this guide, match Core CI's frontend path
filter and the separate Release E2E workflow's pull-request filter. A
documentation-only edit here can therefore incur frontend checks and the release
Chromium suite, which builds the application and backend against a fixture-seeded
Postgres database. Those application checks do not validate Storybook.

- [Frontend Summary](../../docs/technical/FRONTEND_SUMMARY.md)
- [CI/CD Pipeline](../../docs/development/CI_CD_PIPELINE.md)

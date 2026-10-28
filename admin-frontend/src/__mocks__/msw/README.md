# GraphQL test mock helpers

Vitest's shared [test setup](../../setupTests.ts) starts the separate
[`src/mocks/server.ts`](../../mocks/server.ts), using
[`src/mocks/handlers.js`](../../mocks/handlers.js). The helpers in this directory
require explicit setup in tests; neither is selected automatically.

## Choosing a helper

[`server.ts`](server.ts) exports an MSW server that handles `POST /graphql`.
Its `operationName` switch defines the supported operations and scenarios, using
JSON fixtures from [`../graphql`](../graphql). To use this server instead of the
shared server, configure a dedicated Vitest setup file to call `server.listen()`
before tests, `server.resetHandlers()` after each test, and `server.close()` after
all tests. Call this module's `resetMockScenario()` after each test as well:
resetting MSW handlers does not reset the scenario. The initial and reset scenario
is `success`.

[`simpleServer.ts`](simpleServer.ts) provides a fetch-spy alternative.
`setupSimpleMSW()` installs a Vitest spy on `global.fetch` that returns fixtures
for GraphQL requests and passes other requests to the previous fetch function.
Its `mockResponses` map defines the supported operation and scenario pairs;
unsupported pairs return GraphQL errors. Select a scenario explicitly before
making requests, and use `cleanupSimpleMSW()` after the suite. Cleanup calls
`vi.restoreAllMocks()`, so it also restores other spies created by the test.

For a test in `src/pages/__tests__`, initialize the fetch helper as follows:

```ts
import { beforeAll, beforeEach, afterAll } from "vitest";
import {
  setupSimpleMSW,
  setMockScenario,
  cleanupSimpleMSW,
} from "../../__mocks__/msw/simpleServer";

beforeAll(async () => {
  await setupSimpleMSW();
});

beforeEach(() => {
  setMockScenario("success");
});

afterAll(async () => {
  await cleanupSimpleMSW();
});
```

Each test can select another scenario supported by its operation. The fetch
helper's initial scenario and `resetMockScenario()` value are `default`, which
has no entry in `mockResponses`; setting `success` before each test avoids an
unknown-scenario response. [SystemHealthPage.test.tsx](../../pages/__tests__/SystemHealthPage.test.tsx)
also demonstrates the setup and cleanup hooks.

## Debug output

Diagnostic logs from these two helpers are disabled by default. Set the Vite
variable before starting Vitest because the flag is read when the helper modules
are imported. Run this command from `admin-frontend`:

```bash
VITE_MSW_DEBUG=true npm test
```

Depending on the helper, the logs include:

- GraphQL request interception
- Operation names and variables
- The current scenario and selected response
- Available operations and scenarios for the fetch helper

Unknown-operation, unknown-scenario, and request-processing errors are logged
with `console.error` independently of this flag.

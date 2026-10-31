The Train 5 workflow runs against an isolated, real backend with loaded companyfacts and a signed-in browser session. It does not mock API responses. Use a fixture company with multiple periods in all three statements and a SIC peer with compatible dates.

Start the candidate backend and frontend with `financial_components` enabled only in the disposable test build. The committed dev and release profiles remain disabled until release acceptance. Preserve and restore generated flag files after a local test build. `TRAIN5_AUTH_STORAGE_STATE` points to Playwright storage state containing the test account's identity-provider session; notes assertions verify that sign-in succeeded.

From `frontend/`:

```sh
TRAIN5_BASE_URL=http://localhost:18888 \
TRAIN5_CANDIDATE_SHA="$(git rev-parse HEAD)" \
TRAIN5_COMPANY_CIK=0000320193 \
TRAIN5_AUTH_STORAGE_STATE=/tmp/train5-browser-session.json \
npx playwright test --config=playwright.train5.config.ts
```

The JSON report defaults to `test-results/train5-playwright.json`. It records the candidate SHA and the real-backend/authenticated run declaration. The release evidence checker additionally requires the successful workflow result; listing tests or supplying metadata alone does not establish acceptance.

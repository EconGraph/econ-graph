#!/usr/bin/env bash
# qa-scenario: anonymous-browser
# qa-mutates: no
# qa-profiles: lab, ci, production
# qa-timeout: 600
# qa-description: Signed out, in Chromium: search, open a series, its chart, the CSV download, and the world map.
#
# Runs frontend/tests/e2e/qa/anonymous-browsing.spec.ts (frontend/playwright.qa.config.ts)
# against QA_TARGET and records each test as a check.
#
# Inputs: QA_TARGET; QA_SEARCH_QUERY (default "GDP"). Needs frontend/node_modules and
# Playwright's Chromium (`npm ci` and `npx playwright install chromium` in frontend/); skipped
# without them. QA_CHROMIUM_PATH sets a Chromium executable.
set -euo pipefail

repo="${QA_REPO_ROOT:?run through scripts/qa/run-scenarios.sh}"
evidence="${QA_EVIDENCE_FILE:-/dev/null}"
work="${QA_WORK_DIR:?run through scripts/qa/run-scenarios.sh}"

if [ ! -x "$repo/frontend/node_modules/.bin/playwright" ]; then
    python3 -c 'import json; print(json.dumps({"skip": "Playwright is not installed: run npm ci in frontend/"}))' >>"$evidence"
    echo "skip: Playwright is not installed: run npm ci in frontend/"
    exit 77
fi

report="$work/playwright.json"
set +e
(
    cd "$repo/frontend"
    PLAYWRIGHT_JSON_OUTPUT_NAME="$report" \
        QA_PLAYWRIGHT_OUTPUT="$work/test-results" \
        ./node_modules/.bin/playwright test --config=playwright.qa.config.ts --reporter=list,json
)
status=$?
set -e

# One check per test, with the first error line of a failure.
python3 - "$report" >>"$evidence" <<'PY'
import json, re, sys
ANSI = re.compile(r"\x1b\[[0-9;]*m")
try:
    report = json.load(open(sys.argv[1]))
except (OSError, ValueError) as error:
    print(json.dumps({"check": "playwright report", "ok": False, "detail": str(error)}))
    sys.exit()

def specs(suite):
    yield from suite.get("specs", [])
    for child in suite.get("suites", []):
        yield from specs(child)

for suite in report.get("suites", []):
    for spec in specs(suite):
        for test in spec.get("tests", []):
            results = test.get("results") or [{}]
            last = results[-1]
            status = last.get("status", test.get("status"))
            error = ANSI.sub("", (last.get("error") or {}).get("message", "")).strip().splitlines()
            detail = error[0] if error else f"{status} in {last.get('duration', 0) / 1000:.1f}s"
            if status == "skipped":
                continue
            print(json.dumps({"check": spec["title"], "ok": status == "passed", "detail": detail}))
PY
exit "$status"

# Epic search-to-chart demo

Finding an economic series and turning it into a chart makes EconGraph's value
visible: a researcher can move from a question about GDP to observations and
growth calculations in the same interface. The
[Epic recorder](epic-e2e-demo.sh) contains a Playwright sequence for presenting
that journey, with a 1920×1080 video configuration and screenshots along the way.
Its browser actions illustrate a workflow; they do not assert that the displayed
data or calculations are correct.

## Choose the evidence for your demonstration

For a reproducible search-to-chart check, follow the
[release end-to-end suite's run guidance](../frontend/tests/e2e/release/README.md#run-it).
In fixture mode, its [launcher](../scripts/release-e2e.sh) loads checked-in
fixtures through the crawler adapters into PostgreSQL. The backend reads that
same database, and the frontend connects to that backend. The
[release journey](../frontend/tests/e2e/release/journey.spec.ts) searches for GDP,
opens the series, checks the chart and newest observation, and compares
transformation values with fixture expectations. This exercises real adapter,
storage, API and UI code with fixture inputs, rather than crawling live sources.
The suite also offers a headed run for watching the interactions; its
[Playwright configuration](../frontend/playwright.release.config.ts) retains
failure traces and screenshots, without enabling video recording.

Backend database tests provide narrower checks of persistence and queries; see
the [database testing guide](../docs/testing/DATABASE_TESTING.md) for their setup
and isolation requirements. These tests do not start a UI or supply the database
for the Epic recorder.

The Epic shell script invokes backend and frontend test-name filters before
starting separate servers. Neither `epic_e2e_complete_system_demonstration` nor
`Epic Complete User Journey` matches a current test, and the frontend command
passes Jest-style options to the Vitest test script. It does not pass a
test-container database or seeded dataset to its backend server. A recording
therefore cannot establish that crawler-created rows reached the UI.

## Read the Epic launcher before using it

The Epic launcher does not provide a reproducible test-and-recording command.
Its directory changes break relative log paths;
later, it changes into `epic-demo-results` while the generated JavaScript still
prefixes video and screenshot paths with `./epic-demo-results/`. Installing
Playwright alone or changing the launch directory does not correct these paths.
These launcher and artifact defects are covered by
[ECO-592](https://linear.app/econgraph/issue/ECO-592).

The script also installs browser packages into the frontend's development
dependencies, starts backend and frontend processes, and launches headed
Chromium. Its final cleanup uses process-name matches for `cargo run` and
`npm start`, which can affect other matching processes, and has no exit trap to
ensure cleanup after a failure. Use the release suite's setup guidance for
repeatable validation rather than treating this script as a quick start.

## The GDP recording sequence

The generated Playwright sequence expresses one researcher question: “How can I
find GDP data and explore its growth?” It attempts the following steps against
the application served at `http://localhost:3000`:

1. Open the application, then follow **Explore**, falling back to `/explore`.
2. Type `Real GDP` into the first text input and press Enter after a fixed wait.
3. Try to click text matching `GDP`; continue if selection fails.
4. If the first `svg`, `canvas` or `.chart` element is visible, hover at three
   positions. The selector can match other graphics, and the sequence does not
   check whether a chart tooltip appears.
5. If a button containing `Year` is visible, click it. The sequence does not
   verify that a year-over-year transformation was applied.
6. Capture the final page state and close the browser context.

The recorder does not perform an unemployment comparison, choose a date range,
or verify source and revision information. The broader demonstration scope and
its evidence requirements are recorded in
[ECO-638](https://linear.app/econgraph/issue/ECO-638).

## Recognize the assets and their limits

The JavaScript names the following screenshots. Each capture depends on reaching
its step successfully; these names describe capture points, not passed checks.

| Screenshot name | Capture point |
| --- | --- |
| `01_app_loaded.png` | Initial application page |
| `02_series_explorer.png` | Page after Explore navigation |
| `03_search_typed.png` | Text input after typing `Real GDP` |
| `04_search_results.png` | Page after Enter and a fixed wait |
| `05_result_selected.png` | Only after a successful GDP selection attempt |
| `06_chart_tooltip_1.png`, `07_chart_tooltip_2.png`, `08_chart_tooltip_3.png` | Only when the selected graphic is visible and the hovers succeed |
| `09_transformation.png` | Only when the `Year` button is visible and clicked |
| `10_final_state.png` | Page at the end of the sequence |
| `error_state.png` | Attempted capture when the sequence catches an error |

The video configuration requests 1920×1080 output but assigns no fixed filename.
The shell names timestamped logs
`backend_epic_test_<timestamp>.log`, `frontend_epic_test_<timestamp>.log` and
`video_demo_<timestamp>.log`, plus `EPIC_DEMO_REPORT_<timestamp>.md`. Because of
the path defects above, these names do not constitute an expected results tree.

The report is a fixed success template, and caught browser errors do not become
failed assertions. Shell pipelines also check the log writer's exit status
instead of the test command's. Review the actual captures and test results before using them
as evidence. The recorder collects no search, rendering or hover timings, frame
rate, memory measurements, database query analysis, or accessibility results.
Its video settings and fixed waits support presentation, not performance or
production-readiness conclusions.

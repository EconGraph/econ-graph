# EconGraph Demo Tools

This directory contains browser recording scripts, timed narration, presentation generators, and existing demo media. Use the resources below to prepare a walkthrough or find a recording. For application validation, follow the [maintained release E2E suite](../frontend/tests/e2e/release/README.md) and [E2E testing strategy](../docs/development/e2e-testing-strategy.md).

## Recordings, narration, and guides

- [Professional demo recording](professional-econgraph-demo-v2.mp4): the existing video asset.
- [Business demo walkthrough](temp_business_demo_script.md): a three-to-four-minute sequence of spoken narration and browser actions, with manual screen-recording instructions.
- [Business impact narration](professional-business-impact-narration.txt): a timed, roughly ninety-second voiceover script.
- [Epic demo guide](EPIC_DEMO_README.md): the crawling-to-chart demonstration sequence and artifact layout.
- [Professional demo summary](PROFESSIONAL_DEMO_SUMMARY.md): the recording components and presentation topics.
- [Comprehensive demo error screenshot](comprehensive-demo-error.png) and [error screenshot](error-screenshot.png): captured failure states for diagnosing recordings.

The walkthrough and narration provide presentation material; check the actions and feature claims against the application being recorded. The launcher limitations below apply to the commands in the companion guides too.

## Recording and presentation tools

Paths in this section are relative to the repository root. The two orchestration launchers do not work with this checkout as written; their limitations are described below.

### Epic system demonstration

[`demo-tools/epic-e2e-demo.sh`](epic-e2e-demo.sh) attempts to run backend and frontend tests, start both services, and record a browser journey with Playwright. It creates `epic-demo-results/` for videos, screenshots, logs, and a generated report.

It assumes the caller is at the repository root, but retains relative log paths after changing into `backend/` and `frontend/`, so those paths no longer point to the directories it created. Its named backend and frontend demo tests are absent, and it passes Jest options to the frontend's Vitest command. Moving to the root does not make this a working demonstration or validation command.

### Professional recording orchestration

[`demo-tools/professional-demo-orchestrator.sh`](professional-demo-orchestrator.sh) coordinates PostgreSQL, migrations, backend and frontend startup, dependency installation, and [`demo-tools/create-professional-demo-video.js`](create-professional-demo-video.js). The recorder drives a headed Chromium browser at `http://localhost:3000`, overlays narration text, and writes a video and generated documentation.

The orchestrator requires `docker-compose.yml` and `create-professional-demo-video.js` in the caller's directory. The Compose file is at the root, while the recorder is under `demo-tools/`, so neither directory satisfies both checks in this checkout. The recorder also renames Playwright's WebM output to an `.mp4` filename without transcoding it; the filename does not establish the media format.

If execution reaches the upload stage, the orchestrator stages demo files, creates a commit when the checkout has changes, and runs `git push origin main`. It then prepends a demo section to the caller's `README.md`. Its exit trap runs Compose teardown and kills processes matching its backend, frontend, and recorder commands, including when a prerequisite check fails. These effects are part of the script, not a separate optional publishing step.

### Narration and presentation generation

[`demo-tools/create-professional-narration.sh`](create-professional-narration.sh) reads the business impact narration and uses macOS `say` to generate audio. It expects the narration text in the caller's directory and writes under `demo-videos/`; those paths span two different directories in this checkout.

[`demo-tools/combine-real-demo.sh`](combine-real-demo.sh) uses FFmpeg and ffprobe to combine a supplied `real-demo-recording.mov` with `demo-videos/ultra_comprehensive_complete_narration.mp3`, writing a narrated MP4 under `demo-videos/`. Its inputs are relative to the caller.

[`demo-tools/create-static-honest-demo.sh`](create-static-honest-demo.sh) writes `demo-videos/honest-demo-info.html` and opens it with macOS `open`. It generates a presentation page describing sample data and UI concepts; it does not record browser interactions or encode a video.

## Tooling and environment

The [root package manifest](../package.json) and [frontend manifest](../frontend/package.json) require Node.js 26 (`>=26.0.0 <27`) and use npm. Backend builds use Cargo with the [repository Rust toolchain](../rust-toolchain.toml).

The professional orchestrator requires a running Docker daemon and the `docker-compose` executable, as well as Node.js, npm, Cargo, curl, and `timeout`. The epic and professional browser recorders need Playwright, an installed Chromium browser, and a graphical session because they launch with `headless: false`. Installing these prerequisites alone does not repair the launcher problems above.

Media requirements depend on the tool: the professional narration script uses macOS `say` and ffprobe, while the video combiner uses FFmpeg and ffprobe with existing recording and audio inputs. The static presentation generator uses macOS `open`.

## Adjacent testing scripts

These development checks live under `scripts/`:

- [`scripts/test-e2e-locally.sh`](../scripts/test-e2e-locally.sh) copies the checkout to a temporary directory, starts PostgreSQL, builds the application, and attempts the frontend Playwright suite. It uses the frontend dev server's `../build` path, while the current Vite build produces `dist/`.
- [`scripts/test-e2e-simple.sh`](../scripts/test-e2e-simple.sh) checks database setup, backend compilation and brief startup, frontend build, dev-server dependency installation, and browser installation. It does not execute browser tests or create a recording.

Both assume the repository root as the caller's directory and require Docker, `docker-compose`, Cargo, and npm. Their retained setup checks are distinct from the [release suite's browser assertions against the real backend and seeded database](../frontend/tests/e2e/release/README.md#run-it); use that guide for current test execution and its environment requirements.

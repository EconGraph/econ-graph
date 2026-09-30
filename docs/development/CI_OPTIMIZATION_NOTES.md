# CI Optimization Notes

The [CI pipeline guide](CI_CD_PIPELINE.md) describes current job selection and coverage. Historical estimates of 14 parallel backend service jobs and 28–42 minutes of repeated setup no longer describe the workflow.

## Implemented optimizations

- Consolidate database-backed library tests in one job, retaining separate databases for each group.
- Build backend/test targets in Backend Build Cache and restore a shared Swatinem/rust-cache cache in test jobs. Only the build job saves the cache.
- Key Rust cache validity from dependencies/toolchain through the cache action rather than using one permanent cache key.
- Use lighter development debuginfo and disable incremental compilation in CI.
- Download the pinned Diesel CLI binary with checksum verification.
- Skip unrelated frontend/backend PR checks using path filters.
- Gate audit, license, migration and Docker jobs on their relevant PR inputs.
- Cancel superseded runs for the same PR; keep main runs independent.
- Keep legacy Core E2E opt-in and run recorded-fixture Release E2E separately.

## Measure before changing

Use actual GitHub run/job timings, separating cache hits from cold builds and compilation from execution. A quick green run can mean jobs were skipped, not that tests became faster.

Record the workflow revision, event, changed paths, required jobs that executed, cache-hit state and duration before comparing results. Account for allowed failures and scheduled suites separately.

Do not remove checks or add continue-on-error to improve timings. Verify selection on both PRs and main after changing dependency conditions. Composite actions can reduce duplication but do not inherently remove installation cost.

Custom runner images and self-hosted runners remain proposals, not infrastructure implemented by Core CI. Evaluate maintenance, cache invalidation and credential exposure before adopting them; do not use the old speculative timing targets as measured baselines.

# CI Optimization Notes

The [CI pipeline guide](CI_CD_PIPELINE.md) describes job selection and coverage. These notes explain how Core CI reduces repeated setup and how to evaluate performance changes.

## Current optimizations

Database-backed library tests run in one job, with a separate database for each test group.

The Backend Build Cache job builds the backend and test targets and saves a shared dependency cache through `Swatinem/rust-cache`. Test jobs restore that cache; only Backend Build Cache saves it. The action generates cache keys from inputs including the Rust compiler version, `Cargo.lock`, and `Cargo.toml` files. Changes to those inputs produce a different key.

Core CI also reduces work through these settings:

- Development builds use `CARGO_PROFILE_DEV_DEBUG: line-tables-only`, and incremental compilation is disabled.
- Jobs download the pinned Diesel CLI binary with checksum verification.
- Path filters skip unrelated frontend or backend PR checks.
- Audit, license, migration and Docker jobs run when their relevant PR inputs change.
- Superseded runs for the same PR are canceled; main runs remain independent.
- Legacy Core E2E is opt-in, while recorded-fixture Release E2E runs separately.

## Measure before changing

Use actual GitHub workflow and job timings, separating cache hits from cold builds and compilation from execution. A quick green run can mean jobs were skipped, not that tests became faster.

Record the workflow revision, event, changed paths, required jobs that executed, cache-hit state and duration before comparing results. Account for allowed failures and scheduled suites separately.

Do not remove checks or add `continue-on-error` to improve timings. Verify selection on both PRs and main after changing dependency conditions. Composite actions can reduce duplication but do not inherently remove installation cost.

# Train 3 compatibility baseline

Run `scripts/train3/check-compatibility.sh` from a clean candidate checkout. It builds
that checkout's backend and crawler, then separately builds the testkit fixture loader.
Docker, Cargo and Python 3 are required. It creates a uniquely named, owner-labelled
Postgres 18 container with a 512 MiB tmpfs and a random loopback port. Existing database
URLs are overridden, and cleanup removes only that owned container. Parallel invocations
use independent containers and scratch databases. No Keycloak or dev-stack port is used.

Recorded crawler fixtures pass through the existing adapters and persistence code with
external HTTP requests refused. The testkit-only `--deterministic-empty` seed mode refuses any database with public tables
before running migrations. After migration it preserves baseline discovery IDs/references,
preseeds semantic UUID5 source/dataset identities and assigns observation IDs from each full
series/date/revision/original-release key. Normal seed behavior and production persistence are
unchanged. The runner seeds two fresh databases and compares semantic identity/content
snapshots, then verifies a loaded database is refused without changes. Snapshots include exact
nullable values, revisions, units and metadata; runtime timestamps and mock HTTP origins are
excluded. Evidence retains the semantic snapshot's SHA-256 and row counts, alongside input hashes.
The version 1 SQL boundary fixtures add fixed legacy IDs,
unconstrained exact decimals, nullable/revised observations and private annotations,
comments and collaboration grants. Shared store, API, catalog authorization, research
reference and filing identity suites enforce the semantic expectations on current Postgres.
The API and fixture suites each exercise separate and combined database shapes. App and
data migration ownership is checked independently; separate databases require no cross-plane
join or foreign key. Full application database dumps are restored using the same Postgres
version, and retained content, grants and references are compared after restore and after
loss of a data target. The boundary fixture does not run a future company or workspace
migration: its baseline migration replay is intentionally a no-op. Existing release upgrade
rehearsals continue to cover upgrades from the previous product release.

The observation contract's Postgres test is intentionally ignored during ordinary developer
unit runs because it needs a migrated database. This runner explicitly includes it. The
fixture backup round trip is enabled by `TRAIN3_BACKUP_CHECK=1` here and in CI; running only
`cargo test --test train3_fixture_contract` without that variable covers the migration and
retention contracts, without invoking backup tools.

`TRAIN3_EVIDENCE_PATH=/absolute/path/evidence.json scripts/train3/check-compatibility.sh`
retains the successful run's evidence. The manifest records the candidate commit, dirty-tree
status, every recorded input's SHA-256, both migration sets and semantic expectations.
The `Train 3 Compatibility` workflow runs this same entry point for PRs, merge groups,
main and manual dispatch, and uploads that evidence. Migration checks use the event's
actual target SHA in CI and fetched `origin/main` locally. This baseline adds a Rust build
and a disposable Postgres compatibility run to PR CI; it does not publish images or deploy.
A successful feature-branch run does not authorize a milestone tag or claim acceptance
on main.

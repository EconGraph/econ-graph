# Train 3 deterministic fixture baseline

Run `scripts/train3/check-compatibility.sh` from a clean candidate checkout. It builds that
checkout's backend and crawler, then builds the testkit fixture loader separately. Docker,
Cargo and Python 3 are required. The runner creates a uniquely named, owner-labelled
Postgres 18 container limited to 2 GiB RAM and two CPUs, with a 512 MiB tmpfs and a random
loopback port. Existing database URLs
are overridden; cleanup removes only that owned container. Parallel invocations use separate
containers and scratch databases. No Keycloak, development port or provider credentials are
needed.

Recorded fixtures run through the existing crawler adapters and persistence code. The
opt-in, testkit-only `--deterministic-empty` mode refuses any database with public tables
before running migrations. It preserves baseline discovery UUIDs while assigning stable
source/dataset identities and observation IDs from each series/date/revision/original-release
key. Normal seed behavior and production persistence remain unchanged. Optional reference
refresh runs only when its whole request set is recorded, every received request must match
a declared route, and external HTTP requests are refused. Two fresh seeds must have identical
semantic identity/content snapshots; an attempted reseed must fail through the explicit
populated-database guard without changing that snapshot. Snapshots include exact nullable
values, revisions, units and metadata, excluding runtime timestamps and mock HTTP origins.

The version 1 SQL fixtures contain fixed legacy UUIDs, exact nullable/revised observations,
private annotations, comments, collaboration grants and existing financial annotation IDs.
`train3_fixture_contract` exercises separate and combined database shapes, verifies each
migration set creates only its own tables, and checks that rerunning the current baseline
and removing a data target preserve the authored rows and their existing references. This
proves retention of stored UUIDs; it does not establish a new research-reference identity
scheme. Full app and data database dumps are restored with matching PostgreSQL tools and
all public tables/rows compared; the combined shape is dumped once. The fixture's migration
replay is intentionally a no-op, while existing core database tests retain their release
upgrade checks.

The runner statically executes the fixture tests and deterministic seed unit tests introduced
here, plus the existing `catalog_authorization` and core `database::tests` suites. It has no
dependency on separately developed observation-store, API, research-reference, filing-identity
or migration-checker changes. Those packages run their own required CI on their independent
PRs. Their contracts are not silently skipped or claimed as coverage by this fixture runner.
The completed Train 3 milestone still requires every package's checks and required reviews
on the same exact main candidate.

The backup round trips are enabled by `TRAIN3_BACKUP_CHECK=1` in this runner and CI. Running
only `cargo test --test train3_fixture_contract` without that variable checks migration and
retention behavior without invoking backup tools.

`TRAIN3_EVIDENCE_PATH=/absolute/path/evidence.json scripts/train3/check-compatibility.sh`
retains the successful run's evidence: candidate commit, dirty-tree status, all recorded
input hashes, both migration sets, semantic snapshot SHA-256/row counts and the scope above.
The `Train 3 Compatibility` workflow runs this entry point for PRs, merge groups, main and
manual dispatch and uploads the evidence. It adds a Rust build and disposable Postgres run
to CI, reusing the existing backend cache. It does not publish images, deploy, tag a milestone
or claim that all Train 3 acceptance has completed.

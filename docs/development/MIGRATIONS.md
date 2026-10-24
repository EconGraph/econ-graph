# Database migrations

EconGraph has two databases, each with its own set of Diesel migrations (train 2, `v4.1.0`):

| Set | Directory | Holds | Applied by |
|-----|-----------|-------|------------|
| app | `backend/migrations/app` | Users and everything they create: sessions, annotations, comments, collaborators, preferences, audit logs. | The backend, when it starts (`run_migrations(url, MigrationSet::App)`). |
| data | `backend/migrations/data` | Sources, series, observations, crawl state, SEC data. | `crawler migrate` only, run by `deploy.sh` as the Kubernetes Job `crawler-migrate`; also `seed-fixtures` and tests. |

The exact table lists, and the names used below, are in `docs/development/TRAIN2_CONTRACTS.md`
(sections 4 and 5). This page covers how migrations are written between releases, how app
migrations are squashed when a release is tagged (Joe, 2026-10-02), and the rules for data
migrations, which are never squashed after `v4.0.0` (see "Two databases").

## Rules

1. **A migration that shipped in a release tag is never edited or removed.** Databases built
   from that release have run it, and Diesel only remembers its version. The v4.0 split is the
   one exception (see "The v4.0 split exception" below).
2. **Migrations added since the last tag can change.** During development, add incremental
   migrations on `main` as usual. Editing one that has merged is still a new migration, because
   other developers' databases have run the old text.
3. **Before a release is tagged, the app migrations added since the previous tag are squashed
   into one migration for that release** (see "Squashing at a release" below). A new database then
   runs one migration per release instead of every intermediate step, and the release's
   migration is written as the schema it builds rather than as a series of in-place fixes.
4. **Every migration version is unique.** Diesel's version is the directory name up to the first
   `_`, without dashes (`2026-10-01-000100_x` is `20261001000100`). Two directories with the
   same version are one migration to Diesel: one of them silently never runs. This holds across
   `migrations/app` and `migrations/data` as well as within each, because a database that holds
   both sets records them in one `__diesel_schema_migrations` table; the only version in both is
   `00000000000000`, whose two copies are identical. `backend/scripts/check_migration_order.py`
   (CI) rejects duplicates.

Rules 2 and 3 apply to `migrations/app`. `migrations/data` follows rules 1 and 4 and the
additional rules in "Two databases" below.

Other frameworks do the same thing: Django's `squashmigrations` (with `replaces`), Rails loading
`schema.rb` for new databases, and Flyway baselines. Diesel has no built-in squash, so the steps
are below.

## Two databases

**App migrations** (`migrations/app`) follow the rules above: incremental between releases,
squashed into one baseline when a release is tagged. A branch namespace's app database is
disposable and is recreated whenever the branch is redeployed. Production's app database
holds users' work and is upgraded like any release database, never recreated.

**Data migrations** (`migrations/data`) are never squashed after `v4.0.0`. The data database
holds the crawl, which is rebuilt only by crawling again, so it is only ever moved forward:

- Every change is a new incremental migration. A data migration that shipped is never edited,
  renumbered or folded into another (rule 1).
- Data migrations are forward-only: a mistake is fixed by a new migration, never by reverting
  one on a data database.
- A data migration preserves existing rows. A new column gets a default or a backfill in the
  same migration, a changed type or constraint converts the rows it covers, and no migration
  drops or truncates a table that holds crawled data.
- Data migrations are applied by `crawler migrate` (with `--dry-run` to list the pending
  versions), which `deploy.sh` runs as the Job `crawler-migrate` before the crawler-worker rolls
  out, and only in a namespace that owns a data database. No service applies them at startup,
  so deploying an app change never changes the data database. `seed-fixtures` (a deliberate
  tool, not a service) and tests apply the data set themselves.
- The backend never migrates the data database. If the data database is behind the backend's
  binary, `/readyz` keeps failing until `crawler migrate` runs.

### Critical changes

A data migration that cannot preserve rows is a **critical change**: one that drops or rewrites
crawled rows, changes a primary key, or otherwise needs data to be fetched again. It needs all of:

1. Joe's sign-off, written in the PR.
2. A verified `pg_dump` of the data database, taken before the migration runs and kept. Dumps
   and backups are never deleted automatically.
3. A planned recrawl, written in the PR: which sources are crawled again, and when.

Other critical changes to the data plane, such as a bulk delete or re-keying crawled rows,
need the same three things whether or not they are a migration.

### Series ids never change

A series' id is UUIDv5 of `"{SOURCE}:{external_id}"` (for example `FRED:GDP`) in a fixed
namespace (`backend/crates/econ-graph-crawler/src/series_id.rs`), so every database gives a
series the same id. App rows refer to series by that id: `chart_annotations.series_id` holds
`economic_series.id` with no foreign key, because the two tables are in different databases.
Nothing in the app database notices if a series' id changes; its annotations just stop
appearing.

So no data migration and no recrawl, critical or not, may change an existing series' id.
Changing how the id is derived (the namespace, the name format, a source's `SOURCE` string or
its `external_id`) is itself a critical change, and its PR must also ship a mapping from old ids
to new ones for the app rows that hold them.

### The v4.0 split exception

`v4.0.0` shipped one baseline, `2026-10-21-000100_v4_0_baseline`, that built both halves in one
database. Train 2 (DP-1) split it, which is the one time a shipped migration was edited:

- `migrations/data/2026-10-21-000100_v4_0_baseline` keeps the version and directory name, and
  lost the 12 app tables and 5 app enums.
- Those became the app baseline, `migrations/app/2026-10-21-000200_v4_0_app_baseline`, a new
  version. Its guard stops on an existing `users` table.
- The three cross-plane foreign keys were dropped
  (`user_data_source_preferences.data_source_id`, `financial_annotations.statement_id`,
  `financial_annotations.line_item_id`). The columns stay, as plain UUIDs with their indexes.

It is safe because applying the data baseline and then the app baseline to one database builds
the same schema as the old baseline, except for those three foreign keys (checked with
`compare_migrations.sh` in the DP-1 PR), and because every database that ran the old baseline
already has `20261021000100` recorded:

- A new data database runs the data baseline and gets no app tables. A new app database runs
  the app baseline and gets no data tables.
- A database built from the single v4.0 baseline (the live database, and any test database from
  before the split) skips the data baseline. When the app set is applied to it, the app run
  sees `users` and `20261021000100` without `20261021000200`, records `20261021000200` without
  running it, and logs that at `warn`. Its schema is unchanged, so it keeps the app tables and
  the three foreign keys until DP-5 copies the app rows into the app database and drops the app
  tables from it.

One migration was added to `release/v4.0` after the baseline, with Joe's sign-off:
`2026-10-24-000100_audit_logs_keep_on_user_delete` (#429, ECO-412), which makes
`audit_logs.user_id` nullable with `ON DELETE SET NULL`. It ships in `v4.0.0` under
`backend/migrations`, and is forward-ported to `main` into `migrations/app` with its directory
name and version unchanged, because `audit_logs` is an app table and databases built from
`v4.0.0` will have that version recorded. Its version, `20261024000100`, must sort after the
app baseline's, `20261021000200`, and does:

- A new app database then runs the app baseline and then this migration, as a `v4.0.0`
  database did.
- A database built from `v4.0.0` already has its version recorded. When the app set is applied,
  the app baseline is recorded without running (above), and this migration is skipped.
- If it sorted before `20261021000200`, a new app database would run it before `audit_logs`
  exists and fail.

`check_migration_order.py` keeps its version unique across both directories, like any other.

This query returns a row on a database built from the single v4.0 baseline that the app set has
not yet been applied to:

```sql
SELECT version
FROM __diesel_schema_migrations
WHERE version = '20261021000100'
  AND to_regclass('public.users') IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM __diesel_schema_migrations WHERE version = '20261021000200'
  );
```

Once the app set has been applied, it still holds both halves; this returns a row while it
does (that is, until DP-5 has dropped the app tables):

```sql
SELECT version
FROM __diesel_schema_migrations
WHERE version = '20261021000100'
  AND to_regclass('public.users') IS NOT NULL
  AND to_regclass('public.data_sources') IS NOT NULL;
```

No later migration may rely on this exception. From `v4.0.0` on, a shipped data migration is
never edited again.

### One database with both sets

Tests, local development, CI, and `main` until DP-4 gives k8s a second Postgres use one
database for both sets (`APP_DATABASE_URL` and `DATA_DATABASE_URL` may name the same database;
nothing may depend on them being the same or different). Then:

- The data set is applied first, then the app set.
- Both sets share one `__diesel_schema_migrations` table, so versions are unique across both
  directories (rule 4).
- A set counts as current when the newest version embedded for that set is recorded, not when
  the table's highest version is, since that mixes the two sets.
- Both sets take the same advisory lock (`MIGRATION_LOCK_KEY`), so two runs against one
  database take turns, and runs against separate databases never wait for each other.

With the diesel CLI, from `backend/`: `diesel migration run --migration-dir migrations/data`,
then `diesel migration run` (`diesel.toml` points at `migrations/app`).

### The data plane runs tagged releases only

The primary data database, and the backend and crawler in its namespace, only ever run images
built from a release tag, never an in-between `main` commit:

- Until its release is tagged, a data migration on `main` can still be renumbered (for example
  to sort after a migration forward-ported from a release branch) or removed. A data database
  that already ran it keeps the old version, so the data database takes only data migrations
  that have shipped.
- The production app database in the same namespace is squashed at each release. A database
  that ran some of `main`'s unreleased app migrations can't always upgrade through the squash
  (see "Squashing at a release" below); one that only runs release tags always can.

A branch that changes the data schema deploys with its own data database
(`deploy-branch.sh --local-data`) and never runs its data migrations against the primary.

## Baselines so far

Each release's app migrations are squashed into one baseline. Data migrations are not; the
v4.0.0 baseline is the last data squash.

| Release | Migration | Replaces |
|---------|-----------|----------|
| v4.0.0 | `2026-10-21-000100_v4_0_baseline` (split in train 2 into `data/2026-10-21-000100_v4_0_baseline` and `app/2026-10-21-000200_v4_0_app_baseline`, see "The v4.0 split exception") | Every migration before it except `00000000000000_diesel_initial_setup`. It was first squashed as `2026-10-01-000100_v4_0_baseline` while release/v4.0 was in QA, then the six migrations merged after that were folded in as `2026-10-02-000500_v4_0_baseline`, then `2026-10-21-000100_economic_series_source_title_index` (ECO-405) was folded in before tagging. A database migrated from release/v4.0 or v4.0.0-rc.1 before that last fold has `20261002000500` but not `20261021000100`, so the baseline's guard stops it; recreate it. No earlier release was deployed, and the chain had already been renumbered once after v3.7.3, so databases from earlier tags cannot upgrade and are rebuilt. |

## Squashing at a release

Only `migrations/app` is squashed. Data migrations added since the previous tag ship as they
are, one migration each.

The squashed migration takes the **version of the last migration it replaces**. Diesel treats a
migration as applied when its version is recorded in `__diesel_schema_migrations`, and ignores
recorded versions it no longer has, so:

- a new database runs the squashed migration once;
- a database that had run the whole chain it replaces already has that version, and skips it;
- a database part way through that chain does not have it, and would run it on top of a
  partial schema. The squashed migration therefore starts with a guard that stops with a clear
  error when something it creates already exists (a table, or for a squash with no new tables a
  column, index or constraint, checked by name). Such a database is recreated, or brought to
  the head of the old chain first (check out the commit before the squash and start the backend
  once).

Three limits, all only affecting databases that were never deployed from a release tag (a release candidate's tag doesn't count):

- "Has the last version" means "ran the last migration", not "ran all of them". A database that
  ran the last replaced migration before an earlier-numbered one existed skips the squash and
  silently lacks the earlier migration's changes. That happens when it ran migrations from an
  unmerged PR branch, and also when migrations merged out of version order: on release/v4.0,
  `2026-10-02-000500` merged on 2026-10-02 but `2026-10-02-000400` only on 2026-10-06, so a
  database migrated from release/v4.0 in between, or from `main` before `000400` reached it,
  has `20261002000500` without `series_fetch_validators`. Before squashing, check whether the
  last replaced version was also the last to merge (`git log --diff-filter=A` on each replaced
  directory). If not, every database built in between must first run the chain to its head
  (start the backend once on the commit before the squash), or be recreated after it.
  For the v4.0.0 fold, this query returns a row on an affected database:

  ```sql
  SELECT version
  FROM __diesel_schema_migrations
  WHERE version = '20261002000500'
    AND to_regclass('public.series_fetch_validators') IS NULL;
  ```
- Folding a change into a squash that some database has already run (it has the squash's
  version recorded) does not reach that database. On release/v4.0, `seed_reference_codes` gained
  its Last-Modified and body hash arguments in the baseline itself after the baseline had merged
  (ECO-396), so a database migrated from release/v4.0 before that keeps the six-argument
  function, and a recorded seed migration fails on it with "function seed_reference_codes(...)
  does not exist". Recreate such a database. This query returns a row on one:

  ```sql
  SELECT oid::regprocedure
  FROM pg_proc
  WHERE proname = 'seed_reference_codes' AND pronargs = 6;
  ```

  Likewise a database migrated from release/v4.0 before ECO-398 keeps the version of that
  function that skips a dimension with a shared code list, so a World Bank seed leaves out the
  aggregate names (the first crawl's refresh still fetches them). Recreate it as well.
- A database that ran the old chain keeps the replaced versions in `__diesel_schema_migrations`.
  `run_pending_migrations` ignores them, but `diesel migration revert` / `redo` stop with
  `UnknownMigrationVersion` when they reach one, or with the squash's own error when they reach
  the squash. Migrations after the squash can be reverted and redone as usual.

Steps:

1. List the app migrations added since the previous tag, where the previous tag is the latest
   tag of the previous release line, patch releases included
   (`git diff --name-only <prev-tag> -- backend/migrations/app`). Migrations that stay incremental
   may sort between them only if they have shipped already (a hotfix migration released in a
   patch and forward-ported to `main` under the same directory name): a new database then runs
   the previous squash, the hotfix, then the new squash, and an upgraded database has the first
   two recorded and runs only the new squash. An unshipped migration that is not being squashed
   and sorts before the last squashed version would run before the squash on a new database;
   renumber it after the squashed version in the same PR (rule 2 allows it). After a release
   branch is cut, check `main` as well as the release branch, and forward-port any release
   migration that `main` still lacks before forward-porting the squash.
2. Write the squashed `up.sql` as the schema those migrations build: final column lists, final
   constraints and indexes, final seed rows. Keep column order the same as the chain produced.
   Name the directory with the last replaced version, e.g. `2026-10-21-000100_v4_0_baseline`.
   Folding more migrations into an unreleased squash works the same way: the squash is renamed
   to the version of the last migration folded in.
   Its `down.sql` only raises an error: on an upgraded database that version belongs to the last
   replaced migration, so `diesel migration revert` there would otherwise drop the whole schema
   while looking like a revert of one small change. Recreate the database to start over.
3. Delete the replaced directories and anything that only existed to upgrade data the release
   never had (backfills, dedupes, compatibility shims). Tests that loaded a replaced migration
   file with `include_str!` move to the squashed one or to the statement they need.
4. Check it against the chain it replaces, on PostgreSQL 18:

   ```sh
   cd backend
   ADMIN_URL=postgres://postgres@localhost:5432/postgres scripts/compare_migrations.sh <commit-before-the-squash>
   ```

   It builds one database from the migrations at that commit and one from the working tree,
   applying `migrations/data` and then `migrations/app` to each. Data migrations are never
   squashed, so any difference comes from the app squash. It diffs the full catalog (tables,
   columns and their order, constraints, indexes, triggers, views, functions, enums, comments,
   grants) and every seed row. The squash is right when it prints no diff.
5. Regenerate nothing else: `schema.rs` and the GraphQL schema do not change when the schema is
   identical, and `test_schema_compatibility` confirms `schema.rs` in CI.

A squash is a normal PR (draft, review, CodeRabbit, CI). On a release branch it is forward-ported
to `main` like any other fix.

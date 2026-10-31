# Database migrations

The app plane handles users and application state; the data plane handles crawled and reference
data. Their Diesel migrations live in `backend/migrations/app` and `backend/migrations/data`,
respectively. App plane migrations run against `APP_DATABASE_URL`; data plane migrations run
against `DATA_DATABASE_URL`. Both URLs may point to the same physical database, which then holds
tables for both planes.

The backend applies app plane migrations when it starts. Service startup does not apply data
plane migrations; use [`crawler migrate`](#applying-the-data-set-crawler-migrate) to apply them.
This page explains which plane a migration belongs to, how to write migrations between releases,
and how to squash app plane migrations before tagging a release.

## Migration sets

| Plane | Migration directory | Tables |
|-----|-----------|--------|
| data | `backend/migrations/data` | Crawled and reference data: `data_sources`, `datasets`, `economic_series`, `data_points`, `series_metadata`, `crawl_queue`, `crawl_attempts`, `reference_file_cache`, `series_fetch_validators`, publication lifecycle state (`series_lifecycle`, `catalog_lifecycle_scopes`, `catalog_reference_snapshots`), the global indicator tables (`countries`, `global_economic_indicators`, `global_indicator_data`, `country_correlations`, `trade_relationships`, `global_economic_events`, `event_country_impacts`, `leading_indicators`), and SEC/XBRL data (`companies`, `financial_statements`, `financial_line_items`, `financial_ratios`, `xbrl_taxonomy_schemas`, `xbrl_taxonomy_linkbases`, `xbrl_dts_dependencies`, `xbrl_instance_dts_references`, `xbrl_taxonomy_concepts`). |
| app | `backend/migrations/app` | Users and everything they create: `users`, `user_sessions`, `chart_annotations`, `annotation_comments`, `chart_collaborators`, `user_data_source_preferences`, `audit_logs`, `security_events`, `financial_annotations`, `annotation_assignments`, `annotation_replies`, `annotation_templates`, and the annotation enums. |

On a database built from the two baselines, no app table has a foreign key to a data table.
App rows that refer to data rows (`user_data_source_preferences.data_source_id`,
`financial_annotations.statement_id`, `financial_annotations.line_item_id`,
`chart_annotations.series_id`) hold the id as a plain column with an index. A database built
from the single v4.0.0 baseline still has three of those as foreign keys (see
[The v4.0.0 baseline split](#the-v400-baseline-split)).

`backend/diesel.toml` points the diesel CLI at `migrations/app`; pass
`--migration-dir migrations/data` for data plane migrations.

In the kind deployment, both databases are in the namespace `econ-graph`. Data plane migrations
run against `econ_graph` on `data-postgres-service` (Secret `econ-graph-secrets`, key
`data-database-url`); app plane migrations run against `econ_graph_app` on `app-postgres-service`
(key `app-database-url`). A cluster whose `app-database-url` still names the data database keeps
both planes' tables in `econ_graph`; see [database settings](../../k8s/README.md#databases).

## User deletion policy

Deleting a user removes their sessions, chart annotations, annotation comments, chart access
grants (`chart_collaborators.user_id`), and source preferences. Comments on an annotation are
removed with that annotation. Other users' chart grants survive the inviter's deletion with
`invited_by = NULL`; their annotations and comments on surviving annotations remain. Audit
history and security events retain their existing nullable user references.

## Rules

Here, a **release tag** means a final version tag such as `v4.0.0` or `v4.0.1`. Release candidate
tags such as `v4.0.0-rc.2` do not count as release tags for these rules.

1. **A migration that shipped in a release tag is never edited, renumbered or removed.**
   Databases built from that release have run it, and Diesel only remembers its version. The
   one exception is a reference-code seed migration, which `crawler record-reference-seeds`
   replaces (see [Reference-code seed migrations](#reference-code-seed-migrations)).
2. **An unmerged migration can be edited in place; after merge, changes require a new migration.**
   Add incremental migrations on `main` as usual. Other developers' databases may have run the
   merged SQL, and Diesel does not replay changed SQL under a recorded version. Release
   squashing and renumbering follow the separate [squash procedure](#squashing-at-a-release);
   reference-code seeds follow their replacement rule above.
3. **Before tagging a release, squash eligible app plane migrations into one migration.**
   A migration is eligible if it has not shipped in a release tag and sorts after the newest
   app migration that has shipped. The two baselines are excluded. No squash is needed when
   the release added zero or one app migration. A new database then runs one app migration per
   release instead of every intermediate step; see [Squashing at a release](#squashing-at-a-release).
   [Data plane migrations](#data-migrations) are never squashed.
4. **Every migration version is unique, across both sets.** Diesel's version is the directory
   name up to the first `_`, without dashes (`2026-10-01-000100_x` is `20261001000100`). Both
   sets record their versions in the same `__diesel_schema_migrations` table, so two directories
   with the same version, in one set or in different sets, are one migration to Diesel: one of
   them silently never runs. The only version in both sets is `00000000000000`
   (`diesel_initial_setup`), whose two copies are identical.
5. **The two baselines are permanent.** `20261021000100` (the data baseline) and
   `20261021000200` (the app baseline) are never squashed, renumbered or removed, and neither is
   any other migration that has shipped in a release tag.

`backend/scripts/check_migration_order.py` (run in CI) checks the layout and the versions. It
rejects a duplicate version within or across the sets, two copies of `00000000000000` that
differ, an entry in `backend/migrations` other than the two set directories (dotfiles such as
`.diesel_lock` are ignored), and a directory inside a set that has no `up.sql` or whose name
has no readable version. Inside a set it skips dotfiles and plain files, such as
`data/economic_synonyms.syn`.

Other frameworks do the same thing: Django's `squashmigrations` (with `replaces`), Rails loading
`schema.rb` for new databases, and Flyway baselines. Diesel has no built-in squash, so the steps
are below.

## Adding a migration

Put it in the directory for the plane that owns the tables it changes. A migration never touches
tables of both planes, and an app plane migration never adds a foreign key to a data plane table.
From `backend/`:

```sh
# app set (the diesel.toml default)
diesel migration generate add_something
# data set
diesel migration generate --migration-dir migrations/data add_something
```

Give it a version later than every existing version in both sets. The version must be unique
(rule 4, which CI checks); sorting after the rest is a separate rule that CI does not check. It
matters because a new database runs each set's migrations in version order, and because the
listing in [Squashing at a release](#squashing-at-a-release) assumes unshipped app migrations
sort after shipped ones. Run `python3 scripts/check_migration_order.py` before pushing.

Then regenerate `backend/crates/econ-graph-core/src/schema.rs`, which covers both sets, from a
database with both sets applied. `test_schema_compatibility` (in
`crates/econ-graph-core/tests/schema_validation_test.rs`) fails when it differs from what
`diesel print-schema` prints. From `backend/`:

```sh
DATABASE_URL=postgres://... diesel print-schema > crates/econ-graph-core/src/schema.rs
```

`diesel.toml` deliberately omits `[print_schema] file`, so `diesel migration run` does not
write a schema file automatically. Regenerate the shared schema explicitly after applying both
app plane and data plane migrations. Applying migrations for only one plane must not replace
the shared schema with just that plane's tables.

### Reference-code seed migrations

`crawler record-reference-seeds --source <SOURCE>` writes a data migration,
`migrations/data/<timestamp>_seed_<source>_reference_codes/`, versioned by when it was recorded
(see the [crawler deployment guide](../technical/CRAWLER_DEPLOYMENT_GUIDE.md)). It also deletes
that source's previous `*_seed_<source>_reference_codes` directory, whether or not it has
shipped, which is the one exception to rule 1. A new database runs only the new one. A database
that ran the old one keeps its version recorded, and runs the new one, which loads only the
reference files that database does not have yet: `seed_reference_codes` does nothing for a file
that already has a row in `reference_file_cache`.

## Data migrations

Data plane migrations are incremental and are never squashed: the crawled data they manage can
be rebuilt only by crawling again.

- After a migration merges, every change requires a new versioned migration (rule 2). A
  migration that shipped in a release tag is never edited (rule 1). Reference-code seed
  migrations are the exception: recording a source again replaces its previous one.
- They are forward-only: a mistake is fixed by a new migration, not by reverting one.
- Normally, they preserve existing rows. A new column gets a default or a backfill in the same
  migration, and a changed type or constraint converts the rows it covers. Dropping or
  truncating a table that holds crawled data requires the critical-change exception below.

The exception is a **critical change**: a data migration that drops or rewrites crawled rows,
changes a primary key, or otherwise needs data to be fetched again. Its PR needs the maintainer's
sign-off, a verified `pg_dump` of the database taken before it runs and kept, and a written
recrawl plan: which sources are crawled again, and when.

### Series ids

The crawler creates each series with the id UUIDv5 of `"{SOURCE}:{external_id}"` (for example
`FRED:GDP`) in a fixed namespace (`backend/crates/econ-graph-crawler/src/series_id.rs`), so every
database's crawl gives a series the same id. Rows created before stable ids keep their generated
ids (see that file's module doc).

`chart_annotations.series_id` holds a series id as text (`VARCHAR(255)`), with no foreign key.
Nothing notices if a series' id changes; its annotations just stop appearing. So a data
migration or recrawl never changes an existing series' id, and a recrawl updates series rows in
place rather than deleting and re-inserting them. Regenerating existing series ids (moving a row
from before stable ids onto its stable id, or changing the namespace, the name format, a
source's `SOURCE` string or a series' `external_id`) is a critical change, and its PR also ships
a migration that maps the old ids to the new ones in `chart_annotations.series_id`.

## Applying the data set: `crawler migrate`

```sh
crawler migrate [--dry-run]
```

`crawler migrate` applies the pending data migrations to `DATA_DATABASE_URL` (or
`--database-url`) under the migration advisory lock, prints each version it applied, one per
line, and exits 0 once the data set is current. A second run applies nothing and prints nothing.
`--dry-run` prints the pending versions and changes nothing; on a new database it does not even
create `__diesel_schema_migrations`. It never applies or records an app migration, even when the
database holds both sets. From `backend/`: `cargo run --bin crawler -- migrate`. In the
crawler-worker image it is `/app/crawler migrate`.

In Kubernetes, `scripts/deploy/deploy.sh` runs it as the Job `crawler-migrate`
(`k8s/manifests/crawler-migrate-job.yaml`, from the crawler-worker image) before it applies the
backend and the crawler-worker, and stops the deploy if the Job fails. It needs a namespace
that owns a data database: one whose Secret `econ-graph-secrets` has the key
`data-database-url`. Without that key the deploy stops, since the backend would refuse to start.

Before that Job, `deploy.sh` runs `crawler migrate --dry-run` in the Job `crawler-migrate-check`
(the same manifest, changing nothing; `scripts/deploy/lib/data-migrations.sh`):

- **Some pending:** it stops the old crawler-worker with
  `scripts/deploy/precheck-db-migration.sh --stop-crawler`, so the worker doesn't write while
  the migrations run. The worker comes back on the new image when `deploy.sh` applies
  `crawler-worker.yaml` after `crawler-migrate` completes, and stays stopped if the Job fails.
  The precheck refuses while a `postgres-backup` or `crawler-migrate` Job is running or a
  long transaction holds the data database, and then the deploy stops before the migrations.
- **None pending:** the worker keeps running.

`deploy.sh --dry-run` lists both Jobs with the rest of the deploy.

A data migration that bounds its table locks (`lock_timeout`, `statement_timeout`) fails its
run, changing nothing, when other transactions hold its tables. The Job gives it one more pod
(`backoffLimit: 1`) and then fails, and `deploy.sh` stops before the backend and the
crawler-worker are applied. `scripts/deploy/precheck-db-migration.sh` checks the data database
for such transactions and stops the crawler-worker. `deploy.sh` runs it when migrations are
pending, and running it by hand before `deploy.sh` finds the same problems before anything is
applied; see
[Deploying the migration](../technical/OBSERVATION_PRECISION_REFETCH.md#deploying-the-migration)
for the observation-values migration, which does this.

Besides `crawler migrate`, only `seed-fixtures` (which loads recorded fixtures into a test
database) and tests apply the data set.

## Applying both sets to one database

The Rust API represents the plane migrations as `MigrationSet::App` and `MigrationSet::Data`.
`run_migration_set(url, MigrationSet::App)` applies app plane migrations; passing
`MigrationSet::Data` applies data plane migrations. Each call holds a Postgres advisory lock
(`MIGRATION_LOCK_KEY`) for the whole run, so two backends starting together take turns. Advisory
locks are scoped to a database, so runs against separate app and data databases never contend,
and two sets in one database run one after the other. Applying
the app set alone leaves a pending data set untouched. Tests use `run_migrations`, which applies
the data set and then the app set to one database under the same lock.

`readiness_check_set(pool, set)` requires the newest migration the binary embeds for that set to
be recorded. It checks one set at a time, ignoring the other set's versions in a database that
holds both; the table's highest version mixes the two sets, so it is not used.

The backend's `/readyz` fails only when the app database is unreachable or its app set is not
current. It also checks the data set on `DATA_DATABASE_URL` and reports the result in its response
body (`ok`, `unreachable`, `pending_migrations` or `not_configured`) without failing, so an
unreachable or unmigrated data database never takes sign-in and the other app operations out of
rotation; data fields fail on their own until it is back.

With the diesel CLI, from `backend/`, on a new database:

```sh
diesel migration run --migration-dir migrations/data
diesel migration run --migration-dir migrations/app
```

Neither command regenerates the shared schema; use the explicit command in
[Adding a migration](#adding-a-migration) after both sets are applied. On a database built from
the single v4.0.0 baseline, start the backend once instead (see
[The v4.0.0 baseline split](#the-v400-baseline-split)); the diesel CLI stops there at the app
baseline's guard.

## The v4.0.0 baseline split

Before the split, `backend/migrations` held one baseline, `2026-10-21-000100_v4_0_baseline`,
that builds both sets' tables. That is the baseline on `release/v4.0` and in `v4.0.0-rc.2`, and
every database built from `v4.0.0-rc.2`, or from `release/v4.0` after its last fold, has run it.
It is split into two:

- `migrations/data/2026-10-21-000100_v4_0_baseline` keeps the version and directory name, and
  builds only the data tables.
- `migrations/app/2026-10-21-000200_v4_0_app_baseline`, a new version, builds the app tables and
  enums. Its guard stops on an existing `users` table.
- The three foreign keys from app tables to data tables
  (`user_data_source_preferences.data_source_id`, `financial_annotations.statement_id`,
  `financial_annotations.line_item_id`) are dropped. The columns stay, with their indexes.

The data baseline followed by the app baseline builds the same schema and seed rows as the single
baseline, except for those three foreign keys. Run against a commit from before the split,
`scripts/compare_migrations.sh` prints exactly those three foreign keys as its diff, for a person
to read.

A database built from the single baseline already has `20261021000100` recorded, so it skips the
data baseline. A run that applies the app set (the backend's at startup, or `run_migrations`)
first checks whether the database is such a database: `20261021000100` is recorded,
`20261021000200` is not, and all 12 app baseline tables exist (`APP_BASELINE_TABLES` in
`econ_graph_core::database`). If so, it records
`20261021000200` without running it and logs that at `warn`. A database that has only some of
the app tables is not recognised, and the app baseline's guard stops on it. This query returns
a row on a database the check recognises:

```sql
SELECT version
FROM __diesel_schema_migrations
WHERE version = '20261021000100'
  AND NOT EXISTS (
    SELECT 1 FROM __diesel_schema_migrations WHERE version = '20261021000200'
  )
  AND (
    SELECT bool_and(to_regclass(format('public.%I', t)) IS NOT NULL)
    FROM unnest(ARRAY[
      'users', 'user_sessions', 'chart_annotations', 'annotation_comments',
      'chart_collaborators', 'user_data_source_preferences', 'audit_logs', 'security_events',
      'financial_annotations', 'annotation_assignments', 'annotation_replies',
      'annotation_templates'
    ]) AS t
  );
```

Its schema is unchanged, so it keeps the three foreign keys, all `ON DELETE CASCADE`. On such a
database, deleting a `data_sources`, `financial_statements` or `financial_line_items` row also
deletes the `user_data_source_preferences` or `financial_annotations` rows that refer to it.
Every later migration must work both with and without those three foreign keys.

## Baselines

| Release | Migration | Replaces |
|---------|-----------|----------|
| v4.0.0 | `data/2026-10-21-000100_v4_0_baseline` and `app/2026-10-21-000200_v4_0_app_baseline` | Every migration before the v4.0.0 baseline except `00000000000000_diesel_initial_setup`. |

Before [the split](#the-v400-baseline-split), the single baseline
`2026-10-21-000100_v4_0_baseline` built both planes' tables. For an existing database:

- If `20261021000100` is recorded, `20261021000200` is not, and all 12 app baseline tables
  exist, start the backend once to record the app baseline without running it. The split
  section above gives the detection query and explains the three foreign keys this path retains.
- If the database was migrated from `release/v4.0` or `v4.0.0-rc.1` and has `20261002000500`
  recorded but not `20261021000100`, it ran an earlier version of the baseline. The data
  baseline's guard stops it; recreate the database.
- If the database was built from a tag before v4.0, it cannot upgrade; rebuild it.

## Squashing at a release

Only app plane migrations in `migrations/app` are squashed. Eligible migrations have not shipped
in a release tag and sort after the newest app migration that has shipped; the two baselines
are excluded. No squash is needed when the release added zero or one app migration. Data plane
migrations ship as they are, one migration each.

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

These three limits affect databases that have applied unreleased migrations, including release
candidate migrations. They also apply if the database previously ran a final release:

- "Has the last version" means "ran the last migration", not "ran all of them". A database that
  ran the last replaced migration before an earlier-numbered one existed skips the squash and
  silently lacks the earlier migration's changes. That happens when it ran migrations from an
  unmerged PR branch, or when migrations merged out of version order. Before squashing, check
  whether the last replaced version was also the last to merge (`git log --diff-filter=A` on
  each replaced directory). If not, every database built in between must first run the chain to
  its head (start the backend once on the commit before the squash), or be recreated after it.
  For the v4.0.0 baseline, an affected database (migrated from release/v4.0 or `main`) has
  `20261002000500` without `series_fetch_validators`; this query returns a row on one:

  ```sql
  SELECT version
  FROM __diesel_schema_migrations
  WHERE version = '20261002000500'
    AND to_regclass('public.series_fetch_validators') IS NULL;
  ```
- Folding a change into a squash that some database has already run (it has the squash's
  version recorded) does not reach that database. For the v4.0.0 baseline, a database migrated
  from release/v4.0 can have an older `seed_reference_codes`. With the six-argument version, a
  seed migration fails with "function seed_reference_codes(...) does not exist"; recreate the
  database. This query returns a row on one:

  ```sql
  SELECT oid::regprocedure
  FROM pg_proc
  WHERE proname = 'seed_reference_codes' AND pronargs = 6;
  ```

  With a version that skips a dimension with a shared code list, a World Bank seed leaves out
  the aggregate names (the first crawl's refresh still fetches them); recreate it as well.
- A database that ran the old chain keeps the replaced versions in `__diesel_schema_migrations`.
  `run_pending_migrations` ignores them, but `diesel migration revert` / `redo` stop with
  `UnknownMigrationVersion` when they reach one, or with the squash's own error when they reach
  the squash. Migrations after the squash can be reverted and redone as usual.

Steps:

1. List the app migrations to squash: those that have not shipped in a release tag and sort
   after the newest app migration that has. The two baselines are excluded by name (rule 5). Release tags from before the split have only
   `backend/migrations`, so a `git diff` against the previous tag can't find them. From
   `backend/`:

   ```sh
   shipped=$(for t in $(git tag -l 'v*' | grep -v -- -); do
     git ls-tree --name-only "$t" migrations/ migrations/app/
   done | sed 's|.*/||' | grep '^2' | sort -u)
   app=$(ls migrations/app | grep '^2' | sort |
     grep -v -e '^2026-10-21-000100_' -e '^2026-10-21-000200_')
   newest=$(comm -12 <(echo "$shipped") <(echo "$app") | tail -n 1)
   comm -23 <(echo "$app") <(echo "$shipped") | awk -v n="$newest" '$0 > n'
   ```

   It prints the directories to squash. The squash takes the version of the last one, so it
   sorts after every permanent version. An unshipped app migration that sorts before the newest
   shipped one (for example one that merged before a hotfix migration was released in a patch and
   forward-ported to `main` under the same directory name) is left out of the list; renumber it
   after the newest shipped version first, so that it is squashed with the rest. A database
   that already ran it under the old version runs the renumbered copy again, so the renumbered
   copy must be safe to re-run, or such databases are recreated. After a release branch is cut,
   check `main` as well as the release branch, and
   forward-port any release migration that `main` still lacks before forward-porting the squash.
2. Write the squashed `up.sql` as the schema those migrations build: final column lists, final
   constraints and indexes, final seed rows. Keep column order the same as the chain produced.
   Name the directory with the last replaced version. Folding more migrations into an unreleased
   squash works the same way: the squash is renamed to the version of the last migration folded
   in. Its `down.sql` only raises an error: on an upgraded database that version belongs to the
   last replaced migration, so `diesel migration revert` there would otherwise drop the whole
   schema while looking like a revert of one small change. Recreate the database to start over.
3. Delete the replaced directories and anything that only existed to upgrade data the release
   never had (backfills, dedupes, compatibility shims). Tests that loaded a replaced migration
   file with `include_str!` move to the squashed one or to the statement they need.
4. Check it against the chain it replaces, on PostgreSQL 18:

   ```sh
   cd backend
   ADMIN_URL=postgres://postgres@localhost:5432/postgres scripts/compare_migrations.sh <commit-before-the-squash>
   ```

   It builds one database from the migrations at that commit and one from the working tree,
   applying `migrations/data` and then `migrations/app` to each, and diffs the full catalog
   (tables, columns and their order, constraints, indexes, triggers, views, functions, enums,
   comments, grants) and every seed row. The squash is right when it prints no diff.
5. Regenerate nothing else: `schema.rs` and the GraphQL schema do not change when the schema is
   identical, and `test_schema_compatibility` confirms `schema.rs` in CI.

A squash is a normal PR (draft, review, CodeRabbit, CI). On a release branch it is forward-ported
to `main` like any other fix.

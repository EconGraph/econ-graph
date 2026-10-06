# Database migrations

The backend applies Diesel migrations from `backend/migrations` when it starts
(`econ_graph_core::database::run_migrations`). This page covers how migrations are written
between releases and how they are squashed when a release is tagged (Joe, 2026-10-02).

## Rules

1. **A migration that shipped in a release tag is never edited or removed.** Databases built
   from that release have run it, and Diesel only remembers its version.
2. **Migrations added since the last tag can change.** During development, add incremental
   migrations on `main` as usual. Editing one that has merged is still a new migration, because
   other developers' databases have run the old text.
3. **Before a release is tagged, the migrations added since the previous tag are squashed into
   one migration for that release** (see "Squashing at a release" below). A new database then
   runs one migration per release instead of every intermediate step, and the release's
   migration is written as the schema it builds rather than as a series of in-place fixes.
4. **Every migration version is unique.** Diesel's version is the directory name up to the first
   `_`, without dashes (`2026-10-01-000100_x` is `20261001000100`). Two directories with the
   same version are one migration to Diesel: one of them silently never runs.
   `backend/scripts/check_migration_order.py` (CI) rejects duplicates.

Other frameworks do the same thing: Django's `squashmigrations` (with `replaces`), Rails loading
`schema.rb` for new databases, and Flyway baselines. Diesel has no built-in squash, so the steps
are below.

## Baselines so far

| Release | Migration | Replaces |
|---------|-----------|----------|
| v4.0.0 | `2026-10-02-000500_v4_0_baseline` | Every migration before it except `00000000000000_diesel_initial_setup`. It was first squashed as `2026-10-01-000100_v4_0_baseline` while release/v4.0 was in QA, then the six migrations merged after that were folded in before tagging. No earlier release was deployed, and the chain had already been renumbered once after v3.7.3, so databases from earlier tags cannot upgrade and are rebuilt. |

## Squashing at a release

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

Three limits, all only affecting databases that were never deployed from a tag:

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

1. List the migrations added since the previous tag, where the previous tag is the latest
   tag of the previous release line, patch releases included
   (`git diff --name-only <prev-tag> -- backend/migrations`). Migrations that stay incremental
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
   Name the directory with the last replaced version, e.g. `2026-10-02-000500_v4_0_baseline`.
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

   It builds one database from each set of migrations and diffs the full catalog (tables,
   columns and their order, constraints, indexes, triggers, views, functions, enums, comments,
   grants) and every seed row. The squash is right when it prints no diff.
5. Regenerate nothing else: `schema.rs` and the GraphQL schema do not change when the schema is
   identical, and `test_schema_compatibility` confirms `schema.rs` in CI.

A squash is a normal PR (draft, review, CodeRabbit, CI). On a release branch it is forward-ported
to `main` like any other fix.

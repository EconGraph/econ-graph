# /db-migrate

Apply pending migrations for one explicitly selected plane from `backend/`.

- `plane` (required): `app` for users and application state, or `data` for crawled and reference data.
- `database_url` (optional): the name of a preconfigured environment variable containing the URL override for the selected plane only. Omit it to use `APP_DATABASE_URL` for `app` or `DATA_DATABASE_URL` for `data`.

```text
/db-migrate app
/db-migrate data
/db-migrate data database_url=MIGRATION_DATABASE_URL
```

Ask for the plane if it is missing. Use existing connection configuration; never request, inspect, or display credential values. The override example names an environment variable already configured by the operator, not a literal URL.

The planes have separate migrations even when their URLs point to the same database. Before running, follow [Database migrations](../../docs/development/MIGRATIONS.md) to establish the target's baseline history. For a [shared database](../../docs/development/MIGRATIONS.md#applying-both-sets-to-one-database), apply data plane migrations before app plane migrations. A database built from the [legacy single v4.0.0 baseline](../../docs/development/MIGRATIONS.md#the-v400-baseline-split) needs the guide's backend-startup transition; do not use the app Diesel command below for that transition.

For a new database or one already using the split baselines, run the command for the selected plane from `backend/`:

- **App:** `DATABASE_URL="$APP_DATABASE_URL" diesel migration run --migration-dir migrations/app`. Diesel reads `DATABASE_URL`, so explicitly supply the app plane URL rather than relying on an existing generic URL. For an override, use the named variable in place of `$APP_DATABASE_URL`.
- **Data:** `cargo run --bin crawler -- migrate`. This applies only `migrations/data` using `DATA_DATABASE_URL`; the crawler does not read `DATABASE_URL`. For an override, pass `--database-url "$MIGRATION_DATABASE_URL"` after `--`, substituting the requested variable name.

Applying one plane does not apply the other. These commands do not regenerate the shared schema; use the guide's [schema regeneration instructions](../../docs/development/MIGRATIONS.md#adding-a-migration) when needed.

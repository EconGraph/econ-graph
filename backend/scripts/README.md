# Backend Scripts

- `check_migration_order.py` - checks that migration directories are in chronological order (used by CI).
- `compare_migrations.sh` - builds a database from the migrations at a git ref and one from the working tree and diffs their schemas (`schema_snapshot.sql`) and seed rows. Used when squashing migrations at a release; see `docs/development/MIGRATIONS.md`.
- `run-tests-optimized.sh` - local test runner with several execution strategies.

## Populating series catalogs

The old `populate_catalogs.sh` / `catalog_crawler` flow was removed. Catalogs are now
discovered through the crawl queue:

```bash
# enqueue a discover_catalog job per source (writes series metadata when the worker runs it)
cargo run -p econ-graph-crawler --bin crawler -- discover --source FRED
# enqueue data fetches for specific series
cargo run -p econ-graph-crawler --bin crawler -- enqueue --source FRED --series GDP,UNRATE
# drain the queue
cargo run -p econ-graph-crawler-worker --bin crawler-worker
```

See `backend/crates/econ-graph-crawler/README.md`.

# Backend Scripts

- `check_migration_order.py` - checks that migration directories are in chronological order (used by CI).
- `create_consolidated_migration.py` - builds a consolidated migration from a schema dump.
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

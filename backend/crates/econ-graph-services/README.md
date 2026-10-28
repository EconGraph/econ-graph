# EconGraph Services

The business logic and service layer for the EconGraph system: search, series queries, global analysis, crawl-queue statistics and collaboration. Data collection (source adapters, the queue worker and the `crawler` CLI) lives in [`econ-graph-crawler`](../econ-graph-crawler/README.md).

## Features

- **Search & Analysis**: Global economic analysis and intelligent search functionality
- **Queue Management**: `crawl_queue` statistics and admin helpers
- **Collaboration**: User collaboration and data sharing features for team workflows

## Testing

Database-free tests cover search-query normalization and collaboration validation.
PostgreSQL-backed tests exercise search results, global analysis and collaboration,
as well as queue claims, lease-guarded transitions, retries, statistics and error
handling. The queue tests cover service operations; the crawler crate owns the worker.

### Running Tests

Run from `backend/` (use `cd backend` from the repository root); `-p` selects this
crate rather than every default workspace member. First create the disposable
`econ_graph_test` database using the [backend test setup](../../README.md#build-and-test).
Database-backed tests drop and recreate its public schema. Keep Docker available;
the subshell below leaves the application's exported `DATABASE_URL` unchanged.

Set `DATABASE_URL` explicitly for these runs: database-backed search tests skip
when it is absent, while queue tests use the shared helper's fallback of
`postgres://localhost/econ_graph_test`.

```bash
(
  export DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph_test
  # Run this crate's tests
  cargo test -p econ-graph-services

  # Filter by existing service module names
  cargo test -p econ-graph-services services::search_service
  cargo test -p econ-graph-services services::queue_service
)
```

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the [LICENSE file](../../../LICENSE) for complete terms and conditions.

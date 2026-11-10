# /e2e

Run end-to-end integration tests via `scripts/test-e2e.sh`.

- suite (string, optional): one of `all|backend|frontend|combined` (default: `all`)

The script starts its own `econ-graph-test-postgres` container, and first stops and removes any container with that name. Check that no other task is using one before you run it. Never touch containers, volumes or clusters you didn't create; see [AGENTS.md](../../AGENTS.md).

```bash
/e2e combined
```
# /docker-logs

Tail logs for a local Docker container (last 200 lines, follow).

- name (string, required): container name or ID

Only for containers you created for this task. Never touch other containers; see [AGENTS.md](../../AGENTS.md).

```bash
/docker-logs econ-graph-test-postgres
```
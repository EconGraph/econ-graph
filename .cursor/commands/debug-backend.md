# /debug-backend

Ask the agent to reproduce a backend failure, test one change at a time, and verify the result.

Include:

- Links to failing tests, error logs, or relevant files
- Expected and actual behavior

Agent loop:

1. Identify a single failing scenario and capture the exact error message.
2. Reproduce locally with the exact failing test or command.
3. Form a hypothesis and make one targeted change.
4. Rerun the same scenario and collect evidence (logs, diffs).
5. If fixed, write or strengthen regression tests; otherwise, iterate with a new hypothesis.

Never bypass pre-commit; never claim success without proof.

Example request for a hypothetical timeout:

```text
/debug-backend @backend/crates/econ-graph-graphql/src/graphql/query.rs "Timeout on /graphql: health OK, queries hang. Reproduce and iterate with single-change loops."
```

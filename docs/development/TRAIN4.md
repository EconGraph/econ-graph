# Train 4 development and review

Train 4 supplies the self-hosted platform, independently authorized data access,
and Iceberg observation storage tracked by
[ECO-774](https://linear.app/econgraph/issue/ECO-774). The original candidate is
preserved on `feat/train4` at `e4be6296af47403ba2fd1e49fa6e2a09acb0c1d5`.
[The omnibus PR](https://github.com/EconGraph/econ-graph/pull/759) is superseded
by focused review packages; its former `feat/train3` parent is retained
for historical and downstream references.

## Review packages

| Package | Pull request | Dependency |
| --- | --- | --- |
| Persistent crawler administration | [#809](https://github.com/EconGraph/econ-graph/pull/809) | Main |
| Independent read-only data authorization | [#810](https://github.com/EconGraph/econ-graph/pull/810) | Main |
| Object-store rehearsal and guarded host setup | [#811](https://github.com/EconGraph/econ-graph/pull/811) | Main |
| Talos infrastructure and fit tooling | [#812](https://github.com/EconGraph/econ-graph/pull/812) | Main |
| GitOps platform and CNPG tooling | [#813](https://github.com/EconGraph/econ-graph/pull/813) | Main |
| JavaScript dependency compatibility | [#814](https://github.com/EconGraph/econ-graph/pull/814) | Main |
| Lossless observation attributes and core contract | [#815](https://github.com/EconGraph/econ-graph/pull/815) | Main |
| Iceberg library, catalog and write fence | [#816](https://github.com/EconGraph/econ-graph/pull/816) | #815 |
| Selected readers and atomic crawler mirroring | [#817](https://github.com/EconGraph/econ-graph/pull/817) | #815, #816 |
| Backfill, cutover and recovery | [#818](https://github.com/EconGraph/econ-graph/pull/818) | #815, #816, #817 |

Each package has an isolated worktree and feature branch. Observation attributes
are accepted on main. The remaining storage packages form a review stack:
selected reads/writes build on the Iceberg library, and recovery builds on selected
reads/writes. Each child contains only its own changes against its immediate
parent. After a parent changes or merges, rebase the child and validate its exact
new head. A successful local dependency-stack run does not replace hosted
validation or collaborator approval. Review results and changing CI status belong
in the PR discussion, rather than this document.

## Integration contracts

Storage packages must retain exact nullable values, captured attributes, all
promised revisions, stable identities, restricted-series visibility, synthetic
vintage rules and same-snapshot revised-value flags. PostgreSQL remains the
rollback shadow. SQL catalog publication and queue completion share the caller's
lease-fenced transaction. Maintenance and recovery must not silently discard
history or user-authored data.

Scoped workflows cover independent tooling; storage, application and recovery
contracts run through the required Core CI jobs. The Rust storage dependency
graph, real object/history fixtures and pinned provider/chart downloads add CI
cost. Shared checksum helpers and the read-only realm overlay are identical
small runtime dependencies in the packages that consume them.

## Operational acceptance

Merging implementation changes no product version, release tag, running
deployment, host storage or cluster. Host setup and migration tooling default to
reviewable plans; explicit execution remains a separate operational action.
Disposable tests require recorded ownership and bounded resources on shared
hosts. No daemon-wide cleanup or deletion of unrelated resources is permitted.

Rendering, schema validation, offline guards and bounded compatibility tests do
not establish live Talos/KVM rebuild and upgrade behavior, policy enforcement,
CNPG backup/WAL/PITR, node drain during release E2E, or production capacity and
recovery objectives. Those remain explicit release gates, one Linear ticket each
under [ECO-884](https://linear.app/econgraph/issue/ECO-884), with the command,
pass threshold and evidence location for each; a gate passes only when its
evidence is linked on its ticket. The gates run on the scratch Talos cluster
built in [ECO-902](https://linear.app/econgraph/issue/ECO-902):

1. Real Talos boot and fit test: [ECO-903](https://linear.app/econgraph/issue/ECO-903).
2. CNPG operator and backup plugin startup: [ECO-904](https://linear.app/econgraph/issue/ECO-904).
3. Base backup, WAL archive and PITR: [ECO-905](https://linear.app/econgraph/issue/ECO-905).
4. Source-to-copy migration integrity: [ECO-906](https://linear.app/econgraph/issue/ECO-906).
5. One-node drain during the release E2E: [ECO-907](https://linear.app/econgraph/issue/ECO-907).
6. Live alert delivery: [ECO-908](https://linear.app/econgraph/issue/ECO-908).
7. Resource measurements: [ECO-909](https://linear.app/econgraph/issue/ECO-909).
8. Private-CA integration: [ECO-910](https://linear.app/econgraph/issue/ECO-910).
9. Sustained backup history: [ECO-911](https://linear.app/econgraph/issue/ECO-911).

Persistent host-store service form and production targets still require recorded
operator decisions; missing evidence must remain visible. No production cutover,
kind retirement or destructive history retirement is authorized by these packages.

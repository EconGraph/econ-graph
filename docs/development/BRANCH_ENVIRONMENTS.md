# Disposable branch environments

The branch renderer and deployer keep application state and Keycloak in a named
branch namespace. The workspace lane reads shared data anonymously through its
own data route. Storage and SEC lanes may opt into disposable writable data
copies. These tools do not change the primary image policy or deploy a new
platform.

## Candidate images and seed inputs

Pass an explicit context, backend image and hostname-specific frontend image.
Use a full `sha-<40 lowercase hex commit>` tag or `@sha256:<64 lowercase hex>`
digest for candidate images. Immutable candidate images use `IfNotPresent` so
they can be loaded into a scratch kind node. Other application tags use
`Always` and receive a fresh rollout token on each deployment; they must be
available from the registry. Rebuilding a mutable local
tag is not a supported way to update a branch.

Only `--local-data` accepts `--data-image`; a release tag, full commit tag or
digest is required. A mutable `latest`, branch name or short SHA is rejected.
The same image runs the data migration and, when explicitly started, the worker.
The worker initially has zero replicas so restoring pending queue entries cannot
start uncontrolled upstream crawling.

Local mode also requires `--seed-dir` and `--seed-sha256`. Obtain the expected
hash from reviewed fixture evidence outside the bundle. `VERIFIED` and
`SHA256SUMS` alone prove no provenance. The archive must contain only recognized
data-plane tables. An existing seed marker must match; an unmarked nonempty
database is never overwritten.

Seeds are bounded to 128 MiB compressed and 256 MiB expanded SQL. Before restore,
the branch database filesystem must have at least 4 GiB free plus four times the
expanded archive size. Expansion runs with an operating-system file-size limit
and a timeout. Restore is transactional and bounded by a timeout. These checks
are admission safeguards: kind local-path PVC sizes are **not disk quotas**.
Keep workers stopped except for bounded recorded-fixture operations, and run
scratch acceptance on a disposable cluster with its own disk budget. This does
not establish safe capacity for an unbounded live crawl.

A deployment plan contacts no cluster or credentials:

```sh
python3 scripts/deploy/deploy-branch.py up train3-storage \
  --context kind-train3-scratch --dry-run \
  --backend-image registry/backend:sha-<full-candidate-commit> \
  --frontend-image registry/frontend-storage:sha-<full-candidate-commit> \
  --local-data --data-image registry/worker:sha-<full-candidate-commit> \
  --seed-dir /reviewed/fixture --seed-sha256 <reviewed-archive-hash>
```

## Gateway confinement and ownership

Shared-data branches add a small proxy using nginx from the existing frontend
image. It forwards only exact `/data/graphql`, fixes Host to that branch, and
removes Authorization, Cookie and forwarded routing headers. NetworkPolicy lets
only this proxy reach the shared gateway; the application backend cannot choose
a production or another branch route by supplying a different Host. Local-data
branches have neither the proxy nor gateway egress. This adds one lightweight
pod per shared-data branch and assumes the frontend image retains nginx.

The deployer creates namespaces atomically, acquires a namespace lock and checks
ownership and data mode again under that lock. A competing creator fails closed.
Lock cleanup checks its owner token and deletes the inspected object with
UID/resourceVersion preconditions. Teardown requires the exact typed namespace.
If the namespace is absent, an exactly owned orphan grant can be removed after
confirmation and another absence check; grant deletion uses preconditions.
Listing includes branch-labelled grants so orphan resources remain visible.
Job polling reports terminal failure immediately; cluster commands and restores
have bounded timeouts.

## Verification and remaining milestone evidence

Run the offline guard tests and actual Kustomize parity without contacting a
cluster:

```sh
KUSTOMIZE_KUBECTL=/path/to/kubectl python3 scripts/deploy/test-branch-environments.py
```

Set `BRANCH_PROXY_IMAGE` to an already-built frontend image to additionally run
the proxy HTTP check in one network-isolated, read-only, automatically removed
container. `BRANCH_PROXY_DOCKER_HOST` selects the local scratch Docker socket.
This proves that forged Host headers cannot select another route, credentials
are removed and non-data paths are denied. It does not prove Kubernetes CNI
enforcement.

The milestone requires an actual three-lane run after the common baseline is
accepted. Use three scratch namespaces from the same candidate: workspace with
shared-data reads, and storage/SEC with separate writable recorded-fixture
copies. Before and after one local migration/write, record IDs, counts and
ordered content hashes in the primary-shaped fixture and untouched lane. Check
distinct routes/issuers and branch-local annotation writes. Exercise the
connection preflight, verify direct primary PostgreSQL and other-branch gateway
access fail, then tear down only one named lane and confirm the others and their
grants remain. Record baseline/image commits, fixture hashes, node free space,
actual database sizes and cleanup results alongside compatibility evidence.

No live cluster deployment or three-namespace acceptance is implied by the
offline tests, proxy smoke or split/combined database suite. The feature branch
does not authorize changes to primary, a milestone tag or release.

# Release images on the kind cluster

The manifests in `k8s/manifests` run `econ-graph-backend`, `econ-graph-crawler-worker`,
`econ-graph-frontend` and `econ-graph-chart-api` under one fixed tag each, with `imagePullPolicy: Never`, so the kind
nodes never pull. To run a release candidate that `.github/workflows/release.yml` built,
its images are put on the nodes under those names. Three scripts in `scripts/deploy/` do
that around `deploy.sh`, check the result, and go back:

```sh
scripts/deploy/load-release-images.sh --tag vX.Y.Z-rc.N --dry-run   # then without --dry-run
scripts/deploy/deploy.sh
scripts/deploy/verify-release-images.sh --tag vX.Y.Z-rc.N
```

and, to go back to what ran before the load:

```sh
scripts/deploy/rollback-release-images.sh --to pre-vX.Y.Z-rc.N --dry-run   # then without --dry-run
```

Each script's header lists every option and step. All three are safe to run again.
[Deploying a release](RELEASE_DEPLOY.md) runs the load, the backups, the precheck, the
deploy and the verify in one command.

## Which workloads, and which names

The scripts read the names from the manifests and never carry a tag of their own. For each
image, every manifest under `k8s/manifests` that runs it must name the same tag:

| Image | Workloads |
| --- | --- |
| `econ-graph-backend` | Deployment `econ-graph-backend` |
| `econ-graph-crawler-worker` | Deployment `crawler-worker`, Job `crawler-migrate` |
| `econ-graph-frontend` | Deployment `econ-graph-frontend` |
| `econ-graph-chart-api` | Deployment `chart-api-service` |

When two manifests name one image with different tags, the load and the rollback stop before
changing anything and name the files. Each script moves one name per image on the nodes, and
a workload on another name would be left on another image.

The `crawler-migrate` Job (`k8s/manifests/crawler-migrate-job.yaml`) applies the data
migrations with the crawler-worker image. `deploy.sh` deletes it and runs it again on every
deploy, before the backend and crawler-worker, so the deploy after a load runs the
release candidate's migrations. The rollback points the Job's name back together with the
Deployment's, since they are the same name, but it does not run the Job.

## Load

`load-release-images.sh` works on the Docker daemon the kind cluster runs on
(`scripts/deploy/lib/kind-env.sh`: the rootful `unix:///var/run/docker.sock`, or
`ECONGRAPH_DOCKER_HOST`; the shell's own `DOCKER_HOST` is ignored, and a rootless daemon is
refused), with the kind that `scripts/deploy/install-kind.sh` installs. That daemon must keep
images in Docker's classic image store; see [The daemon's image store](#the-daemons-image-store). It runs kubectl on
context `kind-<cluster>` through the kind cluster's own kubectl and kubeconfig when copies are
kept in `~/.cache/econgraph`, and prints that cluster's identity first.

1. It runs `git fetch --tags origin` (also in `--dry-run`) and resolves the tag to its commit.
2. It requires `ghcr.io/econgraph/econ-graph-<name>:sha-<full commit>` and the rc tag to report
   one digest on GHCR. The packages are private, so the Docker client needs a
   `docker login ghcr.io` with a token that has `read:packages`.
3. It decides what to keep for each image: what the Deployment's running pods run now, or,
   once the tag has a load record, what the first record kept. It stops before changing
   anything if an earlier load of the tag moved another name (the record's `local=`) than the
   manifests run now, if the pods of one Deployment run different images, if a node already has a
   `pre-<tag>` copy of another image, if a node lacks the image to keep (unless
   `--allow-missing-nodes`, which loads anyway, records which nodes have no copy, and exits 3),
   or if a node's images can't be read. "Can't read" is never taken as "not there".
4. It pulls each image by digest and stops unless the pulled image carries that digest and an
   `org.opencontainers.image.revision` label equal to the commit, and unless its `.Id` is a
   config digest rather than the registry digest (the `.Id` check below).
5. On every node it tags the image to keep as `econ-graph-<name>:pre-<tag>` in containerd,
   labels it pinned, and requires the CRI to report it pinned, so kubelet's image garbage
   collection keeps it once the manifests' name moves.
6. It tags each pulled image as the manifests' name, runs `kind load docker-image`, and checks
   every node's image ID.
7. It appends the tag, commit, digests, image IDs, the manifests' names, what ran before and
   the `pre-<tag>` copies to `~/econgraph-releases/<tag>/images.txt` (`ECONGRAPH_RELEASES_DIR`
   overrides the root).

Loading changes no pod spec, so `deploy.sh` alone leaves the Deployments' running pods on the
old images. From the load on, though, any of those pods that restarts for another reason
starts the new image, and so does the `crawler-migrate` Job the next `deploy.sh` runs. A load
that fails once it has started changing the nodes leaves some names (and some nodes) on the new
image and the rest on the old one. It records `status=partial` with what it did. Run it again,
or roll back.

A tag's loads all move the same names. Once a load of a tag is on record, a later load of it
from a checkout whose manifests name another tag stops before changing anything: it would move
a second name, and the rollback, which points back one name per image, would then refuse from
either checkout. Load from the checkout the record names, or roll back and give the new
manifests a new rc tag.

### The daemon's image store

The load works only with Docker's classic image store. New Docker installs (Docker 29 and
later) default to the containerd image store. With it, `docker pull` of a multi-platform image
keeps the whole index but only this platform's content, so `kind load docker-image` can fail on
a node part way through. And `docker image inspect --format '{{.Id}}'` then reports the
manifest or index digest, not the config digest the nodes report, so every node check after
the load would fail. Either way the load would stop after some nodes had changed.

So the load refuses that store before it pulls anything, also with `--dry-run`. It reads
`docker info --format '{{json .DriverStatus}}'` on the kind cluster's daemon, refuses when that
names `io.containerd.snapshotter`, and refuses when the store can't be read at all. To switch
the rootful daemon to the classic store, set this in `/etc/docker/daemon.json` and restart
Docker:

```json
{
  "features": {
    "containerd-snapshotter": false
  }
}
```

Images pulled into the containerd store are not visible in the classic one, so the next load
pulls them again. The kind nodes run their own containerd and are not affected by the setting.

The `.Id` check is a second guard. After each pull, before any node changes, the load requires
the image's `.Id` to be a `sha256:` digest that differs from the registry digest it pulled.
On the containerd store `.Id` is that registry digest; on the classic store it is the config
digest, which is what the nodes report and what the checks after `kind load` compare.

## Verify

`verify-release-images.sh` uses kubectl only, on context `kind-<cluster>`. It refuses a
partial load. It restarts the backend, crawler-worker, frontend and chart-api Deployments, waits for
the rollouts, and fails unless every running pod of each runs the loaded image and each
Deployment has a running pod. With `--no-restart` it only checks.

It then checks the pods the current `crawler-migrate` Job left, selected by the Job's own
`batch.kubernetes.io/controller-uid` label, so a pod left over from an earlier run of the Job
is never checked in its place. Every pod whose container started, whether running, succeeded
or failed, must have run the loaded crawler-worker image, and one that ran another image fails
the check. A pod whose container never started (still `Pending`, stuck on
`ErrImageNeverPull` or in an init container), or that is being deleted, is recorded as
`unchecked`. When there is no Job, or the Job has no pod, that is recorded as `unchecked` too,
rather than as a failure: `deploy.sh` skips the Job in a namespace without a data database,
and Kubernetes deletes a finished Job and its pods a day after it finishes
(`ttlSecondsAfterFinished`). The summary at the end lists everything unchecked. Run verify
right after `deploy.sh` to have the Job checked. The result goes to `verified.txt` next to the
load record.

## Rollback

`rollback-release-images.sh` checks each `pre-<tag>` copy against the first load record
before changing anything. The copy must be one image across the nodes, and the one the
record says ran before. Every node that has the manifests' name must have the copy; with
`--allow-missing-nodes` such a node is left as it is, and the script exits 3 even when the
pods check out. The name it points back is the one the load moved, and the manifests must
still run that name. If they have moved to another tag since, retagging the old name would
change nothing the pods run, so the script stops.

It then retags `econ-graph-<name>:pre-<tag>` as the manifests' name in each node's containerd
(`ctr -n k8s.io images tag --force`, through `docker exec`) and checks that the name now
points at the copy. Next it restarts the four Deployments and fails unless every running
pod runs the copy. It appends the result to `rollback.txt` next to the load record, with
`status=partial` and what it retagged if it stops half way.

- The `crawler-migrate` Job runs the same name as Deployment `crawler-worker`, so the retag
  moves the Job's image too. The Job is not run again here; the next `deploy.sh` runs it on
  the restored image. The data migrations the release candidate's Job applied stay applied,
  because rolling back images does not undo them.
- The pods go back to the image they ran before the load even if the manifests named
  another tag then. For example, a crawler that ran a hotfix tag comes back on that image,
  under the manifests' name.
- The rootful daemon's own copy of the manifests' name still names the release candidate.
  The pods run from the nodes' copies.
- `ctr images tag` copies labels, so after a rollback the manifests' name is pinned too.

## Removing old copies

Pinned `pre-<tag>` copies, and a name pinned by a rollback, are never released
automatically. Kubelet's garbage collection skips them and no script removes them. To free
an old copy once nothing needs it, do this on each node. The nodes are containers on the
rootful daemon, so name it for both commands, whatever the shell's own `DOCKER_HOST` is:

```sh
DOCKER_HOST=unix:///var/run/docker.sock kind get nodes --name econ-graph
DOCKER_HOST=unix:///var/run/docker.sock docker exec -it <node> sh
```

In each node's shell, unpin the copy, then remove the ref:

```sh
ctr -n k8s.io images label docker.io/library/econ-graph-<name>:pre-<tag> io.cri-containerd.pinned=
ctr -n k8s.io images rm docker.io/library/econ-graph-<name>:pre-<tag>
```

`images rm` removes that name only. The image's content stays while another name or a
container still uses it. The load record keeps what the copy was.

## Tests

`scripts/deploy/test-release-images.sh` runs all three scripts against stub git, docker (with
two nodes' containerd), kind and kubectl, in a scratch HOME. It touches no registry, daemon
or cluster. CI runs it, with shellcheck, in the Deploy Manifest Rendering job.

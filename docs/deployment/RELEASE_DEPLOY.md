# Deploying a release

`scripts/deploy/release-deploy.sh` deploys a release candidate to the kind cluster
`econ-graph` in one command. It is how a production window deploys. Run it from a clean
checkout of the tag:

```sh
git checkout vX.Y.Z-rc.N
scripts/deploy/release-deploy.sh --tag vX.Y.Z-rc.N --dry-run   # then without --dry-run
```

A final tag (`--tag vX.Y.Z`) deploys the release candidate tag on the same commit (the
highest `vX.Y.Z-rc.N` there): `release.yml` publishes the final tag on that candidate's
images, and the load records go by the candidate's tag.

`--cluster NAME` deploys to another kind cluster (default `KIND_CLUSTER`, else
`econ-graph`): every step gets it, `--cluster` for the load and the verify,
`--context kind-NAME` for the backups and `KIND_CLUSTER` for the precheck and `deploy.sh`.
It is set before `scripts/deploy/lib/kind-env.sh` is read, so a throwaway cluster needs
`ECONGRAPH_SCRATCH_STORAGE` there too, and is refused before any step without it.

## What it runs

Before any step, and with nothing changed yet:

- The checkout must be the tag's commit (`git rev-parse HEAD`) and clean: `deploy.sh`
  applies `k8s/manifests` from the working tree, so anything else would deploy manifests
  the tag doesn't have. Clean means no changes in the index or the working tree and no
  untracked files, asked with the settings that could hide them turned off
  (`status.showUntrackedFiles`, fsmonitor, the untracked cache); no file marked
  skip-worktree or assume-unchanged (`git ls-files -v`), whose changes `git status` does
  not show; and no ignored file (`.gitignore`, `.git/info/exclude`) where `deploy.sh`
  reads: `k8s`, `config`, `scripts`, `grafana-dashboards` and `ports.env`. The checkout is
  checked again just before the deploy step; if it changed, `deploy.sh` is not started.
  `GIT_DIR`, `GIT_WORK_TREE` and the like in the environment are refused, so git reads
  this checkout's own repository. Not caught: a change git can't see (a clean filter
  that hides it) and a change made while `deploy.sh` runs.
- It pins the cluster: the rootful Docker daemon the kind cluster runs on
  (`unix:///var/run/docker.sock`, or `ECONGRAPH_DOCKER_HOST`; never the shell's
  `DOCKER_HOST`) and the kind cluster's own kubectl and kubeconfig, which every step then
  uses, on context `kind-<cluster>`.
- It shows the cluster's identity (with its kube-system UID), the `PUBLIC_HOST` it will
  deploy with and the plan, and asks once. `--yes` answers on a cluster that is not marked
  production. A cluster marked production (`scripts/deploy/mark-production-cluster.sh`)
  needs the first 8 characters of its kube-system UID: typed, or
  `--confirm-production UID-PREFIX`, as `teardown.sh` does. `--yes` alone does not answer
  it, and without a terminal the run stops. A `--confirm-production` that doesn't match
  the cluster is refused on any cluster.

Then, in order, stopping at the first failure:

| Step | Runs | See |
| --- | --- | --- |
| backup | `backup-postgres-now.sh --context kind-NAME`: the nightly backup Job, now | [PostgreSQL backups](POSTGRES_BACKUP_RESTORE.md) |
| load | `load-release-images.sh --tag TAG --cluster NAME` | [Release images](RELEASE_IMAGES.md) |
| keycloak | `backup-keycloak-db.sh --context kind-NAME`: a verified dump of Keycloak's database | the script's header |
| precheck | `precheck-db-migration.sh --stop-crawler` | the script's header |
| deploy | `deploy.sh`, with `PUBLIC_HOST` from the deploy config | [k8s/README.md](../../k8s/README.md) |
| verify | `verify-release-images.sh --tag TAG --cluster NAME` | [Release images](RELEASE_IMAGES.md) |

The database backup comes first. Once the load has put the new images on the nodes under
the names the manifests run, any pod that restarts (a crash, a node reboot) runs the new
image against the old schema, so the backup, which can take a long time, is not in that
window. The backup Job runs stock Postgres images that the load doesn't touch. The
Keycloak dump stops Keycloak until `deploy.sh` applies it again, so it comes after the
load, where a load that refuses leaves sign-in running, and just before the precheck.

Every step is safe to run again with the same tag, so after a failure, fix the cause and
run the same command again.

The verify can pass with things it could not check (a `crawler-migrate` Job that is already
deleted, a pod whose container never started). The run lists them again in its final
summary, so a deploy with unchecked parts is not read as fully checked.

Each step's output goes to the terminal and to a step log under `$XDG_RUNTIME_DIR` (else
`~/.local/state/econgraph`), from which the run reads the backups' paths, the crawler
worker's replicas and verify's unchecked items. The step's own exit status decides
whether it failed: a step log that can't be written doesn't, and neither does a process a
step leaves running in the background with its output open. Each step has a log of its
own.

Ctrl-C, SIGTERM or SIGHUP during a step sends that step and its child processes (found
with `pgrep -P`; a process that detached itself from them is not reached) SIGTERM once,
and waits for them: a step's own cleanup, such as the load writing its record, runs to
the end. A second signal only says the run is still stopping. A third, or
`ECONGRAPH_STOP_TIMEOUT` seconds (default 120) without the step stopping, sends SIGKILL,
with a warning that the step's cleanup may be cut short. The run then prints the same
guidance as a failure of that step, reports `result=interrupted step=NAME
signal=INT|TERM|HUP` and exits 130, 143 or 129; signals during that explanation are
ignored. A step that completed as the signal came counts as done, and the run stops
before the next one. Steps ignore the SIGINT and SIGHUP a terminal sends its whole process
group, so only the run's own signals stop them.

## The run's record

Each run appends to `~/econgraph-releases/<tag>/release-deploy-<cluster>.txt`
(`ECONGRAPH_RELEASES_DIR` overrides the root), one line per item, each starting with the
run's id, the cluster's kube-system UID and the time
(`run=ID cluster_uid=UID date=TIME KEY=VALUE`; the value is the rest of the line, so a
path may hold blanks):

- `keycloak_replicas=N` and `crawler_replicas=N`: their replicas before the run, when
  they run;
- `crawler_replicas=N` also when the precheck scales the crawler worker down from N, even
  if the precheck then fails;
- `backup=PATH`: each dump of a backup step that succeeded (`/backups/econ_graph-<stamp>.dump`
  and the others, on the backup volume), then `backup_complete=1`. A backup step that
  fails partway records nothing;
- `keycloak_backup=DIR`: the verified Keycloak dump;
- `deploy_started=1`: just before the deploy step starts.

A later run of the same tag on the same cluster reads them: by then Keycloak and the
crawler worker may already be at 0, an earlier run's `deploy.sh` may have reset the
crawler worker to the manifest's replicas, and the databases may already be migrated. The
earliest value counts. Lines with another kube-system UID (a cluster recreated under the
same name) are ignored, and the run says how many. A record that can't be written stops
the run: before any step, or at the step whose result it was recording.

## PUBLIC_HOST

`PUBLIC_HOST` comes from the host's deploy config, never from memory:
`~/.config/econgraph/deploy.env` (`$XDG_CONFIG_HOME/econgraph/deploy.env`;
`ECONGRAPH_DEPLOY_ENV` names another file), one line:

```sh
PUBLIC_HOST=econgraph.com
```

The file is read, not sourced: surrounding blanks and one pair of quotes are dropped, and
the name is lowercased (`ECONGRAPH.COM` deploys `econgraph.com`). When there are several
`PUBLIC_HOST` lines, the last one counts, as it would if the file were sourced. On the
production host it holds `econgraph.com`; a test cluster's host has no file, or
`PUBLIC_HOST=` (empty), and serves localhost only. Before any step it refuses:

- a `PUBLIC_HOST` line with a carriage return (a file saved with CRLF endings), a value
  with blanks in it (an inline comment, spaces inside the quotes), or one that is not a
  public DNS name, by the check `deploy.sh` makes too (`render-public-host.py`);
- a cluster marked production with no `PUBLIC_HOST` configured: deploying it without one
  would stop the tunnel connector and take the public site down;
- a cluster marked production whose configured `PUBLIC_HOST` is not the host it serves
  now (its public routes), or that serves none yet, unless `--change-public-host` is
  given; both are printed. A typo such as `econ-graph.com` would otherwise move the
  public routes, the certificate and the tunnel's host to a name nobody visits. A
  cluster whose routes can't be read is refused too;
- a cluster not marked production with a `PUBLIC_HOST` configured (`deploy.sh` would refuse
  it too, but only after the load and the backups);
- a cluster whose marker can't be read;
- a `PUBLIC_HOST` in the environment that differs from the config.

## Dry run

`--dry-run` runs each step's own dry run: both backups, the load (which still fetches the
tags and resolves the registry digests), the precheck and `deploy.sh --dry-run`.
Nothing is changed, no kubectl is cached and nothing is recorded. The verify step checks
the pods against the images a load put on the nodes, so it can only run after a real
load and deploy; the dry run says what it would run.

## When a step fails

The script prints which steps ran, which one failed, the backups this run took, what the
failure leaves changed, and how to go back. Going back from the images is always

```sh
scripts/deploy/rollback-release-images.sh --to pre-vX.Y.Z-rc.N --dry-run   # then without --dry-run
```

with `--cluster NAME` on a cluster other than `econ-graph`.

- **backup**: no image was loaded and the databases are unchanged. When the backup Job
  timed out but is still running, it says so, with the command to wait for it: run again
  only once it has finished, since a run starts another backup Job. After a Ctrl-C during
  the backup, it says the Job may still be running in the cluster, with the commands to
  check it. A backup step that succeeds without naming the dumps it wrote (no
  `Backup OK ... in PATH` line), or a Keycloak backup without its verified dump, stops
  the run there.
- **load**: a load that refuses (an earlier load of the tag under another image name, a
  digest or label mismatch) changes nothing first, as its last line says.
- **keycloak, precheck, deploy**: Keycloak may be stopped (the keycloak step scales it to
  0, and only `deploy.sh` applies it again). It prints the command that starts it at the
  replicas it ran before the tag's first run on this cluster, from the record. When it was
  already at 0 and no run recorded it running, it says so, with the manifest's replicas.
- **precheck, deploy**: the crawler worker may be stopped. The precheck scales it to 0,
  and only `deploy.sh` starts it again, when it applies `crawler-worker.yaml` after the
  `crawler-migrate` Job; any failure before that leaves it at 0. It prints the command
  that starts it at the replicas it ran before the tag's first run on this cluster (the
  earliest record), or says it was already at 0, with the manifest's replicas. The nodes
  hold the new crawler image under the name it runs, so it says to start it only after
  the rollback, or once a deploy has applied the migrations.
- **deploy, verify**: if the data migrations ran, or Keycloak started on a new version,
  the databases need restoring. It names the backups to restore from, with the run's id
  and date: the last complete backup of the data databases, and the last verified
  Keycloak dump, recorded before the first `deploy_started` of the tag on this cluster.
  They hold the state just before the tag's migrations, without losing the writes that
  came after an earlier run that never deployed. The commands are
  `restore-postgres-backup.sh --context kind-NAME --file econ_graph-<stamp>.dump` and
  `restore-keycloak-db.sh DIR --context kind-NAME`. When the tag was first deployed
  before this run, it also names the later backups taken before this run's deploy, to
  use only if the databases were restored to before the tag since then.

## Automation

`--report FILE` appends to `FILE`:

- one line per step, `step=NAME status=ok|failed|dry-run`, with `unchecked=N` on a verify
  that left N things unchecked;
- the backups taken, `backup=PATH` and `keycloak_backup=DIR`;
- after a failed deploy or verify, the backups to restore from, `restore_backup=PATH` and
  `restore_keycloak_backup=DIR`;
- a final `result=ok|failed step=NAME|interrupted step=NAME signal=SIG|dry-run|refused`
  line; `result=ok unchecked=N` when the verify left N things unchecked.

A report that can't be written stops the run before any step; a later write that fails is
warned about and never cuts the run or its failure output short.

## Tests

`scripts/deploy/test-release-deploy.sh` runs `release-deploy.sh` with every step replaced by
a stub, and stub kubectl, kind and docker in a scratch HOME. It checks the order, the stop
at each failing step with its state and rollback output, a step that leaves a background
process holding its output, signals during a step (the escalation from SIGTERM to SIGKILL, the step's cleanup, a burst of them), the restart and restore
guidance across re-runs (a backup that failed partway, records from another cluster,
paths with blanks), an unwritable record, the checkout checks (each way git can hide a
change, and a change during the run), the `PUBLIC_HOST` guard, the production
confirmation, the dry run and the final-tag resolution. CI runs it, with shellcheck, in the Deploy Manifest Rendering job.
`scripts/deploy/test-kind-kubectl-default.sh` checks that `deploy.sh` stops its pod status
monitor when a wait fails.

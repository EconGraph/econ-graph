# Deploying a release

`scripts/deploy/release-deploy.sh` deploys a release candidate to the kind cluster
`econ-graph` in one command. It is how a production window deploys, and how the upgrade
rehearsal in CI deploys:

```sh
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

Before any step, and with nothing changed yet, it pins the cluster: the rootful Docker
daemon the kind cluster runs on (`unix:///var/run/docker.sock`, or
`ECONGRAPH_DOCKER_HOST`; never the shell's `DOCKER_HOST`) and the kind cluster's own
kubectl and kubeconfig, which every step then uses, on context `kind-<cluster>`. It shows
the cluster's identity, the `PUBLIC_HOST` it will deploy with and the plan, and asks once
(`--yes` answers; without a terminal it needs `--yes`).

Then, in order, stopping at the first failure:

| Step | Runs | See |
| --- | --- | --- |
| load | `load-release-images.sh --tag TAG --cluster NAME` | [Release images](RELEASE_IMAGES.md) |
| backup | `backup-postgres-now.sh --context kind-NAME`: the nightly backup Job, now | [PostgreSQL backups](POSTGRES_BACKUP_RESTORE.md) |
| keycloak | `backup-keycloak-db.sh --context kind-NAME`: a verified dump of Keycloak's database | the script's header |
| precheck | `precheck-db-migration.sh --stop-crawler` | the script's header |
| deploy | `deploy.sh`, with `PUBLIC_HOST` from the deploy config | [k8s/README.md](../../k8s/README.md) |
| verify | `verify-release-images.sh --tag TAG --cluster NAME` | [Release images](RELEASE_IMAGES.md) |

Every step is safe to run again with the same tag, so after a failure, fix the cause and
run the same command again.

The verify can pass with things it could not check (a `crawler-migrate` Job that is already
deleted, a pod whose container never started). The run lists them again in its final
summary, so a deploy with unchecked parts is not read as fully checked.

## PUBLIC_HOST

`PUBLIC_HOST` comes from the host's deploy config, never from memory:
`~/.config/econgraph/deploy.env` (`$XDG_CONFIG_HOME/econgraph/deploy.env`;
`ECONGRAPH_DEPLOY_ENV` names another file), one line:

```sh
PUBLIC_HOST=econgraph.com
```

The file is read, not sourced: surrounding blanks and one pair of quotes are dropped, and
nothing else. On the production host it holds `econgraph.com`; a test cluster's host has
no file, or `PUBLIC_HOST=` (empty), and serves localhost only. Before any step it refuses:

- a `PUBLIC_HOST` line with a carriage return (a file saved with CRLF endings), a value
  with blanks in it (an inline comment, spaces inside the quotes), or one that is not a
  public DNS name, by the check `deploy.sh` makes too (`render-public-host.py`);

- a cluster marked production (`scripts/deploy/mark-production-cluster.sh`) with no
  `PUBLIC_HOST` configured: deploying it without one would stop the tunnel connector and
  take the public site down;
- a cluster not marked production with a `PUBLIC_HOST` configured (`deploy.sh` would refuse
  it too, but only after the load and the backups);
- a cluster whose marker can't be read;
- a `PUBLIC_HOST` in the environment that differs from the config.

## Dry run

`--dry-run` runs each step's own dry run: the load (which still fetches the tags and
resolves the registry digests), both backups, the precheck and `deploy.sh --dry-run`.
Nothing is changed, and no kubectl is cached. The verify step checks the pods against the
images a load put on the nodes, so it can only run after a real load and deploy; the dry
run says what it would run.

## When a step fails

The script prints which steps ran, which one failed, what that leaves changed, and how to
go back. A load that refuses (an earlier load of the tag under another image name, a
digest or label mismatch) changes nothing first, as its last line says. Going back from
the images is always

```sh
scripts/deploy/rollback-release-images.sh --to pre-vX.Y.Z-rc.N --dry-run   # then without --dry-run
```

After a failed precheck it also prints the command that starts the crawler worker again.
The keycloak step scales Keycloak to 0 and only `deploy.sh` applies it again, so after a
failed keycloak, precheck or deploy step it prints the command that starts Keycloak at the
replicas it ran before the run.
After a failed deploy or verify, if the data migrations ran, it points to the database
restores from the backups the run took (`restore-postgres-backup.sh` and
`restore-keycloak-db.sh`).

## Automation

`--report FILE` appends one line per step (`step=NAME status=ok|failed|dry-run`, with
`unchecked=N` on a verify that left N things unchecked) and a
final `result=ok|failed step=NAME|dry-run|refused` line to `FILE`. A report that can't be
written stops the run before any step; a later write that fails is warned about and never
cuts the run or its failure output short. The upgrade rehearsal
runs `release-deploy.sh --tag ... --yes --report FILE` and reports the failed step from it.

## Tests

`scripts/deploy/test-release-deploy.sh` runs `release-deploy.sh` with every step replaced by
a stub, and stub kubectl, kind and docker in a scratch HOME. It checks the order, the stop
at each failing step with its state and rollback output, the `PUBLIC_HOST` guard, the dry
run, the confirmation and the final-tag resolution. CI runs it, with shellcheck, in the
Deploy Manifest Rendering job.

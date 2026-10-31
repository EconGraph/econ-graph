# Release lab

A release lab is a disposable kind cluster that runs one release of the app stack on its
own copy of the data, restored from dumps given by path: the data database (`econ_graph`),
Keycloak's database (`keycloak`) and, for a release with an app database, the app
database (`econ_graph_app`). Use it to rehearse anything that changes data (migrations,
the app and data database conversion, a crawl sample) without touching the production
cluster or the shared data plane.

One script runs it: `scripts/deploy/release-lab.sh` (Python 3 and PyYAML). Its usage
header, `scripts/deploy/release-lab.py --help`, is the full reference.

## What a lab rehearses

A lab runs the candidate on production's data: the release whose checkout runs
`release-lab.sh`, on a copy of the data restored from the dumps. Its manifests come from
that checkout, so run it from a worktree at the candidate, and give that candidate's
images; `create` warns when an image's revision label is another commit than the
checkout's. The record keeps the checkout's commit (`source` in `lab.json`).

A lab does not deploy one release and then upgrade it to another: the restored data is
the "before", and the candidate's data migrations (crawler-migrate) and Keycloak's own
migration run on it.

```bash
git worktree add ../econ-graph-candidate vX.Y.Z-rc.N
cd ../econ-graph-candidate
```

## Create a lab

```bash
scripts/deploy/release-lab.sh create --lab NAME \
  --data-dump /path/econ_graph.dump --keycloak-dump /path/keycloak.dump \
  [--app-dump /path/econ_graph_app.dump] [--app-database shared|separate] \
  --backend-image REF --frontend-image REF --crawler-image REF [--release vX.Y.Z] [--dry-run]
```

- `NAME`: 1 to 32 lowercase letters, digits and inner hyphens. The lab is the kind cluster
  `econ-graph-lab-NAME`, context `kind-econ-graph-lab-NAME`.
- The dumps are `pg_dump -Fc` archives, such as those of a backup directory
  (`scripts/deploy/backup-kind-databases.sh`). They are opened read only and streamed into
  the lab; the lab records their SHA-256, size and modification time and checks them again
  at the end. A dump inside the lab root, and the same dump given twice, are refused.
- Where the app tables are (`--app-database`):
  - `shared`, the default without `--app-dump`: in the data database, as in a release
    before the app database. A backup of such a production has no
    `econ_graph_app.dump`, and its users are in `econ_graph`. `app-database-url` names
    the data database, and the app database runs empty, so `convert-app-data.sh` can be
    rehearsed (below).
  - `separate`, the default with `--app-dump`: in the app database, restored from
    `--app-dump` (without one, it starts empty).
- The images (`REF`) must be on the rootful Docker daemon, or pullable there: a digest
  reference (`...@sha256:<64 hex>`) or a full-commit tag (`...:sha-<40 hex>`, whose
  `org.opencontainers.image.revision` label must name that commit). The production tags
  `econ-graph-<image>:vX.Y.Z` are accepted only with `--release vX.Y.Z` of that exact
  release, only if already on the daemon (the lab never pulls them), and only when their
  revision label is the commit of tag `vX.Y.Z` in this checkout.
- `--dry-run` makes the read-only checks for real (the dumps, the Docker daemon, the
  existing clusters, the images, free space) and prints the plan; it creates, pulls and
  loads nothing.

Running `create` again for a lab that is ready does nothing. A lab whose creation failed
is kept for inspection: tear it down before creating it again. Once a lab is torn down
with its storage removed, `create` can use its name again; the old directory is kept as
`.NAME.torn-down-<time>/` in the lab root.

What `create` does, in order:

1. Checks the dumps and the free space: the lab root needs 10 GiB plus 8 times the dumps'
   size, and the Docker data root (`docker info`'s `DockerRootDir`, `/var/lib/docker`)
   10 GiB plus twice the images' size (the node keeps its own copy of each). When both are
   on one filesystem, it must have room for both together, with one 10 GiB reserve.
2. Creates the lab directory `${ECONGRAPH_LAB_ROOT:-~/econgraph-labs}/NAME/` (the lock
   on the name; never under `/tmp` or on a tmpfs) and writes the lab record `lab.json`.
3. Creates the kind cluster with the pinned kind (`scripts/deploy/install-kind.sh`) on the
   rootful daemon (`ECONGRAPH_DOCKER_HOST` overrides it; the shell's `DOCKER_HOST` does
   not), on its own Docker network `econ-graph-lab-NAME`, from the node image
   `terraform/k8s/kind-config.yaml` pins: one node, no port mappings, the API server on
   127.0.0.1, and the node's volume directory bind-mounted from `NAME/node-storage/`.
   The kubeconfig and the cluster's own kubectl go to `NAME/kube/`; `~/.kube` and the
   production kubeconfig are not touched.
4. Writes the ownership marker ConfigMap `kube-system/econgraph-lab-owner`: the record's
   random lab id and the cluster's kube-system UID. If that fails, it deletes the cluster
   it has just created.
5. Tags each image as `econ-graph-lab-NAME/<image>:<image id>` and loads it into the lab
   cluster (`kind load docker-image --name econ-graph-lab-NAME`).
6. Applies the app stack in namespace `econ-graph` from the local-data rendering of
   `scripts/deploy/render-branch.py`: the three Postgres StatefulSets on the cluster's
   local-path storage, Keycloak, the backend, the frontend, crawler-worker and the
   crawler-migrate Job, with lab-generated passwords. ClusterIP Services only; no gateway,
   no cloudflared, no tunnel token, no API keys (`crawler-api-keys` is not created).
7. Restores each dump into its empty database in one transaction (a database that is not
   empty is refused). Objects are created as the role the lab's services connect as
   (`econgraph`, or `keycloak` for Keycloak's), whatever owners the dump names, without
   the dump's grants and comments. Then it cleans the Keycloak copy (below).
8. Runs the data migrations (crawler-migrate), starts Keycloak (which migrates a copy from
   an older Keycloak), the backend and the frontend, adds a lab-only Keycloak admin
   (`kc.sh bootstrap-admin`; the restored admins keep production's passwords, which the
   lab does not have), restarts Keycloak, and applies the realm without identity providers
   (keycloak-realm-import).
9. Checks that crawler-worker has 0 replicas and that the dumps are unchanged.

## Use a lab

```bash
scripts/deploy/release-lab.sh port-forward --lab NAME
```

Forwards, on 127.0.0.1 only, to the ports `create` chose and recorded (the command prints
them):

- the frontend, at `http://localhost:FRONTEND`;
- the backend, with GraphQL at `http://localhost:BACKEND/graphql`;
- Keycloak, with the admin console at `http://localhost:KEYCLOAK/idp/admin`. Keycloak's
  public and admin addresses are that address, so its pages and redirects stay on it.
  The lab admin's name and password are in Secret `econ-graph-keycloak`;
- the data database, at `127.0.0.1:DATA` (user `econgraph`, password in Secret
  `econ-graph-postgres`, key `app-password`).

The lab has no gateway, so the frontend's own requests (`/graphql`, sign-in) do not reach
the backend or Keycloak, and a browser sign-in through the frontend does not work.

For kubectl, use the lab's own:

```bash
~/econgraph-labs/NAME/kube/kubectl --kubeconfig ~/econgraph-labs/NAME/kube/kubeconfig \
  --context kind-econ-graph-lab-NAME -n econ-graph get pods
```

crawler-worker starts at 0 replicas, so a restored crawl queue reaches no upstream API.
Scaling it up is a deliberate step, and the lab has no API keys unless you add
`crawler-api-keys` to it yourself.

`status --lab NAME` prints the record and the pods; `list` lists the labs.

### Run the deploy scripts against a lab

The scripts that use `scripts/deploy/lib/kind-env.sh` default to the production cluster
and its paths. Run each of them against a lab in a subshell that first evaluates
`release-lab.sh env`, so none of its settings stays in your shell:

```bash
( eval "$(scripts/deploy/release-lab.sh env --lab NAME)"; scripts/deploy/SCRIPT ... )
```

`env` sets `KIND_CLUSTER=econ-graph-lab-NAME`; `ECONGRAPH_SCRATCH_STORAGE` to a path in the
lab directory that does not exist (kind-env.sh requires it for any cluster but
`econ-graph`; the lab has no host storage, so the host-storage scripts stop there);
`XDG_CACHE_HOME` (the copy of the lab's kubectl and kubeconfig), `ECONGRAPH_BACKUP_ROOT`
and `KEYCLOAK_BACKUP_ROOT` to directories in the lab; and unsets `KUBECTL`,
`KUBECONFIG`, `ECONGRAPH_KUBE_DIR`, `CLUSTER_ROLE_CONTEXT` and `PUBLIC_HOST`.

- The app and data database conversion (docs/deployment/APP_DATA_CONVERSION.md), on a lab
  created with `--app-database shared`:

  ```bash
  ( eval "$(scripts/deploy/release-lab.sh env --lab NAME)"
    scripts/deploy/convert-app-data.sh --cluster econ-graph-lab-NAME --source-map FILE [--drop-source-app] )
  ```

  It leaves the backend at 0 replicas once the app tables have moved. Start it again with
  the lab's kubectl: `scale deployment/econ-graph-backend --replicas=1`.
- Catalog access in the lab's Keycloak:

  ```bash
  ( eval "$(scripts/deploy/release-lab.sh env --lab NAME)"
    scripts/keycloak/set-catalog-access.sh --context kind-econ-graph-lab-NAME --dry-run --admin-only ROLES )
  ```

- The v4.1 first-deploy measurements, on the data database forwarded by `port-forward`:

  ```bash
  PGPASSWORD="$(~/econgraph-labs/NAME/kube/kubectl --kubeconfig ~/econgraph-labs/NAME/kube/kubeconfig \
    --context kind-econ-graph-lab-NAME -n econ-graph get secret econ-graph-postgres \
    -o jsonpath='{.data.app-password}' | base64 -d)" \
    scripts/deploy/measure-v41-first-deploy.sh --database-url postgresql://econgraph@127.0.0.1:DATA/econ_graph
  ```

- `scripts/deploy/deploy.sh` is not run against a lab. It applies production's static
  storage (the host-storage PVs, which the lab's StatefulSets don't use), the gateway,
  cloudflared at 0 replicas and the monitoring stack; the lab's stack is the one `create`
  applies.

## Tear a lab down

```bash
scripts/deploy/release-lab.sh teardown --lab NAME [--keep-storage]
```

It takes the cluster, context and network from `lab.json`, never from an argument, and
refuses unless all of these hold:

- the record names `econ-graph-lab-NAME` (never a cluster without the `econ-graph-lab-`
  prefix);
- the cluster carries `kube-system/econgraph-lab-owner` with the record's lab id and name
  and its own kube-system UID, which must also be the UID recorded at creation;
- the cluster carries no `kube-system/econgraph-cluster-role` marker.

A cluster whose creation stopped before the ownership marker is refused; the refusal
prints the `kind delete cluster --name econ-graph-lab-NAME` to run by hand after checking
its nodes.

It asks for the lab name at a terminal (or takes `--yes`), checks the marker again, runs
`kind delete cluster --name econ-graph-lab-NAME`, removes the Docker network if no
container uses it, and untags the lab-only image names (an image keeps its other
names). Then it empties `NAME/node-storage/`, the lab's full copy of the data, through a
container that mounts only that directory. `--keep-storage` keeps that copy and warns; a
later `teardown` of the same lab removes it. The dumps are never touched, and `lab.json`
stays as the record of the lab. `--dry-run` shows what it would delete.

## The Keycloak copy

Before Keycloak starts, the lab removes from its copy, in one transaction:

- the identity providers, their mappers, configuration and account links: no sign-in
  through Google or Facebook, and none of their client secrets;
- every key provider of every realm, among them the RSA, HMAC and AES keys. Keycloak makes
  new keys, so tokens production signed are not valid in the lab, and the lab's are not
  valid in production;
- every confidential client's secret, replaced by a random one (the realm import then
  sets the clients it manages from the lab's Secret), and rotated client secrets;
- the JGroups CA certificate and its private key (`server_config` key `crt_jgroups`) and
  the JGroups member addresses (`jgroups_ping`); Keycloak makes a new certificate;
- offline sessions, revoked tokens, login events, admin events and SMTP settings;
- secret-looking values (names containing secret, password, credential, token or key)
  in the remaining components' and authenticators' configuration.

Kept on purpose, so sign-in can be tested with realistic accounts: users, attributes,
groups, roles and consents. The copy therefore still holds its users' production secrets:
in `credential`, password hashes, TOTP seeds and WebAuthn keys; `fed_user_credential`;
`user_federation_config`; and a SAML client's private key in `client_attributes`, if
any. Treat a lab's storage like a production backup. The restored production admins keep
their passwords; the lab adds its own admin.

## Never production

- The lab writes no `econgraph-cluster-role` marker, and refuses a cluster that has one.
- `scripts/deploy/mark-production-cluster.sh` refuses to mark a cluster with
  `econgraph-lab-owner`. If one ever carried a valid production marker too,
  `scripts/deploy/lib/cluster-role.sh` reports conflicting markers: whether it is
  production is unknown, and the scripts refuse to act on it, as when the cluster can't be
  asked.
- No credential from the operator's environment (variables named like a token, secret,
  password or API key) reaches kind, docker or kubectl.

## Measure a lab

```bash
scripts/deploy/release-lab.sh measure [--lab NAME]
```

prints, per lab, the node container's CPU and memory (`docker stats`), the size of the
databases' volumes (`NAME/node-storage/`), the images in the node's containerd, and the
node container's writable layer. The same numbers by hand:

```bash
docker -H unix:///var/run/docker.sock stats --no-stream econ-graph-lab-NAME-control-plane
docker -H unix:///var/run/docker.sock exec econ-graph-lab-NAME-control-plane du -sxh /var/local-path-provisioner /var/lib/containerd
free -g; df -h /home /var/lib/docker
```

To measure what a lab takes:

1. Before creating it, note `free -g`, `df -h /home /var/lib/docker` and the production
   cluster's own usage (`docker stats --no-stream econ-graph-control-plane econ-graph-worker`).
2. Run `measure` during each heavy step (the restore, crawler-migrate, Keycloak's start,
   a crawl if one is scaled up), several times each: CPU is a point sample, and memory
   peaks during those steps.
3. Run it again once the lab is idle, and note the disk figures.

The namespace's ResourceQuota (from the branch rendering) caps the lab's pods at 12 CPUs
and 16 GiB of memory limits. This page sets no number of concurrent labs. Before creating
another, check that the host keeps the production cluster's headroom with the running
labs' measured memory and CPU added, and that `/home` (lab volumes) and the Docker data
root (node images) have room for one more; `create` itself checks the disk space.

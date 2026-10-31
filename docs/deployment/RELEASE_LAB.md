# Release lab

A release lab is a disposable kind cluster that runs the app stack on its own copy of
the data, restored from three dumps given by path: the data database (`econ_graph`), the
app database (`econ_graph_app`) and Keycloak's database (`keycloak`). Use it to rehearse
anything that changes data (migrations, a conversion, a crawl sample) without touching
the production cluster or the shared data plane.

One script runs it: `scripts/deploy/release-lab.sh` (Python 3 and PyYAML). Its usage
header, `scripts/deploy/release-lab.py --help`, is the full reference.

## Create a lab

```bash
scripts/deploy/release-lab.sh create --lab NAME \
  --data-dump /path/econ_graph.dump --app-dump /path/econ_graph_app.dump \
  --keycloak-dump /path/keycloak.dump \
  --backend-image REF --frontend-image REF --crawler-image REF [--release vX.Y.Z]
```

- `NAME`: 1 to 32 lowercase letters, digits and inner hyphens. The lab is the kind cluster
  `econ-graph-lab-NAME`, context `kind-econ-graph-lab-NAME`.
- The dumps are `pg_dump -Fc` archives, such as the `econ_graph.dump`,
  `econ_graph_app.dump` and `keycloak.dump` of a backup directory
  (`scripts/deploy/backup-kind-databases.sh`). They are opened read only and streamed into
  the lab; the lab records their SHA-256, size and modification time and checks them again
  at the end. A dump inside the lab root is refused.
- The images (`REF`) must be on the rootful Docker daemon, or pullable there: a digest
  reference (`...@sha256:<64 hex>`) or a full-commit tag (`...:sha-<40 hex>`, whose
  `org.opencontainers.image.revision` label must name that commit). The production tags
  `econ-graph-<image>:vX.Y.Z` are accepted only with `--release vX.Y.Z` of that exact
  release, only if already on the daemon (the lab never pulls them), and only when their
  revision label is the commit of tag `vX.Y.Z` in this checkout.
- `--dry-run` makes the read-only checks for real (the dumps, the Docker daemon, the
  existing clusters, the images) and prints the plan; it creates, pulls and loads nothing.
- The three dumps must differ: the same file given twice is refused.

Running `create` again for a lab that is ready does nothing. A lab whose creation failed
is kept for inspection: tear it down before creating it again. Once a lab is torn down
with its storage removed, `create` can use its name again; the old directory is kept as
`.NAME.torn-down-<time>/` in the lab root.

What `create` does, in order:

1. Checks the dumps and that the lab root has 10 GiB plus 8 times the dumps' size free.
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
   empty is refused), then cleans the Keycloak copy (below).
8. Runs the data migrations (crawler-migrate), starts Keycloak, the backend and the
   frontend, adds a lab-only Keycloak admin (`kc.sh bootstrap-admin`; the restored
   database's own admins keep production's passwords, which the lab does not have),
   restarts Keycloak, and applies the realm without identity providers
   (keycloak-realm-import).
9. Checks that crawler-worker has 0 replicas and that the dumps are unchanged.

## Use a lab

```bash
scripts/deploy/release-lab.sh port-forward --lab NAME
```

Forwards the frontend, the backend (GraphQL at `/graphql`) and Keycloak (admin console at
`/idp/admin`) to ports on 127.0.0.1 that `create` chose and recorded; the command prints
them. The lab Keycloak admin's name and password are in Secret `econ-graph-keycloak`.
For kubectl, use the lab's own:

```bash
~/econgraph-labs/NAME/kube/kubectl --kubeconfig ~/econgraph-labs/NAME/kube/kubeconfig \
  --context kind-econ-graph-lab-NAME -n econ-graph get pods
```

The lab has no gateway, so a browser sign-in through one origin does not work.

crawler-worker starts at 0 replicas, so a restored crawl queue reaches no upstream API.
Scaling it up is a deliberate step, and the lab has no API keys unless you add
`crawler-api-keys` to it yourself.

`status --lab NAME` prints the record and the pods; `list` lists the labs.

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
- the realms' key providers (RSA, HMAC and AES private keys). Keycloak makes new keys,
  so tokens production signed are not valid in the lab, and the lab's are not valid in
  production;
- every confidential client's secret, replaced by a random one (the realm import then
  sets the clients it manages from the lab's Secret), and rotated client secrets;
- offline sessions, revoked tokens, login events, admin events and SMTP settings;
- secret-looking values (names containing secret, password, credential, token or key)
  in the remaining components' and authenticators' configuration.

Kept on purpose, so sign-in can be tested with realistic accounts: users, their password
hashes and other credentials, attributes, groups, roles and consents. The restored
production admins keep their passwords; the lab adds its own admin.

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
node container's writable layer. CPU is a point sample: run it while the lab does the
work being measured (the restore, the migrations, a crawl), several times. The same
numbers by hand:

```bash
docker -H unix:///var/run/docker.sock stats --no-stream econ-graph-lab-NAME-control-plane
docker -H unix:///var/run/docker.sock exec econ-graph-lab-NAME-control-plane du -sxh /var/local-path-provisioner /var/lib/containerd
free -g; df -h /home /var/lib/docker
```

The namespace's ResourceQuota (from the branch rendering) caps the lab's pods at 12 CPUs
and 16 GiB of memory limits. Several labs can run at once on the production host: before
creating another, check that the host keeps the production cluster's headroom with the
running labs' measured memory and CPU added, and that `/home` (lab volumes) and
`/var/lib/docker` (node images, about the size of the loaded images per lab) have room
for one more.

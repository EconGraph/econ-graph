# Restarting a kind deployment

Use this runbook for an existing EconGraph deployment in the kind cluster
`econ-graph`. A rollout restart replaces application pods using their configured
images. To deploy changes from a checkout, build and load the images, run the
deployment procedure, then restart deployments whose image tags have been reused.

Run commands from the repository root in Bash on the cluster host. This procedure
does not use `restart-k8s-rollout.sh`, which targets MicroK8s.

## Prerequisites and cluster selection

The cluster must already exist on the rootful Docker daemon, with the host storage
mounts and node bindings described in [Host storage](HOST_STORAGE.md). For a missing
cluster, follow that guide's setup and recovery instructions before continuing;
recreating a cluster is not part of a pod restart.

The operator needs access to that Docker daemon and the Kubernetes API. The helpers
default to `unix:///var/run/docker.sock`; use `ECONGRAPH_DOCKER_HOST` to select a
different daemon deliberately. For builds, install the pinned kind v0.33.0 with
[`install-kind.sh`](../../scripts/deploy/install-kind.sh). A full deployment also
uses `jq`, Python 3 with PyYAML, OpenSSL and Mike Farah's `yq` v4, and needs network access for
external images and the cert-manager manifest. Review the
[database settings](../../k8s/README.md#databases),
[Secrets configuration](../../k8s/README.md#secrets) and
[public-site configuration](../../k8s/README.md#-public-site-cloudflare-tunnel)
before deploying changes.

Select the cluster's own kubectl and kubeconfig explicitly. The helper below copies
kubectl from the kind control-plane node and obtains kind's kubeconfig, caching both
under `${XDG_CACHE_HOME:-$HOME/.cache}/econgraph`. It leaves `~/.kube/config` alone.
Plain `kubectl` on the host may be a MicroK8s wrapper; a child deployment script's
shell settings do not configure the shell you use afterward.

```bash
export KIND_CLUSTER=econ-graph
export KIND_CLUSTER_NAME=econ-graph
source scripts/deploy/lib/kind-env.sh
check_docker_daemon
use_kind_kubectl
export KUBECTL="${ECONGRAPH_KUBE_DIR}/bin/kubectl"
kind_kube=("$KUBECTL" "--kubeconfig=$KUBECONFIG" --context=kind-econ-graph)
egk() { "${kind_kube[@]}" -n econ-graph "$@"; }

# Verify the API server, cluster identity and access before changing workloads.
"${kind_kube[@]}" config view --minify -o jsonpath='{.clusters[0].cluster.server}{"\n"}'
"${kind_kube[@]}" get namespace kube-system -o jsonpath='{.metadata.uid}{"\n"}'
"${kind_kube[@]}" get nodes -o wide
egk get deployments
egk auth can-i patch deployments.apps
egk auth can-i get pods
```

Confirm that the server, nodes and kube-system UID identify the intended cluster,
and that both permission checks return `yes`. A shared context name alone does not
identify a production cluster. Stop on an access failure or unexpected identity.
Rerun this selection block after a cluster recreation or in a new shell. All
`egk` commands below use this binary, kubeconfig, context and namespace.

## Restart existing application pods

For a restart using the images already loaded into kind, restart only the affected
deployments. For example, to restart the backend and frontend:

```bash
egk rollout restart deployment/econ-graph-backend
egk rollout status deployment/econ-graph-backend --timeout=300s
egk rollout restart deployment/econ-graph-frontend
egk rollout status deployment/econ-graph-frontend --timeout=300s
```

Wait for each rollout to succeed before proceeding. This does not build images or
apply configuration. Backend startup applies app plane migrations embedded in its
image; restarting pods does not run the separate data plane migration Job. See
[Database migrations](../development/MIGRATIONS.md) when changing images or schemas.

## Rebuild and deploy a checkout

Build and load the checkout's images first:

```bash
./scripts/deploy/build-images.sh
```

The builder reads the version from `backend/Cargo.toml` and builds the backend,
crawler worker and frontend as `v<version>` (currently `v4.0.0`). The chart API image
uses `v1.0.0`. It also creates `latest` tags, but loads the versioned tags referenced
by the manifests into kind. These workloads use `imagePullPolicy: Never`, so the
images must be present on the nodes before pods start. Do not override `VERSION`
independently of the manifests. The admin frontend is not built or deployed by this
workflow.

A deployment applies database, Keycloak, application, gateway route and monitoring
resources; it also creates missing Secrets and runs the data plane migration Job
before applying the backend and crawler worker. It is broader than a pod restart.
Before schema changes, use the [backup procedures](BACKUPS.md). Immediately before
deploying, check for running backup or migration Jobs and transactions that could
block migrations. Record the current worker replica count before the precheck so
you can restore it if you abandon the deployment:

```bash
egk get deployment/crawler-worker -o jsonpath='{.spec.replicas}{"\n"}'
./scripts/deploy/precheck-db-migration.sh --stop-crawler
```

The precheck stops the crawler worker after checking for active Jobs, then checks
for long-running or idle transactions. It lists blocking sessions without ending
them. If it fails after scaling the worker down, it prints a command to restore the
previous replica count. Resolve the reported cause before deploying. A successful
precheck leaves the worker stopped and does not print that recovery command. If
you then abandon the deployment before running `deploy.sh`, restore the count you
recorded with `egk scale deployment/crawler-worker --replicas=<previous-count>`.

On a successful deploy, `deploy.sh` restores the worker from its manifest only after
data migrations succeed. If the migration Job fails, deployment stops and the worker
remains stopped. Investigate the failure and establish that the worker's image is
compatible with the resulting database schema before restoring it. If a deployment
fails before reaching migrations and you abandon it, restore the recorded count
only after checking that the failure left the worker safe to run.

Choose **one** deployment command according to the intended public host:

```bash
# Keep the public site on its production cluster (use its actual hostname).
PUBLIC_HOST=econgraph.com ./scripts/deploy/deploy.sh
```

```bash
# Intentionally serve localhost only, removing any existing public host.
PUBLIC_HOST= ./scripts/deploy/deploy.sh
```

Leaving `PUBLIC_HOST` unset is equivalent to empty only when the cluster serves no
public host. If it already serves one, `deploy.sh` stops and requires an explicit
value so it cannot silently remove the public routes. A nonempty value requires
the [production marker](../../k8s/README.md#production-marker). The tunnel connector
starts only on a marked cluster with a public host and tunnel token; unmarked
clusters keep it at zero replicas. Use the existing production hostname unless
intentionally changing the site configuration.

Use `deploy.sh` to apply these resources. Applying all of `k8s/manifests/` directly
also applies `cloudflared.yaml`, whose committed replica count is zero, stopping a
running tunnel connector. Direct application also bypasses public-host rendering.
See [Public site](../../k8s/README.md#-public-site-cloudflare-tunnel) for tunnel setup
and public access checks.

After `deploy.sh` succeeds, restart any deployments whose images were rebuilt under
the same tag: applying an unchanged pod template does not replace running pods.
For a full rebuild, use the four deployments built above:

```bash
for deployment in econ-graph-backend crawler-worker econ-graph-frontend chart-api-service; do
  egk rollout restart "deployment/$deployment" || break
  egk rollout status "deployment/$deployment" --timeout=300s || break
done
```

Stop and investigate any failed rollout before continuing. A successful apply or an
available old replica does not by itself establish that the new image is running.

## Verify the result

Check the workloads and their recent logs:

```bash
egk get deployments,pods,services,httproutes
egk get events --sort-by=.metadata.creationTimestamp
egk logs deployment/econ-graph-backend --tail=100
egk logs deployment/econ-graph-frontend --tail=100
```

The restarted deployments should have all desired replicas updated and available.
Investigate pending pods, image-loading errors, repeated restarts or failed probes;
use `egk describe pod <pod-name>` and the affected deployment's logs. For a rebuild,
also check `crawler-worker` and `chart-api-service` logs. Append `-f` to a log command
to follow it, and use Ctrl-C to stop.

On the cluster host, the Envoy Gateway proxy exposes the frontend at `http://localhost`,
GraphQL at `http://localhost/graphql`, and backend health at `http://localhost/health`.
The checked-in kind port mappings do not expose the services' NodePorts directly.
For remote access, use the configured public HTTPS hostname and the public-site
checks linked above.

```bash
curl --fail --show-error http://localhost/health
curl --fail --show-error http://localhost/graphql \
  -H 'Content-Type: application/json' \
  --data '{"query":"{ __typename }"}'
```

Expect an HTTP success response and health JSON from `/health`, and a `data` object
with `__typename` and no `errors` from GraphQL. These checks establish HTTP routing
and a basic API response. In the browser, open `/explore`, search for a known series,
and open its detail page to confirm that data loads and its chart renders. If sign-in
is configured, confirm that it returns to the application. These smoke checks do not
replace application tests.

For Grafana, forward to a separate local port to avoid confusing it with the app:

```bash
egk port-forward service/grafana-service 13000:3000
```

Open `http://localhost:13000`; credentials are described in
[Secrets configuration](../../k8s/README.md#secrets). The forward runs until Ctrl-C.

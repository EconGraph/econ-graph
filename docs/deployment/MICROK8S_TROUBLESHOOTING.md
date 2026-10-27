# MicroK8s Troubleshooting Guide

## Overview

Use this runbook to diagnose EconGraph on a development MicroK8s cluster. For setup, see the [MicroK8s Deployment Guide](MICROK8S_DEPLOYMENT.md). Run commands from the repository root on the MicroK8s host.

Unless comparing or configuring a separate client, Kubernetes examples use `microk8s kubectl` to target this installation directly, regardless of the context selected by a separate `kubectl` client. Replace values such as `BACKEND_POD` with names discovered in the preceding queries before running the examples.

## Common Issues and Solutions

### 1. Permission Denied Errors

An error such as this usually means the current shell lacks MicroK8s group access:

```text
Error: open servers store: open /var/snap/microk8s/3410/var/kubernetes/backend/cluster.yaml: permission denied
```

Add your user to the group and activate the membership in a new shell:

```bash
sudo usermod -aG microk8s "$USER"
newgrp microk8s
groups
```

Verify that `groups` includes `microk8s` before retrying the failed command. New login sessions also pick up the membership.

### 2. MicroK8s Not Starting

If `microk8s start` hangs or fails, collect status and service logs first:

```bash
microk8s status
sudo journalctl -u snap.microk8s.daemon-kubelite -n 100 --no-pager
sudo microk8s inspect
free -h
df -h
```

`microk8s inspect` writes a diagnostic archive; examine it locally before sharing it. On versions with separate control-plane services, use the service failures reported by inspection to select the corresponding journal units.

Correct the reported cause, such as a full filesystem or exhausted memory. If services remain unhealthy and an interruption to all local workloads is acceptable, restart MicroK8s:

```bash
microk8s stop
microk8s start
microk8s status --wait-ready
microk8s kubectl get nodes
```

A successful restart leaves the node Ready and the existing workloads able to resume. It does not rebuild cluster state. Use [Complete Reset](#complete-reset) only for a deliberate rebuild of a disposable cluster.

### 3. kubectl Context Issues

A refused connection to `127.0.0.1:16443` can mean the API server is stopped; changing contexts alone will not fix it. Compare the clients:

```bash
microk8s status
microk8s kubectl get nodes
kubectl config current-context
```

If the MicroK8s query fails too, return to [startup troubleshooting](#2-microk8s-not-starting). If it succeeds, continue using `microk8s kubectl`. To use a separate client instead, export a dedicated kubeconfig without replacing your existing default file:

```bash
mkdir -p "$HOME/.kube"
umask 077
microk8s config > "$HOME/.kube/econgraph-microk8s.config"
kubectl --kubeconfig="$HOME/.kube/econgraph-microk8s.config" --context=microk8s get nodes
```

That file contains cluster credentials; keep it private. Supply both flags on subsequent separate-client commands, or use the directly targeted client shown throughout this guide.

### 4. Addon Installation Failures

If an addon enable command fails or hangs, establish API readiness and inspect the failed addon before enabling more addons:

```bash
microk8s status --wait-ready
microk8s kubectl get pods -A
microk8s kubectl get events -A --sort-by='.metadata.creationTimestamp'
```

Enable only the missing addons required by your installation, one at a time:

```bash
microk8s enable dns
microk8s enable hostpath-storage
microk8s enable metrics-server
```

Enable the ingress addon only if this installation uses its controller; the repository rollout installs Envoy Gateway separately, as described under [Gateway Not Answering](#10-gateway-not-answering). After each addon command, inspect its pods and events. `status --wait-ready` establishes cluster readiness, not the readiness of every addon. Hostpath storage enables a dynamic provisioner; it does not supply the directories or node placement required by EconGraph's static database PVs. Inspect [storage prerequisites](#storage-and-rebuild-prerequisites) when database claims or mounts fail.

### 5. Image Loading Issues

Process substitution can fail because containerd cannot open the shell's file descriptor:

```text
microk8s ctr images import <(docker save image)
Error: ctr: open /dev/fd/63: no such file or directory
```

Read the pod's requested image before importing; the tag must match exactly, especially for containers with `imagePullPolicy: Never`:

```bash
microk8s kubectl get deployment econ-graph-backend -n econ-graph \
  -o jsonpath='{.spec.template.spec.containers[*].image}{"\n"}'
```

For a deployment requesting `econ-graph-backend:v4.0.0`, either pipe the archive or save it to a file:

```bash
docker save econ-graph-backend:v4.0.0 | microk8s ctr images import -
```

```bash
IMAGE_ARCHIVE=$(mktemp /tmp/econgraph-backend.XXXXXX.tar)
docker save econ-graph-backend:v4.0.0 > "$IMAGE_ARCHIVE"
microk8s ctr images import "$IMAGE_ARCHIVE"
rm "$IMAGE_ARCHIVE"
```

Check `microk8s ctr images ls` and the pod's events afterward. These commands import onto the local node; on a multi-node cluster, the image must also be available on every node that can run the pod.

### 6. PostgreSQL Bus Error

Start with the current and previous container logs and pod events:

```bash
microk8s kubectl logs postgresql-0 -n econ-graph -c postgresql
microk8s kubectl logs postgresql-0 -n econ-graph -c postgresql --previous
microk8s kubectl describe pod postgresql-0 -n econ-graph
```

A bus error alone does not identify a huge-page problem. Establish whether the failure occurs during `initdb` for a new database or while starting an existing one. Missing previous logs simply means there is no retained previous container instance.

For a new database whose initialization failure has been traced to huge-page use, set the initialization option on the named container:

```bash
microk8s kubectl set env statefulset/postgresql -n econ-graph \
  --containers=postgresql POSTGRES_INITDB_ARGS='--set huge_pages=off'
microk8s kubectl rollout status statefulset/postgresql -n econ-graph --timeout=300s
microk8s kubectl logs postgresql-0 -n econ-graph -c postgresql
```

This template change rolls the PostgreSQL pod and interrupts database availability. In PostgreSQL 18, [`initdb --set`](https://www.postgresql.org/docs/18/app-initdb.html) applies the setting during initialization and writes it to the newly generated `postgresql.conf`. The [official image entrypoint](https://github.com/docker-library/postgres/blob/master/18/bookworm/docker-entrypoint.sh) skips initialization when `PGDATA/PG_VERSION` already exists, so changing `POSTGRES_INITDB_ARGS` does not reconfigure an existing database. Do not remove database files to make this option take effect. A partially initialized directory needs investigation before another initialization attempt.

The [huge-page helper](../../scripts/configure-postgresql-no-hugepages.sh) sets this same initialization variable and deletes the pod. It uses the ambient `kubectl` configuration; the directly targeted command above avoids that ambiguity.

For an existing database, investigate the startup error, memory and storage evidence instead. If huge-page use is confirmed as the cause, configure `huge_pages=off` in that database's server configuration and arrange a controlled PostgreSQL restart. This is a server-start setting; see [PostgreSQL memory configuration](https://www.postgresql.org/docs/18/runtime-config-resource.html#GUC-HUGE-PAGES). Successful recovery requires both a Ready pod and logs showing that the server is ready to accept connections, rather than merely a successful patch.

### 7. Backend Pods Stuck in Init:0/1

Identify a backend pod and inspect its init container:

```bash
microk8s kubectl get pods -n econ-graph -l app=econ-graph-backend
# Set BACKEND_POD to one of the returned pod names.
: "${BACKEND_POD:?Set BACKEND_POD first}"
microk8s kubectl logs "$BACKEND_POD" -n econ-graph -c wait-for-postgres
```

Repeated `PostgreSQL is unavailable - sleeping` messages mean the init container cannot open TCP port 5432 at `postgres-service.econ-graph.svc.cluster.local`. It checks TCP reachability, not database authentication or migration state.

```bash
microk8s kubectl get pods -n econ-graph -l app=postgresql -o wide
microk8s kubectl get service postgres-service -n econ-graph -o wide
microk8s kubectl get endpointslice -n econ-graph \
  -l kubernetes.io/service-name=postgres-service -o yaml
microk8s kubectl exec "$BACKEND_POD" -n econ-graph -c wait-for-postgres -- \
  nslookup postgres-service.econ-graph.svc.cluster.local
microk8s kubectl exec "$BACKEND_POD" -n econ-graph -c wait-for-postgres -- \
  nc -z -w 5 postgres-service.econ-graph.svc.cluster.local 5432
```

If there are no ready endpoints, investigate PostgreSQL readiness and a possible Service selector mismatch; inspect database logs and compare Service selectors with pod labels. A failed name lookup points to DNS or DNS egress. If DNS resolves but the TCP probe fails, compare a probe to the ready endpoint's IP with the Service probe to distinguish an endpoint path problem from Service routing:

```bash
: "${POSTGRES_ENDPOINT_IP:?Set the ready endpoint IP from the EndpointSlice}"
microk8s kubectl exec "$BACKEND_POD" -n econ-graph -c wait-for-postgres -- \
  nc -z -w 5 "$POSTGRES_ENDPOINT_IP" 5432
```

Run probes from the waiting container so they exercise the backend pod's network identity. If it has already completed, inspect the backend's new state rather than treating an exec failure as a network failure.

Inspect policies and the labels they select before changing rules:

```bash
microk8s kubectl get namespace econ-graph --show-labels
microk8s kubectl get pods -n econ-graph --show-labels
microk8s kubectl get networkpolicy -n econ-graph
microk8s kubectl describe networkpolicy -n econ-graph
```

The connection needs backend egress and PostgreSQL ingress on TCP 5432, plus DNS on UDP/TCP 53. The repository policies use namespace and pod selectors, so compare their actual labels as well as their ports. Correct the specific mismatched selector or missing allowance in the policy source and apply that reviewed change; deleting all policies is not a connectivity test. Repeat the DNS and TCP probes after the repair. The wait loop retries automatically; verify that the init container completes and the backend becomes Ready:

```bash
microk8s kubectl rollout status deployment/econ-graph-backend -n econ-graph --timeout=300s
```

### 8. TLS Certificate Issues

First identify which connection is failing. For the configured [public site](../../k8s/README.md#-public-site-cloudflare-tunnel), visitor TLS terminates at Cloudflare; cloudflared then connects over HTTPS to the Gateway origin. Visitors see Cloudflare's certificate. The default origin certificate is self-signed and the tunnel configuration disables origin certificate verification. These are separate TLS connections: a visitor certificate failure does not establish a cert-manager failure in the cluster. Follow [Gateway Not Answering](#10-gateway-not-answering) to separate local routing from the public tunnel.

For the repository Gateway, inspect the HTTPS listener's certificate reference and find the Certificate whose `spec.secretName` matches it. The public listener, Certificate `econ-graph-tls`, and its Secret are in `econ-graph-gateway`, rather than the application namespace:

```bash
microk8s kubectl get gateway econ-graph-gateway -n econ-graph-gateway -o yaml
microk8s kubectl get certificate -n econ-graph-gateway
microk8s kubectl describe certificate econ-graph-tls -n econ-graph-gateway
microk8s kubectl get pods -n cert-manager
microk8s kubectl logs deployment/cert-manager -n cert-manager --tail=100
```

A local installation without a public HTTPS listener need not have this Certificate. For an addon or manual controller, discover its Gateway or Ingress TLS reference and owning Certificate in the actual namespace instead. Inspect references and conditions without printing Secret contents.

Use Certificate conditions and issuance events to diagnose the issuer. ACME Orders and Challenges matter only when the selected issuer uses ACME; the default self-signed issuer has no ACME challenge to repair. If logs identify an unhealthy cert-manager controller, correct its cause before arranging a restart:

```bash
microk8s kubectl rollout restart deployment/cert-manager -n cert-manager
microk8s kubectl rollout status deployment/cert-manager -n cert-manager --timeout=180s
```

Recheck the Certificate's Ready condition, Gateway listener conditions, and the affected HTTPS connection. The route tester disables certificate verification, so its success does not establish certificate trust. An installation without cert-manager needs the certificate configuration of its actual controller or public edge.

### 9. Memory Issues

For a Pending pod, inspect the scheduling event before changing resource settings:

```bash
: "${POD:?Set POD to the Pending pod name}"
microk8s kubectl describe pod "$POD" -n econ-graph
microk8s kubectl describe nodes
microk8s kubectl top nodes
microk8s kubectl top pods -A
```

`Insufficient memory` means the scheduler cannot fit the pod's memory request on an eligible node. Compare node allocatable memory and allocated requests, including other namespaces. Usage metrics complement those figures; low usage does not make already requested capacity available. Metrics require a working metrics-server addon.

Add capacity, reduce replicas of an optional workload you own, or adjust requests when measurements support the smaller reservation. Each choice affects capacity or availability; identify the workload before scaling it. Increasing backend replicas consumes more capacity and cannot fix this scheduling shortage. An `OOMKilled` container is a different case: compare its memory limit and observed usage before changing the limit or application workload. Recheck the pod events and readiness after the change.

### 10. Gateway Not Answering

Discover the routing installation before testing an address. The [MicroK8s ingress addon](https://canonical.com/microk8s/docs/ingress) uses Traefik starting with 1.35; older versions use NGINX. Addon and manually installed controllers can coexist with the repository's Envoy Gateway, but they do not serve its GatewayClass automatically. The MicroK8s rollout script calls `install-gateway.sh --context "$CLUSTER_ROLE_CONTEXT"` and applies the namespace's routes; a manual installation may have a different stack.

```bash
microk8s status
microk8s kubectl get gatewayclass
microk8s kubectl get gateway,httproute -A
microk8s kubectl get ingressclass
microk8s kubectl get ingress -A -o wide
microk8s kubectl get deployments,daemonsets,pods,services -A -o wide
```

If the API reports an unknown Gateway resource type, check whether Gateway API CRDs were installed. Otherwise match the Gateway's class to its `controllerName`, or the Ingress's class to its controller. Use the discovered controller namespace, workload, and logs for that installation. An accepted object alone does not establish that its intended controller is serving traffic.

For the repository Envoy installation, inspect the controller, proxy, Gateway and routes:

```bash
microk8s kubectl get pods,services -n envoy-gateway-system -o wide
microk8s kubectl logs deployment/envoy-gateway -n envoy-gateway-system --tail=100
microk8s kubectl describe gatewayclass econ-graph
microk8s kubectl describe gateway econ-graph-gateway -n econ-graph-gateway
microk8s kubectl describe httproute -n econ-graph
microk8s kubectl get nodes -l ingress-ready=true -o wide
microk8s kubectl get namespace econ-graph --show-labels
```

Check GatewayClass acceptance, Gateway programming and listener conditions, and each route's `Accepted` and `ResolvedRefs` conditions. The HTTP listener accepts namespaces labelled `econgraph.io/gateway-access=true`; the public HTTPS listener requires `econgraph.io/env=production`. A missing Service or port can prevent reference resolution. Compare the actual namespace labels and route parent/listener references with those requirements.

The proxy's node selector requires `ingress-ready=true`. If no eligible node has that label, choose a node with suitable capacity and network access before labelling it; the installer does not create this placement label. Inspect Pending proxy events for other scheduling constraints. In its default auto mode, the installer omits proxy hostPorts when another pod holds port 80 or 443 on an eligible node, including a MicroK8s addon controller. That leaves Envoy reachable through its ClusterIP Service while the other controller still answers the host ports. Inspect the proxy pod's actual ports rather than assuming `localhost:80` reaches Envoy.

When Envoy has a reachable host listener and the route matches `localhost`, test that listener explicitly:

```bash
curl -v -H 'Host: localhost' http://127.0.0.1/
ROUTE_HOST=localhost ./scripts/deploy/test-gateway-routes.sh http://127.0.0.1
```

When host ports are unavailable, target Envoy's Service directly. Keep this port-forward running in one terminal:

```bash
microk8s kubectl -n envoy-gateway-system port-forward \
  svc/econ-graph-gateway-proxy 18080:80
```

Then test from another terminal:

```bash
curl -v -H 'Host: localhost' http://127.0.0.1:18080/
ROUTE_HOST=localhost ./scripts/deploy/test-gateway-routes.sh http://127.0.0.1:18080
```

The script defaults to base URL `http://localhost` and Host `localhost`. Select both for the actual listener and route; otherwise a test can reach the addon controller or match no route. Set `EXPECTED_ISSUER` to the configured Keycloak issuer when it differs from the script's local default; use `SKIP_KEYCLOAK=true` only when Keycloak is intentionally absent. The default checks send HTTP requests, including GraphQL queries and a deliberately invalid bearer token. The optional `--rate-limit` check also sends request bursts and temporarily consumes the client's limit. See [Gateway routing troubleshooting](GATEWAY_ROUTING_TROUBLESHOOTING.md) for route expectations and proxy diagnostics.

A refusal or timeout points to the listener or network path; an Envoy 404 can indicate a host or path mismatch. For an upstream failure, inspect the proxy logs, selected Service, ready EndpointSlices, workload logs and NetworkPolicy allowances. Discover the proxy pod before reading its log:

```bash
microk8s kubectl get pods -n envoy-gateway-system \
  -l gateway.envoyproxy.io/owning-gateway-name=econ-graph-gateway
: "${PROXY_POD:?Set PROXY_POD to the returned proxy pod name}"
microk8s kubectl logs "$PROXY_POD" -n envoy-gateway-system -c envoy --tail=100
```

Test the public host only if this cluster is intended to serve it. When local routing works but the public route fails, inspect the public HTTPRoutes, HTTPS listener, and tunnel:

```bash
microk8s kubectl get httproute -n econ-graph
microk8s kubectl describe gateway econ-graph-gateway -n econ-graph-gateway
microk8s kubectl get pods -n econ-graph -l app=cloudflared
microk8s kubectl logs deployment/cloudflared -n econ-graph --tail=100
```

Use the actual public hostname in the HTTPS request; `curl -v https://econgraph.com/` applies only to the installation serving that domain. Separate public DNS/edge TLS failures from tunnel connection errors and Gateway upstream errors. The remotely managed tunnel origin must reach `https://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443`; a working host listener does not prove that path works from cloudflared. Public routes require the production namespace label, and deployment scripts restrict cloudflared to a cluster marked production. Preserve those safeguards when repairing routing. See the [public-site configuration](../../k8s/README.md#-public-site-cloudflare-tunnel) for the request path and host settings.

### 11. Pod Limit Exceeded

A scheduling event reporting `Too many pods` concerns the eligible node's pod capacity. Inspect its allocatable pod count and assigned pods before raising the limit:

```bash
microk8s kubectl describe nodes
microk8s kubectl get pods -A -o wide
```

Remove completed disposable workloads or add capacity where appropriate. If the node has enough memory, CPU and network address capacity for a higher pod count, a single-node development installation can change kubelet's limit during a maintenance interruption:

```bash
microk8s stop
sudo nano /var/snap/microk8s/current/args/kubelet
# Add or modify --max-pods to the capacity you have established.
microk8s start
microk8s status --wait-ready
microk8s kubectl describe nodes
```

Verify the new allocatable count and the Pending pod's events; raising the count does not remove other scheduling constraints.

## Debugging Commands

### System Information

```bash
microk8s status
free -h
df -h
microk8s ctr images ls
```

If local image builds use Docker, `docker ps` and `docker images` describe the build engine; MicroK8s runs workloads in its own containerd.

### Kubernetes Information

```bash
microk8s kubectl cluster-info
microk8s kubectl get nodes
microk8s kubectl top nodes
microk8s kubectl get all -n econ-graph
microk8s kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
```

### Service Information

```bash
microk8s kubectl get services -n econ-graph
microk8s kubectl get httproute -n econ-graph
microk8s kubectl logs -f deployment/econ-graph-backend -n econ-graph
microk8s kubectl logs -f deployment/econ-graph-frontend -n econ-graph
```

## Performance Optimization

Use `microk8s kubectl top pods -n econ-graph` and `microk8s kubectl top nodes` alongside the requests, limits and scheduling events described under [Memory Issues](#9-memory-issues). Scale the backend up only when it needs more serving capacity and eligible nodes have room for the additional requests. Reduce optional workload replicas only when their lost capacity is acceptable. Keep lasting resource changes in the deployment configuration so a later rollout does not undo them.

## Recovery Procedures

### Application Restart

For an unhealthy application process after its underlying dependency has recovered, restart the affected deployment and wait for the rollout:

```bash
microk8s kubectl rollout restart deployment/econ-graph-backend -n econ-graph
microk8s kubectl rollout status deployment/econ-graph-backend -n econ-graph --timeout=300s
```

Use the corresponding named deployment for a frontend failure. This replaces application pods while retaining database StatefulSets, PVCs, PVs and namespace configuration. A restart uses the existing image and configuration; it does not repair a missing image, database, or invalid manifest. Recheck the original symptom and application logs after readiness returns.

For a full application redeploy, follow the [MicroK8s deployment procedure](MICROK8S_DEPLOYMENT.md), including its rollout script, after verifying the intended manifests and storage prerequisites. The script builds and imports images, reapplies application manifests and Gateway configuration, restarts multiple workloads, and rewrites `~/.kube/config`; preserve any existing kubeconfig before invoking it. Do not delete the entire manifests directory as a restart: it includes the namespace and cluster-scoped storage resources.

### Storage and Rebuild Prerequisites

Before rebuilding a cluster that contains data to keep, establish recoverable database backups, the matching configuration and externally managed Secrets, the intended images, and a restoration procedure for that installation. Record PVC-to-PV mappings and the underlying host paths. Inspect them without exposing Secret contents:

```bash
microk8s kubectl get pvc -n econ-graph -o wide
microk8s kubectl get pv -o yaml
microk8s kubectl get nodes --show-labels
```

The repository's static database PVs use the `econgraph-host` storage class, prebound claims, `Retain`, existing `Directory` host paths, and node affinity requiring `econgraph.io/host-storage=true`. Retained files are distinct from Kubernetes objects and usable PVC bindings. Verify that the directories, ownership, mounts and node affinity fit the MicroK8s host before applying those manifests. Enabling hostpath-storage alone does not satisfy them. The [host-storage layout](HOST_STORAGE.md#layout) describes the paths and its kind-specific mount arrangement; do not copy the kind node-container recovery commands onto a MicroK8s host. See [released PVs](../../k8s/README.md#released-persistentvolumes) for binding considerations. File retention is not a substitute for a verified backup and restore path.

### Complete Reset

Use this only when intentionally discarding and rebuilding a single-node development cluster after diagnosis and ordinary restart have failed. [MicroK8s reset](https://microk8s.io/docs/command-reference#microk8s-reset) removes cluster resources and authentication configuration and disables addons. It affects every namespace, not just EconGraph. The optional `--destroy-storage` also deletes storage-addon files; it is deliberately absent below. Its absence does not guarantee that a recreated database will bind to the right retained data.

Complete the [rebuild prerequisites](#storage-and-rebuild-prerequisites) first if any data or configuration must survive. Run reset while MicroK8s is running, then restore only the addons your installation needs:

```bash
microk8s reset
microk8s start
microk8s status --wait-ready
microk8s enable dns
microk8s enable hostpath-storage
microk8s enable metrics-server
microk8s kubectl get nodes
microk8s kubectl get pods -A
```

At this point only the cluster and addons have been rebuilt. Restore the installation's configuration, storage bindings and databases using its recovery procedure, then redeploy the application and its selected routing controller. Re-establish the storage and proxy node labels on the intended nodes; addon activation does not recreate them. Verify database readiness and expected data, application rollouts, and the intended local or public route. Re-enabling addons or reapplying manifests alone does not establish data recovery.

## Maintenance and Getting Help

Before changing the snap channel or refreshing MicroK8s, check the release's addon behavior and preserve the recovery materials above. Ingress controller changes can affect routing even when the Kubernetes objects are accepted. Track disk usage in both the image store and database filesystem before cleaning up images or volumes.

For help, collect the relevant pod events and logs from the symptom section, `sudo microk8s inspect`, and the service journal shown under [startup troubleshooting](#2-microk8s-not-starting). Review diagnostic output for sensitive configuration before sharing it.

- [MicroK8s Documentation](https://microk8s.io/docs)
- [Kubernetes Troubleshooting](https://kubernetes.io/docs/tasks/debug-application-cluster/)
- [EconGraph Documentation](../README.md)
- [MicroK8s Deployment Guide](MICROK8S_DEPLOYMENT.md)
- [Kubernetes Deployment Guide](KUBERNETES_DEPLOYMENT.md)

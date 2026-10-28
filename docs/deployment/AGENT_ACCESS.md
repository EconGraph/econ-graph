# Restricted Kubernetes sessions for coding agents

Agent tools use a short-lived ServiceAccount kubeconfig through
`scripts/deploy/run-agent-session.sh`. The identity can manage workloads in one
namespace, delete (restart) pods and read pod logs, but its Role grants no access
to Secrets, RBAC, ServiceAccount tokens, bare pod creation or changes, pod exec,
attach or port-forward, service proxying, nodes, namespaces or cluster-scoped
resources.
Use this launcher as the default entry point for every agent session that needs
Kubernetes access. Existing sessions keep their credentials until restarted.

This reduces accidental direct access to Secrets; it is **not a confidentiality
boundary**. An identity that can deploy arbitrary workloads can mount Secrets or
run a pod under another ServiceAccount. It can also expose a database through a
NodePort Service, loosen NetworkPolicies, and delete PersistentVolumeClaims and
StatefulSets, so it does not protect data either. Logs and ConfigMaps can also contain
sensitive data. A process running as the operator's Unix user can read that user's
admin kubeconfig and plaintext backups. Access to the rootful Docker socket can
recover kind's admin credentials or mount host files. Environment flags and script
guards do not prevent those bypasses.

A true boundary requires a separate Unix account with no operator files or Docker
socket access, namespace separation, and admission policy restricting workloads,
ServiceAccounts, volumes and security contexts. Those controls must precede any
claim that agents cannot obtain production secrets. Until then, grant deployment
access only to trusted tools, preferably in a disposable branch namespace.

## Bootstrap and launch

Run the bootstrap yourself in an operator terminal. It chooses the kubectl and
operator kubeconfig as the other operator scripts do (`cluster_role_default_kubectl`
in `lib/cluster-role.sh`): `--kubectl` and `--operator-kubeconfig`, else `KUBECTL`
and `KUBECONFIG`, else the kind cluster's kubectl and kubeconfig kept in
`~/.cache/econgraph`. It never uses plain `kubectl` from `PATH`, and refuses a
MicroK8s kubectl: on this host the snap wrapper named kubectl reaches MicroK8s
and ignores the kubeconfig. It works on context `kind-econ-graph` (`kind-<cluster>`
with `--cluster`) of that kubeconfig, prints that cluster's identity before
writing, and leaves the kubeconfig's current context unchanged. The namespace
must already exist.

```bash
scripts/deploy/create-agent-kubeconfig.sh --dry-run
scripts/deploy/create-agent-kubeconfig.sh
scripts/deploy/run-agent-session.sh -- bash
```

Replace `bash` with the coding agent's command. Bootstrap is idempotent and writes
only namespaced RBAC and a private local kubeconfig. Before publishing the config,
it uses the issued token to check pod and pod-log access and reject direct Secret,
ServiceAccount token, bare pod creation, pod exec/attach/port-forward, ephemeral
container, service and node proxy, impersonation, RBAC write and RBAC escalation
grants across existing namespaces. An operator must be able to list namespaces for
that check. A failed check leaves no new kubeconfig; inspect existing bindings
before retrying. It requests a token lifetime of one hour and refuses a token the
API server issued for longer. Re-run bootstrap to renew it. `--namespace`,
`--cluster`, and `--output-dir` allow an existing scratch namespace/cluster; pass
the matching `--cluster` and `--config-dir` to the launcher. The output directory
is made 0700 and the kubeconfig 0600, and an output directory inside a git work
tree is refused. Never attach or paste the kubeconfig in an issue, terminal output
or chat.

The launcher replaces inherited kubectl/config selections and marks the session.
The shared kind helper preserves that identity and fails when it cannot use the
restricted config; it never falls back to `kind get kubeconfig` in such a session.
The Role also omits Gateway API resources and Envoy policies; installing the
gateway, applying its routes and the live cutover remain operator work.
Full deployment scripts also manage namespaces, Secrets and cluster components,
so they still require an operator. Agents can apply individual permitted workload
manifests; they cannot run the full provisioning workflow with this Role.

`create-secrets.sh` and `set-crawler-keys.sh` refuse before reading or prompting
for credentials when the session marker or configured agent identity is present.
Run these yourself outside the agent session using an operator-only kubeconfig.
For conservative detection, any explicitly selected config containing an
`econgraph-agent` entry is treated as restricted, including an unused entry. Any
future SOPS apply entry point must source `lib/agent-access.sh` and call
`require_operator_session` before decrypting or selecting admin credentials.

## Verify and revoke

From the launched restricted session, direct Secret access must be denied:

```bash
kubectl auth can-i get pods -n econ-graph
kubectl auth can-i get secrets -n econ-graph
kubectl auth can-i list secrets -n econ-graph
kubectl auth can-i watch secrets -n econ-graph
kubectl auth can-i get secrets -n kube-system
```

Expect `yes` only for pods. Broader existing RoleBindings can grant additional
permissions, so verify the effective identity after installation and whenever
bindings or namespaces change. Repeat the negative checks in every namespace,
including `auth can-i impersonate users`, `auth can-i create pods`,
`auth can-i create serviceaccounts --subresource=token`,
`auth can-i create pods --subresource=exec` (and `attach`, `portforward`),
`auth can-i bind roles` and `auth can-i escalate roles`. Name subresources with
`--subresource`: `auth can-i create pods/exec` asks about a pod named `exec`.
Resource-name-specific grants and future binding changes still require an operator
RBAC audit; these probes do not prove a confidentiality boundary. Remove the
`econgraph-agent` RoleBinding and ServiceAccount in the selected namespace to
revoke access; deleting the kubeconfig alone does not revoke a copied token.

## Existing files that need operator review

No secret contents are needed for this review. Existing host backups may contain
`~/econgraph-backups/*/secrets/`, plaintext database dumps (including Keycloak
client secrets), and Terraform state or state backups. age identity files are
private keys too; only age public recipients belong in backup configuration.
Check ownership, mode and parent-directory traversal permissions yourself, using
file metadata only. Keep credential directories 0700 and files 0600, account for
ACLs, and encrypt backup contents before sending them off-site. Restrictive modes
protect against other Unix users, but do not isolate tools running as your UID.

Do not delete existing backups as part of cleanup. The backup encryption work
covers encrypted writers/restores and manual migration of historical copies.
Before moving agents to a separate Unix user, inventory these locations and the
admin kubeconfig, Terraform state, age identities, Docker group membership and
supplementary groups. Verify the new user cannot traverse or read them, and cannot
obtain equivalent access via sudo, Docker, hostPath pods or Kubernetes RBAC.

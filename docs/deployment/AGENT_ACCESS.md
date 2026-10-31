# Read-only Kubernetes sessions for coding agents

Agent tools use a short-lived, read-only ServiceAccount kubeconfig through
`scripts/deploy/run-agent-session.sh`. In one namespace, its Role grants `get`,
`list` and `watch` on pods, pod logs, Services, Endpoints, ConfigMaps, Events,
PersistentVolumeClaims, Deployments, ReplicaSets, StatefulSets, DaemonSets, Jobs,
CronJobs, NetworkPolicies, Ingresses and Gateway API HTTPRoutes, and nothing else:
no Secrets, no `create`, `update`, `patch`, `delete` or `deletecollection` on
anything, no subresource other than pod logs (so no exec, attach, port-forward or
proxy), and nothing cluster-scoped. Use this launcher as the default entry point for
every agent session that needs Kubernetes access. Existing sessions keep their
credentials until restarted.

The identity is read-only because any write to workloads defeats the boundary: a
pod can mount a ServiceAccount token and print it to its log, a privileged pod can
reach cluster-admin where Pod Security does not forbid it, and patching an
`ownerReferences` or a StatefulSet's volume-claim retention policy makes the
garbage collector delete data. Restarts, rollouts and every other change are run
by an operator with the operator scripts.

What a read-only identity still exposes: application pod logs, ConfigMaps, and
literal `env` values and container `command`/`args` in pod and workload specs.
Keep credentials in Secrets, not in those, and do not log them. It can also open
`watch` and log-follow (`kubectl logs -f`) streams, which are unbounded and load
the API server and kubelet; the one-hour token limits how long new ones can be
opened. The identity does not isolate the operator's own
files: a process running as the operator's Unix user can read that user's admin
kubeconfig and plaintext backups, and access to the rootful Docker socket can
recover kind's admin credentials or mount host files. Environment flags and script
guards do not prevent those bypasses; isolating them requires running agents as a
separate Unix account with no operator files and no Docker socket access.

## Bootstrap and launch

Run the bootstrap yourself in an operator terminal. It chooses the kubectl and
operator kubeconfig as the other operator scripts do (`cluster_role_default_kubectl`
in `lib/cluster-role.sh`): `--kubectl` and `--operator-kubeconfig`, else `KUBECTL`
and `KUBECONFIG`, else the kind cluster's kubectl and kubeconfig kept in
`~/.cache/econgraph`. It never uses plain `kubectl` from `PATH`, and refuses a
MicroK8s kubectl: on this host `/snap/bin/kubectl` -> `microk8s.kubectl` ->
`/usr/bin/snap` -> `../lib/snapd/snapd`, which reaches MicroK8s and ignores the
kubeconfig. Every link of the kubectl's symlink chain is checked: a path under
`/snap/bin/`, or a name starting with `microk8s` or equal to `snap` or `snapd`, is
refused. So is a binary inside `/snap` or one that a `/snap/bin` entry resolves
to, and a script (rather than an ELF binary) that mentions snap or MicroK8s. It works on context `kind-econ-graph` (`kind-<cluster>` with `--cluster`)
of that kubeconfig, prints that cluster's identity before writing, and leaves the
kubeconfig's current context unchanged. The namespace must already exist.

```bash
scripts/deploy/create-agent-kubeconfig.sh --dry-run
scripts/deploy/create-agent-kubeconfig.sh
scripts/deploy/run-agent-session.sh -- bash
```

Replace `bash` with the coding agent's command. Bootstrap is idempotent and writes
only namespaced RBAC and a private local kubeconfig. `--namespace`, `--cluster`,
and `--output-dir` allow an existing scratch namespace/cluster; pass the matching
`--cluster` and `--config-dir` to the launcher.

Before publishing the kubeconfig, bootstrap asks the API server, with the issued
token, for everything that token may do in every namespace (a
SelfSubjectRulesReview, the request behind `kubectl auth can-i --list`; grants
from ClusterRoleBindings appear in every namespace's answer). It refuses, listing
each extra permission, when the token can do anything beyond the Role's reads in
its namespace and the discovery, health and self-review requests every
authenticated identity may make, or when the API server cannot list every
permission. An existing RoleBinding or ClusterRoleBinding that grants the
ServiceAccount more therefore stops bootstrap; inspect those bindings before
retrying. It also checks that the token can read pods and pod logs. An operator
must be able to list namespaces for these checks.

The token is requested for one hour, and a token the API server issues for more
than 65 minutes is refused. Re-run bootstrap to renew it. The kubeconfig is written
0600, with a private 0700 copy of the kubectl in `bin/`, in a 0700 directory you
own; none of them may be a symlink or carry an extended or default POSIX ACL.
Bootstrap refuses an output directory that is a symlink, is not yours, or is
inside a git work tree. It also refuses when any directory on the way to it can be
changed by another user: writable by others without the sticky bit, by a group
other than your own private group, or by another user or group through an ACL.
Your private group is your primary group when it has your user name, no group
entry with its GID lists another member, and it is no other user's primary group;
with LDAP or SSSD and enumeration off, only local members and users are seen. An
ACL that cannot be read, for a reason other than there being none or the
filesystem not supporting them, is refused. Only POSIX ACLs are read: keep the
output directory off filesystems with NFSv4 ACLs (NFSv4 mounts, ZFS with
`acltype=nfsv4`) or CIFS ACLs.
That walk follows symlinks one component at a time, so the directory holding a
symlink is checked as well as where it points. Inside a sticky directory such as
`/tmp`, a symlink must be yours or root's. Never attach or paste the
kubeconfig in an issue, terminal output or chat.

The launcher runs nothing from the directory unless these checks still pass. It
replaces inherited kubectl/config selections and marks the session.
The shared kind helper preserves that identity and fails when it cannot use the
restricted config; it never falls back to `kind get kubeconfig` in such a session.
Agents can inspect the namespace; deployments, restarts and the provisioning
workflow are operator work.

`create-secrets.sh` and `set-crawler-keys.sh` refuse before reading or prompting
for credentials when the session marker or configured agent identity is present.
Run these yourself outside the agent session using an operator-only kubeconfig.
For conservative detection, any explicitly selected config containing an
`econgraph-agent` entry is treated as restricted, including an unused entry. Any
future SOPS apply entry point must source `lib/agent-access.sh` and call
`require_operator_session` before decrypting or selecting admin credentials.

## Verify and revoke

From the launched session, list what the identity may do:

```bash
kubectl auth can-i --list -n econ-graph
kubectl auth can-i --list -n kube-system
```

Expect only `get`, `list` and `watch` on the resources above in `econ-graph`, and
in every namespace only the self-review `create` rules and `get` on discovery and
health URLs. Bindings can change after bootstrap, so repeat this whenever bindings
or namespaces change, or re-run bootstrap, which refuses on any difference. Remove
the `econgraph-agent` RoleBinding and ServiceAccount in the selected namespace to
revoke access; deleting the kubeconfig alone does not revoke a copied token before
it expires.

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

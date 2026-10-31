# Talos/KVM infrastructure and fit tooling

This is the feature-branch implementation of [ECO-436](https://linear.app/econgraph/issue/ECO-436).
It prepares a separately named Talos candidate on the existing host. Nothing in
this directory migrates production, retires kind, changes a production tunnel,
touches host object-store data, or accepts the platform on the strength of a
mocked test. All commands below default to dry run unless stated otherwise.

## Pinned interfaces

| Component | Pin | Purpose |
|---|---|---|
| OpenTofu | 1.13.2 | VM/Talos state and plan encryption |
| libvirt provider | 0.9.9 | Actual installed schema validated; no legacy 0.8 fields |
| Talos provider | 0.12.0 | Talos 1.14 configuration contract |
| Talos | 1.14.5 | Current verified stable 1.14 release |
| Kubernetes | 1.36.4 | Shared supported minor with Cilium, CNPG and Envoy |

Talos 1.14 supports Kubernetes 1.36; its newer bundled Kubernetes default is
overridden deliberately. The platform lane supplies Cilium before Flux pods
can run, then Flux reconciles that same release. Talos disables Flannel and
kube-proxy and enables KubePrism on `localhost:7445`. The public `inventory`
output includes the API endpoint and pod/service CIDRs; these are the interface
for the platform overlays, not a second infrastructure manager for add-ons.

Three control planes each have 4 vCPU/8 GiB and a 40 GiB OS disk. Two or three
workers each have 16 vCPU/64 GiB, a 40 GiB OS disk and a separate PostgreSQL
data disk. The default two-worker configuration requests **44 vCPU, 152 GiB
guest RAM and 600 GiB virtual disk capacity**, before the ISO and host services.
Three workers request 60 vCPU, 216 GiB RAM and 840 GiB virtual disk capacity.
Sparse allocation is not a capacity reservation: measure actual available
memory, host/object-store headroom and filesystem space before authorizing a
build. Live allocation, pressure and cleanup remain fit-test evidence.

Workers expose `/dev/vdb` as an XFS `UserVolumeConfig` named `postgres`, mounted
at `/var/mnt/postgres` and shared into kubelet with propagation. Workers carry
`econgraph.io/storage=postgres`; the platform lane's `econgraph-local` provisioner
uses that path and Retain volumes. Different worker zone labels are scheduling
domains on **one physical host**, not independent physical failure domains.

The Image Factory schematic is fixed at
`47f718feafaf5934d42735b54da7805fa768627393a4318a0c2d47fd5b1ecb79`.
[schematic.json](schematic.json) requests `siderolabs/btrfs`,
`siderolabs/qemu-guest-agent` and `siderolabs/util-linux-tools`.
[images.lock.json](images.lock.json) records the verified release-specific
extension references/digests and source URLs. The ISO and installer both use
that schematic and Talos patch version. Do not substitute `latest`.

## Prepare a public inventory

Copy [example.cluster.json](example.cluster.json) to an operator-controlled
location. Give it a unique `econgraph-*` name, a fresh UUID owner token, reserved
LAN addresses, unique MACs and an unused dedicated physical uplink. The example
uses documentation addresses and **cannot be executed unchanged**.

Reserve every configured node MAC/IP pair on the LAN DHCP server before setting
`dhcp_reservations_confirmed` to true. The ISO boots into maintenance mode using
DHCP; the Talos provider must reach those exact reserved addresses for initial
configuration. The API VIP must be reserved outside the DHCP allocation range.
The bridge/host address, API VIP and node addresses must be distinct. Pod and
service networks must not overlap the LAN or one another.

Host preparation supports an **unused dedicated uplink**. It refuses a NIC
carrying a global address or IPv4/IPv6 default route, attached to another bridge,
or an existing unowned bridge. A LAN already connected through a different host
interface is refused to avoid duplicate routes and ARP ambiguity. It never moves the management IP/default route from a
live NIC. A single-NIC host needs a separately reviewed, console-recoverable
network migration before this script can be used. Do not run an ad-hoc bridge
command over the only SSH route.

Runtime and host ownership directories must be absolute, outside the checkout,
and free of symlinks. New directories are claimed with the public inventory's
owner token; existing directories/resources without that token are refused.
The VM subvolume is a new child under `disk_parent` on an existing btrfs
filesystem. It receives nodatacow before any images are allocated. Existing VM
directories are never recursively converted to nodatacow. Object-store and
backup subvolumes are separate and are never deleted by these scripts.

## Inspect the host preparation plan

```sh
python3 scripts/talos/host.py --config /path/to/candidate.json \
  --state-dir /var/lib/econgraph/talos/host-candidate
```

An operator-authorized host execution additionally requires `sudo`, `--execute`
and `--confirm-owner UUID`. The script only configures the explicitly owned
bridge, unused uplink, and new VM subvolume. Bridge configuration uses iproute2
and must be reapplied after a host reboot; install a reviewed boot invocation
under the host's existing network manager if persistence is needed. This helper
does not replace or restart the host's network manager.

## Protect secrets and state

Use the pinned CLI versions and provide SOPS/age locally. No new cloud account,
external secrets service or remote backend is required. First inspect:

```sh
python3 scripts/talos/cluster.py secrets --config /path/to/candidate.json \
  --state-dir /path/outside/git/candidate --age age1PUBLIC_RECIPIENT
```

The execution identity must be able to access the system libvirt socket, the
root-owned host ownership record and its own SOPS identity. Use a deliberately
selected privileged operator identity for execution; the examples omit `sudo`
because dry runs require no privilege. Public download URLs and SHA-256 pins
for the two CLIs are in [toolchain.lock.json](toolchain.lock.json).

Explicit execution generates the Talos secrets in captured process memory and
immediately encrypts them to the supplied age recipients. It also generates a
separate random state key, stored as SOPS ciphertext. Existing secret archives
are retained; an incomplete archive fails closed. Keep off-host/operator
recovery copies according to the existing backup policy; this tooling never
expires, rotates away or deletes backups.

OpenTofu state **and saved plans** require native AES-GCM encryption with that
SOPS-protected key and have no plaintext fallback. Sensitive Talos provider
values do exist in the encrypted state; `sensitive=true` alone would not protect
them. The CLI supplies the state key through a scoped subprocess environment.
`TF_LOG_PATH` is refused to avoid writing plaintext provider logs. Treat the
operator's process/memory access as privileged; encryption at rest is not
protection against an administrator reading a running process.

Before an apply, the original SOPS Talos secrets are imported into the encrypted
state through a Linux anonymous memory file. There is no plaintext disk-file
fallback. Exported `talosconfig`/`kubeconfig` are also SOPS ciphertext; lifecycle
operations decrypt them only into anonymous memory files. The runtime ownership
marker, public inventory and ordinary `.tf` files contain no credentials.

## Build and first-week fit commands

Inspect the build plan, then use the platform lane's bootstrap instructions to
install its pinned Cilium release before waiting for Kubernetes readiness:

```sh
python3 scripts/talos/cluster.py build --config /path/to/candidate.json \
  --state-dir /path/outside/git/candidate \
  --host-state-dir /var/lib/econgraph/talos/host-candidate --age age1PUBLIC_RECIPIENT
```

Explicit execution copies only this pinned module and provider lock into the
owned runtime directory, initializes with `-lockfile=readonly`, imports the
preserved CA/secrets, provisions the VMs, applies their Talos configurations,
bootstraps etcd once and exports encrypted clients. The module has no Helm,
Kubernetes, Flux, CloudNativePG or host-object-store resources.

Each lifecycle step is a separate subcommand of the checked-in script. All
support the same config/runtime arguments and dry-run default:

| Subcommand | Required additional inputs | What execution proves |
|---|---|---|
| `status` | None | Every explicitly named candidate node becomes Ready. |
| `rebuild` | `--host-state-dir`, `--age`, `--destroy-disposable CLUSTER` | Destroys only a `purpose: fit` module after checking VM/pool/volume ownership, then rebuilds using the preserved encrypted secrets. |
| `upgrade-talos` | `--image factory.talos.dev/metal-installer/SCHEMATIC:v1.14.PATCH` | Upgrades one inventory node, waits for its reboot and readiness, then continues. |
| `upgrade-kubernetes` | `--kubernetes-version 1.36.PATCH` | Uses Talos's sequenced `upgrade-k8s` procedure and checks readiness before and after. |
| `kill-worker` | `--node worker-N --namespace NS --deployment NAME` | Stops one owned fit VM and observes a healthy replicated target regain ready replicas on other workers within ten minutes. |
| `restart-worker` | `--node worker-N` | Starts the retained owned fit VM and waits for that node's readiness. |
| `debug` | `--node NODE --debug-image IMAGE@sha256:DIGEST --age RECIPIENT` | Collects service/kubelet/container/kernel diagnostics and runs a Talos debug container; output is saved only as encrypted evidence. |
| `credentials` | `--age RECIPIENT` | Exports client ciphertext without overwriting different existing credentials. |

`rebuild` deletes the fit cluster's VM disks, including its local PostgreSQL
data disks. It does not remove the host bridge/subvolume, original SOPS
archives or backups. Never use it against production; verify the candidate
contains only disposable fixture data first. Runtime state containing foreign
domains, pools, volumes or non-Talos/libvirt resources is refused. If regenerated
client exports differ, explicitly archive the old ciphertext before exporting
the replacement; the script will not overwrite it automatically.

The worker-loss step retains the stopped VM and disks for diagnosis. A missing
target workload, a workload with fewer than two healthy replicas, a foreign VM,
a control-plane target or a production inventory is refused before stopping it.
Use `restart-worker` with the same explicit inventory node after collecting the result;
do not infer disk deletion or physical-host resilience from a successful pod
reschedule.

For upgrades, review the supported Talos/Kubernetes/add-on matrix and update
the source pins together so later applies and replacement nodes do not restore
the previous versions. These helpers restrict live upgrades to the verified
Talos 1.14 and Kubernetes 1.36 lines. They do not silently widen operator support
or directly rewrite Kubernetes component image fields to bypass Talos's upgrade
sequencing. Collect release-E2E results throughout a real node drain and prove
a restore onto a fresh cluster separately; Ready nodes and a toy deployment do
not satisfy those release gates.

## Fit decision and remaining live acceptance

Copy [fit-evidence.example.json](fit-evidence.example.json) into the operator's
evidence area. Record actual build, destroy/rebuild, both upgrades, worker-loss,
pod-debug, release-E2E-during-drain and fresh-cluster-restore artifacts with
completion timestamps. The example intentionally records every result as
pending. Accumulate Talos-specific debugging effort. At 80 engineering hours
(about two work weeks), the gate refuses Talos acceptance and requires the
RKE2-on-Ubuntu fallback decision. Keep Flux, manifests, Cilium and CNPG above the
OS boundary unchanged when evaluating that fallback.

```sh
python3 scripts/talos/cluster.py fit-gate --config /path/to/candidate.json \
  --state-dir /path/outside/git/candidate --evidence /path/to/fit-evidence.json
```

This validates the record's identity/completeness and effort threshold; an
operator must inspect the artifacts. It does not manufacture live evidence,
deploy a fallback, authorize migration or mark ECO-436 complete.

In the implementation environment `/dev/kvm` is absent. No host bridge,
btrfs/libvirt object, existing VM or cluster was changed. Real VM build,
rebuild, upgrade, workload disruption, node-drain E2E, capacity and restore
proofs remain blocked on an explicitly authorized KVM-capable scratch host.

## Offline validation

```sh
python3 -m unittest discover -s scripts/talos -p test_talos.py
tofu -chdir=infra/talos init -backend=false -input=false -lockfile=readonly
tofu -chdir=infra/talos validate
tofu -chdir=infra/talos test
```

Supply a temporary non-runtime `TF_VAR_state_passphrase` for schema checks;
never reuse it for real state. The OpenTofu test mocks libvirt and all live Talos
API resources while running the **real pinned Talos provider** to generate
secrets and parse the machine patches. It checks the user-volume document,
disabled default CNI and shared Kubernetes pin. Python tests exercise ownership,
network safeguards, dry-run no-op behavior, encrypted artifacts and bounded
worker-loss observation. None of these tests prove live hypervisor behavior.

## Primary references

- [Talos 1.14.5 release](https://github.com/siderolabs/talos/releases/tag/v1.14.5)
- [Talos 1.14 support matrix](https://docs.siderolabs.com/talos/v1.14/getting-started/support-matrix)
- [Talos KVM installation](https://docs.siderolabs.com/talos/v1.14/platform-specific-installations/virtualized-platforms/kvm)
- [Talos UserVolumeConfig](https://docs.siderolabs.com/talos/v1.14/reference/configuration/block/uservolumeconfig)
- [Official Cilium Talos guide](https://docs.cilium.io/en/stable/installation/k8s-install-talos-linux/)
- [libvirt provider v0.9.9 domain schema](https://github.com/dmacvicar/terraform-provider-libvirt/blob/v0.9.9/docs/resources/domain.md)
- [Talos provider v0.12.0](https://github.com/siderolabs/terraform-provider-talos/tree/v0.12.0/docs)
- [OpenTofu state and plan encryption](https://opentofu.org/docs/language/state/encryption/)

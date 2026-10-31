#!/usr/bin/env python3
"""Operate one explicitly owned Talos cluster. Every action defaults to dry run."""

import argparse
from contextlib import contextmanager
import datetime
import json
import math
import os
from pathlib import Path
import re
import secrets
import shutil
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

from common import (
    KUBERNETES_VERSION, ROOT, SCHEMATIC, TALOS_VERSION, Refused, call, claim,
    execute_guard, exclusive_write, external_path, identity, load_config, owned, show,
)


def stage_module(state, config):
    claim(state, config)
    module = external_path(str(state / "tofu"))
    module.mkdir(mode=0o700, exist_ok=True)
    expected = {source.name for source in (ROOT / "infra/talos").glob("*.tf")}
    for existing in module.iterdir():
        if existing.is_symlink():
            raise Refused("runtime module entries must not be symlinks")
        extra_config = (existing.name.endswith(".tf") and existing.name not in expected) or existing.name.endswith((".tf.json", ".tofu", ".tofu.json"))
        extra_vars = existing.name.endswith((".tfvars", ".tfvars.json", ".tofuvars", ".tofuvars.json")) and existing.name != "terraform.tfvars.json"
        if extra_config or extra_vars:
            raise Refused("runtime contains extra infrastructure/variable files; refusing an ambiguous apply")
    if (module / ".terraform/terraform.tfstate").is_symlink():
        raise Refused("runtime backend metadata must not be a symlink")
    for source in (ROOT / "infra/talos").iterdir():
        if source.name.endswith(".tf") or source.name == ".terraform.lock.hcl":
            if source.is_symlink():
                raise Refused("pinned module source entries must not be symlinks")
            shutil.copy2(source, module / source.name)
    values = {key: config[key] for key in ("name", "owner_id", "purpose", "bridge", "disk_path", "api_vip", "pod_cidr", "service_cidr", "data_gib", "nodes")}
    values["scratch_owner"] = config.get("scratch_owner")
    (module / "terraform.tfvars.json").write_text(json.dumps({"cluster": values}, indent=2) + "\n")
    return module


@contextmanager
def memory_file(content):
    """Pass secrets through a Linux anonymous memory file, never a disk file."""
    if not hasattr(os, "memfd_create"):
        raise Refused("Linux memfd support is required; no plaintext disk-file fallback")
    descriptor = os.memfd_create("econgraph-talos-secret", os.MFD_CLOEXEC)
    try:
        os.write(descriptor, content)
        os.lseek(descriptor, 0, os.SEEK_SET)
        yield f"/proc/{os.getpid()}/fd/{descriptor}"
    finally:
        os.close(descriptor)


def decrypt(path):
    if path.is_symlink() or not path.is_file():
        raise Refused("missing SOPS ciphertext; plaintext or symlink credentials are not accepted")
    return call(["sops", "--decrypt", str(path)])


def seal(data, recipients, kind="yaml"):
    return call(["sops", "--encrypt", "--input-type", kind, "--output-type", kind, "--age", ",".join(recipients), "/dev/stdin"], data=data)


def make_secrets(state, config, recipients):
    if not recipients or any(not re.fullmatch(r"age1[0-9a-z]+", value) for value in recipients):
        raise Refused("supply explicit public age recipients using --age")
    claim(state, config)
    bundle, key = state / "secrets.sops.yaml", state / "state-key.sops.json"
    if bundle.exists() or key.exists():
        if bundle.exists() and key.exists():
            # Verify decryption now instead of accepting accidentally supplied
            # plaintext files or an unreadable archive as a successful setup.
            decrypt(bundle)
            decrypt(key)
            print("Existing SOPS secrets and state key retained.")
            return
        raise Refused("incomplete secret archive; restore the missing file without replacing existing secrets")
    plaintext = call(["talosctl", "gen", "secrets", "--talos-version", TALOS_VERSION, "--output-file", "-"])
    encrypted_bundle = seal(plaintext, recipients)
    encrypted_key = seal(json.dumps({"passphrase": secrets.token_urlsafe(48)}).encode(), recipients, "json")
    exclusive_write(bundle, encrypted_bundle)
    exclusive_write(key, encrypted_key)
    print("Talos secrets and state encryption key stored only as SOPS ciphertext.")


def tofu_environment(state):
    if any(value for key, value in os.environ.items() if key.startswith("TF_CLI_ARGS") or key in ("TF_LOG_PATH", "TF_ENCRYPTION", "TF_DATA_DIR")) or os.environ.get("TF_WORKSPACE", "default") != "default":
        raise Refused("unset OpenTofu command/state/logging overrides before handling this explicitly scoped encrypted state")
    key = json.loads(decrypt(state / "state-key.sops.json"))
    if not isinstance(key.get("passphrase"), str) or len(key["passphrase"]) < 32:
        raise Refused("invalid SOPS state key")
    return dict(os.environ, TF_VAR_state_passphrase=key["passphrase"])


def tofu(module, action, *args, env):
    return call(["tofu", f"-chdir={module}", action, *args], env=env, timeout=3600)


def apply_without_destruction(module, env):
    # The pinned module enforces plan encryption. Inspect decrypted JSON only
    # in memory, then apply that exact saved plan, never a newly computed plan.
    with tempfile.TemporaryDirectory(prefix="reviewed-plan-", dir=module) as temporary:
        saved = str(Path(temporary) / "apply.tfplan")
        tofu(module, "plan", "-input=false", "-out=" + saved, env=env)
        plan = json.loads(tofu(module, "show", "-json", saved, env=env))
        changes = plan.get("resource_changes", [])
        if not isinstance(changes, list):
            raise Refused("unexpected saved plan resource changes; refusing apply")
        for resource in changes:
            actions = resource.get("change", {}).get("actions")
            if actions not in (["no-op"], ["create"], ["update"], ["read"]):
                raise Refused("saved plan contains deletion, replacement or unsupported actions; use the separately guarded fit rebuild")
        tofu(module, "apply", "-input=false", saved, env=env)


def import_secrets(module, state, env):
    resources = tofu(module, "state", "list", env=env).decode().splitlines() if (module / "terraform.tfstate").exists() else []
    if "talos_machine_secrets.cluster" not in resources:
        with memory_file(decrypt(state / "secrets.sops.yaml")) as path:
            tofu(module, "import", "-input=false", "talos_machine_secrets.cluster", path, env=env)


def export_credentials(module, state, env, recipients):
    if not recipients:
        raise Refused("supply --age recipients for the encrypted client configurations")
    for name in ("talosconfig", "kubeconfig"):
        plaintext = tofu(module, "output", "-raw", name, env=env)
        destination = state / f"{name}.sops.yaml"
        if destination.exists():
            if decrypt(destination) != plaintext:
                raise Refused(f"{name} changed; explicitly archive the old ciphertext before exporting its replacement")
        else:
            exclusive_write(destination, seal(plaintext, recipients))


def verify_host(config, host_state):
    owned(host_state, config)
    if not Path("/dev/kvm").exists():
        raise Refused("/dev/kvm is absent; VM creation/fit tests are unavailable")
    if config.get("dhcp_reservations_confirmed") is not True:
        raise Refused("confirm DHCP MAC/IP reservations for initial ISO boot before building")
    disk = external_path(config["disk_path"])
    marker = disk / ".econgraph-owner.json"
    if not marker.is_file() or marker.is_symlink() or json.loads(marker.read_text()) != {"name": config["name"], "owner_id": config["owner_id"]}:
        raise Refused("VM subvolume ownership does not match")
    call(["btrfs", "subvolume", "show", str(disk)])
    if b"C" not in call(["lsattr", "-d", str(disk)]).split()[0]:
        raise Refused("VM subvolume must be nodatacow before images are allocated")


def verify_domain(config, node, expected_uuid=None):
    name = f"{config['name']}-{node}"
    domain = ET.fromstring(call(["virsh", "--connect", "qemu:///system", "dumpxml", name]))
    expected = f"econgraph-talos owner={config['owner_id']} cluster={config['name']} role={config['nodes'][node]['role']}"
    if domain.findtext("name") != name or domain.findtext("description") != expected:
        raise Refused("libvirt domain ownership does not match; refusing operation")
    if expected_uuid is not None and domain.findtext("uuid") != expected_uuid:
        raise Refused("state domain UUID does not match the owned live VM")
    disks = domain.findall("devices/disk")
    if not disks:
        raise Refused("owned VM unexpectedly has no disks")
    for disk in disks:
        source = disk.find("source")
        path = source.get("file") if source is not None else None
        if not path or not external_path(path).is_relative_to(config["disk_path"]):
            raise Refused("VM has a disk outside the owned disposable subvolume")
    return name


def inspect_state(module, config, env):
    state = json.loads(tofu(module, "show", "-json", env=env))
    root = state.get("values", {}).get("root_module", {})
    if root.get("child_modules"):
        raise Refused("unexpected child modules in disposable-cluster state")
    allowed = {
        "libvirt_pool.cluster": ("libvirt_pool", "cluster", None),
        "libvirt_volume.installer": ("libvirt_volume", "installer", None),
        "talos_machine_secrets.cluster": ("talos_machine_secrets", "cluster", None),
        "talos_machine_bootstrap.cluster": ("talos_machine_bootstrap", "cluster", None),
        "talos_cluster_kubeconfig.cluster": ("talos_cluster_kubeconfig", "cluster", None),
    }
    for node, detail in config["nodes"].items():
        for kind, name in (("libvirt_domain", "node"), ("libvirt_volume", "os"), ("talos_machine_configuration_apply", "node")):
            allowed[f'{kind}.{name}["{node}"]'] = (kind, name, node)
        if detail["role"] == "worker":
            allowed[f'libvirt_volume.data["{node}"]'] = ("libvirt_volume", "data", node)
    addresses = {node["ip"] for node in config["nodes"].values()} | {config["api_vip"]}
    for resource in root.get("resources", []):
        if resource["mode"] != "managed":
            continue
        if allowed.get(resource.get("address")) != (resource["type"], resource.get("name"), resource.get("index")):
            raise Refused("state contains an unexpected resource address/type/instance; refusing destruction")
        kind, values = resource["type"], resource["values"]
        if kind == "libvirt_domain":
            node = next((key for key in config["nodes"] if values["name"] == f"{config['name']}-{key}"), None)
            if node is None:
                raise Refused("state contains a foreign VM")
            if not values.get("uuid"):
                raise Refused("state lacks the live domain UUID")
            verify_domain(config, node, values["uuid"])
        elif kind == "libvirt_pool":
            if values["name"] != config["name"] or values["target"]["path"] != config["disk_path"]:
                raise Refused("state contains a foreign storage pool")
            live = ET.fromstring(call(["virsh", "--connect", "qemu:///system", "pool-dumpxml", values["name"]]))
            if live.findtext("name") != config["name"] or live.findtext("target/path") != config["disk_path"]:
                raise Refused("live storage pool no longer points at the owned disposable subvolume")
            if not values.get("uuid") or live.findtext("uuid") != values["uuid"] or values.get("id") != values["uuid"]:
                raise Refused("state pool UUID does not match the owned live pool")
        elif kind == "libvirt_volume":
            if values["pool"] != config["name"] or not external_path(values["path"]).is_relative_to(config["disk_path"]):
                raise Refused("state contains a volume outside the owned VM subvolume")
            if values.get("id") != values["path"] or values.get("key") != values["path"]:
                raise Refused("state volume key differs from its owned directory-pool path")
        elif kind in ("talos_machine_configuration_apply", "talos_machine_bootstrap", "talos_cluster_kubeconfig"):
            node = resource.get("index", "cp-1")
            if values.get("node") != config["nodes"][node]["ip"] or values.get("endpoint") not in (None, "", *addresses):
                raise Refused("Talos state targets a node/endpoint outside the explicit inventory")
            if (values.get("on_destroy") or {}).get("reset"):
                raise Refused("Talos state requests a destructive on_destroy reset; refusing automatic node wipes")


@contextmanager
def clients(state):
    with memory_file(decrypt(state / "talosconfig.sops.yaml")) as talosconfig:
        with memory_file(decrypt(state / "kubeconfig.sops.yaml")) as kubeconfig:
            yield ["talosctl", "--talosconfig", talosconfig], ["kubectl", "--kubeconfig", kubeconfig]


def check_kube_target(kubectl, config):
    view = json.loads(call([*kubectl, "config", "view", "--minify", "-o", "json"]))
    if [item["cluster"]["server"] for item in view["clusters"]] != [f"https://{config['api_vip']}:6443"]:
        raise Refused("kubeconfig targets a different API endpoint")


def wait_ready(kubectl, config):
    # Explicit names avoid treating an unrelated ready node as success.
    for node in config["nodes"]:
        call([*kubectl, "wait", f"node/{config['name']}-{node}", "--for=condition=Ready", "--timeout=10m"], timeout=660)


def fit_gate(evidence, config):
    value = json.loads(Path(evidence).read_text())
    if value.get("cluster") != identity(config):
        raise Refused("fit evidence belongs to another cluster/owner")
    effort = value.get("talos_debugging_hours")
    if type(effort) not in (int, float) or not math.isfinite(effort) or effort < 0:
        raise Refused("record nonnegative Talos-specific debugging hours")
    if effort >= 80:
        raise Refused("two engineering weeks spent on Talos-specific trouble: choose RKE2/Ubuntu and retain the platform layer")
    for check in ("build", "rebuild", "talos_upgrade", "kubernetes_upgrade", "worker_loss", "pod_debug", "release_e2e_during_drain", "fresh_cluster_restore"):
        item = value.get("checks", {}).get(check, {})
        if item.get("result") != "passed" or not item.get("evidence") or not item.get("completed_at"):
            raise Refused(f"fit acceptance still lacks observed evidence: {check}")
        datetime.datetime.fromisoformat(item["completed_at"].replace("Z", "+00:00"))
    print("Recorded fit evidence is complete; inspect its artifacts before accepting the platform. No deployment performed.")


def plan(args, config, state):
    module = state / "tofu"
    first = config["nodes"]["cp-1"]["ip"]
    if args.action == "secrets":
        show(["talosctl", "gen", "secrets", "--talos-version", TALOS_VERSION, "--output-file", "<anonymous-memory-file>"])
        show(["sops", "--encrypt", "<secrets-and-state-key-in-memory>", "to", str(state)])
    elif args.action in ("build", "rebuild"):
        show(["verify", "host-owner", str(args.host_state_dir), "KVM", "btrfs-nodatacow", "DHCP-reservations"])
        if args.action == "rebuild":
            show(["verify", "fit-only", "owned-domains-and-state", "explicit-disposable-acknowledgment"])
            show(["tofu", f"-chdir={module}", "destroy", "-input=false", "-auto-approve"])
        show(["tofu", f"-chdir={module}", "init", "-input=false", "-lockfile=readonly"])
        show(["tofu", f"-chdir={module}", "import", "talos_machine_secrets.cluster", "<SOPS-decrypted-memory-file>"])
        show(["tofu", f"-chdir={module}", "plan", "-input=false", "-out=<encrypted-reviewed-plan>"])
        show(["verify", "saved-plan-has-no-deletes-or-replacements"])
        show(["tofu", f"-chdir={module}", "apply", "-input=false", "<encrypted-reviewed-plan>"])
        print("Then export encrypted client configs and install the pinned platform Cilium before waiting for Ready.")
    elif args.action == "credentials":
        show(["tofu", f"-chdir={module}", "output", "-raw", "<client-config>", "to-SOPS-only"])
    elif args.action == "upgrade-talos":
        for node in config["nodes"].values():
            show(["talosctl", "--talosconfig", "<memory>", "upgrade", "--nodes", node["ip"], "--image", args.image or f"factory.talos.dev/metal-installer/{SCHEMATIC}:{TALOS_VERSION}", "--wait", "--timeout=20m"])
            show(["kubectl", "--kubeconfig", "<memory>", "wait", "--for=condition=Ready", "<this-node>", "--timeout=10m"])
    elif args.action == "upgrade-kubernetes":
        show(["talosctl", "--talosconfig", "<memory>", "upgrade-k8s", "--nodes", first, "--to", args.kubernetes_version or KUBERNETES_VERSION])
        print("Talos upgrade-k8s sequences control-plane components and kubelets; do not patch image fields directly.")
    elif args.action == "kill-worker":
        show(["verify", "fit-only", "matching-domain-owner", args.node or "<worker>", "replicated-target-deployment"])
        show(["virsh", "--connect", "qemu:///system", "destroy", f"{config['name']}-{args.node or '<worker>'}"])
        show(["observe", "target-deployment-new-ready-pods-on-other-nodes", "bounded-10-minutes"])
        print("The stopped VM and disks are retained. Restart it explicitly after observing recovery.")
    elif args.action == "restart-worker":
        show(["verify", "fit-only", "matching-domain-owner", args.node or "<worker>"])
        show(["virsh", "--connect", "qemu:///system", "start", f"{config['name']}-{args.node or '<worker>'}"])
        show(["kubectl", "--kubeconfig", "<memory>", "wait", "--for=condition=Ready", "<this-node>", "--timeout=10m"])
    elif args.action == "debug":
        for command in (["services"], ["logs", "kubelet"], ["containers", "--namespace", "cri"], ["dmesg"], ["debug", args.debug_image or "<required-digest-pinned-debug-image>", "--args", "/bin/true"]):
            show(["talosctl", "--talosconfig", "<memory>", "--nodes", config["nodes"][args.node or "worker-1"]["ip"], *command])
    else:
        show(["talosctl", "--talosconfig", "<memory>", "--nodes", first, "health", "--wait-timeout=10m"])
        show(["kubectl", "--kubeconfig", "<memory>", "get", "nodes", "-o", "wide"])


def operate(args, config, state):
    if args.action == "fit-gate":
        if not args.evidence:
            raise Refused("--evidence is required for the local acceptance-record check")
        fit_gate(args.evidence, config)
        return
    if args.node and args.node not in config["nodes"]:
        raise Refused("node is outside the explicit cluster inventory")
    if not args.execute:
        plan(args, config, state)
        return
    if args.action == "secrets":
        make_secrets(state, config, args.age)
        return
    owned(state, config)
    if args.action in ("build", "rebuild", "credentials"):
        if not args.age:
            raise Refused("supply --age recipients before cluster operations and credential export")
        if args.action in ("build", "rebuild"):
            if args.host_state_dir is None:
                raise Refused("provide the explicit --host-state-dir prepared by host.py")
            verify_host(config, external_path(str(args.host_state_dir)))
        module = stage_module(state, config)
        env = tofu_environment(state)
        tofu(module, "init", "-input=false", "-lockfile=readonly", env=env)
        if args.action == "rebuild":
            if config["purpose"] != "fit" or args.destroy_disposable != config["name"]:
                raise Refused("rebuild is restricted to a fit cluster explicitly acknowledged by --destroy-disposable")
            inspect_state(module, config, env)
            tofu(module, "destroy", "-input=false", "-auto-approve", env=env)
        if args.action in ("build", "rebuild"):
            import_secrets(module, state, env)
            apply_without_destruction(module, env)
            print("Talos API bootstrapped. Cilium must be installed before Kubernetes nodes can become Ready.")
        export_credentials(module, state, env, args.age)
        return
    with clients(state) as (talosctl, kubectl):
        check_kube_target(kubectl, config)
        if args.action == "upgrade-talos":
            if not args.image or not re.fullmatch(r"factory\.talos\.dev/metal-installer/[0-9a-f]{64}:v1\.14\.[0-9]+", args.image):
                raise Refused("supply an explicit schematic-pinned Talos 1.14 patch installer image")
            wait_ready(kubectl, config)
            for node, detail in config["nodes"].items():
                verify_domain(config, node)
                call([*talosctl, "upgrade", "--nodes", detail["ip"], "--image", args.image, "--wait", "--timeout=20m"], timeout=1260)
                call([*kubectl, "wait", f"node/{config['name']}-{node}", "--for=condition=Ready", "--timeout=10m"], timeout=660)
        elif args.action == "upgrade-kubernetes":
            if not args.kubernetes_version or not re.fullmatch(r"1\.36\.[0-9]+", args.kubernetes_version):
                raise Refused("use an explicitly reviewed Kubernetes 1.36 patch supported by the platform add-ons")
            wait_ready(kubectl, config)
            call([*talosctl, "upgrade-k8s", "--nodes", config["nodes"]["cp-1"]["ip"], "--to", args.kubernetes_version], timeout=3600)
            wait_ready(kubectl, config)
        elif args.action == "kill-worker":
            kill_worker(args, config, talosctl, kubectl)
        elif args.action == "restart-worker":
            if config["purpose"] != "fit" or args.node not in config["nodes"] or config["nodes"][args.node]["role"] != "worker":
                raise Refused("worker recovery is restricted to an explicit worker in a fit cluster")
            name = verify_domain(config, args.node)
            call(["virsh", "--connect", "qemu:///system", "start", name])
            call([*kubectl, "wait", f"node/{name}", "--for=condition=Ready", "--timeout=10m"], timeout=660)
        elif args.action == "debug":
            if not args.node or not args.debug_image or not re.fullmatch(r"[a-zA-Z0-9./:_-]+@sha256:[0-9a-f]{64}", args.debug_image):
                raise Refused("debug requires an inventory --node and an explicit digest-pinned --debug-image")
            if not args.age:
                raise Refused("supply --age recipients for encrypted diagnostic output")
            diagnostics = {}
            for command in (["services"], ["logs", "kubelet"], ["containers", "--namespace", "cri"], ["dmesg"], ["debug", args.debug_image, "--args", "/bin/true"]):
                # Logs may contain secrets. Persist only encrypted evidence.
                diagnostics[" ".join(command)] = call([*talosctl, "--nodes", config["nodes"][args.node]["ip"], *command]).decode(errors="replace")
            destination = state / f"debug-{args.node}-{datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')}.sops.json"
            exclusive_write(destination, seal(json.dumps(diagnostics).encode(), args.age, "json"))
            print(f"Talos service/log/container/kernel/debug probes saved as ciphertext: {destination}")
        elif args.action == "status":
            call([*talosctl, "--nodes", config["nodes"]["cp-1"]["ip"], "health", "--wait-timeout=10m"], timeout=660)
            wait_ready(kubectl, config)
            print("Explicit candidate nodes are Ready.")


def kill_worker(args, config, talosctl, kubectl):
    if config["purpose"] != "fit" or args.node not in config["nodes"] or config["nodes"][args.node]["role"] != "worker":
        raise Refused("worker-loss injection is restricted to an explicit worker in a fit cluster")
    if not args.namespace or not args.deployment or not all(re.fullmatch(r"[a-z0-9][a-z0-9-]*", value) for value in (args.namespace, args.deployment)):
        raise Refused("provide the explicit replicated --namespace and --deployment to observe")
    name = verify_domain(config, args.node)
    deployment_command = [*kubectl, "--namespace", args.namespace, "get", "deployment", args.deployment, "-o", "json"]
    deployment = json.loads(call(deployment_command))
    count = deployment["spec"].get("replicas", 1)
    selector = deployment["spec"]["selector"]
    metadata, status = deployment.get("metadata", {}), deployment.get("status", {})
    uid, generation = metadata.get("uid"), metadata.get("generation")
    revision = metadata.get("annotations", {}).get("deployment.kubernetes.io/revision")
    if (count < 2 or not uid or not isinstance(generation, int) or not revision
            or status.get("observedGeneration", 0) < generation
            or status.get("readyReplicas", 0) != count or status.get("updatedReplicas", 0) != count
            or not selector.get("matchLabels") or selector.get("matchExpressions")):
        raise Refused("target must be a healthy replicated deployment with a simple matchLabels selector")
    labels = ",".join(f"{key}={value}" for key, value in sorted(selector["matchLabels"].items()))
    replicasets = json.loads(call([*kubectl, "--namespace", args.namespace, "get", "replicasets", "-l", labels, "-o", "json"]))["items"]
    replica_uids = {r["metadata"]["uid"] for r in replicasets
                    if r.get("metadata", {}).get("uid")
                    and r["metadata"].get("annotations", {}).get("deployment.kubernetes.io/revision") == revision
                    and any(o.get("controller") is True and o.get("kind") == "Deployment" and o.get("uid") == uid
                            for o in r["metadata"].get("ownerReferences", []))}
    if len(replica_uids) != 1:
        raise Refused("target needs one unambiguous current ReplicaSet owned by the deployment UID")
    pods_command = [*kubectl, "--namespace", args.namespace, "get", "pods", "-l", labels, "-o", "json"]
    pods = json.loads(call(pods_command))["items"]
    workers = {config['name'] + '-' + node for node, detail in config['nodes'].items() if detail['role'] == 'worker'}
    def healthy(pod):
        meta = pod.get("metadata", {})
        return (bool(meta.get("uid")) and not meta.get("deletionTimestamp")
                and pod.get("spec", {}).get("nodeName") in workers
                and pod.get("status", {}).get("phase") == "Running"
                and any(o.get("controller") is True and o.get("kind") == "ReplicaSet" and o.get("uid") in replica_uids
                        for o in meta.get("ownerReferences", []))
                and any(c.get("type") == "Ready" and c.get("status") == "True"
                        for c in pod.get("status", {}).get("conditions", [])))
    initial = {p["metadata"]["uid"] for p in pods if p.get("metadata", {}).get("uid")}
    ready = {p["metadata"]["uid"]: p for p in pods if healthy(p)}
    victims = {key for key, pod in ready.items() if pod["spec"]["nodeName"] == name}
    if not victims:
        raise Refused("target deployment has no healthy current replica on the selected worker; no failover would be tested")
    if len(ready) != count:
        raise Refused("target's current owned pods do not match its healthy replica count")
    call(["virsh", "--connect", "qemu:///system", "destroy", name])
    deadline = time.monotonic() + 600
    while time.monotonic() < deadline:
        current = json.loads(call(deployment_command))
        if (current.get("metadata", {}).get("uid") != uid
                or current["metadata"].get("generation") != generation
                or current["metadata"].get("annotations", {}).get("deployment.kubernetes.io/revision") != revision):
            raise Refused("deployment identity or rollout changed during the worker-loss probe")
        pods = json.loads(call(pods_command))["items"]
        ready = {p["metadata"]["uid"] for p in pods if healthy(p) and p["spec"]["nodeName"] != name}
        if len(ready) >= count and len(ready - initial) >= len(victims):
            print("Observed target deployment readiness on other workers. Stopped VM and disks retained; this is not a physical-host resilience proof.")
            return
        time.sleep(5)
    raise Refused("target did not regain replicas on other nodes within ten minutes; stopped VM retained for diagnosis")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("secrets", "build", "rebuild", "credentials", "status", "upgrade-talos", "upgrade-kubernetes", "kill-worker", "restart-worker", "debug", "fit-gate"))
    parser.add_argument("--config", required=True)
    parser.add_argument("--state-dir", required=True, type=Path)
    parser.add_argument("--host-state-dir", type=Path)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--confirm-owner")
    parser.add_argument("--age", action="append", default=[])
    parser.add_argument("--destroy-disposable")
    parser.add_argument("--node")
    parser.add_argument("--namespace")
    parser.add_argument("--deployment")
    parser.add_argument("--image")
    parser.add_argument("--debug-image")
    parser.add_argument("--kubernetes-version")
    parser.add_argument("--evidence")
    args = parser.parse_args()
    config = load_config(args.config)
    execute_guard(args, config)
    operate(args, config, external_path(str(args.state_dir)))


if __name__ == "__main__":
    try:
        main()
    except (Refused, OSError, ValueError, KeyError, TypeError, ET.ParseError) as error:
        print(f"talos-cluster: {error}", file=sys.stderr)
        sys.exit(1)

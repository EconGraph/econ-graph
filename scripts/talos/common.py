"""Shared ownership, subprocess and public-inventory checks for the Talos lane."""

import ipaddress
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import uuid


ROOT = Path(__file__).resolve().parents[2]
TALOS_VERSION = "v1.14.5"
KUBERNETES_VERSION = "1.36.4"
SCHEMATIC = "47f718feafaf5934d42735b54da7805fa768627393a4318a0c2d47fd5b1ecb79"


class Refused(Exception):
    pass


def call(args, *, data=None, env=None, timeout=300):
    """Capture subprocess output; never leak decrypted credentials on failure."""
    try:
        result = subprocess.run(args, input=data, capture_output=True, env=env, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refused(f"{Path(args[0]).name} did not complete; check tool availability/connectivity") from error
    if result.returncode:
        raise Refused(f"{Path(args[0]).name} failed with exit {result.returncode}; captured output withheld")
    return result.stdout


def show(args):
    print("DRY RUN: " + shlex.join(map(str, args)))


def external_path(value):
    path = Path(value).expanduser()
    if not path.is_absolute() or path.resolve() != path or path == Path("/") or path.is_relative_to(ROOT):
        raise Refused("runtime/storage paths must be absolute, outside the checkout and free of symlinks")
    return path


def load_config(path):
    config = json.loads(Path(path).read_text())
    if config.get("schema_version") != 1 or not re.fullmatch(r"econgraph-[a-z0-9]+(?:-[a-z0-9]+)*", config["name"]):
        raise Refused("use schema_version 1 and an explicit econgraph-* cluster name")
    uuid.UUID(config["owner_id"])
    if config.get("purpose") not in ("fit", "production"):
        raise Refused("purpose must be fit or production")
    if not re.fullmatch(r"[a-zA-Z0-9_-]{1,15}", config["bridge"]):
        raise Refused("invalid Linux bridge name")
    host = ipaddress.ip_interface(config["bridge_address"])
    pod, service = (ipaddress.ip_network(config[key]) for key in ("pod_cidr", "service_cidr"))
    if any(a.overlaps(b) for a, b in ((host.network, pod), (host.network, service), (pod, service))):
        raise Refused("LAN, pod and service networks must not overlap")
    nodes = config["nodes"]
    if {name for name, node in nodes.items() if node["role"] == "controlplane"} != {"cp-1", "cp-2", "cp-3"}:
        raise Refused("exactly cp-1, cp-2 and cp-3 control planes are required")
    workers = {name for name, node in nodes.items() if node["role"] == "worker"}
    if workers not in ({"worker-1", "worker-2"}, {"worker-1", "worker-2", "worker-3"}) or len(nodes) != 3 + len(workers):
        raise Refused("exactly two or three named workers are required")
    addresses = [ipaddress.ip_address(config["api_vip"]), host.ip]
    macs = []
    for node in nodes.values():
        addresses.append(ipaddress.ip_address(node["ip"]))
        mac = node["mac"].lower()
        if not re.fullmatch(r"52:54:00(:[0-9a-f]{2}){3}", mac):
            raise Refused("use unique QEMU locally administered MAC addresses (52:54:00:*)")
        macs.append(mac)
    if len(set(addresses)) != len(addresses) or len(set(macs)) != len(macs):
        raise Refused("host, API VIP, nodes and MAC addresses must be unique")
    if host.version != 4 or any(ip not in host.network or ip in (host.network.network_address, host.network.broadcast_address) for ip in addresses):
        raise Refused("the host, nodes and API VIP must be usable addresses on the same IPv4 LAN")
    if type(config["data_gib"]) is not int or config["data_gib"] < 100:
        raise Refused("worker data disks must be at least 100 GiB")
    config["disk_path"] = str(external_path(config["disk_parent"]) / config["name"])
    return config


def identity(config):
    return {key: config[key] for key in ("name", "owner_id", "bridge", "disk_path")}


def claim(directory, config):
    directory = external_path(str(directory))
    marker = directory / "owner.json"
    if directory.exists():
        if not marker.is_file() or marker.is_symlink() or json.loads(marker.read_text()) != identity(config):
            raise Refused("runtime directory has different or missing ownership; refusing adoption")
        return
    directory.mkdir(parents=True, mode=0o700)
    marker.write_text(json.dumps(identity(config), indent=2) + "\n")
    marker.chmod(0o600)


def owned(directory, config):
    directory = external_path(str(directory))
    marker = directory / "owner.json"
    if not marker.is_file() or marker.is_symlink() or json.loads(marker.read_text()) != identity(config):
        raise Refused("explicit cluster ownership does not match this runtime directory")


def execute_guard(args, config):
    if args.execute and args.confirm_owner != config["owner_id"]:
        raise Refused("--execute requires --confirm-owner matching the public inventory")
    if args.execute:
        documentation = [ipaddress.ip_network(net) for net in ("192.0.2.0/24", "198.51.100.0/24", "203.0.113.0/24")]
        if any(ipaddress.ip_address(config["api_vip"]) in net for net in documentation):
            raise Refused("replace documentation-only addresses before execution")


def exclusive_write(path, content):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(content)

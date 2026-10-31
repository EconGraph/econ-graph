#!/usr/bin/env python3
"""Prepare an owned LAN bridge and a new nodatacow VM subvolume; dry run by default.

Only an unused dedicated uplink is supported. Migrating an active management NIC
is intentionally refused and requires a separate, console-tested network plan.
"""

import argparse
import ipaddress
import json
import os
from pathlib import Path
import sys

from common import Refused, call, claim, execute_guard, external_path, load_config, show


def unused_uplink(links, addresses, routes, bridge):
    master = links[0].get("master")
    if master not in (None, bridge):
        raise Refused("uplink is already attached to another bridge/master")
    if any(a.get("scope") == "global" for a in addresses[0].get("addr_info", [])) or routes:
        raise Refused("uplink carries host addresses/default routes; refusing management-network migration")
    return master


def lan_routes(routes, config):
    network = ipaddress.ip_interface(config["bridge_address"]).network
    for route in routes:
        if route.get("dev") != config["bridge"] and route.get("dst") not in (None, "default"):
            if ipaddress.ip_network(route["dst"], strict=False).overlaps(network):
                raise Refused("candidate LAN overlaps another host interface's connected route; a reviewed network migration is required")


def new_parent_directories(path):
    """Make only missing image parents traversable by libvirt's QEMU user."""
    missing = []
    current = path
    while not current.exists():
        missing.append(current)
        current = current.parent
    for directory in reversed(missing):
        directory.mkdir(mode=0o755)
        directory.chmod(0o755)


def prepare(config, state_dir, execute=False):
    parent = external_path(config["disk_parent"])
    disk = external_path(config["disk_path"])
    bridge, uplink = config["bridge"], config["uplink"]
    if not execute:
        for command in (
            ["check", "unused-LAN-uplink", uplink, "no-address/default-route/foreign-master"],
            ["check", "no-foreign-LAN-connected-route", config["bridge_address"]],
            ["check", "btrfs-parent", str(parent), "and-matching-ownership"],
            ["btrfs", "subvolume", "create", str(disk)],
            ["chattr", "+C", str(disk)],
            ["ip", "link", "add", "name", bridge, "type", "bridge"],
            ["ip", "address", "add", config["bridge_address"], "dev", bridge],
            ["ip", "link", "set", uplink, "master", bridge],
            ["ip", "link", "set", bridge, "up"],
        ):
            show(command)
        return
    if os.geteuid() != 0:
        raise Refused("host prep execution requires root")
    if not Path("/dev/kvm").exists():
        raise Refused("/dev/kvm is absent; enable hardware virtualization before host prep")
    if not uplink or uplink.startswith("REPLACE") or not Path("/sys/class/net", uplink).exists():
        raise Refused("select an existing unused dedicated LAN uplink")
    base = parent
    while not base.exists() and base != base.parent:
        base = base.parent
    if not base.is_dir() or call(["findmnt", "-n", "-o", "FSTYPE", "--target", str(base)]).strip() != b"btrfs":
        raise Refused("disk_parent must be on the intended existing btrfs filesystem")
    links = json.loads(call(["ip", "-j", "link", "show", "dev", uplink]))
    addresses = json.loads(call(["ip", "-j", "address", "show", "dev", uplink]))
    routes = []
    for family in ("-4", "-6"):
        routes.extend(json.loads(call(["ip", "-j", family, "route", "show", "default", "dev", uplink])))
    master = unused_uplink(links, addresses, routes, bridge)
    lan_routes(json.loads(call(["ip", "-j", "-4", "route", "show", "scope", "link"])), config)
    bridge_path = Path("/sys/class/net", bridge)
    marker = state_dir / "owner.json"
    if bridge_path.exists() and not marker.is_file():
        raise Refused("existing unowned bridge will not be adopted")
    if bridge_path.exists() and not (bridge_path / "bridge").is_dir():
        raise Refused("requested bridge name belongs to a different link type")
    if disk.exists():
        disk_marker = disk / ".econgraph-owner.json"
        if not disk_marker.is_file() or disk_marker.is_symlink() or json.loads(disk_marker.read_text()) != {"name": config["name"], "owner_id": config["owner_id"]}:
            raise Refused("existing disk directory has no matching ownership; refusing adoption")
        call(["btrfs", "subvolume", "show", str(disk)])
        attributes = call(["lsattr", "-d", str(disk)]).split()[0]
        if b"C" not in attributes:
            raise Refused("existing VM directory is not nodatacow; do not rewrite populated disks")
    claim(state_dir, config)
    if not disk.exists():
        new_parent_directories(parent)
        call(["btrfs", "subvolume", "create", str(disk)])
        call(["chattr", "+C", str(disk)])
        disk.chmod(0o755)
        disk_marker = disk / ".econgraph-owner.json"
        disk_marker.write_text(json.dumps({"name": config["name"], "owner_id": config["owner_id"]}) + "\n")
    if not bridge_path.exists():
        call(["ip", "link", "add", "name", bridge, "type", "bridge"])
    current = json.loads(call(["ip", "-j", "address", "show", "dev", bridge]))[0].get("addr_info", [])
    actual = {f"{a['local']}/{a['prefixlen']}" for a in current if a.get("scope") == "global"}
    if actual and actual != {config["bridge_address"]}:
        raise Refused("existing bridge has unexpected addresses; refusing changes")
    if not actual:
        call(["ip", "address", "add", config["bridge_address"], "dev", bridge])
    if master != bridge:
        call(["ip", "link", "set", uplink, "master", bridge])
    call(["ip", "link", "set", uplink, "up"])
    call(["ip", "link", "set", bridge, "up"])
    print("Owned bridge and VM subvolume are prepared; no default route or existing host address was moved.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True)
    parser.add_argument("--state-dir", required=True, type=Path)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--confirm-owner")
    args = parser.parse_args()
    config = load_config(args.config)
    execute_guard(args, config)
    prepare(config, external_path(str(args.state_dir)), args.execute)


if __name__ == "__main__":
    try:
        main()
    except (Refused, OSError, ValueError, KeyError, TypeError) as error:
        print(f"talos-host: {error}", file=sys.stderr)
        sys.exit(1)

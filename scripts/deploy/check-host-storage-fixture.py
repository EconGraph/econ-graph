#!/usr/bin/env python3
"""Check the Postgres storage manifests against the objects production runs.

Usage:
  scripts/deploy/check-host-storage-fixture.py [FIXTURE]
  scripts/deploy/check-host-storage-fixture.py --capture SC.yaml PV.yaml STS.yaml > FIXTURE

FIXTURE defaults to scripts/deploy/testdata/production-host-storage.json. It holds,
for the StorageClass econgraph-host, the PVs econgraph-data-plane-pg and
econgraph-app-prod-keycloak-pg and the StatefulSets postgresql and keycloak-postgres:

  last_applied  the object's kubectl.kubernetes.io/last-applied-configuration
  live          the fields Kubernetes will not change in place: for a StatefulSet
                selector, serviceName, podManagementPolicy and volumeClaimTemplates;
                for a PV its spec (claimRef reduced to namespace and name) and labels;
                for the StorageClass provisioner, reclaimPolicy, volumeBindingMode,
                parameters, mountOptions and labels

The check passes when, for every object, the committed manifest (and the manifest as
deploy.sh and restart-k8s-rollout.sh render it for PUBLIC_HOST=econgraph.com) equals
last_applied, so `kubectl apply` sends no change, and its immutable fields, with the
API server's defaults filled in, equal live, so the apply cannot be refused.

--capture builds the fixture from `kubectl get ... -o yaml` output, read-only:
  kubectl get storageclass econgraph-host -o yaml > SC.yaml
  kubectl get pv econgraph-data-plane-pg econgraph-app-prod-keycloak-pg -o yaml > PV.yaml
  kubectl -n econ-graph get sts postgresql keycloak-postgres -o yaml > STS.yaml
Only the annotation and the fields above are kept; the objects hold no Secret values
(the StatefulSets name their Secrets, they don't contain them).
"""

import copy
import json
import os
import subprocess
import sys

import yaml

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DEFAULT_FIXTURE = os.path.join(REPO, "scripts/deploy/testdata/production-host-storage.json")
MANIFESTS = {
    ("StorageClass", "econgraph-host"): "k8s/manifests/host-storage.yaml",
    ("PersistentVolume", "econgraph-data-plane-pg"): "k8s/manifests/host-storage.yaml",
    ("PersistentVolume", "econgraph-app-prod-keycloak-pg"): "k8s/manifests/host-storage.yaml",
    ("StatefulSet", "postgresql"): "k8s/manifests/postgres-deployment.yaml",
    ("StatefulSet", "keycloak-postgres"): "k8s/manifests/keycloak/postgres.yaml",
}
LAST_APPLIED = "kubectl.kubernetes.io/last-applied-configuration"


def immutable(obj):
    """The fields of OBJ that cannot change in place, with the server's defaults."""
    kind, spec = obj["kind"], obj.get("spec", {})
    labels = obj["metadata"].get("labels", {})
    if kind == "StatefulSet":
        templates = []
        for t in spec.get("volumeClaimTemplates", []):
            tspec = copy.deepcopy(t.get("spec", {}))
            tspec.setdefault("volumeMode", "Filesystem")
            templates.append({"name": t["metadata"]["name"], "spec": tspec})
        return {
            "selector": spec.get("selector"),
            "serviceName": spec.get("serviceName"),
            "podManagementPolicy": spec.get("podManagementPolicy", "OrderedReady"),
            "volumeClaimTemplates": templates,
        }
    if kind == "PersistentVolume":
        pv = copy.deepcopy(spec)
        pv.setdefault("volumeMode", "Filesystem")
        if "claimRef" in pv:
            pv["claimRef"] = {k: pv["claimRef"][k] for k in ("namespace", "name")}
        return {"labels": labels, "spec": pv}
    if kind == "StorageClass":
        return {
            "labels": labels,
            "provisioner": obj.get("provisioner"),
            "reclaimPolicy": obj.get("reclaimPolicy", "Delete"),
            "volumeBindingMode": obj.get("volumeBindingMode", "Immediate"),
            "parameters": obj.get("parameters", {}),
            "mountOptions": obj.get("mountOptions", []),
        }
    raise ValueError(f"unexpected kind {kind}")


def capture(paths):
    objects = []
    for path in paths:
        with open(path, encoding="utf-8") as f:
            doc = yaml.safe_load(f)
        for obj in doc.get("items", [doc]):
            key = (obj["kind"], obj["metadata"]["name"])
            if key not in MANIFESTS:
                continue
            objects.append({
                "kind": key[0],
                "name": key[1],
                "last_applied": json.loads(obj["metadata"]["annotations"][LAST_APPLIED]),
                "live": immutable(obj),
            })
    missing = set(MANIFESTS) - {(o["kind"], o["name"]) for o in objects}
    if missing:
        sys.exit(f"not in the input: {sorted(missing)}")
    objects.sort(key=lambda o: (o["kind"], o["name"]))
    json.dump({"objects": objects}, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")


def load_manifest(path, public_host):
    if public_host:
        text = subprocess.run(
            [os.path.join(REPO, "scripts/deploy/render-public-host.py"), public_host, path],
            check=True, capture_output=True, text=True, cwd=REPO).stdout
    else:
        with open(os.path.join(REPO, path), encoding="utf-8") as f:
            text = f.read()
    return {(d["kind"], d["metadata"]["name"]): d for d in yaml.safe_load_all(text) if d}


def normalise(obj):
    obj = json.loads(json.dumps(obj))
    obj["metadata"].setdefault("annotations", {})
    return obj


def check(fixture_path):
    with open(fixture_path, encoding="utf-8") as f:
        fixture = json.load(f)
    failures = 0
    seen = set()
    for entry in fixture["objects"]:
        key = (entry["kind"], entry["name"])
        seen.add(key)
        for public_host in ("", "econgraph.com"):
            label = f"{key[0]}/{key[1]} ({MANIFESTS[key]}{', rendered for ' + public_host if public_host else ''})"
            obj = load_manifest(MANIFESTS[key], public_host).get(key)
            if obj is None:
                print(f"FAIL {label}: not in the manifest")
                failures += 1
                continue
            if normalise(obj) != normalise(entry["last_applied"]):
                print(f"FAIL {label}: differs from production's last-applied configuration")
                failures += 1
            if immutable(obj) != entry["live"]:
                print(f"FAIL {label}: immutable fields differ from production's")
                print("  manifest:   " + json.dumps(immutable(obj), sort_keys=True))
                print("  production: " + json.dumps(entry["live"], sort_keys=True))
                failures += 1
    if seen != set(MANIFESTS):
        print(f"FAIL fixture covers {sorted(seen)}, expected {sorted(MANIFESTS)}")
        failures += 1
    if failures:
        sys.exit(f"{failures} check(s) failed")
    print(f"ok: {len(seen)} objects match production (last-applied and immutable fields)")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--capture":
        if len(sys.argv) != 5:
            sys.exit(__doc__)
        capture(sys.argv[2:])
    elif len(sys.argv) <= 2:
        check(sys.argv[1] if len(sys.argv) == 2 else DEFAULT_FIXTURE)
    else:
        sys.exit(__doc__)

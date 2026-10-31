#!/usr/bin/env python3
"""The kind cluster's kubectl, faked for scripts/keycloak/test-set-catalog-access.sh.

Appends every call to $STUB_LOG and answers the reads the script makes. For
`run ... --overrides JSON` it checks that the pod takes the Keycloak credentials from Secret
econ-graph-keycloak by reference, then runs the pod's command with this process's stdin
(the helper script) against the fake Keycloak at $STUB_KEYCLOAK_URL, with the credentials
the Secret would hold.

  STUB_NO_KIND=1        the kubeconfig has no kind-econ-graph context
  STUB_PRODUCTION=false the cluster carries no production marker
  STUB_IMPORT=running|complete|failed|unreadable  job/keycloak-realm-import's state (default:
                        no such Job)
"""
import json
import os
import subprocess
import sys

args = sys.argv[1:]
with open(os.environ["STUB_LOG"], "a", encoding="utf-8") as log:
    log.write("kubectl " + " ".join(a.replace("\n", " ") for a in args) + "\n")
joined = " " + " ".join(args) + " "


def done(text=""):
    if text:
        print(text)
    sys.exit(0)


if " version " in joined:
    done()
if " config get-contexts -o name " in joined:
    done(("" if os.environ.get("STUB_NO_KIND") == "1" else "kind-econ-graph\n") + "other")
if " config current-context " in joined:
    done("other")
if " config view --minify " in joined:
    done("" if os.environ.get("STUB_NO_KIND") == "1" else "https://127.0.0.1:6443")
if " get nodes " in joined:
    done("econ-graph-control-plane econ-graph-worker")
if " get namespace kube-system " in joined:
    done("11111111-2222-3333-4444-555555555555")
if " get configmap econgraph-cluster-role " in joined:
    # The marker (lib/cluster-role.sh): exists, then each value with a trailing "#".
    if os.environ.get("STUB_PRODUCTION", "true") != "true":
        done()
    if " -o name" in joined:
        done("configmap/econgraph-cluster-role")
    if "{.data.role}" in joined:
        done("production#")
    done("11111111-2222-3333-4444-555555555555#")
if " get configmap econgraph-lab-owner " in joined:
    # Not a release lab (lib/cluster-role.sh, cluster_role_is_lab).
    done()
if " get job keycloak-realm-import " in joined:
    state = os.environ.get("STUB_IMPORT", "")
    if state == "unreadable":
        print("connection refused", file=sys.stderr)
        sys.exit(1)
    done({"": "", "running": "keycloak-realm-import ", "complete": "keycloak-realm-import Complete",
          "failed": "keycloak-realm-import Failed"}[state])
if " run " in joined:
    overrides = json.loads(next(a for a in args if a.startswith("--overrides="))[len("--overrides="):])
    container = overrides["spec"]["containers"][0]
    env = {}
    for entry in container["env"]:
        if "value" in entry:
            env[entry["name"]] = entry["value"]
        else:
            ref = entry["valueFrom"]["secretKeyRef"]
            assert ref["name"] == "econ-graph-keycloak", ref
            env[entry["name"]] = {"admin-username": "admin", "admin-password": "test-admin-password"}[ref["key"]]
    assert env["KEYCLOAK_URL"] == "http://keycloak-service:8080/idp", env
    env["KEYCLOAK_URL"] = os.environ["STUB_KEYCLOAK_URL"]
    command = ["python3" if c == "python" else c for c in container["command"]]
    result = subprocess.run(command, env={"PATH": os.environ["PATH"], **env}, check=False)
    if result.returncode:
        print("pod %s terminated (Error)" % args[args.index("run") + 1], file=sys.stderr)
    sys.exit(1 if result.returncode else 0)
sys.exit(0)

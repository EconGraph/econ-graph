#!/usr/bin/env python3
"""Read-only CNPG backup/WAL health. No backup deletion or credentials access."""
import datetime as dt
import json
import os
from pathlib import Path
import ssl
import sys
import urllib.request

LIMITS = {"econgraph-app": 7200, "econgraph-data": 28800, "econgraph-keycloak": 7200}


def assess(clusters, backups, now):
    failures = []
    by_name = {item["metadata"]["name"]: item for item in clusters}
    for name, maximum_age in LIMITS.items():
        cluster = by_name.get(name)
        if not cluster:
            failures.append(f"{name}: cluster missing")
            continue
        status = cluster.get("status", {})
        if not any(c.get("type") == "ContinuousArchiving" and c.get("status") == "True"
                   for c in status.get("conditions", [])):
            failures.append(f"{name}: WAL archiving is not confirmed healthy")
        timestamp = status.get("lastSuccessfulBackup")
        try:
            age = (now - dt.datetime.fromisoformat(timestamp.replace("Z", "+00:00"))).total_seconds()
            if age > maximum_age or age < -300:
                failures.append(f"{name}: backup age outside approved check window")
        except (AttributeError, TypeError, ValueError):
            failures.append(f"{name}: no valid successful backup timestamp")
        own = [b for b in backups if b.get("spec", {}).get("cluster", {}).get("name") == name]
        if own:
            latest = max(own, key=lambda b: b.get("metadata", {}).get("creationTimestamp", ""))
            if latest.get("status", {}).get("phase", "").lower() == "failed":
                failures.append(f"{name}: latest requested backup failed")
    return failures


def main():
    service_account = Path("/var/run/secrets/kubernetes.io/serviceaccount")
    namespace = os.environ["NAMESPACE"]
    host = os.environ["KUBERNETES_SERVICE_HOST"]
    host = f"[{host}]" if ":" in host else host
    address = f"https://{host}:{os.environ.get('KUBERNETES_SERVICE_PORT_HTTPS', '443')}"
    # This projected, namespace-scoped token is used only for the intended API.
    token = (service_account / "token").read_text().strip()
    tls = ssl.create_default_context(cafile=str(service_account / "ca.crt"))
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPSHandler(context=tls))

    def read(resource):
        request = urllib.request.Request(
            f"{address}/apis/postgresql.cnpg.io/v1/namespaces/{namespace}/{resource}",
            headers={"Authorization": "Bearer " + token})
        with opener.open(request, timeout=15) as response:
            return json.load(response)["items"]

    try:
        failures = assess(read("clusters"), read("backups"), dt.datetime.now(dt.timezone.utc))
    except Exception as error:
        print(json.dumps({"healthy": False, "error": f"Kubernetes backup status unavailable: {type(error).__name__}"}))
        return 1
    print(json.dumps({"healthy": not failures, "failures": failures}, sort_keys=True))
    return int(bool(failures))


if __name__ == "__main__":
    sys.exit(main())

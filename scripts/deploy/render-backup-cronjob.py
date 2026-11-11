#!/usr/bin/env python3
"""Render the postgres-backup CronJob with the maintainer's public age recipient.

Usage: render-backup-cronjob.py RECIPIENT [MANIFEST]

Writes MANIFEST (default k8s/manifests/postgres-backup-cronjob.yaml) to stdout with
BACKUP_AGE_RECIPIENT on the encrypt container set to RECIPIENT. The committed manifest
leaves it empty, and encrypt.sh fails every run without one, so deploy.sh renders it
with the recipient the host dumps already use (BACKUP_AGE_RECIPIENT, from the
environment or ~/.config/econgraph/backup.env; scripts/backup/encryption.sh's
backup_recipient_load). RECIPIENT must be a native public age1 recipient: anything
else, a private AGE-SECRET-KEY identity included, is refused.
Tests: scripts/deploy/test-render-backup-cronjob.sh.
"""

import os
import re
import sys

import yaml

RECIPIENT_RE = re.compile(r"^age1[0-9a-z]+$")
CRONJOB = "postgres-backup"
CONTAINER = "encrypt"
VAR = "BACKUP_AGE_RECIPIENT"
DEFAULT_MANIFEST = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "..", "k8s", "manifests", "postgres-backup-cronjob.yaml"
)


def render(docs, recipient):
    """Set VAR on CONTAINER in the CronJob; returns how many values were set."""
    set_count = 0
    for doc in docs:
        if doc.get("kind") != "CronJob" or (doc.get("metadata") or {}).get("name") != CRONJOB:
            continue
        spec = doc["spec"]["jobTemplate"]["spec"]["template"]["spec"]
        for container in (spec.get("initContainers") or []) + (spec.get("containers") or []):
            if container.get("name") != CONTAINER:
                continue
            for var in container.get("env") or []:
                if var.get("name") == VAR:
                    var.pop("valueFrom", None)
                    var["value"] = recipient
                    set_count += 1
    return set_count


def main(argv):
    """Render the manifest named in argv (or the committed one) with argv[1] as recipient."""
    if len(argv) not in (2, 3):
        print("usage: render-backup-cronjob.py RECIPIENT [MANIFEST]", file=sys.stderr)
        return 2
    recipient = argv[1].strip()
    if not RECIPIENT_RE.match(recipient):
        print(
            "render-backup-cronjob: the recipient must be a public age1... recipient; "
            "never a private identity",
            file=sys.stderr,
        )
        return 2
    path = argv[2] if len(argv) == 3 else DEFAULT_MANIFEST
    with open(path, encoding="utf-8") as stream:
        docs = [d for d in yaml.safe_load_all(stream) if d]
    if render(docs, recipient) != 1:
        print(
            f"render-backup-cronjob: {path} has no single {VAR} on container {CONTAINER} "
            f"of CronJob {CRONJOB}",
            file=sys.stderr,
        )
        return 1
    yaml.safe_dump_all(docs, sys.stdout, sort_keys=False, default_flow_style=False)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

#!/usr/bin/env python3
"""Reject mutable external action references in workflows and composite actions."""

from pathlib import Path
import re
import sys

import yaml


def check_document(document, location):
    """Collect mutable action references while ignoring scalar script bodies."""
    errors = []
    if isinstance(document, dict):
        for key, value in document.items():
            here = f"{location}.{key}"
            if key == "uses":
                if not isinstance(value, str):
                    errors.append(f"{here}: uses must be a string")
                elif value.startswith("./"):
                    pass  # Local actions are versioned with the calling commit.
                elif value.startswith("docker://"):
                    if not re.fullmatch(r"docker://[^@\s]+@sha256:[0-9a-f]{64}", value):
                        errors.append(f"{here}: Docker actions need a sha256 digest: {value}")
                elif not re.fullmatch(r"[^@\s]+/[^@\s]+@[0-9a-f]{40}", value):
                    errors.append(f"{here}: external actions need a full commit SHA: {value}")
            errors.extend(check_document(value, here))
    elif isinstance(document, list):
        for index, value in enumerate(document):
            errors.extend(check_document(value, f"{location}[{index}]"))
    return errors


def main():
    """Validate workflow and composite-action YAML, returning a CI exit code."""
    root = Path(__file__).resolve().parents[2]
    files = sorted({path for directory in (root / ".github/workflows", root / ".github/actions")
                    for pattern in ("*.yml", "*.yaml") for path in directory.rglob(pattern)})
    if not files:
        print("No workflow files found", file=sys.stderr)
        return 1
    errors = []
    for path in files:
        try:
            errors.extend(check_document(yaml.safe_load(path.read_text()), str(path.relative_to(root))))
        except yaml.YAMLError as error:
            errors.append(f"{path.relative_to(root)}: {error}")
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print(f"External actions are pinned in {len(files)} YAML files")
    return 0


if __name__ == "__main__":
    sys.exit(main())

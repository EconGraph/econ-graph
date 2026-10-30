#!/usr/bin/env python3
"""Reject mutable or uncommented action references and persisted checkout credentials."""

from pathlib import Path
import re
import sys

import yaml


def check_document(document, location):
    """Check action references and checkout inputs, ignoring scalar script bodies."""
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
                # Include repository-root path aliases such as actions/checkout/.
                if (isinstance(value, str)
                        and value.split("@", 1)[0].lower().split("/")[:2] == ["actions", "checkout"]):
                    inputs = document.get("with")
                    if not isinstance(inputs, dict) or inputs.get("persist-credentials") is not False:
                        errors.append(f"{location}.with.persist-credentials: checkout must explicitly set false")
            errors.extend(check_document(value, here))
    elif isinstance(document, list):
        for index, value in enumerate(document):
            errors.extend(check_document(value, f"{location}[{index}]"))
    return errors


PINNED_USES = re.compile(r"""^\s*(?:-\s+)?["']?uses["']?\s*:\s*["']?([^\s"'#]+@[0-9a-f]{40})["']?\s*(#.*)?$""")
# A version such as v7, v0.36.0 or 1.99.1, not just any comment.
VERSION_COMMENT = re.compile(r"#\s*v?\d")


def pinned_references(document):
    """Every SHA-pinned external uses value in a parsed document."""
    if isinstance(document, dict):
        found = []
        for key, value in document.items():
            if key == "uses" and isinstance(value, str) and re.fullmatch(r"[^@\s]+/[^@\s]+@[0-9a-f]{40}", value):
                found.append(value)
            found.extend(pinned_references(value))
        return found
    if isinstance(document, list):
        return [found for value in document for found in pinned_references(value)]
    return []


def check_version_comments(text, location, document=None):
    """Require a version comment after every SHA-pinned uses line.

    With the parsed document, also fail when a pinned uses is not written as a
    single block-style line (a flow mapping or a folded scalar), since the line
    check cannot see its comment.
    """
    errors = []
    lines = 0
    for number, line in enumerate(text.splitlines(), 1):
        match = PINNED_USES.match(line)
        if match:
            lines += 1
            if not VERSION_COMMENT.match(match.group(2) or ""):
                errors.append(f"{location}:{number}: pinned action needs a version comment: {match.group(1)}")
    if document is not None and len(pinned_references(document)) != lines:
        errors.append(f"{location}: pinned uses must each be one block-style line with a version comment")
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
            text = path.read_text()
            location = str(path.relative_to(root))
            document = yaml.safe_load(text)
            errors.extend(check_document(document, location))
            errors.extend(check_version_comments(text, location, document))
        except yaml.YAMLError as error:
            errors.append(f"{path.relative_to(root)}: {error}")
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print(f"Actions are pinned and checkout credentials are not persisted in {len(files)} YAML files")
    return 0


if __name__ == "__main__":
    sys.exit(main())

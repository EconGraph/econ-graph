#!/usr/bin/env python3
"""Exit 0 for a directory, 3 for an absent optional root, 1 for unsafe inspection."""

import os
import stat
import sys


def check_root(path, required):
    try:
        metadata = os.stat(path)
    except FileNotFoundError:
        if not required:
            return 3
        print(f"configured Keycloak backup root is missing: {path}", file=sys.stderr)
        return 1
    except OSError:
        print(f"could not inspect Keycloak backup root: {path}", file=sys.stderr)
        return 1
    if not stat.S_ISDIR(metadata.st_mode):
        print(f"could not inspect Keycloak backup root (not a directory): {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(check_root(sys.argv[1], sys.argv[2] == "true"))

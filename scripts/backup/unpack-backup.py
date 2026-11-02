#!/usr/bin/env python3
"""Extract an operator-decrypted backup stream into a new private directory."""
import os
from pathlib import Path, PurePosixPath
import shutil
import sys
import tarfile

root = Path(sys.argv[1])
with tarfile.open(fileobj=sys.stdin.buffer, mode="r|") as archive:
    for member in archive:
        name = PurePosixPath(member.name)
        if name.is_absolute() or ".." in name.parts:
            raise ValueError("unsafe backup path")
        target = root.joinpath(*name.parts)
        if member.isdir():
            target.mkdir(mode=0o700, parents=True, exist_ok=True)
        elif member.isfile():
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            with os.fdopen(os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as output:
                with archive.extractfile(member) as source:
                    shutil.copyfileobj(source, output)
        else:
            raise ValueError("backup contains an unsupported member")

# Consume trailing padding so age must finish authentication before a consumer
# is allowed to run. Do not retain arbitrarily large trailing input in memory.
while sys.stdin.buffer.read(1024 * 1024):
    pass

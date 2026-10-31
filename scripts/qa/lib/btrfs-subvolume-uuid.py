#!/usr/bin/env python3
"""Read a btrfs subvolume's UUID without privileged tree searches.

GET_SUBVOL_INFO is a read-only ioctl available to the owner of an open directory.
Unlike `btrfs subvolume show`, it does not search the filesystem's root tree.
"""

import argparse
import fcntl
import os
import sys
import uuid

# Linux UAPI: struct btrfs_ioctl_get_subvol_info_args is 504 bytes; its UUID
# follows treeid, name[256], parent_id, dirid, generation and flags (296 bytes).
# _IOR(0x94, 60, struct btrfs_ioctl_get_subvol_info_args).
BTRFS_IOC_GET_SUBVOL_INFO = 0x81F8943C
SUBVOL_INFO_SIZE = 504
UUID_OFFSET = 296


def subvolume_uuid(path, *, require_root=True):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        if require_root and os.fstat(fd).st_ino != 256:
            raise ValueError(f'{path} is not a btrfs subvolume root')
        info = bytearray(SUBVOL_INFO_SIZE)
        fcntl.ioctl(fd, BTRFS_IOC_GET_SUBVOL_INFO, info, True)
        value = uuid.UUID(bytes=bytes(info[UUID_OFFSET:UUID_OFFSET + 16]))
        if value.int == 0:
            raise ValueError(f'{path} has no subvolume UUID')
        return str(value)
    finally:
        os.close(fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true',
                        help='check metadata access on a containing directory before creating storage')
    parser.add_argument('path')
    args = parser.parse_args()
    try:
        value = subvolume_uuid(args.path, require_root=not args.check)
    except (OSError, ValueError) as exc:
        print(f'cannot read btrfs subvolume identity: {exc}', file=sys.stderr)
        return 1
    print(value)
    return 0


if __name__ == '__main__':
    sys.exit(main())

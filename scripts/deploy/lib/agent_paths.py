#!/usr/bin/env python3
"""Check that the agent kubeconfig's paths are private to the invoking user.

Used by create-agent-kubeconfig.sh and run-agent-session.sh; not run by hand.

  check-chain PATH
      Every directory traversed to reach PATH, following symlinks component by
      component (so the directory holding a symlink is checked too, not only
      where it points), must be changeable only by you or root: owned by you
      or root, and, unless sticky (like /tmp), not writable by others, not
      group-writable except by your own private group, and with no POSIX ACL
      entry giving another user or group write access. A symlink inside a
      sticky directory must be owned by you or root. A missing tail of PATH is
      fine: it is created under the last checked directory.

  check-private LABEL dir|file MODE PATH
      PATH is a directory or regular file, not a symlink, owned by you, with
      exactly MODE (octal) and no extended or default POSIX ACL.

Your private group is your primary group when it has your user name, no
listed members and is no other user's primary group.

Only POSIX ACLs are read. An error reading one, other than "no ACL" or "ACLs
not supported here", is refused. NFSv4 ACLs (system.nfs4_acl) are not read, so
a write grant made only through one is not seen; don't use an NFSv4 mount.

Exit status 1 with an error naming LABEL or the offending directory.
"""

import errno
import grp
import os
import pwd
import stat
import struct
import sys

ACL_ACCESS = "system.posix_acl_access"
ACL_DEFAULT = "system.posix_acl_default"
# Tags of the Linux POSIX ACL xattr format (linux/posix_acl_xattr.h).
ACL_USER, ACL_GROUP, ACL_MASK = 0x02, 0x08, 0x10
WRITE = 0o2


def fail(message):
    print(f"error: {message}", file=sys.stderr)
    sys.exit(1)


def acl_entries(path, name):
    """(tag, perm, id) entries of a POSIX ACL xattr; [] when there is none."""
    try:
        data = os.getxattr(path, name, follow_symlinks=False)
    except OSError as error:
        if error.errno in (errno.ENODATA, errno.ENOTSUP):
            return []
        fail(f"cannot read the ACL of {path}: {error.strerror}")
    return [struct.unpack_from("<HHI", data, offset) for offset in range(4, len(data) - 7, 8)]


def own_private_group(gid):
    try:
        me = pwd.getpwuid(os.getuid()).pw_name
        group = grp.getgrgid(gid)
    except KeyError:
        return False
    if gid != os.getgid() or group.gr_name != me:
        return False
    if any(member != me for member in group.gr_mem):
        return False
    return not any(user.pw_gid == gid and user.pw_name != me for user in pwd.getpwall())


def trusted(path, st):
    if st.st_uid not in (0, os.getuid()):
        return False
    if st.st_mode & stat.S_ISVTX:
        return True
    if st.st_mode & 0o002:
        return False
    if st.st_mode & 0o020 and not own_private_group(st.st_gid):
        return False
    entries = acl_entries(path, ACL_ACCESS)
    mask = next((perm for tag, perm, _ in entries if tag == ACL_MASK), 0o7)
    for tag, perm, ident in entries:
        if not perm & mask & WRITE:
            continue
        if tag == ACL_USER and ident != os.getuid():
            return False
        if tag == ACL_GROUP and not own_private_group(ident):
            return False
    return True


def check_chain(path):
    if not path.startswith("/"):
        fail(f"{path} is not absolute")
    current = "/"
    current_st = os.lstat(current)
    if not trusted(current, current_st):
        fail("/ can be changed by other users; choose another --output-dir")
    pending = path.split("/")
    hops = 0
    while pending:
        name = pending.pop(0)
        if name in ("", "."):
            continue
        if name == "..":
            current = os.path.dirname(current)
            current_st = os.lstat(current)
            continue
        candidate = os.path.join(current, name)
        try:
            st = os.lstat(candidate)
        except FileNotFoundError:
            return
        except OSError as error:
            fail(f"cannot check {candidate}: {error.strerror}")
        if stat.S_ISLNK(st.st_mode):
            # The link sits in `current`, already checked; now walk its target.
            # In a sticky directory others may add entries, and the kernel's
            # protected_symlinks covers only world-writable ones: the link
            # itself must then be yours or root's.
            if current_st.st_mode & stat.S_ISVTX and st.st_uid not in (0, os.getuid()):
                fail(f"{candidate} is a symlink owned by another user in a sticky directory")
            hops += 1
            if hops > 40:
                fail(f"too many symlinks resolving {path}")
            target = os.readlink(candidate)
            if target.startswith("/"):
                current = "/"
                current_st = os.lstat(current)
            pending = target.split("/") + pending
            continue
        if not stat.S_ISDIR(st.st_mode):
            fail(f"{candidate} is not a directory")
        if not trusted(candidate, st):
            fail(f"{candidate} can be changed by other users; choose another --output-dir")
        current, current_st = candidate, st


def check_private(label, kind, mode, path):
    try:
        st = os.lstat(path)
    except FileNotFoundError:
        fail(f"{label} {path} is missing")
    if stat.S_ISLNK(st.st_mode):
        fail(f"{label} is a symlink")
    if kind == "dir" and not stat.S_ISDIR(st.st_mode):
        fail(f"{label} is not a directory")
    if kind == "file" and not stat.S_ISREG(st.st_mode):
        fail(f"{label} is not a regular file")
    if st.st_uid != os.getuid():
        fail(f"{label} must be owned by you")
    # Before the mode: with an extended ACL the group mode bits show its mask.
    if any(tag in (ACL_USER, ACL_GROUP, ACL_MASK) for tag, _, _ in acl_entries(path, ACL_ACCESS)) \
            or acl_entries(path, ACL_DEFAULT):
        fail(f"{label} has an extended ACL; remove it (setfacl -b)")
    if stat.S_IMODE(st.st_mode) != int(mode, 8):
        fail(f"{label} has mode {stat.S_IMODE(st.st_mode):o}, expected {mode}")


def main(argv):
    if len(argv) == 2 and argv[0] == "check-chain":
        check_chain(argv[1])
    elif len(argv) == 5 and argv[0] == "check-private" and argv[2] in ("dir", "file"):
        check_private(*argv[1:])
    else:
        fail("usage: agent_paths.py check-chain PATH | check-private LABEL dir|file MODE PATH")


if __name__ == "__main__":
    main(sys.argv[1:])

"""Unit tests for agent_paths.py; no Kubernetes, no root, nothing outside a private temp dir.

Run: python3 scripts/deploy/lib/test_agent_paths.py
"""
import errno
import grp
import importlib.util
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "agent_paths.py"
spec = importlib.util.spec_from_file_location("agent_paths", SCRIPT)
agent_paths = importlib.util.module_from_spec(spec)
spec.loader.exec_module(agent_paths)

try:
    ME = pwd.getpwuid(os.getuid()).pw_name
except KeyError:  # no passwd entry, as in some containers
    ME = None
OTHER_UID = next(uid for uid in range(60000, 70000) if uid != os.getuid())


def as_owner(st, uid):
    """A copy of stat result `st` with st_uid replaced."""
    fields = list(st[:10])
    fields[4] = uid
    return os.stat_result(fields)


class PrivateBase(unittest.TestCase):
    """Each test works in its own 0700 directory with a 077 umask, so neither
    the host's group or umask nor its /var, /srv, /opt and /run matter."""

    def setUp(self):
        old_umask = os.umask(0o077)
        self.addCleanup(os.umask, old_umask)
        # Resolved, so the paths the patched lstat compares are the ones walked.
        self.base = Path(os.path.realpath(tempfile.mkdtemp(prefix="agent-paths-")))
        self.addCleanup(shutil.rmtree, self.base)
        os.chmod(self.base, 0o700)
        # The chain to the base must be acceptable, or the tests below prove nothing.
        agent_paths.check_chain(str(self.base))

    def refused(self, call, message):
        with mock.patch("sys.stderr") as stderr, self.assertRaises(SystemExit) as raised:
            call()
        self.assertEqual(raised.exception.code, 1)
        written = "".join(c.args[0] for c in stderr.write.call_args_list)
        self.assertIn(message, written)


class TrustedTests(PrivateBase):
    def test_own_directory_is_trusted(self):
        self.assertTrue(agent_paths.trusted(str(self.base), os.lstat(self.base)))

    def test_directory_owned_by_another_user_is_refused(self):
        # 0700 and no ACL: only the owner rule can refuse it.
        st = as_owner(os.lstat(self.base), OTHER_UID)
        self.assertFalse(agent_paths.trusted(str(self.base), st))

    def test_sticky_directory_owned_by_another_user_is_refused(self):
        os.chmod(self.base, 0o1777)
        st = as_owner(os.lstat(self.base), OTHER_UID)
        self.assertFalse(agent_paths.trusted(str(self.base), st))

    def test_chain_through_a_directory_owned_by_another_user_is_refused(self):
        (self.base / "theirs").mkdir()
        real_lstat = os.lstat
        theirs = str(self.base / "theirs")

        def lstat(path):
            st = real_lstat(path)
            return as_owner(st, OTHER_UID) if os.fspath(path) == theirs else st

        with mock.patch.object(agent_paths.os, "lstat", lstat):
            self.refused(lambda: agent_paths.check_chain(theirs + "/agent"),
                         f"{theirs} can be changed by other users")


@unittest.skipIf(ME is None, "no passwd entry for this uid")
class OwnPrivateGroupTests(unittest.TestCase):
    def patched(self, members=(), other_primary=False, alias_members=None):
        group = grp.struct_group((ME, "x", os.getgid(), list(members)))
        groups = [group]
        if alias_members is not None:
            # A second /etc/group entry with the same GID.
            groups.append(grp.struct_group(("alias", "x", os.getgid(), list(alias_members))))
        users = [pwd.struct_passwd((ME, "x", os.getuid(), os.getgid(), "", "/", "/bin/sh"))]
        if other_primary:
            users.append(pwd.struct_passwd(("mallory", "x", OTHER_UID, os.getgid(), "", "/", "/bin/sh")))
        return [
            mock.patch.object(agent_paths.grp, "getgrgid", return_value=group),
            mock.patch.object(agent_paths.grp, "getgrall", return_value=groups),
            mock.patch.object(agent_paths.pwd, "getpwall", return_value=users),
        ]

    def check(self, **kwargs):
        patches = self.patched(**kwargs)
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        return agent_paths.own_private_group(os.getgid())

    def test_group_with_only_you_is_private(self):
        self.assertTrue(self.check())
        self.assertTrue(self.check(members=[ME]))

    def test_group_with_another_member_is_not_private(self):
        self.assertFalse(self.check(members=["mallory"]))

    def test_another_entry_with_the_same_gid_and_a_member_is_not_private(self):
        self.assertFalse(self.check(alias_members=["mallory"]))

    def test_group_that_is_another_users_primary_group_is_not_private(self):
        self.assertFalse(self.check(other_primary=True))

    def test_other_gid_is_not_private(self):
        self.assertFalse(agent_paths.own_private_group(os.getgid() + 1))


class AclReadTests(PrivateBase):
    def getxattr_raising(self, code):
        return mock.patch.object(agent_paths.os, "getxattr", side_effect=OSError(code, os.strerror(code)))

    def test_no_acl_and_no_acl_support_mean_no_entries(self):
        for code in (errno.ENODATA, errno.ENOTSUP):
            with self.subTest(code=errno.errorcode[code]), self.getxattr_raising(code):
                self.assertEqual(agent_paths.acl_entries(str(self.base), agent_paths.ACL_ACCESS), [])
                self.assertTrue(agent_paths.trusted(str(self.base), os.lstat(self.base)))

    def test_other_errors_are_refused(self):
        for code in (errno.EIO, errno.EACCES, errno.ERANGE):
            with self.subTest(code=errno.errorcode[code]), self.getxattr_raising(code):
                self.refused(lambda: agent_paths.trusted(str(self.base), os.lstat(self.base)),
                             f"cannot read the ACL of {self.base}")


class SymlinkTests(PrivateBase):
    def test_symlink_loop_is_refused_without_hanging(self):
        (self.base / "a").symlink_to("b")
        (self.base / "b").symlink_to("a")
        try:
            result = subprocess.run([sys.executable, str(SCRIPT), "check-chain", str(self.base / "a" / "agent")],
                                    capture_output=True, text=True, timeout=10)
        except subprocess.TimeoutExpired:
            self.fail("check-chain did not finish on a symlink loop")
        self.assertEqual(result.returncode, 1)
        self.assertIn("too many symlinks", result.stderr)

    def sticky_with_link(self):
        sticky = self.base / "sticky"
        sticky.mkdir()
        os.chmod(sticky, 0o1770)
        (self.base / "target").mkdir()
        (sticky / "link").symlink_to(self.base / "target")
        return sticky / "link"

    def test_link_reached_through_dotdot_is_judged_by_the_sticky_directory(self):
        link = self.sticky_with_link()
        (link.parent / "sub").mkdir()
        real_lstat = os.lstat

        def lstat(path):
            st = real_lstat(path)
            return as_owner(st, OTHER_UID) if os.fspath(path) == str(link) else st

        with mock.patch.object(agent_paths.os, "lstat", lstat):
            self.refused(lambda: agent_paths.check_chain(f"{link.parent}/sub/../link/agent"),
                         f"{link} is a symlink owned by another user in a sticky directory")

    def test_dotdot_after_a_missing_component_is_refused(self):
        self.refused(lambda: agent_paths.check_chain(f"{self.base}/missing/../../../../tmp/agent"),
                     f"has .. after {self.base}/missing, which is missing")

    def test_your_own_link_in_a_sticky_directory_is_followed(self):
        link = self.sticky_with_link()
        agent_paths.check_chain(str(link / "agent"))

    def test_link_owned_by_another_user_in_a_sticky_directory_is_refused(self):
        link = self.sticky_with_link()
        real_lstat = os.lstat

        def lstat(path):
            st = real_lstat(path)
            return as_owner(st, OTHER_UID) if os.fspath(path) == str(link) else st

        with mock.patch.object(agent_paths.os, "lstat", lstat):
            self.refused(lambda: agent_paths.check_chain(str(link / "agent")),
                         f"{link} is a symlink owned by another user in a sticky directory")

    def test_link_owned_by_another_user_in_a_private_directory_is_followed(self):
        # Only you can add entries there, so the link's owner does not matter.
        (self.base / "target").mkdir()
        link = self.base / "link"
        link.symlink_to(self.base / "target")
        real_lstat = os.lstat

        def lstat(path):
            st = real_lstat(path)
            return as_owner(st, OTHER_UID) if os.fspath(path) == str(link) else st

        with mock.patch.object(agent_paths.os, "lstat", lstat):
            agent_paths.check_chain(str(link / "agent"))


if __name__ == "__main__":
    unittest.main()

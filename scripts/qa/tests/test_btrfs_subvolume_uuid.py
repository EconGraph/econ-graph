"""Read-only ioctl contract tests; no real filesystem or cluster is changed."""
import importlib.util
import io
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import uuid

SPEC = importlib.util.spec_from_file_location(
    'subvolume_uuid', Path(__file__).resolve().parents[1] / 'lib/btrfs-subvolume-uuid.py')
helper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(helper)


class SubvolumeUuidTests(unittest.TestCase):
    def setUp(self):
        self.expected = uuid.UUID('44da6907-4da6-4faf-a0db-192b874d6129')
        self.calls = []
        self.inode = 256
        self.error = None
        self.zero_uuid = False
        for name, value in [('open', lambda *a: 9),
                            ('fstat', lambda fd: SimpleNamespace(st_ino=self.inode)),
                            ('close', lambda fd: self.calls.append(('close', fd)))]:
            p = patch.object(helper.os, name, value)
            p.start()
            self.addCleanup(p.stop)
        p = patch.object(helper.fcntl, 'ioctl', self.ioctl)
        p.start()
        self.addCleanup(p.stop)

    def ioctl(self, fd, request, data, mutate):
        self.assertEqual((fd, request, len(data), mutate), (9, 0x81F8943C, 504, True))
        if self.error:
            raise self.error
        if not self.zero_uuid:
            data[296:312] = self.expected.bytes

    def test_returns_uuid_from_kernel_layout(self):
        self.assertEqual(helper.subvolume_uuid('/qa/storage'), str(self.expected))
        self.assertEqual(self.calls, [('close', 9)])

    def test_ordinary_directory_is_refused(self):
        self.inode = 123
        with self.assertRaisesRegex(ValueError, 'not a btrfs subvolume root'):
            helper.subvolume_uuid('/qa/storage')
        self.assertEqual(self.calls, [('close', 9)])

    def test_preflight_accepts_directory_inside_subvolume(self):
        self.inode = 123
        self.assertEqual(helper.subvolume_uuid('/qa', require_root=False), str(self.expected))

    def test_ioctl_failure_is_propagated_and_fd_closed(self):
        self.error = PermissionError('metadata unreadable')
        with self.assertRaises(PermissionError):
            helper.subvolume_uuid('/qa/storage')
        self.assertEqual(self.calls, [('close', 9)])

    def test_preflight_accepts_top_level_tree_without_uuid(self):
        self.inode = 123
        self.zero_uuid = True
        self.assertEqual(helper.subvolume_uuid('/qa', require_root=False), str(uuid.UUID(int=0)))

    def test_check_cli_accepts_directory_inside_subvolume(self):
        self.inode = 123
        with patch.object(helper.sys, 'argv', ['btrfs-subvolume-uuid.py', '--check', '/qa']), \
                patch.object(helper.sys, 'stdout', new_callable=io.StringIO) as output:
            self.assertEqual(helper.main(), 0)
        self.assertEqual(output.getvalue().strip(), str(self.expected))
        self.assertEqual(self.calls, [('close', 9)])

    def test_check_cli_accepts_top_level_tree_without_uuid(self):
        self.inode = 123
        self.zero_uuid = True
        with patch.object(helper.sys, 'argv', ['btrfs-subvolume-uuid.py', '--check', '/qa']), \
                patch.object(helper.sys, 'stdout', new_callable=io.StringIO) as output:
            self.assertEqual(helper.main(), 0)
        self.assertEqual(output.getvalue().strip(), str(uuid.UUID(int=0)))
        self.assertEqual(self.calls, [('close', 9)])

    def test_zero_uuid_is_refused(self):
        self.zero_uuid = True
        with self.assertRaisesRegex(ValueError, 'no subvolume UUID'):
            helper.subvolume_uuid('/qa/storage')

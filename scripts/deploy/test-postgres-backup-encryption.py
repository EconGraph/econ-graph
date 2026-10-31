#!/usr/bin/env python3
"""Exercise the deployed encryption script with real age and disposable keys."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = yaml.safe_load((ROOT / 'k8s/manifests/postgres-backup-configmap.yaml').read_text())['data']


class EncryptionTest(unittest.TestCase):
    def setUp(self):
        """Create isolated synthetic source, ciphertext and operator-identity paths."""
        self.tmp = tempfile.TemporaryDirectory(dir="/dev/shm")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.source = self.root / 'plaintext'
        self.dest = self.root / 'ciphertexts'
        self.source.mkdir()
        self.dest.mkdir()
        self.status = self.root / 'failed'
        self.status.write_text('')
        self.script = self.root / 'encrypt.sh'
        self.script.write_text(SCRIPTS['encrypt.sh'])
        self.age = os.environ['AGE_BIN']
        self.keygen = os.environ['AGE_KEYGEN_BIN']
        self.identity = self.root / 'operator-identity'
        subprocess.run([self.keygen, '-o', str(self.identity)], check=True, capture_output=True)
        self.recipient = subprocess.check_output([self.keygen, '-y', str(self.identity)], text=True).strip()
        self.payload = b'synthetic Keycloak and app fixture; never production data\n' * 100
        self.dump = self.source / 'keycloak-20261026-000000.dump'
        self.dump.write_bytes(self.payload)
        (self.source / 'latest-keycloak.dump').symlink_to(self.dump)

    def encrypt(self, **overrides):
        """Run the ConfigMap script without an identity in its environment or mounts."""
        env = dict(os.environ, BACKUP_AGE_RECIPIENT=self.recipient, AGE_BIN=self.age,
                   BACKUP_PLAINTEXT_DIR=str(self.source), BACKUP_DIR=str(self.dest),
                   BACKUP_STATUS_FILE=str(self.status))
        env.update(overrides)
        return subprocess.run(['sh', str(self.script)], env=env, capture_output=True)

    def test_roundtrip_permissions_and_plaintext_isolation(self):
        """An operator can decrypt the ciphertext; only ciphertext reaches the PVC path."""
        result = self.encrypt()
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.dest / (self.dump.name + '.age')
        self.assertTrue(archive.read_bytes().startswith(b'age-encryption.org/v1\n'))
        self.assertNotIn(self.payload, archive.read_bytes())
        self.assertEqual(archive.stat().st_mode & 0o777, 0o600)
        self.assertEqual(subprocess.check_output([self.age, '-d', '-i', str(self.identity), str(archive)]),
                         self.payload)
        self.assertEqual((self.dest / 'latest-keycloak.dump.age').resolve(), archive)
        self.assertEqual(sorted(p.name for p in self.dest.iterdir()),
                         sorted([archive.name, 'latest-keycloak.dump.age']))
        self.assertFalse(self.dump.exists())

    def test_missing_and_invalid_recipients_fail_without_output(self):
        """Misconfiguration cannot fall back to publishing plaintext."""
        for recipient in ('', 'AGE-SECRET-KEY-example', 'age1invalid'):
            with self.subTest(recipient=recipient):
                self.assertNotEqual(self.encrypt(BACKUP_AGE_RECIPIENT=recipient).returncode, 0)
                self.assertEqual(list(self.dest.iterdir()), [])

    def test_missing_dump_status_refuses_publication(self):
        """A crashed dump stage cannot be mistaken for a successful backup."""
        self.status.unlink()
        self.assertNotEqual(self.encrypt().returncode, 0)
        self.assertEqual(list(self.dest.iterdir()), [])

    def test_collision_preserves_existing_backup_and_fails_job_status(self):
        """A rerun cannot overwrite an existing archive or follow a partial symlink."""
        archive = self.dest / (self.dump.name + '.age')
        archive.write_bytes(b'existing encrypted backup')
        previous = self.dest / 'keycloak-20261025-000000.dump.age'
        previous.write_bytes(b'previous good archive')
        latest = self.dest / 'latest-keycloak.dump.age'
        latest.symlink_to(previous.name)
        self.assertEqual(self.encrypt().returncode, 0)
        self.assertEqual(archive.read_bytes(), b'existing encrypted backup')
        self.assertIn('encryption:', self.status.read_text())
        self.assertEqual(latest.resolve(), previous)
        archive.unlink()
        partial = self.dest / (self.dump.name + '.age.partial')
        partial.symlink_to(self.dump)
        self.encrypt()
        self.assertTrue(partial.is_symlink())
        self.assertEqual(self.dump.read_bytes(), self.payload)

    def test_failed_encryption_does_not_publish_a_partial(self):
        """A failing encryptor records failure and leaves no misleading final archive."""
        broken = self.root / 'broken-age'
        broken.write_text('#!/bin/sh\ndata=$(cat)\n[ -z "$data" ] && exit 0\nprintf partial-output\nexit 1\n')
        broken.chmod(0o755)
        self.assertEqual(self.encrypt(AGE_BIN=str(broken)).returncode, 0)
        self.assertIn('encryption:', self.status.read_text())
        self.assertEqual(list(self.dest.iterdir()), [])

    def test_real_container_as_uid_999_with_readonly_root(self):
        """The production image can install and encrypt on its actual restricted runtime."""
        image = os.environ.get('BACKUP_ENCRYPT_IMAGE')
        if not image:
            self.skipTest('Run through test-postgres-backup-scripts.sh for container coverage')
        # Rootful Docker runs in the host's mount namespace. Script fixtures
        # must use the shared workspace, not this process's private /dev/shm.
        mount_tmp = tempfile.TemporaryDirectory(dir=ROOT)
        self.addCleanup(mount_tmp.cleanup)
        mount_root = Path(mount_tmp.name)
        installer = mount_root / 'install-age.sh'
        installer.write_text(SCRIPTS['install-age.sh'])
        installer.chmod(0o555)
        encrypter = mount_root / 'encrypt.sh'
        encrypter.write_text(SCRIPTS['encrypt.sh'])
        encrypter.chmod(0o555)
        command = 'set -eu\nsh /scripts/install-age.sh >&2\nprintf \'%s\' "$SYNTHETIC_PAYLOAD" >/plaintext/keycloak-20261026-000000.dump\nprintf \'\' >/status/failed\nsh /scripts/encrypt.sh >&2\ntest "$(stat -c %a /backups/keycloak-20261026-000000.dump.age)" = 600\ntest ! -e /plaintext/keycloak-20261026-000000.dump\ncat /backups/keycloak-20261026-000000.dump.age\n'
        args = ['docker', 'run', '--rm', '--user', '999:999', '--read-only',
                '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges']
        for directory in ('tools', 'plaintext', 'backups', 'status'):
            args.extend(['--tmpfs', f'/{directory}:rw,exec,uid=999,gid=999,mode=0700'])
        args.extend(['-v', f'{installer}:/scripts/install-age.sh:ro',
                     '-v', f'{encrypter}:/scripts/encrypt.sh:ro',
                     '-e', 'BACKUP_AGE_RECIPIENT=' + self.recipient,
                     '-e', 'BACKUP_STATUS_FILE=/status/failed',
                     '-e', 'SYNTHETIC_PAYLOAD=' + self.payload.decode(),
                     '--entrypoint', '/bin/sh', image, '-c', command])
        encrypted = subprocess.check_output(args)
        decrypted = subprocess.check_output([self.age, '-d', '-i', str(self.identity)], input=encrypted)
        self.assertEqual(decrypted, self.payload)

    def test_disk_backed_plaintext_source_is_refused(self):
        """A miswired source cannot silently place unencrypted archives on disk."""
        with tempfile.TemporaryDirectory(dir=ROOT) as source:
            result = self.encrypt(BACKUP_PLAINTEXT_DIR=source)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(list(self.dest.iterdir()), [])

    def test_original_dump_failure_survives_encryption(self):
        """Successful dumps can be encrypted while the Job still reports another failure."""
        self.status.write_text(' econ_graph_app\n')
        self.assertEqual(self.encrypt().returncode, 0)
        self.assertIn('econ_graph_app', self.status.read_text())
        self.assertTrue((self.dest / (self.dump.name + '.age')).exists())

    def test_default_restore_refuses_encrypted_archives_with_missing_latest_metadata(self):
        """Present, dangling or missing latest metadata cannot select stale plaintext."""
        restore = self.root / 'restore.sh'
        restore.write_text(SCRIPTS['restore.sh'])
        old = self.dest / 'econ_graph-20261025-000000.dump'
        old.write_bytes(b'older plaintext fixture')
        (self.dest / 'latest.dump').symlink_to(old.name)
        encrypted = self.dest / 'econ_graph-20261026-000000.dump.age'
        (self.dest / 'latest.dump.age').symlink_to(encrypted.name)
        binary_dir = self.root / 'bin'
        binary_dir.mkdir()
        marker = self.root / 'database-command-called'
        for name in ('psql', 'pg_restore'):
            binary = binary_dir / name
            binary.write_text('#!/bin/sh\ntouch "$DATABASE_CALL_MARKER"\nexit 99\n')
            binary.chmod(0o755)
        env = dict(os.environ, BACKUP_DIR=str(self.dest), BACKUP_FILE='',
                   PGDATABASE='econ_graph', PGHOST='unused', PGPORT='5432', PGUSER='postgres',
                   PATH=str(binary_dir) + os.pathsep + os.environ['PATH'],
                   DATABASE_CALL_MARKER=str(marker))
        for metadata in ('dangling', 'present', 'missing'):
            with self.subTest(latest_metadata=metadata):
                if metadata == 'present':
                    encrypted.write_bytes(b'ciphertext fixture')
                elif metadata == 'missing':
                    (self.dest / 'latest.dump.age').unlink()
                result = subprocess.run(['sh', str(restore)], env=env, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b'refusing default plaintext restore', result.stderr)
                self.assertFalse(marker.exists())
        # Explicit recovery of a retained plaintext archive is still an operator choice.
        env['BACKUP_FILE'] = str(old)
        subprocess.run(['sh', str(restore)], env=env, capture_output=True)
        self.assertTrue(marker.exists())

    def test_manifest_keeps_plaintext_and_private_keys_off_pvc(self):
        """The production dump stage sees tmpfs and the encrypt stage has only a public key."""
        spec = yaml.safe_load((ROOT / 'k8s/manifests/postgres-backup-cronjob.yaml').read_text())[
            'spec']['jobTemplate']['spec']['template']['spec']
        volumes = {v['name']: v for v in spec['volumes']}
        self.assertEqual(volumes['plaintext']['emptyDir']['medium'], 'Memory')
        dump = next(c for c in spec['initContainers'] if c['name'] == 'pg-dump')
        self.assertNotIn('backup-data', {v['name'] for v in dump['volumeMounts']})
        self.assertEqual(next(e['value'] for e in dump['env'] if e['name'] == 'BACKUP_DIR'), '/plaintext')
        encrypt = next(c for c in spec['initContainers'] if c['name'] == 'encrypt')
        self.assertEqual({e['name'] for e in encrypt['env']}, {'BACKUP_AGE_RECIPIENT', 'BACKUP_STATUS_FILE'})
        self.assertFalse(any('secret' in v for v in volumes.values()))
        self.assertEqual(spec['securityContext']['runAsUser'], 999)
        for name in ('install-age', 'encrypt'):
            container = next(c for c in spec['initContainers'] if c['name'] == name)
            self.assertIn('@sha256:', container['image'])
            self.assertTrue(container['securityContext']['readOnlyRootFilesystem'])


if __name__ == '__main__':
    unittest.main()

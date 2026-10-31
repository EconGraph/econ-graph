#!/usr/bin/env python3
"""The release lab's restore against a real PostgreSQL 18, with synthetic dumps only.

Starts one uniquely named postgres:18 container (removed afterwards) on the Docker daemon
DOCKER_HOST names (the rootless one in an operator's shell), publishing its port on
127.0.0.1 only. Without DOCKER_HOST it refuses to run, except in CI (CI=true), where the
runner's default socket is used: on the production host that socket is the rootful
daemon beside the production cluster. It builds synthetic data, app and Keycloak
databases, dumps them with pg_dump -Fc, and runs release-lab.py's own restore,
Keycloak clean-up and row counts through a stand-in kubectl that turns
`kubectl exec POD -c C -- CMD` into `docker exec CONTAINER CMD`. It never reads a real
dump, a cluster or production configuration. Scratch lives under $RELEASE_LAB_TEST_DIR,
else ~/.cache/econgraph-release-lab-tests, and is removed afterwards.

Run: python3 scripts/deploy/test-release-lab-restore.py [-v]
"""
import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
import uuid

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location('release_lab', HERE / 'release-lab.py')
lab = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(lab)
DOCKER_HOST = os.environ.get('DOCKER_HOST') or ('unix:///var/run/docker.sock' if os.environ.get('CI') == 'true'
                                                 else None)
DOCKER = ['docker', f'--host={DOCKER_HOST}']

SOURCES = {
    'data': ('src_data', 'econgraph', '''
        CREATE TABLE economic_series (id serial PRIMARY KEY, title text NOT NULL);
        CREATE TABLE data_points (series_id int REFERENCES economic_series, value numeric);
        INSERT INTO economic_series (title) SELECT 'synthetic ' || g FROM generate_series(1, 7) g;
        INSERT INTO data_points SELECT 1 + g % 7, g FROM generate_series(1, 50) g;
    '''),
    'app': ('src_app', 'econgraph', '''
        CREATE TABLE users (id serial PRIMARY KEY, email text);
        CREATE TABLE chart_annotations (id serial PRIMARY KEY, user_id int REFERENCES users, note text);
        INSERT INTO users (email) SELECT 'user' || g || '@example.invalid' FROM generate_series(1, 3) g;
        INSERT INTO chart_annotations (user_id, note) VALUES (1, 'a'), (2, 'b');
    '''),
    # The Keycloak tables the lab clears, with their foreign keys, beside two it keeps.
    'keycloak': ('src_keycloak', 'keycloak', '''
        CREATE TABLE realm (id text PRIMARY KEY);
        CREATE TABLE user_entity (id text PRIMARY KEY, realm_id text REFERENCES realm);
        CREATE TABLE identity_provider (internal_id text PRIMARY KEY, realm_id text REFERENCES realm);
        CREATE TABLE identity_provider_config (identity_provider_id text REFERENCES identity_provider,
                                               name text, value text);
        CREATE TABLE identity_provider_mapper (id text PRIMARY KEY, realm_id text REFERENCES realm);
        CREATE TABLE idp_mapper_config (idp_mapper_id text REFERENCES identity_provider_mapper, value text);
        CREATE TABLE federated_identity (user_id text REFERENCES user_entity, identity_provider text);
        CREATE TABLE offline_user_session (user_session_id text PRIMARY KEY, user_id text);
        CREATE TABLE offline_client_session (user_session_id text REFERENCES offline_user_session, data text);
        CREATE TABLE realm_smtp_config (realm_id text REFERENCES realm, value text, name text);
        INSERT INTO realm VALUES ('master'), ('econ-graph');
        INSERT INTO user_entity VALUES ('u1', 'econ-graph'), ('u2', 'econ-graph'), ('admin', 'master');
        INSERT INTO identity_provider VALUES ('google', 'econ-graph');
        INSERT INTO identity_provider_config VALUES ('google', 'clientSecret', 'synthetic');
        INSERT INTO identity_provider_mapper VALUES ('m1', 'econ-graph');
        INSERT INTO idp_mapper_config VALUES ('m1', 'x');
        INSERT INTO federated_identity VALUES ('u1', 'google');
        INSERT INTO offline_user_session VALUES ('s1', 'u1');
        INSERT INTO offline_client_session VALUES ('s1', 'x');
        INSERT INTO realm_smtp_config VALUES ('econ-graph', 'smtp.example.invalid', 'host');
        CREATE TABLE component (id text PRIMARY KEY, name text, provider_type text, realm_id text REFERENCES realm);
        CREATE TABLE component_config (id text PRIMARY KEY, component_id text REFERENCES component,
                                       name text, value text);
        INSERT INTO component VALUES ('k1', 'rsa-generated', 'org.keycloak.keys.KeyProvider', 'econ-graph'),
                                     ('k2', 'hmac-generated', 'org.keycloak.keys.KeyProvider', 'econ-graph'),
                                     ('l1', 'ldap', 'org.keycloak.storage.UserStorageProvider', 'econ-graph'),
                                     ('p1', 'declarative-user-profile', 'org.keycloak.userprofile.UserProfileProvider',
                                      'econ-graph');
        INSERT INTO component_config VALUES ('c1', 'k1', 'privateKey', 'synthetic-private-key'),
                                            ('c2', 'k2', 'secret', 'synthetic-hmac-secret'),
                                            ('c3', 'l1', 'bindCredential', 'synthetic-bind-password'),
                                            ('c4', 'l1', 'connectionUrl', 'ldap://ldap.example.invalid'),
                                            ('c5', 'p1', 'kc.user.profile.config', '{}');
        CREATE TABLE client (id text PRIMARY KEY, client_id text, secret text, public_client boolean);
        INSERT INTO client VALUES ('a', 'econ-graph-api', 'synthetic-api-secret', false),
                                  ('b', 'econ-graph-web', NULL, true),
                                  ('c', 'econgraph-anonymous', 'synthetic-anonymous-secret', false);
        CREATE TABLE client_attributes (client_id text REFERENCES client, name text, value text);
        INSERT INTO client_attributes VALUES ('a', 'client.secret.rotated', 'synthetic-old-secret'),
                                             ('a', 'access.token.lifespan', '300');
        CREATE TABLE authenticator_config_entry (authenticator_id text, value text, name text);
        INSERT INTO authenticator_config_entry VALUES ('r', 'synthetic-recaptcha-secret', 'secret'),
                                                      ('r', '3', 'max.attempts');
        CREATE TABLE revoked_token (id text PRIMARY KEY, expire bigint);
        INSERT INTO revoked_token VALUES ('t1', 0);
        CREATE TABLE event_entity (id text PRIMARY KEY, ip_address text);
        INSERT INTO event_entity VALUES ('e1', '192.0.2.1');
        CREATE TABLE admin_event_entity (id text PRIMARY KEY, representation text);
        INSERT INTO admin_event_entity VALUES ('a1', '{}');
        CREATE TABLE credential (id text PRIMARY KEY, user_id text REFERENCES user_entity, secret_data text);
        INSERT INTO credential VALUES ('cr1', 'u1', 'synthetic-password-hash');
        CREATE TABLE user_attribute (user_id text REFERENCES user_entity, name text, value text);
        INSERT INTO user_attribute VALUES ('u1', 'country', 'synthetic');
    '''),
}
CLEARED = ['idp_mapper_config', 'identity_provider_mapper', 'identity_provider_config', 'federated_identity',
           'identity_provider', 'offline_client_session', 'offline_user_session', 'realm_smtp_config',
           'revoked_token', 'event_entity', 'admin_event_entity']

KUBECTL = textwrap.dedent('''\
    #!/usr/bin/env python3
    # Stand-in kubectl: exec into the scratch container, logs say the server started.
    import os, sys
    args = sys.argv[1:]
    if 'logs' in args:
        print('PostgreSQL init process complete; ready for start up.')
        sys.exit(0)
    if 'exec' not in args:
        sys.exit('unexpected kubectl call: ' + ' '.join(args))
    rest = args[args.index('exec') + 1:]
    cut = rest.index('--')
    flags = ['-i'] if '-i' in rest[:cut] else []
    os.execvp('docker', ['docker', '--host=HOST', 'exec', *flags, 'CONTAINER', *rest[cut + 1:]])
    ''')


def scratch_parent():
    parent = Path(os.environ.get('RELEASE_LAB_TEST_DIR') or Path.home() / '.cache/econgraph-release-lab-tests')
    parent.mkdir(parents=True, exist_ok=True)
    return parent


class RestoreRehearsal(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if DOCKER_HOST is None:
            raise RuntimeError('set DOCKER_HOST to the Docker daemon for the scratch PostgreSQL container (the '
                               'rootless one here); without it this test runs only in CI (CI=true)')
        cls.root = Path(tempfile.mkdtemp(prefix='release-lab-restore-', dir=scratch_parent()))
        cls.container = 'econgraph-release-lab-test-' + uuid.uuid4().hex[:12]
        subprocess.run(DOCKER + ['run', '-d', '--name', cls.container, '--publish', '127.0.0.1::5432',
                                 '-e', 'POSTGRES_HOST_AUTH_METHOD=trust', 'postgres:18'],
                       check=True, stdout=subprocess.DEVNULL)
        try:
            ports = subprocess.run(DOCKER + ['port', cls.container, '5432/tcp'], check=True,
                                   capture_output=True, text=True).stdout.split()
            assert ports and all(p.startswith('127.0.0.1:') for p in ports), ports
            for _ in range(120):
                if subprocess.run(DOCKER + ['exec', cls.container, 'pg_isready', '-q', '-h', '127.0.0.1'],
                                  check=False).returncode == 0:
                    break
                time.sleep(0.5)
            else:
                raise RuntimeError('scratch PostgreSQL did not start')
            cls.setup_databases()
        except BaseException:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        subprocess.run(DOCKER + ['rm', '-f', '-v', cls.container], check=False, stdout=subprocess.DEVNULL)
        subprocess.run(['chmod', '-R', 'u+rwx', str(cls.root)], check=False)
        shutil.rmtree(cls.root, ignore_errors=True)

    @classmethod
    def sql(cls, database, statement, user='postgres'):
        return subprocess.run(DOCKER + ['exec', '-i', cls.container, 'psql', '-v', 'ON_ERROR_STOP=1', '-h',
                                        '127.0.0.1', '-U', user, '-d', database, '-Atq'],
                              input=statement, check=True, capture_output=True, text=True).stdout.strip()

    @classmethod
    def setup_databases(cls):
        # The roles the lab's database images create: econgraph (init scripts) and the
        # keycloak superuser (POSTGRES_USER of keycloak-postgres).
        cls.sql('postgres', 'CREATE ROLE econgraph LOGIN; CREATE ROLE keycloak LOGIN SUPERUSER;')
        dumps = cls.root / 'dumps'
        dumps.mkdir()
        cls.dumps = {}
        for role, (source, owner, ddl) in SOURCES.items():
            cls.sql('postgres', f'CREATE DATABASE {source} OWNER {owner}')
            cls.sql(source, ddl, user=owner)
            if role == 'data':
                # As in production: an extension the database's init created, a comment on
                # it, and a table owned by the superuser rather than the app's role.
                cls.sql(source, "CREATE EXTENSION IF NOT EXISTS pg_trgm; "
                                "CREATE INDEX economic_series_title_trgm ON economic_series "
                                "USING gin (title gin_trgm_ops); "
                                "CREATE TABLE series_notes (id int PRIMARY KEY); "
                                "INSERT INTO series_notes VALUES (1); GRANT SELECT ON series_notes TO PUBLIC;")
            path = dumps / f'{role}.dump'
            with open(path, 'wb') as out:
                subprocess.run(DOCKER + ['exec', cls.container, 'pg_dump', '-Fc', '-h', '127.0.0.1', '-U',
                                         'postgres', '-d', source], check=True, stdout=out)
            path.chmod(0o444)
            cls.dumps[role] = path
        dumps.chmod(0o555)
        for spec in lab.DATABASES.values():
            owner = 'keycloak' if spec['user'] == 'keycloak' else 'econgraph'
            cls.sql('postgres', f'CREATE DATABASE {spec["database"]} OWNER {owner}')
            if spec['database'] == 'econ_graph':
                # What postgres-init.yaml does in the lab's data database.
                cls.sql('econ_graph', 'CREATE EXTENSION IF NOT EXISTS pg_trgm')
        cls.facts = {role: cls.file_facts(p) for role, p in cls.dumps.items()}
        directory = cls.root / 'labs/rehearsal'
        (directory / 'kube').mkdir(parents=True)
        kubectl = directory / 'kube/kubectl'
        kubectl.write_text(KUBECTL.replace('HOST', DOCKER_HOST).replace('CONTAINER', cls.container))
        kubectl.chmod(0o755)
        record = {'name': 'rehearsal', 'context': 'kind-econ-graph-lab-rehearsal', 'network': 'unused'}
        cls.tools = lab.Tools(record, directory, environ={'PATH': os.environ['PATH']})

    @staticmethod
    def file_facts(path):
        stat = path.stat()
        return hashlib.sha256(path.read_bytes()).hexdigest(), stat.st_size, stat.st_mtime_ns, stat.st_mode

    def test_1_restores_each_dump_into_its_empty_database(self):
        counts = {}
        for role, spec in lab.DATABASES.items():
            lab.wait_postgres_settled(self.tools, spec, attempts=3, interval=0)
            lab.restore_dump(self.tools, spec, str(self.dumps[role]))
            if role == 'keycloak':
                lab.psql(self.tools, spec, lab.KEYCLOAK_SANITIZE_SQL)
            counts[role] = lab.row_counts(self.tools, spec)
        self.assertEqual(counts, {'data': {'economic_series': 7, 'data_points': 50},
                                  'app': {'users': 3, 'chart_annotations': 2},
                                  'keycloak': {'realm': 2, 'user_entity': 3}})
        # --no-owner --role: everything belongs to the role the services connect as.
        self.assertEqual(self.sql('econ_graph', "select string_agg(distinct tableowner, ',') from pg_tables "
                                                "where schemaname = 'public'"), 'econgraph')
        self.assertEqual(self.sql('econ_graph', 'select count(*) from series_notes'), '1')
        self.assertEqual(self.sql('econ_graph', "select tableowner from pg_tables where tablename = 'data_points'"),
                         'econgraph')

    def test_2_keycloak_copy_has_no_identity_providers_sessions_or_smtp(self):
        for table in CLEARED:
            self.assertEqual(self.sql('keycloak', f'select count(*) from {table}'), '0', table)
        self.assertEqual(self.sql('src_keycloak', 'select count(*) from identity_provider'), '1')

    def test_2b_keycloak_copy_has_no_production_keys_or_secrets(self):
        self.assertEqual(self.sql('keycloak', "select count(*) from component "
                                              "where provider_type = 'org.keycloak.keys.KeyProvider'"), '0')
        self.assertEqual(self.sql('keycloak', 'select id from component order by id').split(), ['l1', 'p1'])
        self.assertEqual(self.sql('keycloak', 'select id, value from component_config order by id').split('\n'),
                         ['c3|', 'c4|ldap://ldap.example.invalid', 'c5|{}'])
        secrets = dict(line.split('|') for line in
                       self.sql('keycloak', 'select client_id, coalesce(secret, \'\') from client').split('\n'))
        self.assertEqual(secrets['econ-graph-web'], '')
        for client in ('econ-graph-api', 'econgraph-anonymous'):
            self.assertRegex(secrets[client], r'^[0-9a-f]{64}$', client)
        self.assertNotEqual(secrets['econ-graph-api'], secrets['econgraph-anonymous'])
        self.assertEqual(self.sql('keycloak', 'select name from client_attributes'), 'access.token.lifespan')
        self.assertEqual(self.sql('keycloak', 'select name, value from authenticator_config_entry order by name')
                         .split('\n'), ['max.attempts|3', 'secret|'])
        # The source (what the dump holds) still has them; the copy has none.
        everything = "select string_agg(t::text, ' ') from {} t"
        for table in ('client', 'component_config', 'client_attributes', 'authenticator_config_entry'):
            source = self.sql('src_keycloak', everything.format(table))
            copy_ = self.sql('keycloak', everything.format(table))
            self.assertIn('synthetic-', source, table)
            self.assertNotIn('synthetic-', copy_, table)
        # Kept on purpose: accounts and their credentials, for realistic sign-in tests.
        self.assertEqual(self.sql('keycloak', 'select secret_data from credential'), 'synthetic-password-hash')
        self.assertEqual(self.sql('keycloak', 'select value from user_attribute'), 'synthetic')

    def test_3_refuses_a_database_that_is_not_empty(self):
        before = self.sql('econ_graph', 'select count(*) from data_points')
        with self.assertRaisesRegex(lab.LabError, 'already has'):
            lab.restore_dump(self.tools, lab.DATABASES['data'], str(self.dumps['data']))
        self.assertEqual(self.sql('econ_graph', 'select count(*) from data_points'), before)

    def test_4_a_broken_dump_leaves_the_database_empty(self):
        self.sql('postgres', 'CREATE DATABASE broken_restore OWNER econgraph')
        broken = self.root / 'broken.dump'
        data = self.dumps['data'].read_bytes()
        broken.write_bytes(data[:len(data) * 3 // 4])
        spec = dict(copy.deepcopy(lab.DATABASES['data']), database='broken_restore')
        with self.assertRaises(lab.LabError):
            lab.restore_dump(self.tools, spec, str(broken))
        self.assertEqual(self.sql('broken_restore', "select count(*) from pg_tables where schemaname = 'public'"),
                         '0')

    def test_5_dumps_are_unchanged(self):
        for role, path in self.dumps.items():
            self.assertEqual(self.file_facts(path), self.facts[role], role)
            self.assertEqual(path.stat().st_mode & 0o777, 0o444)


if __name__ == '__main__':
    unittest.main()

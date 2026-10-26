#!/usr/bin/env python3
"""Rehearse host dump/count snapshots with real PostgreSQL and concurrent writes."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import uuid

DOCKER = ['docker', '--host=unix:///var/run/docker.sock']
HELPER = Path(__file__).resolve().parents[1] / 'deploy/lib/kind-env.sh'


def command(args, **kwargs):
    """Run a fixture command and return only its explicitly requested output."""
    return subprocess.run(args, check=True, **kwargs)


def psql(container, database, statement):
    """Execute a query in the disposable server, never through a host kubeconfig."""
    return command(DOCKER + ['exec', container, 'psql', '-X', '-qAt', '-F', ',',
                            '-v', 'ON_ERROR_STOP=1', '-U', 'postgres', '-d', database,
                            '-c', statement], capture_output=True, text=True).stdout.strip()


def rehearse(container, root):
    """Inject committed writes after each export, then restore and compare counts."""
    for database in ('econ_graph', 'econ_graph_app', 'keycloak'):
        psql(container, 'postgres', f'CREATE DATABASE {database}')
    psql(container, 'econ_graph', '''
        CREATE TABLE economic_series(id int PRIMARY KEY);
        CREATE TABLE data_points(id int PRIMARY KEY, series_id int REFERENCES economic_series);
        INSERT INTO economic_series VALUES(1);
    ''')
    psql(container, 'econ_graph_app', '''
        CREATE TABLE users(id int PRIMARY KEY);
        CREATE TABLE chart_annotations(id int PRIMARY KEY, user_id int REFERENCES users);
    ''')
    psql(container, 'keycloak', '''
        CREATE TABLE realm(id int PRIMARY KEY);
        CREATE TABLE user_entity(id int PRIMARY KEY, realm_id int REFERENCES realm);
        INSERT INTO realm VALUES(1),(2);
        INSERT INTO user_entity VALUES(1,1);
    ''')
    driver = root / 'snapshot-driver.sh'
    driver.write_text(r'''#!/bin/bash
set -euo pipefail
umask 077
source "$SNAPSHOT_TEST_HELPER"
# Replace only kubectl transport. The actual dump_one coprocess, exported
# snapshot, count queries, pg_dump and archive verification run unchanged.
k() {
    [[ "$1" == exec ]] || return 97
    shift
    local input=false
    if [[ "$1" == -i ]]; then input=true; shift; fi
    shift # synthetic POD
    [[ "$1" == -c ]] || return 98
    shift 2 # -c CONTAINER
    [[ "$1" == -- ]] || return 99
    shift
    local tool="$1" db='' previous='' arg
    shift
    for arg in "$@"; do
        [[ "$previous" != -d ]] || db="$arg"
        previous="$arg"
    done
    if [[ "$tool" == pg_dump ]]; then
        # These separate transactions commit after the holder exported its
        # snapshot and before pg_dump connects. A live count would see them.
        local statement
        case "$db" in
            econ_graph)
                statement='INSERT INTO economic_series VALUES(2); INSERT INTO data_points VALUES(1,1);'
                ;;
            econ_graph_app)
                statement='INSERT INTO users VALUES(1); INSERT INTO chart_annotations VALUES(1,1);'
                ;;
            keycloak)
                statement='INSERT INTO realm VALUES(3); INSERT INTO user_entity VALUES(2,1);'
                ;;
            *) return 96 ;;
        esac
        docker --host=unix:///var/run/docker.sock exec "$SNAPSHOT_TEST_CONTAINER" \
            psql -X -qAt -v ON_ERROR_STOP=1 -U postgres -d "$db" -c "$statement" >/dev/null
    fi
    if $input; then
        docker --host=unix:///var/run/docker.sock exec -i "$SNAPSHOT_TEST_CONTAINER" "$tool" "$@"
    else
        docker --host=unix:///var/run/docker.sock exec "$SNAPSHOT_TEST_CONTAINER" "$tool" "$@"
    fi
}
for db in econ_graph econ_graph_app keycloak; do
    dump_one synthetic-pod postgres postgres "$db" "$SNAPSHOT_TEST_ROOT/$db.dump"
done
''')
    environment = dict(os.environ, SNAPSHOT_TEST_HELPER=str(HELPER),
                       SNAPSHOT_TEST_CONTAINER=container, SNAPSHOT_TEST_ROOT=str(root),
                       KIND_CLUSTER='econ-graph', ECONGRAPH_SCRATCH_STORAGE='')
    command(['bash', str(driver)], env=environment, timeout=90)
    policies = {
        'econ_graph': ('SELECT (SELECT count(*) FROM economic_series), (SELECT count(*) FROM data_points)',
                       ['SERIES_COUNT', 'DATA_POINTS_COUNT'], (1, 0), (2, 1)),
        'econ_graph_app': ('SELECT (SELECT count(*) FROM users), (SELECT count(*) FROM chart_annotations)',
                           ['APP_USER_COUNT', 'APP_ANNOTATION_COUNT'], (0, 0), (1, 1)),
        'keycloak': ('SELECT (SELECT count(*) FROM realm), (SELECT count(*) FROM user_entity)',
                     ['KEYCLOAK_REALM_COUNT', 'KEYCLOAK_USER_COUNT'], (2, 1), (3, 2)),
    }
    results = {}
    for database, (count_query, keys, expected, expected_live) in policies.items():
        restored_db = 'restored_' + database
        psql(container, 'postgres', f'CREATE DATABASE {restored_db}')
        with (root / (database + '.dump')).open('rb') as archive:
            command(DOCKER + ['exec', '-i', container, 'pg_restore', '--no-owner', '--no-privileges',
                              '--exit-on-error', '-U', 'postgres', '-d', restored_db], stdin=archive)
        metadata = dict(line.split('=', 1) for line in (root / (database + '.dump.counts')).read_text().splitlines())
        recorded = tuple(int(metadata[key]) for key in keys)
        restored = tuple(map(int, psql(container, restored_db, count_query).split(',')))
        live = tuple(map(int, psql(container, database, count_query).split(',')))
        if recorded != expected or restored != expected or live != expected_live:
            raise AssertionError(f'{database}: metadata={recorded}, restored={restored}, live={live}')
        results[database] = {'metadata': recorded, 'restored': restored, 'live_after_writes': live}
    # Successful shell return must also release every exporter session.
    exporters = psql(container, 'postgres', "SELECT count(*) FROM pg_stat_activity WHERE state='idle in transaction'")
    if exporters != '0':
        raise AssertionError('A snapshot holder connection remained idle in a transaction')
    print(json.dumps({'snapshot_counts_verified': results}))


def main():
    """Create and remove only this test's uniquely named PostgreSQL container."""
    container = 'econgraph-snapshot-test-' + uuid.uuid4().hex
    with tempfile.TemporaryDirectory(prefix='econgraph-snapshot-test-') as temporary:
        try:
            command(DOCKER + ['run', '-d', '--network', 'none', '--name', container,
                             '-e', 'POSTGRES_HOST_AUTH_METHOD=trust', 'postgres:18'],
                    stdout=subprocess.DEVNULL)
            for attempt in range(60):
                readiness = subprocess.run(DOCKER + ['exec', container, 'pg_isready', '-q', '-U', 'postgres'],
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                if readiness.returncode == 0:
                    # Check the real TCP server; the image entrypoint briefly
                    # exposes a temporary socket-only server during initialization.
                    readiness = subprocess.run(DOCKER + ['exec', container, 'pg_isready', '-q', '-h', '127.0.0.1', '-U', 'postgres'],
                                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    if readiness.returncode == 0:
                        break
                time.sleep(0.5)
            else:
                raise RuntimeError('Scratch PostgreSQL did not start')
            rehearse(container, Path(temporary))
        finally:
            subprocess.run(DOCKER + ['rm', '-f', '-v', container], check=False,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == '__main__':
    main()

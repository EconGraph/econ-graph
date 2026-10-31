#!/usr/bin/env python3
"""Matching-version clients inside the explicitly selected disposable test Postgres."""
import os
import re
from pathlib import Path
import subprocess
import sys
from urllib.parse import parse_qsl, urlsplit, urlunsplit


def main():
    operation, *args = sys.argv[1:]
    if operation not in ('dump', 'restore'):
        raise ValueError('only dump/restore clients are supported')
    container = os.environ.get('TRAIN4_TEST_PG_CONTAINER')
    if not container:
        raise ValueError('TRAIN4_TEST_PG_CONTAINER must identify the disposable test service')
    index = args.index('--dbname') + 1
    source = urlsplit(args[index])
    if source.scheme not in ('postgres', 'postgresql') or source.hostname not in ('localhost', '127.0.0.1', '::1'):
        raise ValueError('test clients require an explicit loopback Postgres URL')
    if any(key in {'dbname','host','hostaddr','port','service','servicefile'} for key,_ in parse_qsl(source.query)):
        raise ValueError('test clients refuse URI routing overrides')
    if not re.fullmatch(r'/eg_train4_recovery_[a-z0-9_]+', source.path):
        raise ValueError('test clients may access only generated recovery scratch databases')
    # URI credentials remain the caller's, but this execution runs inside its test service.
    userinfo = source.netloc.rpartition('@')[0]
    authority = (userinfo + '@' if userinfo else '') + '127.0.0.1:5432'
    args[index] = urlunsplit((source.scheme, authority, source.path, source.query, source.fragment))
    environment = {key: value for key, value in os.environ.items()
                   if key not in ('DOCKER_HOST', 'DOCKER_CONTEXT', 'DOCKER_TLS',
                                  'DOCKER_TLS_VERIFY', 'DOCKER_CERT_PATH')}
    command = ['docker', '--host=unix:///var/run/docker.sock', 'exec', '-i', container,
               'pg_' + operation]
    if operation == 'dump':
        index = args.index('--file')
        path = Path(args[index + 1])
        del args[index:index + 2]
        with path.open('xb') as destination:
            result = subprocess.run(command + args, stdout=destination, env=environment)
            destination.flush()
            os.fsync(destination.fileno())
    else:
        path = Path(args.pop())
        with path.open('rb') as source_stream:
            result = subprocess.run(command + args, stdin=source_stream, env=environment)
    return result.returncode


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, IndexError):
        # Do not print argv: a libpq connection URI can contain credentials.
        print('recovery test client refused its target or arguments', file=sys.stderr)
        sys.exit(1)

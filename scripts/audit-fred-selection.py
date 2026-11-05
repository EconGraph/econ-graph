#!/usr/bin/env python3
"""Temporary COUNT-ONLY audit: no membership/witness completeness claim, no DB access.

Only sanitized artifacts are written. Crawler output stays in memory until redacted;
never upload a runner-wide log directory. Requires Python 3.11+ (tomllib).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import time
import tomllib


def redact(data, key):
    # Match normal and percent-encoded query names/separators, including empty values.
    data = re.sub(rb'(?i)(api(?:_|%5f)key(?:=|%3d))[^&\s\x22\x27<>]*',
                  rb'\1[REDACTED]', data)
    if key:
        data = data.replace(key.encode(), b'[REDACTED]')
    return data


def child_environment(data_dir, key):
    return {'PATH': os.defpath, 'LANG': 'C.UTF-8', 'RUST_LOG': 'warn',
            'CRAWLER_DATA_DIR': str(data_dir), 'FRED_API_KEY': key}


def copy_config(source, scratch):
    original = (source / 'catalog_selection/fred.toml').read_bytes()
    config = tomllib.loads(original.decode())
    if (config['max_catalog_series'], config['max_series'], len(config['rules']),
            len(config['include'])) != (20000, 500, 83, 30):
        raise ValueError('Reviewed configuration shape changed; audit needs review')
    shutil.copytree(source, scratch)
    copied, count = re.subn(rb'(?m)^max_catalog_series = 20000$',
                            b'max_catalog_series = 100000', original)
    if count != 1:
        raise ValueError('Expected exactly one catalog bound')
    (scratch / 'catalog_selection/fred.toml').write_bytes(copied)
    return original, copied


def capture(command, env, timeout):
    started = time.monotonic()
    process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
        status = process.returncode
        if status < 0:
            status = 128 - status
    except subprocess.TimeoutExpired:
        # Kill the whole audit process group: never leave a rate-limited crawler behind.
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        status = 124
    return status, stdout, stderr, round(time.monotonic() - started, 3)


def run(binary, data_dir, output, key, revision, timeout=1800):
    output.mkdir(parents=True, exist_ok=False)
    provenance = {'scope': 'COUNT-ONLY; not complete membership/witness evidence',
                  'revision': revision, 'original_catalog_guard': 20000,
                  'audit_catalog_guard': 100000, 'selected_guard': 500,
                  'count_uncapped': True, 'rate_percent': 25, 'timeout_seconds': timeout,
                  'membership_witness_complete': False}
    artifacts = {}
    status = 2
    try:
        with tempfile.TemporaryDirectory(prefix='fred-count-data-') as temporary:
            scratch = Path(temporary) / 'data'
            original, copied = copy_config(data_dir, scratch)
            artifacts.update({'original-fred.toml': original, 'audit-fred.toml': copied})
            provenance.update(original_config_sha256=hashlib.sha256(original).hexdigest(),
                              audit_config_sha256=hashlib.sha256(copied).hexdigest(),
                              rules=83, includes=30)
            command = [str(binary.resolve()), 'check-selection', '--source', 'FRED',
                       '--json', '--count-uncapped', '--rate-percent', '25']
            provenance['command'] = command
            if not key:
                status, stdout, stderr, elapsed = 3, b'', b'FRED_API_KEY is missing\n', 0
            else:
                status, stdout, stderr, elapsed = capture(
                    command, child_environment(scratch, key), timeout)
            artifacts.update({'stdout.json': stdout, 'stderr.txt': stderr})
            provenance['elapsed_seconds'] = elapsed
    except Exception:
        # Do not stringify exceptions: subprocess exceptions may contain credential output.
        artifacts['stderr.txt'] = b'Audit setup or execution failed; no raw exception emitted\n'
        status = 2
    provenance['exit_status'] = status
    artifacts['provenance.json'] = json.dumps(provenance, indent=2).encode() + b'\n'
    artifacts['exit-status.txt'] = f'{status}\n'.encode()
    for name, data in artifacts.items():
        (output / name).write_bytes(redact(data, key))
    return status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--data-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    args = parser.parse_args()
    return run(args.binary, args.data_dir, args.output, os.environ.get('FRED_API_KEY', ''),
               args.revision)


if __name__ == '__main__':
    raise SystemExit(main())

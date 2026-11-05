#!/usr/bin/env python3
"""Temporary FRED count/evidence runner: no DB access or deployment acceptance.

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
import stat
import subprocess
import tempfile
import time
import tomllib

HISTORY_SHA256 = '006745a4e19f2faca561cbc8ca8ae295365aa2afbb637a7bec32f5a2a22343a8'
EXPORT_TEST = 'sources::fred::audit::authenticated_catalog_evidence'
SCOPE_TEST = 'sources::fred::audit::authenticated_catalog_scope_counts'
SCOPE_RULES_SHA256 = '825a4f835488b775a79dcd520eb6c4ad8a39bbc71e0ccdce8bca5fdcdc29f955'
EXPORT_FILE = re.compile(r'(?:manifest\.json|(?:(?:release|category)-[0-9]+-[0-9]+|'
                         r'(?:series|ownership)-[0-9a-f]{64})\.(?:response-text|normalized)\.json)')


def credential_pattern(key):
    """Match only this credential, including mixed Unicode/percent representations.

    Repeated backslashes cover nested JSON escaping without repeatedly decoding
    arbitrary public content. The pattern has one bounded set of choices per key
    character; it never interprets an entire response as an escape sequence.
    """
    def hex_case(value):
        return ''.join(f'[{c.lower()}{c.upper()}]' if c.isalpha() else c for c in value)

    parts = []
    for char in key:
        units = char.encode('utf-16-be')
        escaped = ''.join(r'\\+u' + hex_case(units[i:i + 2].hex())
                          for i in range(0, len(units), 2))
        percent = ''.join('%' + hex_case(f'{byte:02x}') for byte in char.encode())
        parts.append('(?:' + '|'.join((re.escape(char), escaped, percent)) + ')')
    return re.compile(''.join(parts))


def redact(data, key):
    pattern = credential_pattern(key) if key else None

    def mask(text):
        if pattern:
            text = pattern.sub('[REDACTED]', text)
        return re.sub(r'(?i)(api(?:_|%5f)key(?:=|%3d))[^&\s\x22\x27<>]*',
                      r'\1[REDACTED]', text)

    text = data.decode('utf-8', errors='replace')
    try:
        # Validate structure, but do not round/re-serialize public numbers.
        json.loads(text)
    except (ValueError, RecursionError):
        return mask(text).encode()

    def string_token(match):
        original = json.loads(match.group())
        cleaned = mask(original)
        return match.group() if cleaned == original else json.dumps(cleaned, ensure_ascii=True)

    # Decode only JSON string tokens, including object keys. Nested escaped key
    # spellings are then matched directly; all other tokens remain byte-identical.
    return re.sub(r'"(?:[^"\\]|\\.)*"', string_token, text).encode()


def child_environment(data_dir, key):
    return {'PATH': os.defpath, 'LANG': 'C.UTF-8', 'RUST_LOG': 'warn',
            'CRAWLER_DATA_DIR': str(data_dir), 'FRED_API_KEY': key}


def copy_config(source, scratch, raise_scan=True):
    original = (source / 'catalog_selection/fred.toml').read_bytes()
    config = tomllib.loads(original.decode())
    if (config['max_catalog_series'], config['max_series'], len(config['rules']),
            len(config['include'])) != (20000, 500, 83, 30):
        raise ValueError('Reviewed configuration shape changed; audit needs review')
    shutil.copytree(source, scratch)
    if not raise_scan:
        if hashlib.sha256(original).hexdigest() != SCOPE_RULES_SHA256:
            raise ValueError('Frozen scope configuration changed')
        return original, original
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


def collect_evidence(directory, output, key, revision, config_hash, scope_counts=False,
                     scope_catalogs=None):
    """Copy verified evidence unchanged; never retain hashes for rewritten data.

    Invalid entries are omitted, not copied into an uploadable quarantine. A
    manifest referring to any omitted/changed/missing file is also omitted.
    Only the separate runner report can declare the artifact bundle complete.
    """
    issues, accepted = [], {}
    if not directory.exists():
        return {}, ['missing_output'], False
    if not stat.S_ISDIR(directory.lstat().st_mode) or directory.is_symlink():
        return {}, ['invalid_output_directory'], False
    total_bytes = 0
    for index, entry in enumerate(directory.iterdir()):
        if index >= 10000:
            issues.append('file_count_limit')
            break
        if not EXPORT_FILE.fullmatch(entry.name):
            issues.append('unexpected_name')
            continue
        if redact(entry.name.encode(), key) != entry.name.encode():
            issues.append('unsafe_identity')
            continue
        try:
            # O_NOFOLLOW + descriptor inspection prevents symlink swaps. Reject
            # hard links as well; output must consist solely of owned files.
            fd = os.open(entry, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            with os.fdopen(fd, 'rb') as stream:
                info = os.fstat(stream.fileno())
                if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > 64 * 1024**2:
                    issues.append('invalid_file_type_or_size')
                    continue
                content = stream.read(64 * 1024**2 + 1)
            content.decode('utf-8', errors='strict')
            total_bytes += len(content)
            if total_bytes > 512 * 1024**2:
                issues.append('total_size_limit')
                break
            if len(content) > 64 * 1024**2 or redact(content, key) != content:
                issues.append('unsafe_content')
                continue
            # Raw response-text may be malformed JSON on a parse failure. Keep
            # those safe bytes as evidence; normalized files/manifest must parse.
            if not entry.name.endswith('.response-text.json'):
                json.loads(content)
            accepted[entry.name] = content
        except (OSError, ValueError, RecursionError):
            issues.append('unreadable_or_invalid_file')
    manifest_bytes = accepted.get('manifest.json')
    complete = False
    try:
        manifest = json.loads(manifest_bytes) if manifest_bytes else None
        if (not isinstance(manifest, dict) or type(manifest.get('format')) is not int
                or manifest.get('format') != 1
                or manifest.get('revision') != revision or manifest.get('rules_sha256') != config_hash):
            raise ValueError('invalid manifest binding')
        if not scope_counts and manifest.get('historical', {}).get('sha256') != HISTORY_SHA256:
            raise ValueError('invalid historical binding')

        def verify(value):
            if isinstance(value, dict):
                for prefix in ('raw', 'normalized'):
                    path, digest = value.get(prefix + '_path'), value.get(prefix + '_sha256')
                    if path is not None or digest is not None:
                        if (not isinstance(path, str) or path == 'manifest.json'
                                or not EXPORT_FILE.fullmatch(path) or path not in accepted
                                or not isinstance(digest, str)
                                or hashlib.sha256(accepted[path]).hexdigest() != digest):
                            raise ValueError('invalid evidence reference')
                for child in value.values():
                    verify(child)
            elif isinstance(value, list):
                for child in value:
                    verify(child)
        verify(manifest)
        if scope_counts:
            catalogs = manifest.get('catalogs')
            if (config_hash != SCOPE_RULES_SHA256 or manifest.get('mode') != 'scope_counts'
                    or manifest.get('complete') is not False or manifest.get('membership_complete') is not False
                    or manifest.get('expected_catalogs') != 71 or not isinstance(catalogs, dict)
                    or len(catalogs) != 71 or set(catalogs) != scope_catalogs
                    or manifest.get('production_scan_guard') != 20000
                    or manifest.get('production_selected_guard') != 500):
                raise ValueError('invalid scope contract')
            rows, pages, validated = 0, 0, 0
            for catalog in catalogs.values():
                if catalog.get('status') != 'header_validated':
                    continue
                count = catalog.get('provider_count')
                page = catalog.get('page', {})
                if (type(count) is not int or count < 0 or page.get('advertised_count') != count
                        or page.get('response_offset') != 0 or page.get('response_limit') != 1
                        or 'raw_path' not in page or 'normalized_path' not in page
                        or catalog.get('projected_page_requests') != max(1, (count + 999) // 1000)):
                    raise ValueError('invalid counted header')
                rows += count
                pages += max(1, (count + 999) // 1000)
                validated += 1
            if (manifest.get('validated_catalogs') != validated
                    or manifest.get('projected_catalog_rows') != rows
                    or manifest.get('projected_catalog_page_requests') != pages):
                raise ValueError('invalid scope totals')
            complete = manifest.get('counts_complete') is True and validated == 71
        else:
            complete = all(manifest.get(field) is True for field in
                           ('complete', 'selection_complete', 'retrieval_complete', 'witness_bindings_accepted'))
    except (ValueError, TypeError, AttributeError, RecursionError):
        issues.append('invalid_manifest_or_reference')
    if issues:
        accepted.pop('manifest.json', None)
        complete = False
    output.mkdir()
    hashes = {}
    for name, content in accepted.items():
        (output / name).write_bytes(content)
        hashes[name] = hashlib.sha256(content).hexdigest()
    return hashes, sorted(set(issues)), complete


def run_exporter(binary, data_dir, output, key, revision, historical_csv, timeout=1830,
                 scope_counts=False):
    output.mkdir(parents=True, exist_ok=False)
    status, stdout, stderr, elapsed = 2, b'', b'', 0
    report = {'scope': 'Authenticated catalog evidence; acceptance requires review',
              'revision': revision, 'complete': False, 'artifact_validation_issues': [],
              'original_catalog_guard': 20000, 'audit_catalog_guard': 100000,
              'selected_guard': 500, 'rate_percent': 25, 'timeout_seconds': timeout}
    if scope_counts:
        report.update(scope='FIRST-PAGE scope counts only; not membership or vintage cost acceptance',
                      audit_catalog_guard=20000, membership_complete=False, counts_complete=False)
    artifacts = {}
    try:
        if not re.fullmatch('[0-9a-fA-F]{40}', revision):
            raise ValueError('invalid revision')
        if not scope_counts:
            history = historical_csv.read_bytes()
            if hashlib.sha256(history).hexdigest() != HISTORY_SHA256:
                raise ValueError('invalid history checksum')
        with tempfile.TemporaryDirectory(prefix='fred-export-') as temporary:
            scratch = Path(temporary)
            original, copied = copy_config(data_dir, scratch / 'data', raise_scan=not scope_counts)
            artifacts.update({'original-fred.toml': original, 'audit-fred.toml': copied})
            config_hash = hashlib.sha256(copied).hexdigest()
            scope_catalogs = None
            if scope_counts:
                rules = tomllib.loads(copied.decode())['rules']
                scope_catalogs = {f'release-{rule["release_id"]}' if 'release_id' in rule
                                  else f'category-{rule["category_id"]}' for rule in rules}
                if len(scope_catalogs) != 71:
                    raise ValueError('frozen catalog count changed')
            report.update(original_config_sha256=hashlib.sha256(original).hexdigest(),
                          audit_config_sha256=config_hash)
            env = child_environment(scratch / 'data', key)
            env.update(FRED_AUDIT_OUTPUT_DIR=str(scratch / 'evidence'), FRED_AUDIT_REVISION=revision)
            if not scope_counts:
                history_copy = scratch / 'historical.csv'
                history_copy.write_bytes(history)
                env['FRED_AUDIT_HISTORICAL_CSV'] = str(history_copy)
                report['historical_csv_sha256'] = HISTORY_SHA256
            command = [str(binary.resolve()), '--exact', SCOPE_TEST if scope_counts else EXPORT_TEST,
                       '--ignored', '--nocapture']
            if key:
                status, stdout, stderr, elapsed = capture(command, env, timeout)
                hashes, issues, complete = collect_evidence(
                    scratch / 'evidence', output / 'evidence', key, revision, config_hash,
                    scope_counts, scope_catalogs)
                report.update(files_sha256=hashes, artifact_validation_issues=issues,
                              exporter_complete=complete)
                report['complete'] = status == 0 and complete and not issues
                if scope_counts:
                    report['counts_complete'] = report['complete']
                # Keep a real failure/timeout unchanged; never green-wrap an
                # invalid or partial bundle from a nominally successful process.
                if status == 0 and not report['complete']:
                    status = 2
            else:
                status, stderr = 3, b'FRED_API_KEY is missing\n'
    except Exception:
        stderr = b'Exporter setup or validation failed; no raw exception emitted\n'
        if status == 0:
            status = 2
        report['artifact_validation_issues'].append('setup_or_validation_failed')
    report.update(exit_status=status, elapsed_seconds=elapsed)
    artifacts.update({'stdout.txt': stdout, 'stderr.txt': stderr,
                      'runner-report.json': json.dumps(report, indent=2).encode() + b'\n',
                      'exit-status.txt': f'{status}\n'.encode()})
    for name, content in artifacts.items():
        (output / name).write_bytes(redact(content, key))
    return status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--data-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--mode', choices=('count', 'exporter', 'scope-counts'), default='count')
    parser.add_argument('--historical-csv', type=Path)
    args = parser.parse_args()
    if args.mode in ('exporter', 'scope-counts'):
        if args.mode == 'exporter' and args.historical_csv is None:
            parser.error('--historical-csv is required for exporter mode')
        return run_exporter(args.binary, args.data_dir, args.output,
                            os.environ.get('FRED_API_KEY', ''), args.revision, args.historical_csv,
                            scope_counts=args.mode == 'scope-counts')
    return run(args.binary, args.data_dir, args.output, os.environ.get('FRED_API_KEY', ''),
               args.revision)


if __name__ == '__main__':
    raise SystemExit(main())

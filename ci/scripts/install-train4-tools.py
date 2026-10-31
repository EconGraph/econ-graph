#!/usr/bin/env python3
"""Install only checksum-locked validation binaries into an explicit scratch directory."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir', required=True, type=Path)
    parser.add_argument('--tool', required=True, action='append', choices=['tofu', 'helm', 'kubectl'])
    args = parser.parse_args()
    for tool in args.tool:
        if tool == 'tofu':
            pin = json.loads((ROOT / 'infra/talos/toolchain.lock.json').read_text())['opentofu']
            url, digest, member = pin['linux_amd64_url'], pin['sha256'], 'tofu'
        else:
            pin = json.loads((ROOT / 'platform/toolchain.lock.json').read_text())[tool]['linux_amd64']
            url, digest, member = pin['url'], pin['sha256'], pin.get('archive_member')
        if not url.startswith('https://'):
            raise SystemExit('validation tools require HTTPS')
        with urllib.request.urlopen(url, timeout=120) as response:
            payload = response.read(128 * 1024 * 1024 + 1)
        if len(payload) > 128 * 1024 * 1024 or hashlib.sha256(payload).hexdigest() != digest:
            raise SystemExit(f'{tool} payload does not match the checked-in checksum')
        if member:
            with tarfile.open(fileobj=io.BytesIO(payload), mode='r:gz') as archive:
                entry = archive.getmember(member)
                if not entry.isfile() or entry.size > 128 * 1024 * 1024:
                    raise SystemExit(f'{tool} archive member is not a bounded regular file')
                payload = archive.extractfile(entry).read()
        args.bin_dir.mkdir(parents=True, exist_ok=True)
        target = args.bin_dir / tool
        if target.is_symlink():
            raise SystemExit('refusing a symlinked validation binary')
        target.write_bytes(payload)
        target.chmod(0o755)
        print(f'Installed checksum-verified {tool} into {args.bin_dir}')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Git-tracked physical line counts; category scope preserves the original report."""
import argparse
import json
from pathlib import Path
import subprocess

CATEGORIES = {
    'backend_production': ['backend/crates/*.rs'],
    'backend_tests': ['backend/crates/*.rs'],
    'frontend_production': ['frontend/src/*.ts', 'frontend/src/*.tsx', 'frontend/src/*.js', 'frontend/src/*.jsx'],
    'frontend_tests': ['frontend/tests/*.ts', 'frontend/tests/*.tsx', 'frontend/tests/*.js', 'frontend/tests/*.jsx'],
    'configuration': ['*.yaml', '*.yml', '*.json', '*.toml', 'Dockerfile*', '*.tf'],
    'documentation': ['*.md', 'README*', '*.txt'],
    'scripts': ['*.sh'],
}


def collect(root, overrides=None):
    stats = {}
    seen = set()
    for category, patterns in CATEGORIES.items():
        paths = subprocess.check_output(['git', 'ls-files', '-z', '--', *patterns], cwd=root).decode().split('\0')
        selected = []
        for path in filter(None, paths):
            if path.endswith(('package-lock.json', 'Cargo.lock')) or path == 'data/cost-analysis.json':
                continue
            if category.startswith('backend_') and ('test' in path) != (category == 'backend_tests'):
                continue
            if path in seen:
                raise ValueError(f'File belongs to multiple categories: {path}')
            seen.add(path)
            selected.append(path)
        # Like wc -l: count newline bytes, not logical lines; fail on missing files.
        stats[category] = sum((overrides[path] if overrides and path in overrides else (root / path).read_bytes()).count(b'\n') for path in selected)
    stats['total_lines'] = sum(stats.values())
    return stats


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--json', action='store_true', help='machine-readable, unformatted integer statistics')
    args = parser.parse_args()
    stats = collect(Path(__file__).resolve().parent.parent)
    if args.json:
        print(json.dumps(stats, sort_keys=True))
    else:
        print('EconGraph Codebase Corrected Stats')
        print('Git-tracked physical lines; excludes lock files and generated cost-analysis.json.')
        for key, value in stats.items():
            print(f"  {key.replace('_', ' ').title()}: {value:,}")
        total = stats['total_lines']
        for label, keys in [('Production Code', ['backend_production', 'frontend_production']),
                            ('Test Code', ['backend_tests', 'frontend_tests']),
                            ('Infrastructure', ['configuration', 'scripts'])]:
            count = sum(stats[k] for k in keys)
            percentage = f'{count * 100 / total:.1f}%' if total else 'N/A (empty codebase)'
            print(f'  {label}: {count:,} ({percentage})')
        print('Development cost estimate: run scripts/update-cost-analysis.sh (canonical USD/line model).')


if __name__ == '__main__':
    main()

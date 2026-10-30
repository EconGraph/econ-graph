#!/usr/bin/env python3
"""Describe the exact candidate and recorded fixture bytes; never contacts a provider."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
files = set((ROOT / 'backend/crates/econ-graph-core/tests/fixtures/train3').glob('*.sql'))
files.add(ROOT / 'backend/crates/econ-graph-core/tests/fixtures/research-anchor-v1.json')
manifest = ROOT / 'backend/crates/econ-graph-crawler/tests/fixtures/e2e-seed.json'
files.add(manifest)
files.add(ROOT / 'scripts/train3/semantic-snapshot.sql')
seed = json.loads(manifest.read_text())
for series in seed['series']:
    for route in series.get('discovery', []) + series['routes']:
        fixture = (manifest.parent / route['fixture']).resolve()
        if not fixture.is_relative_to(ROOT):
            raise SystemExit('fixture escapes repository')
        files.add(fixture)
snapshot = Path(os.environ['TRAIN3_SEMANTIC_SNAPSHOT']) if os.environ.get('TRAIN3_SEMANTIC_SNAPSHOT') else None
semantic = json.loads(snapshot.read_text()) if snapshot else None
print(json.dumps({
    'format': 1,
    'baseline_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
    'working_tree_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT, text=True).strip()),
    'semantic_snapshot': {'format': semantic['format'], 'sha256': hashlib.sha256(snapshot.read_bytes()).hexdigest(), 'rows': {key: len(value) for key, value in semantic.items() if isinstance(value, list)}} if semantic else None,
    'fixtures': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)},
    'migrations': {plane: sorted(p.name for p in (ROOT / 'backend/migrations' / plane).iterdir() if p.is_dir()) for plane in ('app', 'data')},
    'expectations': ['separate and combined databases', 'nullable exact decimal revisions', 'legacy IDs and filed-fact references', 'request-scoped catalog authorization', 'retained annotations/comments/grants', 'matching-version backup restore'],
}, indent=2, sort_keys=True))

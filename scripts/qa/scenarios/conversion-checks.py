#!/usr/bin/env python3
# qa-scenario: conversion-checks
# qa-mutates: no
# qa-profiles: lab
# qa-timeout: 1800
# qa-description: After the data conversion (ECO-426): per-table row counts and checksums unchanged, annotations still on their original series.
"""Checks a data conversion's before-and-after report. Labs only.

The conversion itself (ECO-426's script) runs before the suite; this scenario reads the report
it leaves and judges it. Inputs, one of:
  QA_CONVERSION_REPORT         path to the report (JSON, below).
  QA_CONVERSION_CHECK_COMMAND  run with bash; prints the report on stdout.

The report:
  {
    "tables": [
      {"name": "economic_series",
       "before": {"rows": 812, "checksum": "md5:..."},
       "after":  {"rows": 812, "checksum": "md5:..."}}
    ],
    "annotations": [
      {"id": "...", "series_id_before": "...", "series_id_after": "..."}
    ]
  }

`checksum` is whatever the script computes for the table (e.g. an md5 of its ordered ids); it
must be present on both sides. Every table must keep its row count and checksum, and every
annotation its series. An empty table list fails. Skipped without either input.
"""

import json
import os
import subprocess
from pathlib import Path

from qa import check, fail, finish, skip

path = os.environ.get('QA_CONVERSION_REPORT', '')
command = os.environ.get('QA_CONVERSION_CHECK_COMMAND', '')
if path:
    try:
        text = Path(path).read_text(encoding='utf-8')
    except OSError as error:
        fail('report', f'{path}: {error.strerror}')
elif command:
    run = subprocess.run(['bash', '-c', command], stdin=subprocess.DEVNULL, capture_output=True, text=True)
    if run.returncode:
        fail('report', f'QA_CONVERSION_CHECK_COMMAND exited {run.returncode}: {run.stderr.strip()[-300:]}')
    text = run.stdout
else:
    skip('no conversion report: set QA_CONVERSION_REPORT or QA_CONVERSION_CHECK_COMMAND (ECO-426)')

try:
    report = json.loads(text)
    tables = report['tables']
    annotations = report.get('annotations', [])
except (ValueError, KeyError, TypeError) as error:
    fail('report', f'not a conversion report: {error}')

check('report lists tables', bool(tables), f'{len(tables)} tables')
for table in tables:
    name = table.get('name', '?')
    before, after = table.get('before') or {}, table.get('after') or {}
    same_rows = before.get('rows') is not None and before.get('rows') == after.get('rows')
    same_sum = bool(before.get('checksum')) and before.get('checksum') == after.get('checksum')
    check(f'{name} unchanged', same_rows and same_sum,
          f'rows {before.get("rows")} -> {after.get("rows")}, checksum '
          f'{"unchanged" if same_sum else "missing or changed"}',
          rows=after.get('rows'))

moved = [a for a in annotations
         if not a.get('series_id_before') or a.get('series_id_before') != a.get('series_id_after')]
check('annotations on their original series', not moved,
      f'{len(annotations)} annotations checked' if not moved
      else f'{len(moved)} of {len(annotations)} moved, e.g. {moved[0].get("id")}')

finish()

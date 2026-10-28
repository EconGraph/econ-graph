#!/usr/bin/env python3
# qa-scenario: crawl-sample
# qa-mutates: yes
# qa-profiles: lab
# qa-timeout: 3600
# qa-description: Crawl one series per source through the lab's crawler, one source at a time, spaced for the providers' politeness limits.
"""A crawl sample. Labs only: it fetches from the real data providers.

Inputs:
  QA_CRAWL_SAMPLE_COMMAND   run with bash once per source, with QA_CRAWL_SOURCE set (e.g. FRED);
                            fetches one series of that source through the lab's own crawler
                            (e.g. `crawler enqueue` in its crawler-worker, then waiting for the
                            job) and prints one JSON object as its last line:
                              {"source": "FRED", "series": "GDP", "ok": true,
                               "observations": 312, "detail": "..."}
  QA_CRAWL_SOURCES          comma-separated sources (default FRED,BLS,CENSUS,BEA,FHFA,WDI).
  QA_CRAWL_INTERVAL_SECONDS pause between sources (default 30), on top of the crawler's own
                            per-provider rate limits.

One check per source: the command exited 0, reported ok for that source, exactly one series,
and at least one observation. Sources run one after another, never in parallel. Skipped
without QA_CRAWL_SAMPLE_COMMAND.
"""

import json
import os
import subprocess
import time

from qa import check, finish, skip

COMMAND = os.environ.get('QA_CRAWL_SAMPLE_COMMAND', '')
SOURCES = [s.strip() for s in os.environ.get('QA_CRAWL_SOURCES', 'FRED,BLS,CENSUS,BEA,FHFA,WDI').split(',')
           if s.strip()]
INTERVAL = float(os.environ.get('QA_CRAWL_INTERVAL_SECONDS', '30'))

if not COMMAND:
    skip('no crawl hook: set QA_CRAWL_SAMPLE_COMMAND (the lab crawls one series per source)')

for i, source in enumerate(SOURCES):
    if i:
        time.sleep(INTERVAL)
    started = time.monotonic()
    run = subprocess.run(['bash', '-c', COMMAND], env=dict(os.environ, QA_CRAWL_SOURCE=source),
                         stdin=subprocess.DEVNULL, capture_output=True, text=True)
    elapsed = time.monotonic() - started
    lines = [line for line in run.stdout.splitlines() if line.strip()]
    try:
        result = json.loads(lines[-1]) if lines else {}
    except ValueError:
        result = {}
    if not isinstance(result, dict):
        result = {}
    series = result.get('series')
    ok = (run.returncode == 0 and result.get('ok') is True and result.get('source') == source
          and isinstance(series, str) and bool(series)
          and isinstance(result.get('observations'), int) and result['observations'] > 0)
    detail = (f'{series}: {result.get("observations")} observations in {elapsed:.0f}s' if ok else
              f'exit {run.returncode}; reported {json.dumps(result)[:300] or "nothing"}'
              + (f'; {run.stderr.strip().splitlines()[-1]}' if run.returncode and run.stderr.strip() else ''))
    check(f'crawl {source}', ok, detail, series=series, seconds=round(elapsed))

finish()

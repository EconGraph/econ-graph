#!/usr/bin/env python3
"""Check crawler credential sources and dispatch-input SQL boundaries."""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

import yaml


class CrawlerSecretsTest(unittest.TestCase):
    def test_single_series_is_normalized_before_enqueue(self):
        """Keep the CLI and SQL identifier identical and reject multi-series input."""
        root = Path(__file__).resolve().parents[2]
        workflow = yaml.safe_load((root / ".github/workflows/crawler-integration-test.yml").read_text())
        steps = workflow["jobs"]["crawler-integration-test"]["steps"]
        crawl = next(step for step in steps if step.get("name") == "Crawl one series through the queue")
        # Stop after enqueue, before the worker starts or a database is needed.
        prefix = crawl["run"].split("# One worker", 1)[0]
        for series, expected in ((" \tGDP\n", "GDP"), ("GDP", "GDP"),
                                 ("GDP,CPI", None), (" \t\n", None), ("", None)):
            with self.subTest(series=series), tempfile.TemporaryDirectory() as directory:
                crawler = Path(directory) / "target/release/crawler"
                crawler.parent.mkdir(parents=True)
                crawler.write_text('#!/bin/bash\nprintf "%s\\0" "$@" >> "$CALLS_FILE"\n')
                crawler.chmod(0o755)
                calls = Path(directory) / "calls"
                result = subprocess.run(
                    ["bash", "-c", prefix + '\nprintf "%s" "$SERIES"'],
                    cwd=directory, capture_output=True, text=True,
                    env={**os.environ, "SOURCE": "WORLD_BANK", "SERIES": series, "CALLS_FILE": str(calls)},
                )
                if expected is None:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(calls.exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stdout, expected)
                    self.assertEqual(calls.read_bytes().split(b'\0')[:-1],
                                     [b'sources', b'enqueue', b'--source', b'WORLD_BANK', b'--series', expected.encode()])

    def test_sql_inputs_are_passed_as_literal_psql_variables(self):
        """Exercise shell quoting with input that would break interpolated SQL."""
        root = Path(__file__).resolve().parents[2]
        workflow = yaml.safe_load((root / ".github/workflows/crawler-integration-test.yml").read_text())
        steps = workflow["jobs"]["crawler-integration-test"]["steps"]
        crawl = next(step for step in steps if step.get("name") == "Crawl one series through the queue")
        queries = re.findall(r"psql [^\n]+ <<'SQL'\n.*?\nSQL", crawl["run"], re.DOTALL)
        self.assertEqual(len(queries), 3)
        # Capture exactly what the shell would pass to psql, without a database.
        capture = '''psql() {
          python3 -c 'import json, sys; print(json.dumps([sys.argv[1:], sys.stdin.read()]))' "$@"
        }
'''
        source = "fred'; SELECT pg_sleep(10); --"
        series = "GDP' OR true; --\n$(exit 42) \\ :series"
        for query in queries:
            with self.subTest(query=query):
                result = subprocess.run(
                    ["bash", "-eu", "-c", capture + query], check=True, capture_output=True, text=True,
                    env={**os.environ, "DATABASE_URL": "postgresql://unused", "SOURCE": source, "SERIES": series},
                )
                args, sql = json.loads(result.stdout)
                self.assertIn("series=" + series, args)
                self.assertIn("ON_ERROR_STOP=1", args)
                self.assertIn(":'series'", sql)
                if ":'source'" in sql:
                    self.assertIn("source=" + source, args)
                self.assertNotIn(source, sql)
                self.assertNotIn(series, sql)

    def test_crawler_keys_only_come_from_repository_secrets(self):
        """Keep credential values out of the manual dispatch input surface."""
        root = Path(__file__).resolve().parents[2]
        workflow = yaml.safe_load((root / ".github/workflows/crawler-integration-test.yml").read_text())
        # PyYAML's YAML 1.1 loader reads the unquoted `on` key as True.
        trigger = workflow.get("on", workflow.get(True))
        self.assertEqual(set(trigger["workflow_dispatch"]["inputs"]), {"source", "series"})
        steps = workflow["jobs"]["crawler-integration-test"]["steps"]
        crawl = next(step for step in steps if step.get("name") == "Crawl one series through the queue")
        for source in ("FRED", "BLS", "CENSUS"):
            self.assertEqual(crawl["env"][f"{source}_API_KEY"], "${{ secrets." + source + "_API_KEY }}")


if __name__ == "__main__":
    unittest.main()

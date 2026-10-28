#!/usr/bin/env python3
"""Check crawler credential sources and dispatch-input SQL boundaries."""

import json
import os
from pathlib import Path
import re
import subprocess
import unittest

import yaml


class CrawlerSecretsTest(unittest.TestCase):
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

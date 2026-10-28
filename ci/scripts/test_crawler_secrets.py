#!/usr/bin/env python3
"""Check that crawler dispatch inputs never accept API credentials."""

from pathlib import Path
import unittest

import yaml


class CrawlerSecretsTest(unittest.TestCase):
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

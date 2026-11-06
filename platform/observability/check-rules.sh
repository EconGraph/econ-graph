#!/usr/bin/env bash
# Checks the platform PrometheusRule objects with promtool: `promtool check rules` on each,
# then the rule unit tests in tests/*.test.yaml.
#
# Every PrometheusRule document in platform/observability/*.yaml (files may hold several
# documents) has its spec written to <tmp>/<metadata.name>.yml; a test file names it that way in
# `rule_files` (for example econgraph-crawler.yml). Rule names must be unique.
#
# Needs promtool and python3 with PyYAML. Usage: platform/observability/check-rules.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

python3 - "$tmp" "$here"/*.yaml <<'PY'
import os, sys, yaml
out, written = sys.argv[1], {}
for path in sys.argv[2:]:
    with open(path) as f:
        for doc in yaml.safe_load_all(f):
            if not doc or doc.get("kind") != "PrometheusRule":
                continue
            name = doc["metadata"]["name"]
            if name in written:
                sys.exit(f"PrometheusRule {name} is defined in {written[name]} and {path}")
            if doc["metadata"].get("labels", {}).get("release") != "monitoring":
                sys.exit(f"PrometheusRule {name} in {path} lacks label release: monitoring; Prometheus would not load it")
            written[name] = path
            with open(os.path.join(out, name + ".yml"), "w") as f2:
                yaml.safe_dump(doc["spec"], f2, sort_keys=False)
if not written:
    sys.exit("no PrometheusRule found")
print("rule files:", ", ".join(sorted(n + ".yml" for n in written)))
PY
promtool check rules "$tmp"/*.yml

shopt -s nullglob
for test in "$here"/tests/*.test.yaml; do
  echo "Testing tests/$(basename "$test")"
  cp "$test" "$tmp/"
  promtool test rules "$tmp/$(basename "$test")"
done

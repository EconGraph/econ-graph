#!/usr/bin/env bash
# Checks the Prometheus rule ConfigMaps (prometheus-rules-*.yaml) with promtool: `promtool
# check rules` on each rule file they hold, then the rule unit tests in tests/*.test.yaml.
#
# Each ConfigMap data key (e.g. crawler.rules.yml) is written to <tmp>/<key>, the name
# Prometheus sees under /etc/prometheus/rules; a test file names it in `rule_files`.
#
# Needs promtool and python3 with PyYAML. Usage: k8s/monitoring/check-prometheus-rules.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for manifest in "$here"/prometheus-rules-*.yaml; do
  python3 - "$manifest" "$tmp" <<'PY'
import os, sys, yaml
for key, text in yaml.safe_load(open(sys.argv[1]))["data"].items():
    with open(os.path.join(sys.argv[2], key), "w") as f:
        f.write(text)
PY
done
promtool check rules "$tmp"/*.yml

shopt -s nullglob
for test in "$here"/tests/*.test.yaml; do
  cp "$test" "$tmp/"
  promtool test rules "$tmp/$(basename "$test")"
done

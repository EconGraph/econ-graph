#!/usr/bin/env bash
# Checks the Alertmanager configuration the pinned kube-prometheus-stack chart actually renders
# from platform/addons/monitoring/release.yaml: `amtool check-config`, the Slack templates, and
# `amtool config routes test` for the receivers that matter.
#
# The rendered config reads receiver URLs from /etc/alertmanager/secrets/alertmanager-receivers/;
# those paths are rewritten to dummy files in a temporary directory. No real URL is used.
#
# Needs helm, amtool and python3 with PyYAML; CACHE is the validate.py --cache directory holding
# the checksum-locked chart. Usage: platform/observability/check-alertmanager.sh CACHE
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cache="${1:?usage: check-alertmanager.sh CACHE}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

chart="$(python3 - "$root" "$cache" <<'PY'
import hashlib, json, sys, yaml
root, cache = sys.argv[1:]
entry = json.load(open(root + "/platform/versions.lock.json"))["charts"]["monitoring"]
chart = cache + "/charts/" + entry["package"]
if hashlib.sha256(open(chart, "rb").read()).hexdigest() != entry["sha256"]:
    sys.exit(f"{chart} does not match versions.lock.json; run validate.py --fetch")
print(chart)
PY
)"
python3 - "$root/platform/addons/monitoring/release.yaml" > "$tmp/values.yaml" <<'PY'
import sys, yaml
print(yaml.safe_dump(yaml.safe_load(open(sys.argv[1]))["spec"]["values"]))
PY
helm template monitoring "$chart" --namespace monitoring --values "$tmp/values.yaml" \
  --show-only templates/alertmanager/secret.yaml > "$tmp/secret.yaml"
helm template monitoring "$chart" --namespace monitoring --values "$tmp/values.yaml" \
  --show-only templates/alertmanager/alertmanager.yaml > "$tmp/alertmanager-cr.yaml"

mkdir "$tmp/secrets"
printf 'https://slack.example.invalid/check\n' > "$tmp/secrets/slack-webhook-url"
printf 'https://healthchecks.example.invalid/check\n' > "$tmp/secrets/watchdog-url"
command -v amtool >/dev/null || { echo "amtool not found on PATH" >&2; exit 1; }
python3 - "$tmp" "$(amtool --version 2>&1 | head -n 1)" <<'PY'
import base64, os, sys, yaml
tmp, amtool = sys.argv[1:]
alertmanager = yaml.safe_load(open(os.path.join(tmp, "alertmanager-cr.yaml")))
if f"version {alertmanager['spec']['version'].lstrip('v')} " not in amtool:
    sys.exit(f"amtool ({amtool}) must match the chart's Alertmanager {alertmanager['spec']['version']}")
if alertmanager["spec"].get("secrets") != ["alertmanager-receivers"]:
    sys.exit("Alertmanager must mount the alertmanager-receivers Secret")
secret = yaml.safe_load(open(os.path.join(tmp, "secret.yaml")))
config = base64.b64decode(secret["data"]["alertmanager.yaml"]).decode()
mount = "/etc/alertmanager/secrets/alertmanager-receivers/"
if config.count(mount) != 2:
    sys.exit("expected the Slack and Watchdog URLs to come only from " + mount)
parsed = yaml.safe_load(config)
for receiver in parsed["receivers"]:
    for slack in receiver.get("slack_configs", []):
        for field in ["title", "text"]:
            with open(os.path.join(tmp, f"{receiver['name']}-{field}.txt"), "w") as f:
                f.write(slack[field])
with open(os.path.join(tmp, "alertmanager.yaml"), "w") as f:
    f.write(config.replace(mount, tmp + "/secrets/"))
PY

amtool check-config "$tmp/alertmanager.yaml"
touch "$tmp/empty.tmpl"
for field in title text; do
  echo "Parsing slack $field template"
  amtool template render --template.glob="$tmp/empty.tmpl" --template.text="$(cat "$tmp/slack-$field.txt")"
  echo
done

route() {
  local receiver="$1"; shift
  echo "Route $* -> $receiver"
  amtool config routes test --config.file="$tmp/alertmanager.yaml" --verify.receivers="$receiver" "$@"
}
route healthchecks alertname=Watchdog severity=none
route null alertname=InfoInhibitor severity=none namespace=econ-graph
route slack alertname=EconGraphBackupHealthCheckStale severity=critical namespace=econ-graph
route slack alertname=EconGraphPostgresInstanceNotReady severity=warning namespace=econ-graph
route slack alertname=TargetDown severity=warning namespace=monitoring

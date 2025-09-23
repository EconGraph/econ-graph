#!/bin/bash

# Script to fix Grafana dashboard JSON format for proper provisioning
# The issue is that dashboards are wrapped in a 'dashboard' object but Grafana expects them at root level

set -e

echo "🔧 Fixing Grafana dashboard JSON format for proper provisioning"

# Extract dashboard content from nested structure
echo "📊 Extracting dashboard content from nested structure..."

# Get the current ConfigMap data and extract dashboard content
kubectl get configmap grafana-dashboards -n econ-graph -o jsonpath='{.data.econgraph-overview\.json}' | jq '.dashboard' > /tmp/econgraph-overview-fixed.json
kubectl get configmap grafana-dashboards -n econ-graph -o jsonpath='{.data.logging-dashboard\.json}' | jq '.dashboard' > /tmp/logging-dashboard-fixed.json

# Create corrected ConfigMap
echo "📝 Creating corrected ConfigMap..."
kubectl create configmap grafana-dashboards-fixed -n econ-graph \
  --from-file=econgraph-overview.json=/tmp/econgraph-overview-fixed.json \
  --from-file=logging-dashboard.json=/tmp/logging-dashboard-fixed.json \
  --dry-run=client -o yaml | kubectl apply -f -

# Update Grafana StatefulSet to use corrected ConfigMap
echo "🔄 Updating Grafana StatefulSet..."
kubectl patch statefulset grafana -n econ-graph --type='json' -p='[{"op": "replace", "path": "/spec/template/spec/volumes/2/configMap/name", "value": "grafana-dashboards-fixed"}]'

# Wait for rollout
echo "⏳ Waiting for Grafana pod to restart..."
kubectl rollout status statefulset/grafana -n econ-graph --timeout=120s

# Clean up temp files
rm -f /tmp/econgraph-overview-fixed.json /tmp/logging-dashboard-fixed.json

echo "✅ Grafana dashboard format fixed! Dashboards should now load properly."
echo "🔍 Check Grafana logs for any remaining errors:"
echo "   kubectl logs grafana-0 -n econ-graph --tail=10 | grep -i dashboard"

#!/bin/bash

# Memory usage monitoring script
# Helps track Node.js memory consumption

echo "📊 Node.js Memory Usage Monitor"
echo "================================"

# Check current Node.js processes
echo "🔍 Current Node.js processes:"
ps aux | grep node | grep -v grep || echo "No Node.js processes running"

echo ""
echo "💾 System memory usage:"
free -h

echo ""
echo "📈 Top memory-consuming processes:"
ps -o pid,ppid,cmd,%mem,%cpu --sort=-%mem | head -10

echo ""
echo "🧠 Node.js memory optimization status:"
echo "NODE_OPTIONS: ${NODE_OPTIONS:-'Not set'}"
echo "JEST_WORKER_ID: ${JEST_WORKER_ID:-'Not set'}"

# Check if memory optimization is active
if [[ -n "$NODE_OPTIONS" ]]; then
    echo "✅ Memory optimization is active"
else
    echo "⚠️  Memory optimization not configured"
    echo "   Run: source .env.memory-optimization"
fi

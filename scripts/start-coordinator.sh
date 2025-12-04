# ============================================
# scripts/start-coordinator.sh
# Start Zcash coordinator
# ============================================

#!/bin/bash
set -e

echo "🌉 Starting ZeroBridge Coordinator..."

# Check if Zcash node is running
if ! zcash-cli getblockchaininfo &> /dev/null; then
    echo "❌ Zcash node not running"
    echo "Start with: zcashd -daemon"
    exit 1
fi

echo "✓ Zcash node is running"

# Check sync status
BLOCKS=$(zcash-cli getblockchaininfo | jq -r '.blocks')
HEADERS=$(zcash-cli getblockchaininfo | jq -r '.headers')
PROGRESS=$(zcash-cli getblockchaininfo | jq -r '.verificationprogress')

if (( $(echo "$PROGRESS < 0.99" | bc -l) )); then
    echo "⚠️  Zcash node still syncing: $BLOCKS/$HEADERS blocks ($PROGRESS)"
    echo "Waiting for sync to complete..."
    exit 1
fi

echo "✓ Zcash node fully synced"

# Build coordinator
cd zcash-coordinator
echo "🔨 Building coordinator..."
cargo build --release

# Start coordinator
echo "🚀 Starting coordinator..."
./target/release/zcash-coordinator \
    --config ../config/coordinator.toml \
    --verbose \
    2>&1 | tee logs/coordinator.log &

COORDINATOR_PID=$!
echo $COORDINATOR_PID > coordinator.pid

echo "✓ Coordinator started (PID: $COORDINATOR_PID)"
echo "📊 Logs: tail -f logs/coordinator.log"
echo "🛑 Stop: kill $(cat coordinator.pid)"

# Wait for coordinator to be ready
echo "Waiting for coordinator to be ready..."
for i in {1..30}; do
    if curl -s http://localhost:8080/health > /dev/null; then
        echo "✓ Coordinator is ready!"
        break
    fi
    sleep 1
done

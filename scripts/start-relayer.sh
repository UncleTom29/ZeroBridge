# ============================================
# scripts/start-relayer.sh
# Start relayer node
# ============================================

#!/bin/bash
set -e

echo "🔄 Starting ZeroBridge Relayer..."

# Check if coordinator is running
if ! curl -s http://localhost:8080/health > /dev/null; then
    echo "❌ Coordinator not running"
    echo "Start with: ./scripts/start-coordinator.sh"
    exit 1
fi

echo "✓ Coordinator is running"

# Build relayer
cd relayer
echo "🔨 Building relayer..."
cargo build --release

# Check stake
echo "Checking relayer stake..."
# TODO: Query hub contract for stake

# Start relayer
echo "🚀 Starting relayer..."
./target/release/zerobridge-relayer \
    --config ../config/relayer-config.toml \
    --keypair keys/relayer-keypair.json \
    --verbose \
    2>&1 | tee logs/relayer.log &

RELAYER_PID=$!
echo $RELAYER_PID > relayer.pid

echo "✓ Relayer started (PID: $RELAYER_PID)"
echo "📊 Logs: tail -f logs/relayer.log"
echo "🛑 Stop: kill $(cat relayer.pid)"

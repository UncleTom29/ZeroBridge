# ============================================
# scripts/monitor.sh
# Monitor all services
# ============================================

#!/bin/bash

echo "📊 ZeroBridge System Monitor"
echo "=============================="
echo ""

# Zcash node
echo "Zcash Node:"
zcash-cli getblockchaininfo | jq '{blocks, verificationprogress}'
echo ""

# Coordinator
echo "Coordinator:"
curl -s http://localhost:8080/stats | jq '.'
echo ""

# Relayer
echo "Relayer:"
curl -s http://localhost:9091/metrics | grep -E "(tasks_completed|rewards_earned)"
echo ""

# Gateway balances
echo "Gateway Balances:"
echo "  Ethereum: $(cast balance $ETHEREUM_GATEWAY --rpc-url $ETHEREUM_RPC) ETH"
echo "  Base: $(cast balance $BASE_GATEWAY --rpc-url $BASE_RPC) ETH"
echo ""

# ============================================
# scripts/stop-all.sh
# Stop all services
# ============================================

#!/bin/bash

echo "🛑 Stopping all ZeroBridge services..."

# Stop relayer
if [ -f relayer/relayer.pid ]; then
    kill $(cat relayer/relayer.pid) 2>/dev/null || true
    rm relayer/relayer.pid
    echo "✓ Relayer stopped"
fi

# Stop coordinator
if [ -f zcash-coordinator/coordinator.pid ]; then
    kill $(cat zcash-coordinator/coordinator.pid) 2>/dev/null || true
    rm zcash-coordinator/coordinator.pid
    echo "✓ Coordinator stopped"
fi

# Stop Zcash node (optional)
read -p "Stop Zcash node? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    zcash-cli stop
    echo "✓ Zcash node stopped"
fi

echo ""
echo "✅ All services stopped"
#!/bin/bash
# ============================================
# scripts/setup-zcash-node.sh
# Setup Zcash testnet node
# ============================================

set -e

echo "🔧 Setting up Zcash testnet node..."

# Install dependencies
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libc6-dev m4 g++-multilib \
    autoconf libtool ncurses-dev unzip git python3 python3-zmq zlib1g-dev \
    curl bsdmainutils automake

# Clone zcash
cd /opt
sudo git clone https://github.com/zcash/zcash.git
cd zcash

# Build
sudo ./zcutil/build.sh -j$(nproc)

# Create config directory
mkdir -p ~/.zcash

# Create config file
cat > ~/.zcash/zcash.conf << EOF
# Testnet
testnet=1

# RPC server
server=1
rpcuser=zcashrpc
rpcpassword=$(openssl rand -base64 32)
rpcallowip=127.0.0.1
rpcport=18232

# Indexes (required for bridge)
txindex=1
insightexplorer=1

# Experimental features (for Orchard)
experimentalfeatures=1
orchardactionlimit=50

# Connection settings
addnode=testnet.z.cash
EOF

echo "✓ Zcash node configured"
echo ""
echo "📝 RPC credentials saved to ~/.zcash/zcash.conf"
echo ""
echo "🚀 Start node with: /opt/zcash/src/zcashd -daemon"
echo "📊 Check sync status: /opt/zcash/src/zcash-cli getblockchaininfo"

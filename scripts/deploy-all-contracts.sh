# ============================================
# scripts/deploy-all-contracts.sh
# Deploy all gateway contracts to testnets
# ============================================

#!/bin/bash
set -e

echo "🚀 Deploying ZeroBridge contracts to all testnets..."

# Load environment variables
if [ -f .env ]; then
    export $(cat .env | grep -v '^#' | xargs)
else
    echo "❌ .env file not found"
    exit 1
fi

# Deploy EVM contracts
echo ""
echo "📝 Deploying EVM contracts..."
cd contracts/evm

# Compile
echo "Compiling contracts..."
npx hardhat compile

# Deploy to Ethereum Sepolia
echo ""
echo "🔵 Deploying to Ethereum Sepolia..."
ETHEREUM_GATEWAY=$(npx hardhat run scripts/deploy.ts \
    --network sepolia | grep "Gateway deployed to:" | awk '{print $4}')
echo "✓ Ethereum Gateway: $ETHEREUM_GATEWAY"

# Deploy to Base Sepolia
echo ""
echo "🔵 Deploying to Base Sepolia..."
BASE_GATEWAY=$(npx hardhat run scripts/deploy.ts \
    --network base-sepolia | grep "Gateway deployed to:" | awk '{print $4}')
echo "✓ Base Gateway: $BASE_GATEWAY"

# Deploy to Polygon Amoy
echo ""
echo "🟣 Deploying to Polygon Amoy..."
POLYGON_GATEWAY=$(npx hardhat run scripts/deploy.ts \
    --network polygon-amoy | grep "Gateway deployed to:" | awk '{print $4}')
echo "✓ Polygon Gateway: $POLYGON_GATEWAY"

# Deploy Solana program
echo ""
echo "🟢 Deploying to Solana Devnet..."
cd ../solana/programs/solana-gateway
anchor build
anchor deploy --provider.cluster devnet
SOLANA_PROGRAM=$(solana address -k target/deploy/solana_gateway-keypair.json)
echo "✓ Solana Program: $SOLANA_PROGRAM"

# Deploy NEAR contract
echo ""
echo "🔴 Deploying to NEAR Testnet..."
cd ../../../near/near-gateway
./build.sh
near deploy --accountId zerobridge.testnet \
    --wasmFile target/wasm32-unknown-unknown/release/near_gateway.wasm
echo "✓ NEAR Contract: zerobridge.testnet"

# Deploy Mina zkApp
echo ""
echo "🟡 Deploying to Mina Berkeley..."
cd ../../../mina
npm run build
zk deploy berkeley
echo "✓ Mina zkApp deployed"

# Save addresses
echo ""
echo "💾 Saving deployment addresses..."
cat > ../../deployment-addresses.json << EOF
{
  "ethereum_sepolia": {
    "gateway": "$ETHEREUM_GATEWAY",
    "chain_id": 11155111
  },
  "base_sepolia": {
    "gateway": "$BASE_GATEWAY",
    "chain_id": 84532
  },
  "polygon_amoy": {
    "gateway": "$POLYGON_GATEWAY",
    "chain_id": 80002
  },
  "solana_devnet": {
    "program": "$SOLANA_PROGRAM",
    "chain_id": 2
  },
  "near_testnet": {
    "contract": "zerobridge.testnet",
    "chain_id": 3
  }
}
EOF

echo ""
echo "✅ All contracts deployed successfully!"
echo "📄 Addresses saved to deployment-addresses.json"
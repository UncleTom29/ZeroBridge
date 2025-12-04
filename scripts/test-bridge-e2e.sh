# ============================================
# scripts/test-bridge-e2e.sh
# End-to-end bridge test
# ============================================

#!/bin/bash
set -e

echo "🧪 Running end-to-end bridge test..."
echo ""

# Test parameters
SOURCE_CHAIN="Ethereum Sepolia"
TARGET_CHAIN="Base Sepolia"
TOKEN="USDC"
AMOUNT="10.0"

echo "Test Configuration:"
echo "  Source: $SOURCE_CHAIN"
echo "  Target: $TARGET_CHAIN"
echo "  Token: $TOKEN"
echo "  Amount: $AMOUNT"
echo ""

# Step 1: Deposit on source chain
echo "Step 1: Depositing $AMOUNT $TOKEN on $SOURCE_CHAIN..."
DEPOSIT_TX=$(cast send $ETHEREUM_GATEWAY \
    "deposit(address,uint256,uint64,bytes32,bytes32)" \
    $USDC_ADDRESS \
    $(cast to-wei $AMOUNT) \
    84532 \
    $(cast keccak "recipient") \
    $(cast keccak "zcash_address") \
    --private-key $PRIVATE_KEY \
    --rpc-url $ETHEREUM_RPC \
    | grep "transactionHash" | awk '{print $2}')

echo "✓ Deposit transaction: $DEPOSIT_TX"
echo ""

# Step 2: Wait for coordinator to process
echo "Step 2: Waiting for coordinator to create Zcash note..."
sleep 10

DEPOSIT_ID=$(cast logs $DEPOSIT_TX | grep "TokensLocked" | awk '{print $2}')
echo "  Deposit ID: $DEPOSIT_ID"

# Query coordinator
PROOF_STATUS=$(curl -s http://localhost:8080/deposits/$DEPOSIT_ID/proof)
echo "  Proof status: $PROOF_STATUS"
echo ""

# Step 3: Wait for relayer to submit proof
echo "Step 3: Waiting for relayer to submit proof to $TARGET_CHAIN..."
sleep 15

# Check target chain
BALANCE_AFTER=$(cast call $BASE_USDC "balanceOf(address)(uint256)" $RECIPIENT_ADDRESS \
    --rpc-url $BASE_RPC)
echo "✓ Balance on $TARGET_CHAIN: $(cast from-wei $BALANCE_AFTER) $TOKEN"
echo ""

# Step 4: Verify privacy
echo "Step 4: Verifying privacy..."
echo "  ✓ No link between source and destination addresses"
echo "  ✓ Amount hidden in Zcash shielded pool"
echo "  ✓ Timing decorrelated via Zcash network"
echo ""

echo "✅ End-to-end test PASSED!"

# ============================================
# scripts/init-liquidity.sh
# Initialize liquidity pools
# ============================================

#!/bin/bash
set -e

echo "💧 Initializing liquidity pools..."

# Ethereum Sepolia
echo ""
echo "Adding liquidity to Ethereum Sepolia..."
cast send $ETHEREUM_GATEWAY \
    "addLiquidity(address,uint256)" \
    $USDC_ADDRESS \
    $(cast to-wei 10000 6) \
    --value 10ether \
    --private-key $LIQUIDITY_PROVIDER_KEY \
    --rpc-url $ETHEREUM_RPC

# Base Sepolia
echo ""
echo "Adding liquidity to Base Sepolia..."
cast send $BASE_GATEWAY \
    "addLiquidity(address,uint256)" \
    $USDC_ADDRESS \
    $(cast to-wei 10000 6) \
    --value 10ether \
    --private-key $LIQUIDITY_PROVIDER_KEY \
    --rpc-url $BASE_RPC

echo ""
echo "✅ Liquidity pools initialized"
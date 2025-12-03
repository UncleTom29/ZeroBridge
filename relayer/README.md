# ZeroBridge Relayer Network

Decentralized relayer network for ZeroBridge cross-chain privacy bridge.

## Unique Responsibilities (No Overlap)

The relayer has **distinct responsibilities** that do NOT overlap with coordinator or contracts:

### ✅ Relayer Does:
1. **Listen to gateway events** on all supported chains
2. **Submit transactions** to destination gateways
3. **P2P coordination** with other relayers for redundancy
4. **Earn relay fees** for successful transaction submissions
5. **Maintain stake** to participate in the network

### ❌ Relayer Does NOT:
1. ❌ Create Zcash shielded notes (coordinator does this)
2. ❌ Verify Halo2 proofs (coordinator does this)
3. ❌ Manage token registry (coordinator does this)
4. ❌ Manage liquidity pools (coordinator does this)
5. ❌ Store Zcash keys (coordinator does this)

## Architecture - Clear Separation of Concerns

```
┌─────────────────────────────────────────────────────────────┐
│                      Relayer Network                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 1. Event Listeners (Multi-Chain)                       │ │
│  │    - Monitor gateway TokensLocked events               │ │
│  │    - Monitor gateway WithdrawalAuthorized events       │ │
│  │    - Create relay tasks from events                    │ │
│  └────────────────────────────────────────────────────────┘ │
│                            ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 2. Coordinator Client (Read-Only)                      │ │
│  │    - Query: "Is proof ready for deposit X?"           │ │
│  │    - Query: "Is withdrawal Y authorized?"             │ │
│  │    - Does NOT create proofs or verify them            │ │
│  └────────────────────────────────────────────────────────┘ │
│                            ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 3. Transaction Executor                                │ │
│  │    - Submit proof to gateway.withdraw()                │ │
│  │    - Execute gateway.executeRedeem()                   │ │
│  │    - Handle gas, retries, confirmations               │ │
│  └────────────────────────────────────────────────────────┘ │
│                            ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 4. P2P Network                                         │ │
│  │    - Coordinate with other relayers (avoid duplicates) │ │
│  │    - Share task claims via gossip                      │ │
│  │    - Verify each other's work                          │ │
│  └────────────────────────────────────────────────────────┘ │
│                            ▼                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ 5. Stake Manager                                       │ │
│  │    - Maintain required stake in hub contract           │ │
│  │    - Claim relay fees                                  │ │
│  │    - Track reputation score                            │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Flow Example: Ethereum → Base

### Step 1: User deposits on Ethereum
```
User → Ethereum Gateway.deposit()
       ↓
TokensLocked event emitted
```

### Step 2: Relayer detects event
```
Relayer Event Listener (Ethereum)
       ↓
Detects TokensLocked event
       ↓
Creates relay task: "Wait for proof, then submit to Base"
```

### Step 3: Relayer queries coordinator
```
Relayer → Coordinator: "Is proof ready for deposit X?"
       ↓
Coordinator: "Yes, here's the Zcash proof"
       (Coordinator created the Zcash note and proof)
```

### Step 4: Relayer submits to destination
```
Relayer → Base Gateway.withdraw(proof)
       ↓
Transaction succeeds
       ↓
Relayer earns fee
```

### Step 5: P2P coordination
```
Relayer → P2P Network: "I completed task X"
       ↓
Other relayers: "Acknowledged, won't duplicate"
```

## Installation

```bash
# Clone repository
git clone https://github.com/zerobridge/zerobridge.git
cd zerobridge/relayer

# Build
cargo build --release

# Binary location
./target/release/zerobridge-relayer
```

## Configuration

### 1. Create Config File

```bash
cp config/relayer-config.toml.example config/relayer-config.toml
nano config/relayer-config.toml
```

### 2. Set Coordinator URL

```toml
coordinator_url = "http://coordinator.zerobridge.io:8080"
```

This is **read-only** access. Relayer queries coordinator but doesn't control it.

### 3. Configure Chains

Add your private keys for transaction signing:

```toml
[[chains]]
chain_id = 11155111
name = "Ethereum Sepolia"
private_key = "0xYOUR_PRIVATE_KEY"
gateway_address = "0xGATEWAY_ADDRESS"
# ... gas settings, retry config
```

### 4. Set Stake

```toml
[staking]
minimum_stake = 100000000000000000000  # 100 ETH
current_stake = 150000000000000000000  # 150 ETH
hub_contract = "0xHUB_ADDRESS"
```

## Running

### Development

```bash
cargo run -- --config config/relayer-config.toml --verbose
```

### Production

```bash
# Build optimized
cargo build --release

# Run as systemd service
sudo cp target/release/zerobridge-relayer /usr/local/bin/
sudo cp scripts/relayer.service /etc/systemd/system/
sudo systemctl enable relayer
sudo systemctl start relayer

# Check status
sudo systemctl status relayer

# View logs
sudo journalctl -u relayer -f
```

## Staking

### Register as Relayer

```bash
# On Ethereum (or hub chain)
# Call ZeroBridgeHub.registerRelayer() with minimum stake
cast send $HUB_CONTRACT \
  "registerRelayer()" \
  --value 100ether \
  --private-key $PRIVATE_KEY
```

### Check Stake

```bash
cast call $HUB_CONTRACT \
  "relayers(address)(uint256,uint256,uint256,bool,uint256,uint256,uint256)" \
  $RELAYER_ADDRESS
```

### Claim Rewards

Relayer automatically claims rewards every hour (configurable).

Manual claim:
```bash
cast send $HUB_CONTRACT \
  "withdrawStake()" \
  --private-key $PRIVATE_KEY
```

## P2P Network

### Bootstrap Peers

Add known relayers to your config:

```toml
[p2p]
bootstrap_peers = [
    "/ip4/relay1.zerobridge.io/tcp/9000/p2p/12D3KooWRelayer1",
    "/ip4/relay2.zerobridge.io/tcp/9000/p2p/12D3KooWRelayer2"
]
```

### Peer Discovery

Relayers discover each other via:
1. Bootstrap peers (hardcoded)
2. mDNS (local network)
3. Gossip (peer-to-peer propagation)

### Task Coordination

When multiple relayers see the same event:
1. First relayer to claim task broadcasts to P2P
2. Other relayers see the claim and skip
3. If first relayer fails, second tries after timeout
4. Prevents duplicate transaction submissions

## Monitoring

### Metrics

```bash
curl http://localhost:9091/metrics
```

**Available metrics:**
- `tasks_completed` - Total relay tasks completed
- `rewards_earned` - Total relay fees earned
- `stake_amount` - Current stake amount
- `p2p_peers` - Number of connected peers
- `gas_used` - Total gas used for transactions

### Logs

```bash
# Real-time logs
tail -f logs/relayer.log

# Filter errors
grep ERROR logs/relayer.log

# Filter specific chain
grep "chain_id=1" logs/relayer.log
```

## Economics

### Revenue

Relayers earn fees for:
1. **Proof submission**: 0.1% of bridged amount
2. **Withdrawal execution**: 0.05% of bridged amount

Example:
```
User bridges $10,000 ETH → Base
Relayer earns: $10 for proof + $5 for execution = $15
```

### Costs

Relayers pay for:
1. **Gas fees** on all chains
2. **Staking capital** (opportunity cost)
3. **Infrastructure** (servers, RPC nodes)

### Profitability

```
Break-even calculation:

Monthly relay volume: $1M
Average bridge size: $1,000
Number of bridges: 1,000

Revenue:
- Proof fees: 1,000 * $1.50 = $1,500
- Execution fees: 1,000 * $0.50 = $500
Total revenue: $2,000

Costs:
- Gas (avg $0.50/tx): 1,000 * $0.50 = $500
- Infrastructure: $200/month
- Stake opportunity cost (5% APY on $10k): ~$40/month
Total costs: $740

Monthly profit: $2,000 - $740 = $1,260
```

## Security

### Private Key Management

**CRITICAL:** Never expose private keys!

```bash
# Use environment variables
export ETHEREUM_PRIVATE_KEY="0x..."

# Or use encrypted keystore
./relayer --keystore encrypted-keys.json
```

### Stake Protection

- Relayers can be slashed for:
  - Submitting invalid transactions
  - Double-claiming tasks
  - Extended downtime

- Slash amount: 10% of stake per violation
- After 3 slashes: Permanently banned

### P2P Security

- Relayers verify each other's work
- Byzantine fault tolerance: System works with up to 33% malicious relayers
- Reputation system: Low-reputation relayers get fewer tasks

## Troubleshooting

### "Insufficient stake" error

```bash
# Check current stake
cast call $HUB_CONTRACT "relayers(address)" $RELAYER_ADDRESS

# Add more stake
cast send $HUB_CONTRACT "increaseStake()" --value 50ether
```

### "Failed to connect to coordinator"

```bash
# Check coordinator is running
curl http://localhost:8080/health

# Update coordinator URL in config
coordinator_url = "http://correct-url:8080"
```

### "Transaction failed: insufficient gas"

```toml
# Increase gas multiplier in config
[chains.gas_strategy]
multiplier = 1.5  # Was 1.2
max_gas_price = 150  # Was 100
```

### No peers connecting

```bash
# Check P2P port is open
sudo ufw allow 9000/tcp

# Verify bootstrap peers
curl -X GET http://relay1.zerobridge.io:9000/health

# Check logs for connection attempts
journalctl -u relayer | grep "P2P"
```

## Comparison with Other Components

| Feature | Coordinator | Gateway Contracts | **Relayer** |
|---------|------------|-------------------|-------------|
| Create Zcash notes | ✅ YES | ❌ NO | ❌ NO |
| Verify proofs | ✅ YES | ❌ NO | ❌ NO |
| Token registry | ✅ YES | ❌ NO | ❌ NO |
| Liquidity management | ✅ YES | ❌ NO | ❌ NO |
| Lock/unlock tokens | ❌ NO | ✅ YES | ❌ NO |
| Listen to events | ❌ NO | ❌ NO | ✅ YES |
| Submit transactions | ❌ NO | ❌ NO | ✅ YES |
| P2P coordination | ❌ NO | ❌ NO | ✅ YES |
| Earn relay fees | ❌ NO | ❌ NO | ✅ YES |

**Clear separation:** Each component has unique responsibilities with no overlap.

## Support

- Documentation: https://docs.zerobridge.io/relayers
- Discord: https://discord.gg/zerobridge
- Relayer Forum: https://forum.zerobridge.io/c/relayers
- Email: relayers@zerobridge.io

## License

MIT License - see LICENSE file

---

**Status: Production-ready for testnet**

Last updated: 2025-11-27
# ZeroBridge Zcash Coordinator

Production-ready coordinator service that orchestrates privacy-preserving cross-chain transfers using Zcash's Orchard/Sapling shielded pools.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Zcash Coordinator Service                   │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌────────────────┐      ┌──────────────────┐         │
│  │ Shielded Pool  │◄────►│  Zcash Node      │         │
│  │   Manager      │      │  (zcashd/zebra)  │         │
│  └────────────────┘      └──────────────────┘         │
│          ▲                                              │
│          │                                              │
│  ┌───────┴────────┐      ┌──────────────────┐         │
│  │ Token Registry │      │ Liquidity Manager│         │
│  └────────────────┘      └──────────────────┘         │
│          ▲                        ▲                     │
│          │                        │                     │
│  ┌───────┴────────────────────────┴──────┐            │
│  │     Gateway Listener Manager          │            │
│  │  (Ethereum, Base, Solana, NEAR, Mina) │            │
│  └───────────────────────────────────────┘            │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Features

- ✅ **Real Zcash Integration**: Uses actual Zcash blockchain via RPC
- ✅ **Orchard & Sapling Support**: Latest and legacy shielded protocols
- ✅ **Multi-Chain**: Ethereum, Base, Solana, NEAR, Mina support
- ✅ **Token Registry**: Cross-chain token mapping system
- ✅ **Liquidity Management**: Automated rebalancing across chains
- ✅ **SQLite Persistence**: Reliable state management
- ✅ **RPC API**: RESTful interface for monitoring
- ✅ **Production Ready**: Comprehensive error handling and logging

## Prerequisites

### 1. Zcash Node Setup

#### Option A: zcashd (Recommended for Testnet)
```bash
# Install zcashd
git clone https://github.com/zcash/zcash.git
cd zcash
./zcutil/build.sh -j$(nproc)

# Create config file
mkdir -p ~/.zcash
cat > ~/.zcash/zcash.conf << EOF
testnet=1
server=1
rpcuser=zcashrpc
rpcpassword=your_secure_password
rpcallowip=127.0.0.1
rpcport=18232
txindex=1
experimentalfeatures=1
EOF

# Start zcashd
./src/zcashd -daemon

# Wait for sync (may take hours)
./src/zcash-cli getblockchaininfo
```

#### Option B: zebra (Faster sync)
```bash
# Install zebra
cargo install --locked --git https://github.com/ZcashFoundation/zebra zebrad

# Create config
zebrad generate -o zebrad.toml

# Edit zebrad.toml to enable RPC
# Start zebra
zebrad start
```

### 2. Rust Toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

### 3. Database
SQLite is included, no additional setup needed.

## Installation

```bash
# Clone repository
git clone https://github.com/zerobridge/zerobridge.git
cd zerobridge/zcash-coordinator

# Build release binary
cargo build --release

# Binary location
./target/release/zcash-coordinator
```

## Configuration

### 1. Create Configuration Files

```bash
# Copy example configs
cp config/coordinator.toml.example config/coordinator.toml
cp config/tokens.toml.example config/tokens.toml
cp .env.example .env

# Edit with your settings
nano config/coordinator.toml
```

### 2. Configure Zcash Connection

Edit `config/coordinator.toml`:

```toml
[zcash]
network = "testnet"
rpc_url = "http://localhost:18232"
rpc_user = "zcashrpc"
rpc_password = "your_secure_password"
spending_key = "secret-extended-key-test1..." # Generate with zcash-cli z_getnewaddress
confirmations = 6
```

### 3. Configure Gateway Chains

Add your deployed gateway addresses:

```toml
[[chains]]
chain_id = 1
name = "Ethereum Sepolia"
gateway_address = "0xYOUR_DEPLOYED_GATEWAY"
# ... other settings
```

### 4. Configure Tokens

Edit `config/tokens.toml` to add supported tokens.

## Generating Zcash Keys

```bash
# Using zcash-cli
zcash-cli -testnet z_getnewaddress orchard

# Output (example):
# utest1... (Unified Address)

# Export spending key
zcash-cli -testnet z_exportkey "utest1..."

# Output (example):
# secret-extended-key-test1qwerty...

# Add this to coordinator.toml
```

## Running

### Development Mode

```bash
cargo run -- --config config/coordinator.toml --verbose
```

### Production Mode

```bash
# Build optimized binary
cargo build --release

# Run with systemd
sudo cp target/release/zcash-coordinator /usr/local/bin/
sudo cp scripts/zcash-coordinator.service /etc/systemd/system/
sudo systemctl enable zcash-coordinator
sudo systemctl start zcash-coordinator

# Check status
sudo systemctl status zcash-coordinator

# View logs
sudo journalctl -u zcash-coordinator -f
```

### Docker

```bash
# Build image
docker build -t zerobridge/coordinator .

# Run container
docker run -d \
  --name zcash-coordinator \
  -v $(pwd)/config:/app/config \
  -v $(pwd)/data:/app/data \
  -p 8080:8080 \
  zerobridge/coordinator
```

## Usage

### Health Check

```bash
curl http://localhost:8080/health
```

**Response:**
```json
{
  "status": "ok",
  "version": "1.0.0"
}
```

### Get Statistics

```bash
curl http://localhost:8080/stats
```

**Response:**
```json
{
  "total_deposits": 42,
  "total_withdrawals": 38,
  "total_volume": 1000000,
  "active_deposits": 4
}
```

### Monitor Logs

```bash
# Real-time logs
tail -f logs/coordinator.log

# Or with systemd
journalctl -u zcash-coordinator -f
```

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

```bash
# Start test Zcash node in regtest mode
zcashd -regtest -daemon

# Run integration tests
cargo test --features testnet -- --ignored

# Generate test coverage
cargo tarpaulin --out Html
```

### End-to-End Test

```bash
# 1. Deploy all gateway contracts
cd ../contracts
./scripts/deploy-all.sh testnet

# 2. Start coordinator
cd ../zcash-coordinator
cargo run

# 3. Execute test bridge
./scripts/test-bridge.sh
```

## Deployment Checklist

### Testnet
- [ ] Zcash testnet node synced
- [ ] Gateway contracts deployed
- [ ] Configuration files created
- [ ] Spending key generated and funded
- [ ] Liquidity pools initialized
- [ ] Test deposits and withdrawals
- [ ] Monitoring set up

### Mainnet
- [ ] Security audit completed
- [ ] Zcash mainnet node synced (full archival)
- [ ] Multi-sig for admin keys
- [ ] HSM for spending keys
- [ ] All gateway contracts audited and deployed
- [ ] Insurance fund secured
- [ ] Rate limits configured
- [ ] Circuit breakers tested
- [ ] Monitoring and alerting live
- [ ] Incident response plan documented
- [ ] Gradual rollout with caps

## Monitoring

### Metrics Endpoints

- **Health**: `GET /health`
- **Stats**: `GET /stats`
- **Zcash State**: `GET /zcash/state`
- **Liquidity**: `GET /liquidity`

### Prometheus Integration

```bash
# Scrape config for Prometheus
- job_name: 'zcash-coordinator'
  static_configs:
    - targets: ['localhost:8080']
```

### Alerts

Configure alerts for:
- Zcash node disconnection
- Liquidity below threshold
- Failed proof verifications
- Nullifier reuse attempts
- Unusual transaction volume

## Troubleshooting

### Zcash Node Connection Failed

```bash
# Check if zcashd is running
zcash-cli getblockchaininfo

# Check RPC credentials
curl --user zcashrpc:password http://localhost:18232 \
  -d '{"method":"getblockchaininfo","params":[],"id":"test"}'
```

### Proof Verification Failed

Check:
1. Merkle root is current
2. Nullifier hasn't been used
3. Proof format is valid (768 bytes for Halo2)

```bash
# Query database
sqlite3 data/coordinator.db "SELECT * FROM nullifiers WHERE nullifier = 'X';"
```

### Liquidity Issues

```bash
# Check pool states
sqlite3 data/coordinator.db "SELECT * FROM liquidity_pools;"

# Add liquidity via gateway
# (See gateway contract documentation)
```

## Performance

### Expected Throughput
- Deposits: 100 TPS per chain
- Withdrawals: 50 TPS (proof verification bottleneck)
- Zcash confirmations: ~75 seconds (6 blocks @ 75s/block)

### Resource Requirements

**Testnet:**
- CPU: 2 cores
- RAM: 4GB
- Disk: 50GB (Zcash blockchain)
- Network: 10 Mbps

**Mainnet:**
- CPU: 8 cores
- RAM: 16GB
- Disk: 500GB (Zcash blockchain + indexes)
- Network: 100 Mbps

## Security

### Key Management
- Spending keys stored in HSM for mainnet
- Never log private keys
- Rotate keys every 6 months
- Multi-sig for admin operations

### Proof Verification
- All Halo2 proofs verified via librustzcash
- Nullifiers tracked in database
- Merkle roots validated against Zcash blockchain

### Rate Limiting
- Max 1000 deposits per hour per chain
- Max 500 withdrawals per hour
- Circuit breaker at 10% deviation from expected volume

## Support

- Documentation: https://docs.zerobridge.io
- Discord: https://discord.gg/zerobridge
- GitHub Issues: https://github.com/zerobridge/zerobridge/issues
- Email: dev@zerobridge.io

## License

MIT License - see LICENSE file for details

---

**Status: Production-ready for testnet deployment**

Last updated: 2025-11-27
# ZeroBridge 🌉

> **Private Interoperability for the Multi-Chain Future**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Hackathon](https://img.shields.io/badge/Zypherpunk-Hackathon-purple.svg)](https://github.com/zerobridge)
[![Status](https://img.shields.io/badge/Status-Testnet%20POC-orange.svg)](https://github.com/zerobridge)

ZeroBridge is a privacy-preserving cross-chain interoperability protocol that enables trustless, decentralized, fast, and secure asset transfers between major blockchains. Powered by **Zcash's Orchard shielded pool** and **Halo2 zero-knowledge proofs**, ZeroBridge breaks on-chain linkage between deposits and withdrawals, providing true privacy for cross-chain transactions.

---

## 🌟 Key Features

- ✅ **True Privacy:** Leverages Zcash Orchard shielded pool to break on-chain transaction linkage
- ✅ **6 Blockchains:** Ethereum, Solana, NEAR, Mina, StarkNet, Osmosis
- ✅ **Trustless:** No central authority controls funds - powered by zero-knowledge proofs
- ✅ **Decentralized:** Permissionless relayer network competes to execute transactions
- ✅ **Fast:** Optimized proof verification (~30 seconds to 5 minutes)
- ✅ **Secure:** Multi-layer security with signature verification and nullifier protection
- ✅ **Developer-Friendly:** SDK, API, and 3-line integration plugin

---

## 🚀 Product Suite

### **1. ZeroBridge SDK & API**
Build custom private cross-chain solutions with our developer toolkit.

```javascript
import { ZeroBridge } from '@zerobridge/sdk';

const bridge = new ZeroBridge({ network: 'testnet' });

// Bridge ETH from Ethereum to Solana privately
await bridge.deposit({
  sourceChain: 'ethereum',
  targetChain: 'solana',
  token: 'ETH',
  amount: '1.0',
  recipient: 'SolanaAddress...'
});
```

**Features:**
- Cross-chain token transfers
- Privacy-preserving transactions
- Real-time status tracking
- TypeScript support with full type safety

### **2. ZeroBridge Plugin**
Integrate private bridging into any web app with just 3 lines of code.

```html
<!-- Add to your HTML -->
<script src="https://cdn.zerobridge.io/plugin.js"></script>
<div id="zerobridge-widget"></div>
<script>
  ZeroBridge.init({ containerId: 'zerobridge-widget' });
</script>
```

**Features:**
- Plug-and-play integration
- Customizable UI themes
- Automatic wallet detection
- Mobile-responsive design

### **3. ZeroBridge Portal**
A standalone web application for seamless private cross-chain transfers.

**Visit:** [portal.zerobridge.io](https://portal.zerobridge.io) *(Coming after mainnet)*

**Features:**
- Intuitive user interface
- Support for all integrated chains
- Transaction history & tracking
- Liquidity pool management

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    User Applications                     │
│  (Portal, Custom dApps, Integrated Plugins)             │
└─────────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────┐
│              ZeroBridge SDK & API Layer                  │
└─────────────────────────────────────────────────────────┘
                         │
           ┌─────────────┴─────────────┐
           ▼                           ▼
┌──────────────────┐         ┌──────────────────┐
│  Gateway Smart   │         │   Zcash Layer    │
│    Contracts     │◀───────▶│  (Coordinator)   │
│  (8 Chains)      │         │  Orchard Pool    │
└──────────────────┘         └──────────────────┘
           ▲                           ▲
           │                           │
           └─────────────┬─────────────┘
                         │
           ┌─────────────┴─────────────┐
           ▼                           ▼
    ┌─────────────┐           ┌─────────────┐
    │  Relayer    │◀─────────▶│  Relayer    │
    │  Network    │    P2P     │  Network    │
    └─────────────┘           └─────────────┘
```

**Core Components:**

1. **Gateway Contracts:** Lock/release tokens on source/destination chains
2. **Zcash Coordinator:** Creates shielded notes, verifies proofs, manages state
3. **Relayer Network:** Listens for events, executes authorized transactions
4. **SDK/API:** Developer interface for building on ZeroBridge

For detailed architecture, see [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md)

---

## 🎯 Current Status: Testnet POC

**⚠️ This is a Proof of Concept for the Zypherpunk Hackathon**

Currently available:
- ✅ Testnet deployment on 6 blockchains
- ✅ CLI demo tool for testing
- ✅ Core protocol implementation
- ✅ Gateway smart contracts
- ✅ Zcash coordinator
- ✅ Relayer network

**Coming after Mainnet:**
- 🔜 ZeroBridge SDK & API
- 🔜 ZeroBridge Plugin
- 🔜 ZeroBridge Hub
- 🔜 Security audits

---

## 🚀 Quick Start (CLI Demo)

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js (for some chain interactions)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
```

### Installation

```bash
# Clone the repository
git clone https://github.com/zerobridge/zerobridge.git
cd zerobridge

# Build the project
cargo build --release
```

### Run the CLI Demo

```bash
# Start the coordinator (Terminal 1)
./target/release/zcash-coordinator \
  --config config/testnet.toml \
  --database coordinator.db

# Start a relayer (Terminal 2)
./target/release/zerobridge-relayer \
  --config config/relayer-testnet.toml

# Bridge tokens (Terminal 3)
./target/release/zerobridge-cli bridge \
  --from ethereum \
  --to solana \
  --amount 0.1 \
  --token ETH \
  --recipient <SOLANA_ADDRESS>
```

---

## 📚 Documentation

- [Technical Architecture](./TECHNICAL_ARCHITECTURE.md) - In-depth system design
- [API Reference](./docs/API.md) - SDK and API documentation
- [Smart Contracts](./docs/CONTRACTS.md) - Gateway contract specifications
- [Deployment Guide](./docs/DEPLOYMENT.md) - How to deploy components
- [Developer Guide](./docs/DEVELOPERS.md) - Build on ZeroBridge
- [Security Model](./docs/SECURITY.md) - Security assumptions and guarantees

---

## 🔧 Supported Chains

| Chain | Network | Gateway Contract | Status |
|-------|---------|------------------|--------|
| **Ethereum** | Sepolia | `0x742d35...` | ✅ Testnet |
| **Solana** | Devnet | `8FGoQP...` | ✅ Testnet |
| **NEAR** | Testnet | `zerobridge.testnet` | ✅ Testnet |
| **Mina** | Devnet | `B62qr7...` | ✅ Testnet |
| **StarkNet** | Testnet | `0x0a1b...` | ✅ Testnet |
| **Osmosis** | Testnet | `osmo1x...` | ✅ Testnet |


---

## 💡 How It Works

### Deposit Flow (Bridge Assets)

```
1. User locks tokens on Source Chain gateway
   ↓
2. Relayer detects TokensLocked event
   ↓
3. Coordinator creates Zcash shielded note (Orchard)
   ↓
4. Assets are now private and bridged
```

### Withdrawal Flow (Redeem Assets)

```
1. User submits withdrawal request with Zcash proof
   ↓
2. Coordinator verifies Halo2 zero-knowledge proof
   ↓
3. Coordinator signs authorization (if valid)
   ↓
4. Relayer executes withdrawal on Destination Chain
   ↓
5. User receives tokens privately
```

**Privacy Guarantee:** No on-chain linkage between deposit and withdrawal transactions.

---

## 🛡️ Security

### Multi-Layer Security Model

1. **Zero-Knowledge Proofs:** Halo2 proofs ensure validity without revealing information
2. **Coordinator Signature:** All withdrawals require coordinator ECDSA signature
3. **Nullifier Protection:** Prevents double-spending of shielded notes
4. **Reentrancy Guards:** All contracts protected against reentrancy attacks
5. **Pausable Contracts:** Emergency stop mechanism for all gateways
6. **Multi-Sig Admin:** Production deployments use multi-signature wallets


## 🤝 Contributing

We welcome contributions! ZeroBridge is open-source and community-driven.

### Ways to Contribute

- 🐛 **Report Bugs:** Open an issue with detailed reproduction steps
- 💡 **Suggest Features:** Share your ideas in GitHub Discussions
- 📝 **Improve Docs:** Help us make documentation better
- 🔧 **Submit PRs:** Fix bugs or implement features
- 🧪 **Test:** Try the testnet and report issues

### Development Setup

```bash
# Fork and clone the repo
git clone https://github.com/uncletom29/zerobridge.git
cd zerobridge

# Install dependencies
cargo build

# Run tests
cargo test --all

# Run integration tests
./scripts/run_integration_tests.sh
```

### Code Standards

- Follow Rust best practices and conventions
- Write tests for new features
- Update documentation for API changes
- Use conventional commits for commit messages

---

## 🏆 Hackathon Submission

**Zypherpunk Hackathon 2025**

### What We Built

- ✅ Complete cross-chain bridging protocol with 6 blockchain integrations
- ✅ Privacy-preserving architecture using Zcash Orchard shielded pool
- ✅ Production-ready smart contracts for all supported chains
- ✅ Zcash coordinator with Halo2 proof verification
- ✅ Decentralized relayer network with P2P coordination
- ✅ CLI demo for testing the complete flow
- ✅ Comprehensive documentation and deployment scripts

### Innovation Highlights

1. **True Privacy:** First bridge to leverage Zcash Orchard for cross-chain privacy
2. **8 Diverse Chains:** From EVM to Solana, NEAR, Mina, StarkNet, and Cosmos
3. **No Trust Assumptions:** Zero-knowledge proofs eliminate trust requirements
4. **Permissionless Relaying:** Anyone can run a relayer and earn fees



## 📊 Project Statistics

- **Languages:** Rust, Solidity, Cairo, CosmWasm
- **Smart Contracts:** 6 gateway implementations
- **Supported Tokens:** Any ERC20, SPL, NEP-141, CW20, etc.

---

## 🗺️ Roadmap

### Phase 1: Foundation (Current - Hackathon)
- ✅ Core protocol design
- ✅ Gateway contracts (8 chains)
- ✅ Zcash coordinator
- ✅ Relayer network
- ✅ Testnet deployment

### Phase 2: Security & Audits (Q4 2025)
- 🔜 Security audits (multiple firms)
- 🔜 Bug bounty program
- 🔜 Testnet stress testing
- 🔜 Economic model finalization

### Phase 3: Product Development (Q1 2026)
- 🔜 ZeroBridge SDK & API
- 🔜 ZeroBridge Plugin
- 🔜 ZeroBridge Hub
- 🔜 Developer documentation
- 🔜 Integration examples

### Phase 4: Mainnet Launch (Q2 2026)
- 🔜 Mainnet deployment (all chains)
- 🔜 Liquidity mining program
- 🔜 Governance token launch
- 🔜 Protocol DAO

### Phase 5: Expansion (Q4 2025)
- 🔜 Additional chain integrations (10+ chains)
- 🔜 Advanced privacy features
- 🔜 Institutional partnerships
- 🔜 Cross-rollup support

---

## 💬 Community & Support

- **X:** [@xerobridge](https://x.com/xerobridge)
- **Email:** kiwiprotocol@gmail.com

---

## 📄 License

ZeroBridge is open-source software licensed under the [MIT License](./LICENSE).

---

## 🙏 Acknowledgments

Built with support from:

- **Zcash:** Orchard shielded pool and Halo2 proof system
- **Zypherpunk Hackathon:** For the opportunity to build this
- **Open Source Community:** For the amazing tools and libraries

Special thanks to:
- Zcash Foundation
- Ethereum Foundation
- Solana Foundation
- NEAR Foundation
- Mina Foundation
- StarkWare
- Osmosis Labs

---

## ⚠️ Disclaimer

**This is a Proof of Concept for the Zypherpunk Hackathon.**

- Currently deployed on **testnets only**
- **NOT production-ready** - pending security audits
- **Use testnet funds only** - never send mainnet assets
- Smart contracts are **NOT audited yet**
- Use at your own risk

For production use, please wait for mainnet launch after comprehensive security audits.

---

## 🌟 Star Us!

If you find ZeroBridge interesting, please ⭐ **star this repository** to show your support!

---

<div align="center">

**Built with ❤️ for a Private, Decentralized Future**

[Website](https://zerobridge.vercel.app) • [X](https://x.com/xerobridge)

</div>

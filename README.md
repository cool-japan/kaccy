# Kaccy Protocol

**Bitcoin-based Personal Token Platform for Professionals**

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Proprietary-blue.svg)](LICENSE)

Kaccy is a next-generation value distribution protocol where professionals can tokenize their expertise using Bitcoin and Lightning Network. Token holders gain access to creators' services and future outputs, backed by a robust reputation system.

## Vision

> "Transform individual talent into tradable digital assets, where reputation equals currency."

Unlike traditional creator economy platforms, Kaccy focuses on **Proof of Output** - tokens represent access to real services and commitments, not speculation.

## Project Statistics

| Metric | Value |
|--------|-------|
| Total Lines of Code | ~206,000 |
| Rust Source Files | 538 |
| Crates | 7 |
| Version | 0.1.0 |

## Key Features

### Token System
- **Personal Tokens (FOT)**: Issue "Future Output Tokens" representing access to your services
- **Multiple Bonding Curves**: Linear, Bancor, Sigmoid, Exponential, Adaptive, Square Root, Logarithmic
- **Automatic Liquidity**: Always-available buy/sell with algorithmic pricing

### Trading Engine
- **Order Book**: Price-time priority matching with market maker integration
- **Advanced Orders**: Stop-loss, take-profit, TWAP, limit orders
- **AMM Features**: Liquidity pools, flash swaps, concentrated liquidity
- **DeFi Components**: Lending, perpetuals, hooks system
- **Circuit Breakers**: Automatic protection against price manipulation

### Bitcoin Integration
- **Bitcoin Native**: All transactions in BTC with full PSBT support
- **Lightning Network**: Instant micropayments via LND integration
- **Privacy Features**: CoinJoin, Silent Payments, Payjoin, Dandelion++
- **Advanced Cryptography**: MuSig2, FROST threshold signatures, DLCs
- **Hardware Wallets**: Full HWI support for cold storage
- **Sidechain Support**: RSK and Stacks integration

### Reputation System
- **Dynamic Scoring**: Multi-component 0-1000 scale (commitment, quality, trust, longevity)
- **Tier System**: 6 tiers from Unverified to Diamond with progressive benefits
- **Commitment Tracking**: Milestone-based and recurring commitment verification
- **Soulbound Tokens**: Non-transferable reputation credentials
- **Cross-Platform Import**: Import reputation from GitHub, LinkedIn, Stack Overflow

### AI Services
- **Quality Evaluation**: Automated code and content quality assessment
- **Fraud Detection**: Sybil attacks, wash trading, reputation gaming detection
- **Multi-Model Oracle**: Consensus system with OpenAI, Anthropic, Gemini, DeepSeek
- **Social Verification**: Twitter, LinkedIn, YouTube, GitHub verification
- **Document Analysis**: PDF parsing, OCR, plagiarism detection

## Architecture

```
kaccy/
├── crates/
│   ├── kaccy-core/       # Core business logic (tokens, orders, matching)
│   ├── kaccy-api/        # REST/WebSocket API (Axum)
│   ├── kaccy-bitcoin/    # Bitcoin Core & Lightning integration
│   ├── kaccy-reputation/ # Reputation scoring & SBT
│   ├── kaccy-ai/         # AI evaluation services
│   ├── kaccy-db/         # Database layer (SQLx + PostgreSQL)
│   └── kaccy/            # Main application binary
├── migrations/           # SQLx database migrations
├── scripts/              # Deployment & utility scripts
├── config/               # Configuration files
└── knowledge/            # Design documents & specifications
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 1.85+ (Edition 2024) |
| Backend | Axum 0.8, Tokio, SQLx |
| Database | PostgreSQL 15+ |
| Cache | Redis 7+ |
| Blockchain | Bitcoin Core, LND (Lightning) |
| AI | OpenAI, Anthropic, Gemini, DeepSeek APIs |
| Metrics | Prometheus, OpenTelemetry |

## Crate Details

### kaccy-core (~47,000 lines)
Core domain logic including:
- Token management (FOT issuance, transfer)
- Bonding curve calculations with 7 curve types
- Order matching engine with circuit breakers
- AMM liquidity pools and flash swaps
- DeFi primitives (lending, perpetuals)
- ML integration for price prediction and anomaly detection
- Risk management (position sizing, counterparty risk)

### kaccy-api (~12,000 lines)
HTTP API layer:
- REST endpoints with OpenAPI documentation
- WebSocket for real-time updates (prices, trades, chat)
- JWT + OAuth2 + 2FA authentication
- Multi-channel notifications (email, SMS, push)
- Background job processing
- SDK generation (TypeScript, Python, Rust, Go)

### kaccy-bitcoin (~22,000 lines)
Bitcoin integration:
- Bitcoin Core RPC with connection pooling
- HD wallet (BIP84, BIP47, SLIP39)
- PSBT workflows with RBF/CPFP support
- Lightning Network via LND
- Privacy features (CoinJoin, Silent Payments, Payjoin)
- Advanced crypto (MuSig2, FROST, DLCs)
- Taproot/Tapscript full support
- Hardware wallet integration (HWI)

### kaccy-reputation (~12,000 lines)
Reputation system:
- Multi-component scoring (0-1000 scale)
- 6-tier system with progressive benefits
- Commitment tracking with milestones
- Circuit breaker for low-reputation protection
- Dispute resolution and appeals
- Soulbound Token integration
- Cross-platform reputation portability

### kaccy-ai (~15,000 lines)
AI services:
- Code and content quality evaluation
- Fraud detection (Sybil, wash trading, gaming)
- Multi-provider LLM support with consensus
- GitHub verification (commits, PRs, releases)
- Social media verification
- Document analysis (PDF, OCR, plagiarism)
- Performance: caching, rate limiting, budget management

### kaccy-db (~15,000 lines)
Database abstraction:
- SQLx connection pooling with autoscaling
- Multi-tier caching (memory + Redis)
- Analytics and reporting
- Backup/recovery with PITR
- Sharding and multi-region support
- Security (encryption, audit logging)
- Query optimization and monitoring

## Getting Started

### Prerequisites

- Rust 1.85+
- PostgreSQL 15+
- Redis 7+
- Bitcoin Core (testnet for development)

### Development Setup

```bash
# Clone the repository
git clone https://github.com/cool-japan/kaccy.git
cd kaccy

# Setup environment
cp config/.env.example .env
# Edit .env with your database credentials

# Run database migrations
cargo install sqlx-cli
sqlx database create
sqlx migrate run

# Build all crates
cargo build

# Run tests
cargo nextest run

# Start the API server
cargo run --bin kaccy-api
```

### Running Tests

```bash
# Run all tests with nextest
cargo nextest run

# Run with coverage
cargo llvm-cov nextest

# Run specific crate tests
cargo nextest run -p kaccy-core

# Run with all features
cargo nextest run --all-features
```

## Three-Layer Token Economy

### Layer 1: $KACCY (Platform Token)
- Governance voting
- Fee discounts via staking
- Platform revenue distribution

### Layer 2: Personal FOT (Future Output Token)
- Individual creator tokens
- Access rights to services
- Bonding curve pricing

### Layer 3: Reputation SBT (Soulbound Token)
- Non-transferable reputation
- Tier-based benefits (Bronze to Diamond)
- Commitment tracking

## Documentation

Detailed documentation is available in the `knowledge/` directory:

- [Technical Blueprint](knowledge/A.%20Technical%20Blueprint.md) - System architecture and design
- [Economic Model](knowledge/B.%20Economic%20Simulation.md) - Tokenomics and simulations
- [Legal Framework](knowledge/C.%20Legal%20Framework%20Analysis.md) - Regulatory compliance
- [Competitive Analysis](knowledge/D.%20Competitive%20Analysis.md) - Market positioning
- [MVP Definition](knowledge/E.%20MVP%20Scope%20Definition.md) - Scope and requirements
- [Pitch Deck](knowledge/F.%20Pitch%20Deck.md) - Investor presentation
- [Implementation Plan](knowledge/G.%20Detailed%20Implementation%20Plan.md) - Development roadmap
- [Financial Model](knowledge/H.%20Financial%20Model.md) - Business projections

## API Documentation

When running the API server, OpenAPI documentation is available at:
- Swagger UI: `http://localhost:3000/swagger-ui/`
- OpenAPI JSON: `http://localhost:3000/api-docs/openapi.json`

## Contributing

This project is currently in private development. Contribution guidelines will be published after public release.

## License

Apache-2.0

## Contact

- Website: https://kaccy.net
- Email: contact@cooljapan.tech
- Repository: https://github.com/cool-japan/kaccy

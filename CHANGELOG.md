# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-01-18

### Added

#### Core Platform
- Personal Token (FOT - Future Output Token) issuance system
- Multiple bonding curves: Linear, Bancor, Sigmoid, Exponential, Adaptive, Square Root, Logarithmic
- Order book trading engine with price-time priority matching
- Advanced order types: Stop-loss, take-profit, TWAP, limit orders
- AMM features: Liquidity pools, flash swaps, concentrated liquidity
- DeFi components: Lending, perpetuals, hooks system
- Circuit breakers for automatic price manipulation protection

#### Bitcoin Integration (kaccy-bitcoin)
- Full Bitcoin Core integration via RPC
- HD wallet support with BIP32/BIP39/BIP44 derivation
- UTXO management and coin selection algorithms
- PSBT (Partially Signed Bitcoin Transactions) support
- Lightning Network integration via LND
- Privacy features: CoinJoin, Silent Payments, Payjoin, Dandelion++
- Advanced cryptography: MuSig2, FROST threshold signatures, DLCs
- Hardware wallet support via HWI
- Sidechain integration: RSK and Stacks

#### Reputation System (kaccy-reputation)
- Dynamic multi-component scoring (0-1000 scale)
- Score components: Commitment, quality, trust, longevity
- 6-tier system: Unverified, Verified, Bronze, Silver, Gold, Diamond
- Milestone-based and recurring commitment tracking
- Soulbound Token (SBT) credentials
- Cross-platform reputation import: GitHub, LinkedIn, Stack Overflow

#### AI Services (kaccy-ai)
- Multi-model AI oracle with consensus system
- Provider support: OpenAI, Anthropic, Gemini, DeepSeek
- Automated code and content quality evaluation
- Fraud detection: Sybil attacks, wash trading, reputation gaming
- Social verification: Twitter, LinkedIn, YouTube, GitHub
- Document analysis: PDF parsing, OCR, plagiarism detection

#### API Layer (kaccy-api)
- RESTful API with OpenAPI/Swagger documentation
- WebSocket support for real-time updates
- GraphQL API via async-graphql
- JWT authentication with refresh tokens
- OAuth2 integration (Google, GitHub, Twitter)
- Two-factor authentication (TOTP)
- Rate limiting and request throttling
- Prometheus metrics and OpenTelemetry tracing

#### Database Layer (kaccy-db)
- PostgreSQL with SQLx for type-safe queries
- Redis for caching and session management
- Distributed caching with consistency protocols
- Database migrations via SQLx
- Repository pattern with async support
- Data encryption at rest with AES-GCM
- Secure password hashing with Argon2

### Technical Details
- **Language**: Rust 1.85+
- **Total Lines of Code**: ~206,000
- **Test Coverage**: 3,296 tests
- **Crates**: 7 workspace members

### Security
- No unsafe code in application layer
- Memory-safe Bitcoin transaction handling
- Encrypted key storage with hardware wallet support
- Rate limiting and circuit breakers for DoS protection

[0.1.0]: https://github.com/cool-japan/kaccy/releases/tag/v0.1.0

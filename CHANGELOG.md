# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-14

### Added

- Complete rustdoc coverage: all public APIs across all 7 crates now have `///` documentation (0 missing_docs warnings workspace-wide)
- Query result caching verified complete: multi-tier L1/L2 cache (DashMap + Redis), cache warming, and cascade invalidation in kaccy-db
- Memory profiling module (`kaccy-core::utils::memory_profiler`) with `TrackingAllocator<A>` (`GlobalAlloc` wrapper with atomic counters), `MemoryProfiler` (baseline/delta snapshots), `MemoryGuard` RAII scope (logs delta via `tracing::debug!` on drop), and `MemoryStats`/`MemoryDelta` value types

#### Infrastructure
- GitHub Actions CI/CD workflows (`ci.yml`, `security-audit.yml`, `docs.yml`)
- Dependabot configuration for automated dependency updates
- `Dockerfile` with multi-stage build (cargo-chef + distroless runtime)
- `docker-compose.yml` with PostgreSQL 15, Redis 7, and optional Bitcoin Core testnet profile
- `rust-toolchain.toml` pinning stable Rust channel with clippy/rustfmt/rust-src components
- `scripts/test.sh`, `scripts/lint.sh`, `scripts/release.sh` helper scripts

#### kaccy-bitcoin
- `rpc_advanced` module: Advanced Bitcoin Core RPC operations (descriptor import/export, block templates, transaction priority management)
- `bech32m_utils` module: BIP 350 bech32m encoding/decoding utilities for Taproot addresses
- `schnorr` module: BIP 340 Schnorr signature wrapper (sign, verify, batch verify) over secp256k1
- `mock_explorer` module: `MockBlockchainExplorer` for testing, `RotatingExplorerClient` with round-robin failover, `TorExplorerClient` with SOCKS5 proxy support
- Integration test suite (`tests/`) with end-to-end payment flows, multi-party coordination, and failure scenarios
- Property-based tests using proptest for bitcoin utilities
- `tapscript_utils` module now fully documented in TODO (was implemented but unchecked)

#### kaccy-core
- Property-based tests (proptest) for all 7 bonding curve implementations: 32 invariant tests covering spread, monotonicity, positivity, and price impact (`kaccy-core/tests/bonding_curve_proptest.rs`)
- Criterion benchmark suite for bonding curves: spot price, buy/sell price, and supply sweep benchmarks for all curve types (`kaccy-core/benches/bonding_curves.rs`)
- Closed-form antiderivatives for SquareRoot and Logarithmic bonding curves replacing 100-step numerical integration (O(1) exact computation)
- Comprehensive unit tests for all previously-uncovered trading and model modules: order book, market maker, trade executor, price circuit breaker, staking, token, liquidity, user (375 new tests total across workspace)

#### kaccy-api
- Real HTTP batch endpoint execution: `/api/batch` now routes sub-requests to the server via reqwest instead of returning mock responses

#### kaccy-ai
- Custom LLM endpoint support: `CustomEndpointClient`, `CustomEndpointRegistry`, `IssuerPersonalization`, `EndpointRequestFormat` with OpenAI/Anthropic/custom JSON formats (`kaccy-ai/src/custom_endpoint.rs`)

### Fixed
- Stale comment in `kaccy-db/src/lib.rs` incorrectly stated security module was disabled
- Potential zero-division in `MarketMakerService::generate_quote` when bid/ask price is zero
- `kaccy-bitcoin/TODO.md` now correctly reflects tapscript_utils as implemented
- kaccy-ai `CustomAgentConfig` now supports optional custom endpoint URL and personalization configuration
- kaccy-db sanitization module re-enabled: root cause was an unescaped `"` in a raw regex literal (`r"['\";...]"`) misidentified as a compiler E0762 bug; fixed with hash-delimited raw string (`r#"..."#`)

### Changed
- Workspace version bumped from `0.1.0` to `0.2.0`

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

[0.2.0]: https://github.com/cool-japan/kaccy/releases/tag/v0.2.0
[0.1.0]: https://github.com/cool-japan/kaccy/releases/tag/v0.1.0

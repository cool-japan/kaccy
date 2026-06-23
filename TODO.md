# Kaccy Protocol - Development TODO

---

## 0.2.1 De-Fabrication Audit & Remediation (2026-06-23)

This section records the systematic identification and replacement of "silent fabrications"
across the workspace — code that compiled, passed shallow tests, and claimed to be
production-ready, but returned placeholder or hardcoded values instead of doing real work.

**Final state after this session:**
`cargo clippy --workspace --all-features --all-targets -- -D warnings` = 0 warnings/errors
`cargo nextest run --workspace --all-features` = 4215 passed / 0 failed / 22 skipped

### Per-crate remediation

#### kaccy-core (~47,000 SLoC)

Real implementations replacing fabrications:

- `utils/encryption.rs` — AES-256-GCM + ChaCha20-Poly1305 (was base64 encode/decode)
- `trading/private_pool.rs` — real AEAD (was XOR)
- `utils/threshold_encryption.rs` — Shamir SSS over GF(256) + secp256k1 Pedersen
  commitment (was hash-based fake)
- `trading/mev_mitigation.rs` — real AEAD commit-reveal (was passthrough no-op)
- `utils/post_quantum_crypto.rs` — real ML-DSA-65 + ML-KEM-768 + secp256k1 hybrid (was
  zero-filled key buffers)
- `ml/garch.rs` — real GARCH/GJR variance recursion (removed artificial 0.95/0.05 damping
  that masked the formula)
- `utils/advanced_risk.rs` — real Monte Carlo Expected Shortfall + inverse-normal CDF (was
  historical-delegate + hardcoded z-table)
- `trading/tca.rs` — real Almgren square-root market-impact model (was silent `return 0.0`)
- `ml/deep_learning.rs` — real LSTM with BPTT / scaled-dot-product attention /
  self-attention transformer / MLP GAN (was dummy loss + naive last-value)
- `ml/reinforcement_learning.rs` + `trading/rl_market_maker.rs` — real Q-learning TD
  updates with experience replay (was hardcoded heuristic + no-op update function)
- `ml/advanced_forecasting.rs` — real 3-model ensemble + probabilistic prediction intervals
  (was relabeled exponential smoothing)

Remaining follow-ups: Feldman-VSS verification simplified; PQC SPHINCS+/code-based labels
map to ML-DSA (acknowledged approximation); GAN uses SPSA not full backprop; parametric-ES
sign convention needs review.

#### kaccy-bitcoin (~22,000 SLoC)

Real implementations replacing fabrications:

- `slip39.rs` — real SLIP-0039 (GF(256) log/antilog, RS1024 checksum, PBKDF2-Feistel);
  passes official test vectors
- `frost.rs` — real FROST threshold Schnorr (Shamir + Feldman VSS, BIP-340 verify)
- `musig2.rs` — real BIP-327 message-bound nonce + adaptor signatures
- `silent_payments.rs` — real BIP-352 bech32m address + input_hash ECDH
- `taproot.rs` — real key-path and script-path spends with TapLeaf/TapBranch merkle tree
- `bip324.rs` — real ChaCha20-Poly1305 packet AEAD
- `psbt_v2.rs` — real HASH160 (was truncated SHA-256 — a fund-loss bug)
- `miniscript_support.rs` — real compilation via miniscript 12.3
- `compact_filters.rs` — real BIP-158 GCS matching
- `coinjoin.rs` — real P2WPKH signing and PSBT finalization

Remaining follow-ups: FROST trusted-dealer only (distributed DKG deferred); BIP-324
ElligatorSwift handshake simplified; silent-payments bulk scan returns honest error;
compact-filters P2P download returns honest error (needs live network).

#### kaccy-api (~12,000 SLoC)

Real implementations replacing fabrications:

- `notifications/email.rs` — real lettre SMTP; honest `NotConfigured` error when unconfigured
  (was always `Ok("Sent")`)
- `notifications/sms.rs` — real Twilio + AWS SigV4 SNS; honest error when unconfigured
- `notifications/push.rs` — real FCM HTTP v1/legacy + APNS HTTP/2 ES256-JWT; honest error
  when unconfigured
- `jobs/worker.rs` — wired to real notifiers (removed `simulate_email_send`)
- `routes/oauth2.rs` — real Redis-backed one-time-use CSRF + PKCE-verifier state

Remaining follow-ups: per-user device-token registry; FCM OAuth2 token refresh; APNS JWT
caching; workspace dep hoist of kaccy-api per-crate version pins.

#### kaccy-reputation (~12,000 SLoC)

Real implementations replacing fabrications:

- `portability.rs` — real secp256k1 ECDSA credential signing + honest external-data error
  (was returning invented platform scores)
- `maintenance.rs` — real transactional audit-log archival (was count-only)
- `score.rs` + `confidence.rs` — response-time component from real commitment-fulfilment
  latency; honest neutral/low confidence when data is absent (was hardcoded 70 / "high")
- `ai_scoring.rs` — injectable `QualityEvaluator` seam; description-length heuristic now
  explicitly flagged and confidence-capped (never masquerades as AI)

Remaining follow-ups: `score.rs` ~2012 lines (>2000-line limit) — split with `splitrs`.

#### kaccy-db (~15,000 SLoC)

Real implementations replacing fabrications:

- `pool_autoscaling.rs` — real pool utilization metrics from `pool.size()` / `num_idle()`
  / `max_connections()` (was hardcoded 10/7/3/0.7)

Remaining follow-up: runtime pool resizing still blocked by sqlx `PgPool` API limitations.

#### kaccy-ai (~15,000 SLoC)

Real implementations replacing fabrications:

- `social.rs` — real Twitter API v2 + YouTube Data API v3 when configured; honest
  `NeedsManualReview` when credentials absent (was always `Verified`)
- `transcript.rs` — real YouTube timedtext XML fetch + parse; honest
  `TranscriptUnavailable` (was treating partial response as success)

Remaining follow-ups: LinkedIn verification deferred (requires user OAuth2 flow); YouTube
transcript language selection not yet configurable.

### Honest-error follow-ups (deferred, not done this session)

All the items below are now explicitly honest errors (they return typed `Err` variants
rather than silently returning wrong data). They are tracked here for future implementation:

- FROST distributed DKG (multi-round interactive protocol)
- BIP-324 ElligatorSwift full handshake
- Silent-payments bulk transaction-set scan
- Compact-filters P2P download from live network
- APNS JWT key caching / FCM token refresh
- Per-user push notification device-token registry
- kaccy-reputation real AI quality evaluation via kaccy-ai LLM clients
- LinkedIn social verification (requires user OAuth2)

### Explicitly out of scope for this session

The following were reviewed and deliberately not touched:

- **Infrastructure / deployment**: VPS/PostgreSQL/Bitcoin-Core/DNS/SSL setup, GitHub
  Actions changes, Docker, the operator runbook checklists in
  `crates/kaccy-bitcoin/DEPLOYMENT.md` and `crates/kaccy-db/docs/PERFORMANCE_TUNING.md`.
- **Frontend**: Next.js frontend (Phase 8 below).
- **Legal / compliance**: ToS, Privacy Policy, VASP application (Phase 10 below).
- **`rand` -> SciRS2-Core migration**: deliberate documented exception — see
  `SCIRS2_POLICY.md` at the workspace root.
- **Bitcoin cross-input signature aggregation / half-aggregation**: research-grade; not
  implemented.
- **kaccy-ai custom model training**: requires external ML infrastructure (already marked
  `[ ]` in kaccy-ai TODO).
- **Refactors pending splitrs**: `crates/kaccy-bitcoin/src/slip39.rs` (~2048 lines) and
  `crates/kaccy-reputation/src/score.rs` (~2012 lines) both exceed the 2000-line guideline
  and should be split in a follow-up session.

---

## Implementation Status

### Completed (v0.1.0)

> Note: "Completed" below means the feature module exists and was functional at v0.1.0.
> It does NOT mean every implementation was free of silent fabrications. The de-fabrication
> audit above (2026-06-23) documents which specific functions had placeholder values and
> what replaced them.

#### kaccy-core (Core Business Logic) - ~47,000 SLoC
- [x] **Bonding Curves**: Linear, Bancor, Sigmoid, Exponential, Adaptive, Square Root, Logarithmic
- [x] **Trading Engine**: Order book with price-time priority, market maker integration, circuit breakers
- [x] **Advanced Order Types**: Stop-loss, take-profit, TWAP, limit orders
- [x] **AMM Features**: Liquidity pools, flash swaps, concentrated liquidity
- [x] **DeFi Components**: Lending, perpetuals, hooks system
- [x] **Risk Management**: Position sizing, counterparty risk, systemic risk monitoring
- [x] **ML Integration**: Price prediction, anomaly detection, strategy optimization
- [x] **Fee System**: Platform fees, reputation discounts, volume tiers, maker/taker differentiation
- [x] **Utilities**: Caching, pagination, validation, compliance, audit system

#### kaccy-api (REST/WebSocket API) - ~12,000 SLoC
- [x] **Authentication**: JWT, OAuth2, 2FA/TOTP, API keys
- [x] **REST Endpoints**: Users, tokens, orders, trades, commitments
- [x] **Real-time**: WebSocket for prices, trades, orders, chat
- [x] **Notifications**: Email (SMTP), SMS (Twilio/AWS SNS), Push (FCM/APNS), In-app
- [x] **Background Jobs**: Async processing with retry logic
- [x] **Observability**: Prometheus metrics, OpenTelemetry tracing, structured logging
- [x] **Security**: Rate limiting, request signing, IP filtering, CORS
- [x] **Documentation**: OpenAPI/Swagger, SDK generation (TypeScript, Python, Rust, Go)

#### kaccy-bitcoin (Bitcoin Integration) - ~22,000 SLoC
- [x] **Core RPC**: Bitcoin Core client with connection pooling
- [x] **HD Wallet**: BIP84, BIP47 (PaymentCodes), SLIP39 (Shamir backup)
- [x] **Transaction Management**: PSBT workflows, RBF, CPFP, fee bumping
- [x] **Privacy**: CoinJoin, Silent Payments, Payjoin, Dandelion++, Tor proxy
- [x] **Advanced Features**: MuSig2, FROST, DLCs (Discreet Log Contracts)
- [x] **Lightning Network**: LND client, channels, invoices, submarine swaps
- [x] **Taproot**: Full Taproot/Tapscript support, key path and script path spending
- [x] **Mempool**: Analytics, monitoring, fee estimation
- [x] **Sidechains**: RSK and Stacks integration
- [x] **Security**: Multisig, hardware wallet (HWI), timelock contracts

#### kaccy-reputation (Reputation System) - ~12,000 SLoC
- [x] **Dynamic Scoring**: Multi-component 0-1000 scale with weighted averages
- [x] **Tier System**: 6 tiers (Unverified to Diamond) with progressive benefits
- [x] **Commitment Tracking**: Create, verify, milestone-based, recurring
- [x] **Circuit Breaker**: Automatic protection against low-reputation actors
- [x] **Caching**: High-performance in-memory caching
- [x] **AI Integration**: AI-assisted quality evaluation
- [x] **Disputes**: Resolution and appeals workflow
- [x] **SBT**: Soulbound Token integration
- [x] **Analytics**: System-wide statistics, leaderboards, achievements
- [x] **Portability**: Cross-platform reputation import/export

#### kaccy-ai (AI Services) - ~15,000 SLoC
- [x] **Quality Evaluation**: Code quality, content verification
- [x] **Fraud Detection**: Sybil, wash trading, reputation gaming, image manipulation
- [x] **Document Analysis**: PDF parsing, metadata extraction, quality analysis
- [x] **LLM Integration**: OpenAI, Anthropic, Gemini, DeepSeek, Ollama clients
- [x] **GitHub Verification**: Commits, PRs, issues, releases
- [x] **Social Media**: Twitter, LinkedIn, YouTube verification
- [x] **OCR**: Screenshot analysis, image text extraction
- [x] **Plagiarism Detection**: Similarity analysis with batch processing
- [x] **Oracle System**: Multi-model consensus with adaptive weighting
- [x] **Performance**: Caching, rate limiting, circuit breaker, budget management

#### kaccy-db (Database Layer) - ~15,000 SLoC
- [x] **Connection Pooling**: SQLx with autoscaling and circuit breaker
- [x] **Caching**: Multi-tier (memory + Redis), invalidation, warming
- [x] **Analytics**: Real-time platform stats, OHLC data, volume history
- [x] **Backup/Recovery**: Automated backups, PITR support
- [x] **Sharding**: Consistent hash ring, shard coordination
- [x] **Multi-Region**: Replication, failover, lag monitoring
- [x] **Security**: Encryption, audit logging, compliance
- [x] **Performance**: Query analysis, index optimization, vacuum scheduling
- [x] **Streaming**: Cursor pagination, query streaming
- [x] **Monitoring**: Health checks, lock monitoring, leak detection

---

## In Progress

### Phase 7: Infrastructure & Deployment
- [ ] VPS setup (Hetzner CPX41)
- [ ] PostgreSQL 15 production configuration
- [ ] Bitcoin Core setup (testnet then mainnet)
- [ ] Domain DNS configuration (kaccy.net)
- [ ] SSL certificate (Let's Encrypt)
- [x] GitHub Actions CI/CD
- [x] Docker configuration
- [x] Deployment scripts

### Phase 8: Frontend Development
- [ ] Next.js 14 project setup
- [ ] Tailwind CSS configuration
- [ ] Layout components (Header, Footer)
- [ ] Authentication pages (Login, Register)
- [ ] Dashboard (protected route)
- [ ] Token creation wizard with bonding curve preview
- [ ] Token browsing and detail pages
- [ ] Buy modal with QR code
- [ ] Portfolio page
- [ ] Transaction history
- [ ] Commitment management
- [ ] Admin dashboard

---

## Future Phases (Post-MVP)

### Phase 9: Production Readiness
- [ ] Security audit (penetration testing)
- [ ] Load testing (100+ req/sec target)
- [ ] Database indexes optimization
- [ ] Error handling audit
- [ ] Accessibility (WCAG AA)

### Phase 10: Legal & Compliance
- [ ] Terms of Service
- [ ] Privacy Policy
- [ ] Cookie consent
- [ ] Estonian VASP application

### Phase 11: $KACCY Platform Token
- [ ] Platform token contract design
- [ ] Staking mechanism
- [ ] Fee discounts
- [ ] Governance voting

### Phase 12: Advanced Features
- [ ] Secondary market (full order book)
- [ ] Mobile app (React Native)
- [ ] Third-party API integrations
- [ ] DAO transition

---

## Technical Debt & Improvements

### Code Quality
- [x] Increase test coverage to 80%+
- [x] Add property-based tests for bonding curves
- [x] Benchmark all critical paths
- [x] Document all public APIs

### Performance
- [x] Profile memory usage
- [x] Optimize hot paths
- [x] Add query result caching
- [x] Implement batch operations for high-volume scenarios

---

## Success Metrics (Month 3 Post-Launch)

### Minimum Success
- [ ] 50 registered users
- [ ] 20 KYC-verified users
- [ ] 10 tokens issued
- [ ] 100 total trades
- [ ] $5,000 trading volume
- [ ] 0 security incidents
- [ ] 95% uptime

### Target Success
- [ ] 100 registered users
- [ ] 50 KYC-verified users
- [ ] 20 tokens issued
- [ ] 300 trades
- [ ] $20,000 trading volume
- [ ] 80% commitment fulfillment rate

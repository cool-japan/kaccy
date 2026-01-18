# kaccy-core TODO

## Phase 0 (MVP)

### Models

- [x] User model with basic fields
- [x] Token model with linear bonding curve params
- [x] Order model (buy/sell)
- [x] Trade model
- [x] Balance model
- [x] Add validation methods to models
- [x] Add `Display` trait implementations
- [x] Add `FromRow` derive for SQLx

### Pricing

- [x] `BondingCurve` trait with 6 default methods
- [x] `LinearBondingCurve` implementation
- [x] Buy/sell price calculation with spread
- [x] Unit tests for linear curve
- [x] Add price impact calculation
- [x] Add slippage protection

### Trading

- [x] Implement `TradeExecutor`
  - [x] Execute bonding curve orders
  - [x] Update token supply atomically
  - [x] Update user balances atomically
  - [x] Create trade records
- [x] Add transaction rollback on failure
- [x] Add fee calculation (FeeSchedule)

### Error Handling

- [x] Basic error types
- [x] Add more specific error variants (SlippageExceeded, OrderExpired, etc.)
- [x] Implement `From` conversions for external errors

## Phase 1 (Post-MVP)

### Models

- [x] Add `TokenMetadata` with access benefits
- [x] Add `OutputCommitment` linked to tokens (in kaccy-reputation)
- [x] Add `UtilityDefinition` for token utility
- [x] Add `UtilityRedemption` for tracking utility usage
- [x] Add `RedemptionStatus` enum

### Pricing

- [x] `BancorCurve` implementation
  - [x] Reserve ratio configuration
  - [x] Power approximation for buy/sell
- [x] `SigmoidCurve` implementation
  - [x] S-curve for price stabilization
  - [x] Numerical integration for buy/sell
- [x] `ExponentialCurve` implementation
  - [x] Exponential growth rate
- [x] `AdaptiveCurve` implementation
  - [x] Phase-based curve switching (CurvePhase enum)
  - [x] Early adopter → Growth → Maturity phases
  - [x] Configurable thresholds (with_defaults() at 10%/50%)

### Trading

- [x] Order book for limit orders (OrderBook, LimitOrder, OrderBookManager)
  - [x] Price-time priority matching
  - [x] Partial fills
  - [x] OrderBookDepth for market data
- [x] Market maker integration (market_maker.rs)
  - [x] TokenMarketMaker with configurable spread and depth
  - [x] MarketMakerManager for multiple tokens
  - [x] Inventory management and skew-based pricing
  - [x] Dynamic spread based on volatility
  - [x] Rebalance recommendations (RebalanceAction)
- [x] Circuit breaker for price manipulation (PriceCircuitBreaker)
  - [x] Max price change per hour (configurable)
  - [x] Max volume per user (hourly/daily)
  - [x] Cooldown period (configurable)
  - [x] Rapid trading detection
  - [x] Volume spike detection
  - [x] Wash trading detection hooks

### Fee System

- [x] Platform fee calculation (2.5%)
- [x] Issuer royalty (0.5%)
- [x] Fee discount based on reputation
- [x] Fee distribution to $KACCY stakers (FeeDistributor, FeePool, FeeDistribution)
  - [x] Weight-based distribution (tier multiplier, loyalty bonus)
  - [x] Staking tiers (Basic, Silver, Gold, Platinum, Diamond)
- [x] Fee discount based on $KACCY holdings (StakingTier, FeeDiscountCalculator)
  - [x] Combined discounts (reputation + staking)
  - [x] Lock bonuses for longer stakes
  - [x] Max 60% total discount

## Phase 2 (Future)

### Advanced Features

- [x] Liquidity pool (AMM-style)
  - [x] x*y=k constant product
  - [x] LP token generation
  - [x] Impermanent loss calculation
  - [x] Add/remove liquidity operations
  - [x] Swap operations with slippage protection
  - [x] Pool statistics and analytics
- [x] Token vesting schedules
  - [x] Linear vesting
  - [x] Graded vesting (quarterly releases)
  - [x] Cliff-only vesting
  - [x] Milestone-based vesting
  - [x] Vesting templates (4-year, quarterly, etc.)
  - [x] Releasable amount calculation
- [x] Token burn mechanism
  - [x] Voluntary burns
  - [x] Protocol-enforced burns
  - [x] Deflationary burn mechanism
  - [x] Buyback and burn
  - [x] Burn statistics and analytics
  - [x] Price impact calculation
- [x] Cross-token arbitrage prevention
  - [x] Arbitrage opportunity detection
  - [x] Rate limiting per user
  - [x] User flagging for excessive arbitrage
  - [x] Price discrepancy monitoring
  - [x] Optimal route calculation

## Phase 3 (Advanced)

### Advanced Trading

- [x] Advanced order types
  - [x] Stop-loss orders
  - [x] Take-profit orders
  - [x] Trailing stop orders
  - [x] TWAP (Time-Weighted Average Price) orders
  - [x] Limit order expiration and GTT (Good-Till-Time)
- [x] Position management
  - [x] Position tracking per user
  - [x] Position limits and risk controls
  - [x] PnL (Profit and Loss) calculation
  - [x] Position history and analytics
  - [x] Portfolio summary and statistics
  - [x] Daily loss limits and risk management

### Analytics & Metrics

- [x] Market data aggregation
  - [x] OHLCV (candlestick) data generation
  - [x] Volume-weighted average price (VWAP)
  - [x] Time-weighted average price (TWAP) calculation
  - [x] Market depth snapshots
- [x] Trading statistics
  - [x] 24h trading volume metrics
  - [x] Price changes and volatility tracking
  - [x] All-time high/low tracking
  - [x] Trade count analytics

### Advanced Liquidity Features

- [x] Dynamic AMM features
  - [x] Dynamic fee adjustment based on volatility
  - [x] Oracle price integration for price bounds
  - [x] Flash swap support
  - [x] Multi-hop routing optimization (enhanced with output comparison)
- [x] Liquidity incentives
  - [x] Liquidity mining rewards
  - [x] Reward distribution mechanisms
  - [x] Boost multipliers for long-term LPs (1.0x to 2.0x based on lock duration)

### Governance & Security

- [x] Governance system
  - [x] Proposal creation and submission
  - [x] Voting mechanism (token-weighted)
  - [x] Time-locked execution
  - [x] Quorum requirements
  - [x] Participation rate tracking
  - [x] Multiple proposal types
- [x] Security features
  - [x] Emergency pause mechanism
  - [x] Multi-signature operations
  - [x] Rate limiting for withdrawals
  - [x] Anomaly detection


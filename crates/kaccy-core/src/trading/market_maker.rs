//! Automated Market Maker (AMM) integration
//!
//! Provides automated liquidity and price stability for tokens by:
//! - Maintaining bid/ask spreads around the bonding curve price
//! - Automatically rebalancing inventory
//! - Managing risk through position limits

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::order_book::{LimitOrder, OrderSide};

/// Market maker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerConfig {
    /// Target spread as percentage (e.g., 0.02 = 2%)
    pub target_spread_pct: Decimal,
    /// Number of price levels to maintain on each side
    pub depth_levels: usize,
    /// Size per level as percentage of total inventory
    pub level_size_pct: Decimal,
    /// Distance between price levels as percentage
    pub level_spacing_pct: Decimal,
    /// Inventory skew threshold to trigger rebalance
    pub rebalance_threshold: Decimal,
    /// Maximum position size (total inventory)
    pub max_position: Decimal,
    /// Minimum order size
    pub min_order_size: Decimal,
    /// Maximum slippage tolerance for rebalancing
    pub max_slippage_pct: Decimal,
    /// Enable dynamic spread based on volatility
    pub dynamic_spread: bool,
    /// Enable inventory-based skew
    pub inventory_skew: bool,
}

impl Default for MarketMakerConfig {
    fn default() -> Self {
        Self {
            target_spread_pct: dec!(0.02),   // 2% spread
            depth_levels: 5,                 // 5 levels each side
            level_size_pct: dec!(0.10),      // 10% of inventory per level
            level_spacing_pct: dec!(0.005),  // 0.5% between levels
            rebalance_threshold: dec!(0.30), // Rebalance when 30% skewed
            max_position: dec!(1000000),     // Max 1M tokens
            min_order_size: dec!(1),         // Min 1 token
            max_slippage_pct: dec!(0.05),    // 5% max slippage
            dynamic_spread: true,
            inventory_skew: true,
        }
    }
}

impl MarketMakerConfig {
    /// Create a tight spread config for high-volume tokens
    pub fn tight() -> Self {
        Self {
            target_spread_pct: dec!(0.005), // 0.5% spread
            depth_levels: 10,
            level_size_pct: dec!(0.05),
            level_spacing_pct: dec!(0.002),
            ..Default::default()
        }
    }

    /// Create a wide spread config for low-volume tokens
    pub fn wide() -> Self {
        Self {
            target_spread_pct: dec!(0.05), // 5% spread
            depth_levels: 3,
            level_size_pct: dec!(0.15),
            level_spacing_pct: dec!(0.01),
            ..Default::default()
        }
    }
}

/// Current inventory state
#[derive(Debug, Clone, Serialize, Default)]
pub struct InventoryState {
    /// Token balance
    pub token_balance: Decimal,
    /// Quote (BTC) balance
    pub quote_balance: Decimal,
    /// Target token balance (for rebalancing)
    pub target_token_balance: Decimal,
    /// Current inventory skew (-1 to 1, negative = too many tokens)
    pub skew: Decimal,
    /// Total value in quote currency
    pub total_value: Decimal,
}

impl InventoryState {
    /// Compute inventory state from current balances and price
    pub fn new(token_balance: Decimal, quote_balance: Decimal, token_price: Decimal) -> Self {
        let token_value = token_balance * token_price;
        let total_value = token_value + quote_balance;

        // Target is 50% tokens, 50% quote
        let target_token_balance = if token_price > Decimal::ZERO {
            (total_value / dec!(2)) / token_price
        } else {
            Decimal::ZERO
        };

        let skew = if target_token_balance > Decimal::ZERO {
            (token_balance - target_token_balance) / target_token_balance
        } else {
            Decimal::ZERO
        };

        Self {
            token_balance,
            quote_balance,
            target_token_balance,
            skew,
            total_value,
        }
    }

    /// Check if rebalancing is needed
    pub fn needs_rebalance(&self, threshold: Decimal) -> bool {
        self.skew.abs() > threshold
    }

    /// Calculate how many tokens to trade to rebalance
    pub fn rebalance_amount(&self) -> Decimal {
        self.token_balance - self.target_token_balance
    }
}

/// Quote for market maker orders
#[derive(Debug, Clone, Serialize)]
pub struct MarketMakerQuote {
    /// Token this quote is for
    pub token_id: Uuid,
    /// Bid levels (highest price first)
    pub bids: Vec<QuoteLevel>,
    /// Ask levels (lowest price first)
    pub asks: Vec<QuoteLevel>,
    /// Current mid price used to compute the quote
    pub mid_price: Decimal,
    /// Absolute bid-ask spread
    pub spread: Decimal,
    /// Spread as a fraction of mid price
    pub spread_pct: Decimal,
}

/// A single price level in the quote
#[derive(Debug, Clone, Serialize)]
pub struct QuoteLevel {
    /// Price at this level
    pub price: Decimal,
    /// Quantity available at this level
    pub amount: Decimal,
    /// Total value (price × amount) at this level
    pub total_value: Decimal,
}

/// Market maker for a single token
pub struct TokenMarketMaker {
    /// Token this market maker is quoting
    pub token_id: Uuid,
    /// Market-making configuration
    pub config: MarketMakerConfig,
    /// Current inventory state
    inventory: InventoryState,
    /// IDs of active bid orders placed by this market maker
    active_bid_orders: Vec<Uuid>,
    /// IDs of active ask orders placed by this market maker
    active_ask_orders: Vec<Uuid>,
    /// Last mid price used to generate orders
    last_mid_price: Option<Decimal>,
    /// Exponential moving average of realised price volatility
    volatility_estimate: Decimal,
}

impl TokenMarketMaker {
    /// Create a new token market maker with the given configuration
    pub fn new(token_id: Uuid, config: MarketMakerConfig) -> Self {
        Self {
            token_id,
            config,
            inventory: InventoryState::default(),
            active_bid_orders: Vec::new(),
            active_ask_orders: Vec::new(),
            last_mid_price: None,
            volatility_estimate: dec!(0.02), // 2% default volatility
        }
    }

    /// Update inventory state
    pub fn update_inventory(
        &mut self,
        token_balance: Decimal,
        quote_balance: Decimal,
        token_price: Decimal,
    ) {
        self.inventory = InventoryState::new(token_balance, quote_balance, token_price);
    }

    /// Calculate the effective spread based on config and market conditions
    pub fn calculate_spread(&self) -> Decimal {
        let mut spread = self.config.target_spread_pct;

        // Adjust for volatility if dynamic spread is enabled
        if self.config.dynamic_spread {
            // Increase spread when volatility is high
            spread = spread.max(self.volatility_estimate * dec!(1.5));
        }

        // Adjust for inventory skew if enabled
        if self.config.inventory_skew {
            // Widen spread when inventory is skewed
            let skew_adjustment = self.inventory.skew.abs() * dec!(0.5);
            spread += spread * skew_adjustment;
        }

        spread
    }

    /// Calculate bid/ask prices with inventory skew
    pub fn calculate_prices(&self, mid_price: Decimal) -> (Decimal, Decimal) {
        let spread = self.calculate_spread();
        let half_spread = spread / dec!(2);

        let mut bid_price = mid_price * (Decimal::ONE - half_spread);
        let mut ask_price = mid_price * (Decimal::ONE + half_spread);

        // Apply inventory skew - if we have too many tokens, lower ask price
        if self.config.inventory_skew && self.inventory.skew.abs() > dec!(0.1) {
            let skew_adjustment = self.inventory.skew * self.config.target_spread_pct;

            if self.inventory.skew > Decimal::ZERO {
                // Too many tokens - lower ask to sell more
                ask_price -= mid_price * skew_adjustment;
            } else {
                // Too few tokens - raise bid to buy more
                bid_price += mid_price * skew_adjustment.abs();
            }
        }

        (bid_price, ask_price)
    }

    /// Generate a complete quote with multiple levels
    pub fn generate_quote(&self, mid_price: Decimal) -> MarketMakerQuote {
        let (base_bid, base_ask) = self.calculate_prices(mid_price);
        let spread = base_ask - base_bid;
        let spread_pct = if mid_price > Decimal::ZERO {
            spread / mid_price
        } else {
            Decimal::ZERO
        };

        let level_size = self.inventory.token_balance * self.config.level_size_pct;

        let mut bids = Vec::new();
        let mut asks = Vec::new();

        for i in 0..self.config.depth_levels {
            let level_offset = Decimal::from(i as u32) * self.config.level_spacing_pct;

            // Bid levels (decreasing price)
            let bid_price = base_bid * (Decimal::ONE - level_offset);
            if bid_price > Decimal::ZERO {
                let bid_amount = level_size.min(self.inventory.quote_balance / bid_price);
                if bid_amount >= self.config.min_order_size {
                    bids.push(QuoteLevel {
                        price: bid_price,
                        amount: bid_amount,
                        total_value: bid_price * bid_amount,
                    });
                }
            }

            // Ask levels (increasing price)
            let ask_price = base_ask * (Decimal::ONE + level_offset);
            if ask_price > Decimal::ZERO {
                let ask_amount = level_size.min(self.inventory.token_balance);
                if ask_amount >= self.config.min_order_size {
                    asks.push(QuoteLevel {
                        price: ask_price,
                        amount: ask_amount,
                        total_value: ask_price * ask_amount,
                    });
                }
            }
        }

        MarketMakerQuote {
            token_id: self.token_id,
            bids,
            asks,
            mid_price,
            spread,
            spread_pct,
        }
    }

    /// Generate orders to place on the order book
    pub fn generate_orders(&mut self, mid_price: Decimal, mm_user_id: Uuid) -> Vec<LimitOrder> {
        let quote = self.generate_quote(mid_price);
        let mut orders = Vec::new();

        // Generate bid orders
        for level in quote.bids {
            orders.push(LimitOrder::new(
                mm_user_id,
                self.token_id,
                OrderSide::Buy,
                level.price,
                level.amount,
            ));
        }

        // Generate ask orders
        for level in quote.asks {
            orders.push(LimitOrder::new(
                mm_user_id,
                self.token_id,
                OrderSide::Sell,
                level.price,
                level.amount,
            ));
        }

        // Track active orders
        self.active_bid_orders = orders
            .iter()
            .filter(|o| o.side == OrderSide::Buy)
            .map(|o| o.order_id)
            .collect();
        self.active_ask_orders = orders
            .iter()
            .filter(|o| o.side == OrderSide::Sell)
            .map(|o| o.order_id)
            .collect();

        self.last_mid_price = Some(mid_price);

        orders
    }

    /// Update volatility estimate based on price changes
    pub fn update_volatility(&mut self, new_price: Decimal) {
        if let Some(last_price) = self.last_mid_price {
            if last_price > Decimal::ZERO {
                let return_pct = (new_price - last_price).abs() / last_price;
                // Exponential moving average of volatility
                self.volatility_estimate =
                    self.volatility_estimate * dec!(0.9) + return_pct * dec!(0.1);
            }
        }
        self.last_mid_price = Some(new_price);
    }

    /// Check if orders need to be refreshed
    pub fn needs_refresh(&self, current_price: Decimal, threshold_pct: Decimal) -> bool {
        if let Some(last_price) = self.last_mid_price {
            if last_price > Decimal::ZERO {
                let price_change = (current_price - last_price).abs() / last_price;
                return price_change > threshold_pct;
            }
        }
        true // Refresh if no last price
    }

    /// Get active order IDs
    pub fn active_order_ids(&self) -> Vec<Uuid> {
        let mut ids = self.active_bid_orders.clone();
        ids.extend(self.active_ask_orders.clone());
        ids
    }

    /// Get inventory state
    pub fn inventory(&self) -> &InventoryState {
        &self.inventory
    }

    /// Check if rebalancing is needed
    pub fn needs_rebalance(&self) -> bool {
        self.inventory
            .needs_rebalance(self.config.rebalance_threshold)
    }

    /// Get rebalance recommendation
    pub fn rebalance_recommendation(&self) -> RebalanceAction {
        if !self.needs_rebalance() {
            return RebalanceAction::None;
        }

        let amount = self.inventory.rebalance_amount();
        if amount > Decimal::ZERO {
            RebalanceAction::Sell(amount)
        } else {
            RebalanceAction::Buy(amount.abs())
        }
    }
}

/// Rebalance action recommendation
#[derive(Debug, Clone, Serialize)]
pub enum RebalanceAction {
    /// No rebalancing required
    None,
    /// Buy the specified quantity to restore balance
    Buy(Decimal),
    /// Sell the specified quantity to restore balance
    Sell(Decimal),
}

/// Market maker manager for multiple tokens
pub struct MarketMakerManager {
    /// Per-token market makers indexed by token ID
    makers: HashMap<Uuid, TokenMarketMaker>,
    /// System user ID used when placing market-maker orders
    mm_user_id: Uuid,
    /// Default configuration applied to newly created market makers
    default_config: MarketMakerConfig,
}

impl MarketMakerManager {
    /// Create a new market maker manager using the given system user ID
    pub fn new(mm_user_id: Uuid) -> Self {
        Self {
            makers: HashMap::new(),
            mm_user_id,
            default_config: MarketMakerConfig::default(),
        }
    }

    /// Set default configuration for new market makers
    pub fn set_default_config(&mut self, config: MarketMakerConfig) {
        self.default_config = config;
    }

    /// Add a market maker for a token
    pub fn add_maker(&mut self, token_id: Uuid, config: Option<MarketMakerConfig>) {
        let config = config.unwrap_or_else(|| self.default_config.clone());
        self.makers
            .insert(token_id, TokenMarketMaker::new(token_id, config));
    }

    /// Remove market maker for a token
    pub fn remove_maker(&mut self, token_id: Uuid) -> Option<TokenMarketMaker> {
        self.makers.remove(&token_id)
    }

    /// Get market maker for a token
    pub fn get_maker(&self, token_id: &Uuid) -> Option<&TokenMarketMaker> {
        self.makers.get(token_id)
    }

    /// Get mutable market maker for a token
    pub fn get_maker_mut(&mut self, token_id: &Uuid) -> Option<&mut TokenMarketMaker> {
        self.makers.get_mut(token_id)
    }

    /// Update inventory for a token's market maker
    pub fn update_inventory(
        &mut self,
        token_id: &Uuid,
        token_balance: Decimal,
        quote_balance: Decimal,
        token_price: Decimal,
    ) {
        if let Some(maker) = self.makers.get_mut(token_id) {
            maker.update_inventory(token_balance, quote_balance, token_price);
        }
    }

    /// Generate orders for a token
    pub fn generate_orders(&mut self, token_id: &Uuid, mid_price: Decimal) -> Vec<LimitOrder> {
        if let Some(maker) = self.makers.get_mut(token_id) {
            maker.generate_orders(mid_price, self.mm_user_id)
        } else {
            Vec::new()
        }
    }

    /// Get quote for a token
    pub fn get_quote(&self, token_id: &Uuid, mid_price: Decimal) -> Option<MarketMakerQuote> {
        self.makers
            .get(token_id)
            .map(|m| m.generate_quote(mid_price))
    }

    /// Get all tokens that need order refresh
    pub fn tokens_needing_refresh(
        &self,
        prices: &HashMap<Uuid, Decimal>,
        threshold: Decimal,
    ) -> Vec<Uuid> {
        self.makers
            .iter()
            .filter(|(id, maker)| {
                if let Some(price) = prices.get(id) {
                    maker.needs_refresh(*price, threshold)
                } else {
                    false
                }
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all tokens that need rebalancing
    pub fn tokens_needing_rebalance(&self) -> Vec<(Uuid, RebalanceAction)> {
        self.makers
            .iter()
            .filter_map(|(id, maker)| {
                let action = maker.rebalance_recommendation();
                match action {
                    RebalanceAction::None => None,
                    _ => Some((*id, action)),
                }
            })
            .collect()
    }

    /// Get summary of all market makers
    pub fn summary(&self) -> Vec<MarketMakerSummary> {
        self.makers
            .iter()
            .map(|(id, maker)| MarketMakerSummary {
                token_id: *id,
                inventory: maker.inventory.clone(),
                volatility: maker.volatility_estimate,
                needs_rebalance: maker.needs_rebalance(),
                active_orders: maker.active_order_ids().len(),
            })
            .collect()
    }
}

/// Summary of a market maker's state
#[derive(Debug, Clone, Serialize)]
pub struct MarketMakerSummary {
    /// Token this summary is for
    pub token_id: Uuid,
    /// Current inventory state
    pub inventory: InventoryState,
    /// Current volatility estimate
    pub volatility: Decimal,
    /// Whether this market maker requires rebalancing
    pub needs_rebalance: bool,
    /// Number of currently active orders
    pub active_orders: usize,
}

/// Statistics for market maker performance
#[derive(Debug, Clone, Serialize, Default)]
pub struct MarketMakerStats {
    /// Total volume executed on the buy side (in quote currency)
    pub total_buy_volume: Decimal,
    /// Total volume executed on the sell side (in quote currency)
    pub total_sell_volume: Decimal,
    /// Cumulative spread profit earned
    pub total_profit: Decimal,
    /// Total number of trades executed
    pub trades_count: u64,
    /// Running mean of spread earned per trade
    pub avg_spread_earned: Decimal,
    /// Number of inventory rebalancing operations performed
    pub rebalance_count: u64,
}

impl MarketMakerStats {
    /// Record a completed trade and update statistics
    pub fn record_trade(
        &mut self,
        side: OrderSide,
        amount: Decimal,
        price: Decimal,
        spread_earned: Decimal,
    ) {
        match side {
            OrderSide::Buy => self.total_buy_volume += amount * price,
            OrderSide::Sell => self.total_sell_volume += amount * price,
        }
        self.total_profit += spread_earned;
        self.trades_count += 1;

        // Update average spread
        let n = Decimal::from(self.trades_count);
        self.avg_spread_earned = (self.avg_spread_earned * (n - Decimal::ONE) + spread_earned) / n;
    }

    /// Increment the rebalance counter
    pub fn record_rebalance(&mut self) {
        self.rebalance_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_token_id() -> Uuid {
        Uuid::new_v4()
    }

    fn make_user_id() -> Uuid {
        Uuid::new_v4()
    }

    // ----------------------------------------------------------------
    // InventoryState computation
    // ----------------------------------------------------------------

    #[test]
    fn test_inventory_state_balanced() {
        // Equal token value and quote — skew should be 0
        let price = dec!(2.0);
        let inv = InventoryState::new(dec!(50), dec!(100), price);

        assert_eq!(
            inv.total_value,
            dec!(200),
            "total_value = token_balance * price + quote_balance"
        );
        // target_token_balance = (200 / 2) / 2 = 50
        assert_eq!(inv.target_token_balance, dec!(50));
        assert_eq!(
            inv.skew,
            dec!(0),
            "Perfectly balanced inventory must have zero skew"
        );
    }

    #[test]
    fn test_inventory_state_token_heavy() {
        // More tokens than target — positive skew (too many tokens)
        let price = dec!(1.0);
        let inv = InventoryState::new(dec!(80), dec!(20), price);

        // total_value = 80 + 20 = 100; target_tokens = 50/1 = 50
        // skew = (80 - 50) / 50 = 0.6
        assert!(
            inv.skew > dec!(0),
            "Excess token holdings must produce positive skew"
        );
        assert_eq!(
            inv.rebalance_amount(),
            inv.token_balance - inv.target_token_balance,
            "rebalance_amount must equal token_balance minus target"
        );
    }

    #[test]
    fn test_inventory_state_zero_price() {
        // Should not panic; skew and target should be zero
        let inv = InventoryState::new(dec!(100), dec!(100), dec!(0));
        assert_eq!(inv.target_token_balance, dec!(0));
        assert_eq!(inv.skew, dec!(0));
    }

    #[test]
    fn test_needs_rebalance_above_threshold() {
        let price = dec!(1.0);
        let inv = InventoryState::new(dec!(80), dec!(20), price);
        // skew = 0.6 — above default threshold of 0.30
        assert!(
            inv.needs_rebalance(dec!(0.30)),
            "Skew of 0.6 must trigger rebalance at threshold 0.30"
        );
    }

    #[test]
    fn test_needs_rebalance_below_threshold() {
        let price = dec!(1.0);
        let inv = InventoryState::new(dec!(52), dec!(48), price);
        // total = 100; target = 50; skew = (52-50)/50 = 0.04
        assert!(
            !inv.needs_rebalance(dec!(0.30)),
            "Skew of 0.04 must not trigger rebalance at threshold 0.30"
        );
    }

    // ----------------------------------------------------------------
    // Spread and price calculation
    // ----------------------------------------------------------------

    #[test]
    fn test_spread_at_least_target_when_no_skew() {
        let token_id = make_token_id();
        let config = MarketMakerConfig {
            dynamic_spread: false,
            inventory_skew: false,
            target_spread_pct: dec!(0.02),
            ..MarketMakerConfig::default()
        };
        let maker = TokenMarketMaker::new(token_id, config.clone());

        let spread = maker.calculate_spread();
        assert_eq!(
            spread, config.target_spread_pct,
            "With dynamic and skew disabled, spread must equal target_spread_pct"
        );
    }

    #[test]
    fn test_spread_widens_with_high_volatility() {
        let token_id = make_token_id();
        let config = MarketMakerConfig {
            dynamic_spread: true,
            inventory_skew: false,
            target_spread_pct: dec!(0.02),
            ..MarketMakerConfig::default()
        };
        let mut maker = TokenMarketMaker::new(token_id, config);
        // Force a high volatility estimate
        maker.volatility_estimate = dec!(0.10);

        let spread = maker.calculate_spread();
        // With volatility = 0.10, dynamic spread = max(0.02, 0.10 * 1.5) = 0.15
        assert!(
            spread > dec!(0.02),
            "High volatility must cause spread to exceed target_spread_pct"
        );
    }

    #[test]
    fn test_bid_below_mid_and_ask_above_mid() {
        let token_id = make_token_id();
        let config = MarketMakerConfig {
            dynamic_spread: false,
            inventory_skew: false,
            ..MarketMakerConfig::default()
        };
        let maker = TokenMarketMaker::new(token_id, config);
        let mid = dec!(100.0);

        let (bid, ask) = maker.calculate_prices(mid);
        assert!(bid < mid, "Bid must be below mid price");
        assert!(ask > mid, "Ask must be above mid price");
        assert!(bid < ask, "Bid must be strictly less than ask");
    }

    // ----------------------------------------------------------------
    // Quote generation
    // ----------------------------------------------------------------

    #[test]
    fn test_generate_quote_positive_spread() {
        let token_id = make_token_id();
        let config = MarketMakerConfig {
            dynamic_spread: false,
            inventory_skew: false,
            ..MarketMakerConfig::default()
        };
        let mut maker = TokenMarketMaker::new(token_id, config);
        maker.update_inventory(dec!(1000), dec!(1000), dec!(1.0));

        let mid = dec!(1.0);
        let quote = maker.generate_quote(mid);

        assert_eq!(quote.token_id, token_id);
        assert!(
            quote.spread > dec!(0),
            "Quote spread must be strictly positive"
        );
        assert!(
            quote.spread_pct > dec!(0),
            "Quote spread_pct must be strictly positive"
        );
        assert_eq!(quote.mid_price, mid);
    }

    #[test]
    fn test_generate_quote_empty_inventory_produces_no_asks() {
        let token_id = make_token_id();
        let config = MarketMakerConfig {
            dynamic_spread: false,
            inventory_skew: false,
            min_order_size: dec!(1),
            ..MarketMakerConfig::default()
        };
        let mut maker = TokenMarketMaker::new(token_id, config);
        // Zero token balance → level_size = 0 → ask_amount < min_order_size
        maker.update_inventory(dec!(0), dec!(1000), dec!(1.0));

        let quote = maker.generate_quote(dec!(1.0));
        assert!(
            quote.asks.is_empty(),
            "Zero token inventory must produce no ask levels"
        );
    }

    // ----------------------------------------------------------------
    // Needs refresh
    // ----------------------------------------------------------------

    #[test]
    fn test_needs_refresh_when_no_last_price() {
        let token_id = make_token_id();
        let maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        assert!(
            maker.needs_refresh(dec!(1.0), dec!(0.01)),
            "needs_refresh must be true when there is no last mid price"
        );
    }

    #[test]
    fn test_needs_refresh_when_price_moved_beyond_threshold() {
        let token_id = make_token_id();
        let mut maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        // Set a last mid price via update_volatility which also sets last_mid_price
        maker.update_volatility(dec!(100.0));

        // Move price 10% — above a 5% threshold
        assert!(
            maker.needs_refresh(dec!(110.0), dec!(0.05)),
            "A 10% price move must exceed the 5% refresh threshold"
        );
    }

    #[test]
    fn test_does_not_need_refresh_for_tiny_move() {
        let token_id = make_token_id();
        let mut maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        maker.update_volatility(dec!(100.0));

        // 0.5% move, threshold 1%
        assert!(
            !maker.needs_refresh(dec!(100.5), dec!(0.01)),
            "A 0.5% price move must not exceed the 1% refresh threshold"
        );
    }

    // ----------------------------------------------------------------
    // Volatility update (EMA)
    // ----------------------------------------------------------------

    #[test]
    fn test_volatility_estimate_moves_toward_returns() {
        let token_id = make_token_id();
        let mut maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        let initial_vol = maker.volatility_estimate;

        // Simulate a large return from 100 to 200 (100%)
        maker.update_volatility(dec!(100.0));
        maker.update_volatility(dec!(200.0));

        // After the jump the EMA should have risen above the initial value
        assert!(
            maker.volatility_estimate > initial_vol,
            "Volatility estimate must increase after a large price move"
        );
    }

    // ----------------------------------------------------------------
    // Rebalance recommendation
    // ----------------------------------------------------------------

    #[test]
    fn test_rebalance_recommendation_sell_when_token_heavy() {
        let token_id = make_token_id();
        let mut maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        // 80 tokens @ $1, 20 quote → heavily long tokens
        maker.update_inventory(dec!(80), dec!(20), dec!(1.0));

        let rec = maker.rebalance_recommendation();
        assert!(
            matches!(rec, RebalanceAction::Sell(_)),
            "Excess token inventory must recommend Sell action"
        );
    }

    #[test]
    fn test_rebalance_recommendation_none_when_balanced() {
        let token_id = make_token_id();
        let mut maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        // Perfectly balanced
        maker.update_inventory(dec!(50), dec!(50), dec!(1.0));

        let rec = maker.rebalance_recommendation();
        assert!(
            matches!(rec, RebalanceAction::None),
            "Balanced inventory must recommend no rebalance action"
        );
    }

    #[test]
    fn test_rebalance_recommendation_buy_when_quote_heavy() {
        let token_id = make_token_id();
        let mut maker = TokenMarketMaker::new(token_id, MarketMakerConfig::default());
        // Only 20 tokens @ $1, but 80 quote → long quote, short tokens
        maker.update_inventory(dec!(20), dec!(80), dec!(1.0));

        let rec = maker.rebalance_recommendation();
        assert!(
            matches!(rec, RebalanceAction::Buy(_)),
            "Excess quote inventory must recommend Buy action"
        );
    }

    // ----------------------------------------------------------------
    // MarketMakerStats
    // ----------------------------------------------------------------

    #[test]
    fn test_stats_buy_volume_accumulated() {
        let mut stats = MarketMakerStats::default();
        stats.record_trade(OrderSide::Buy, dec!(10), dec!(2.0), dec!(0.04));

        assert_eq!(
            stats.total_buy_volume,
            dec!(20),
            "Buy volume must equal amount * price"
        );
        assert_eq!(stats.trades_count, 1);
        assert_eq!(stats.total_profit, dec!(0.04));
    }

    #[test]
    fn test_stats_sell_volume_accumulated() {
        let mut stats = MarketMakerStats::default();
        stats.record_trade(OrderSide::Sell, dec!(5), dec!(3.0), dec!(0.05));

        assert_eq!(
            stats.total_sell_volume,
            dec!(15),
            "Sell volume must equal amount * price"
        );
    }

    #[test]
    fn test_stats_average_spread_running_mean() {
        let mut stats = MarketMakerStats::default();
        stats.record_trade(OrderSide::Buy, dec!(1), dec!(1.0), dec!(0.02));
        stats.record_trade(OrderSide::Buy, dec!(1), dec!(1.0), dec!(0.04));

        // avg = (0.02 + 0.04) / 2 = 0.03
        assert_eq!(
            stats.avg_spread_earned,
            dec!(0.03),
            "Average spread must be the running mean of spread_earned values"
        );
    }

    #[test]
    fn test_stats_rebalance_count_increments() {
        let mut stats = MarketMakerStats::default();
        stats.record_rebalance();
        stats.record_rebalance();
        assert_eq!(stats.rebalance_count, 2);
    }

    // ----------------------------------------------------------------
    // MarketMakerManager
    // ----------------------------------------------------------------

    #[test]
    fn test_manager_add_and_remove_maker() {
        let mm_user = make_user_id();
        let mut manager = MarketMakerManager::new(mm_user);
        let token_id = make_token_id();

        manager.add_maker(token_id, None);
        assert!(
            manager.get_maker(&token_id).is_some(),
            "Maker must be accessible after add_maker"
        );

        let removed = manager.remove_maker(token_id);
        assert!(
            removed.is_some(),
            "remove_maker must return the removed maker"
        );
        assert!(
            manager.get_maker(&token_id).is_none(),
            "Maker must be gone after remove_maker"
        );
    }

    #[test]
    fn test_manager_get_quote_returns_none_for_unknown_token() {
        let mm_user = make_user_id();
        let manager = MarketMakerManager::new(mm_user);
        let unknown = make_token_id();

        assert!(
            manager.get_quote(&unknown, dec!(1.0)).is_none(),
            "get_quote must return None for tokens with no maker"
        );
    }

    #[test]
    fn test_manager_tokens_needing_rebalance() {
        let mm_user = make_user_id();
        let mut manager = MarketMakerManager::new(mm_user);
        let token_id = make_token_id();

        manager.add_maker(token_id, None);
        manager.update_inventory(&token_id, dec!(80), dec!(20), dec!(1.0));

        let needs_rebalance = manager.tokens_needing_rebalance();
        assert!(
            !needs_rebalance.is_empty(),
            "Manager must report a token that needs rebalancing"
        );
        assert_eq!(
            needs_rebalance[0].0, token_id,
            "The reported token must be the one with skewed inventory"
        );
    }
}

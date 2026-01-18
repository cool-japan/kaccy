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
    pub token_id: Uuid,
    pub bids: Vec<QuoteLevel>,
    pub asks: Vec<QuoteLevel>,
    pub mid_price: Decimal,
    pub spread: Decimal,
    pub spread_pct: Decimal,
}

/// A single price level in the quote
#[derive(Debug, Clone, Serialize)]
pub struct QuoteLevel {
    pub price: Decimal,
    pub amount: Decimal,
    pub total_value: Decimal,
}

/// Market maker for a single token
pub struct TokenMarketMaker {
    pub token_id: Uuid,
    pub config: MarketMakerConfig,
    inventory: InventoryState,
    active_bid_orders: Vec<Uuid>,
    active_ask_orders: Vec<Uuid>,
    last_mid_price: Option<Decimal>,
    volatility_estimate: Decimal,
}

impl TokenMarketMaker {
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
            let bid_amount = level_size.min(self.inventory.quote_balance / bid_price);
            if bid_amount >= self.config.min_order_size {
                bids.push(QuoteLevel {
                    price: bid_price,
                    amount: bid_amount,
                    total_value: bid_price * bid_amount,
                });
            }

            // Ask levels (increasing price)
            let ask_price = base_ask * (Decimal::ONE + level_offset);
            let ask_amount = level_size.min(self.inventory.token_balance);
            if ask_amount >= self.config.min_order_size {
                asks.push(QuoteLevel {
                    price: ask_price,
                    amount: ask_amount,
                    total_value: ask_price * ask_amount,
                });
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
    None,
    Buy(Decimal),
    Sell(Decimal),
}

/// Market maker manager for multiple tokens
pub struct MarketMakerManager {
    makers: HashMap<Uuid, TokenMarketMaker>,
    mm_user_id: Uuid, // System user ID for MM orders
    default_config: MarketMakerConfig,
}

impl MarketMakerManager {
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
    pub token_id: Uuid,
    pub inventory: InventoryState,
    pub volatility: Decimal,
    pub needs_rebalance: bool,
    pub active_orders: usize,
}

/// Statistics for market maker performance
#[derive(Debug, Clone, Serialize, Default)]
pub struct MarketMakerStats {
    pub total_buy_volume: Decimal,
    pub total_sell_volume: Decimal,
    pub total_profit: Decimal,
    pub trades_count: u64,
    pub avg_spread_earned: Decimal,
    pub rebalance_count: u64,
}

impl MarketMakerStats {
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

    pub fn record_rebalance(&mut self) {
        self.rebalance_count += 1;
    }
}

//! Testing utilities for model factories, fixtures, and mock data generation

use chrono::{DateTime, Utc};
use rand::RngExt;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::models::*;

/// Model factory trait
pub trait Factory<T> {
    /// Build a model instance
    fn build() -> T;

    /// Build multiple model instances
    fn build_many(count: usize) -> Vec<T> {
        (0..count).map(|_| Self::build()).collect()
    }
}

/// User factory
pub struct UserFactory;

impl Factory<User> for UserFactory {
    fn build() -> User {
        let mut rng = rand::rng();
        let random_num: u32 = rng.random_range(1000..9999);

        User {
            user_id: Uuid::new_v4(),
            email: format!("user{}@example.com", random_num),
            password_hash: "hashed_password".to_string(),
            username: format!("user{}", random_num),
            display_name: Some(format!("User {}", random_num)),
            bio: None,
            avatar_url: None,
            btc_withdrawal_address: None,
            created_at: Utc::now(),
            kyc_status: KycStatus::Pending,
            reputation_score: Decimal::from(rng.random_range(0..100)),
            role: UserRole::User,
        }
    }
}

impl UserFactory {
    /// Build a user with a specific username
    pub fn with_username(username: &str) -> User {
        let mut user = Self::build();
        user.username = username.to_string();
        user
    }

    /// Build a user with a specific reputation score
    pub fn with_reputation(score: Decimal) -> User {
        let mut user = Self::build();
        user.reputation_score = score;
        user
    }
}

/// Token factory
pub struct TokenFactory;

impl Factory<Token> for TokenFactory {
    fn build() -> Token {
        let mut rng = rand::rng();
        let random_num: u32 = rng.random_range(1000..9999);

        Token {
            token_id: Uuid::new_v4(),
            issuer_user_id: Uuid::new_v4(),
            symbol: format!("$TOK{}", random_num),
            name: format!("Token {}", random_num),
            description: Some(format!("Test token {}", random_num)),
            total_supply: Decimal::from(1000000),
            circulating_supply: Decimal::from(0),
            initial_price_btc: Decimal::from(1),
            price_increment_btc: Decimal::from_str_exact("0.0001").expect("\"0.0001\" is a valid Decimal string"),
            created_at: Utc::now(),
            status: TokenStatus::Active,
        }
    }
}

impl TokenFactory {
    /// Build a token with a specific issuer
    pub fn with_issuer(issuer_user_id: Uuid) -> Token {
        let mut token = Self::build();
        token.issuer_user_id = issuer_user_id;
        token
    }

    /// Build a token with a specific symbol
    pub fn with_symbol(symbol: &str) -> Token {
        let mut token = Self::build();
        token.symbol = symbol.to_string();
        token
    }

    /// Build a token with a specific supply
    pub fn with_supply(supply: Decimal) -> Token {
        let mut token = Self::build();
        token.total_supply = supply;
        token
    }
}

/// Order factory
pub struct OrderFactory;

impl Factory<Order> for OrderFactory {
    fn build() -> Order {
        let mut rng = rand::rng();

        let amount = Decimal::from(rng.random_range(1..100));
        let price_btc = Decimal::from(rng.random_range(1..1000));

        Order {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            order_type: OrderType::Buy,
            amount,
            price_btc,
            total_btc: amount * price_btc,
            status: OrderStatus::Pending,
            btc_address: None,
            btc_txid: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }
}

impl OrderFactory {
    /// Build a buy order
    pub fn buy() -> Order {
        let mut order = Self::build();
        order.order_type = OrderType::Buy;
        order
    }

    /// Build a sell order
    pub fn sell() -> Order {
        let mut order = Self::build();
        order.order_type = OrderType::Sell;
        order
    }

    /// Build an order with a specific amount
    pub fn with_amount(amount: Decimal) -> Order {
        let mut order = Self::build();
        order.amount = amount;
        order
    }

    /// Build an order with a specific price
    pub fn with_price(price_btc: Decimal) -> Order {
        let mut order = Self::build();
        order.price_btc = price_btc;
        order.total_btc = order.amount * price_btc;
        order
    }
}

/// Trade factory
pub struct TradeFactory;

impl Factory<Trade> for TradeFactory {
    fn build() -> Trade {
        let mut rng = rand::rng();

        let amount = Decimal::from(rng.random_range(1..100));
        let price_btc = Decimal::from(rng.random_range(1..1000));
        let total_btc = amount * price_btc;

        Trade {
            trade_id: Uuid::new_v4(),
            buyer_user_id: Uuid::new_v4(),
            seller_user_id: Some(Uuid::new_v4()),
            token_id: Uuid::new_v4(),
            amount,
            price_btc,
            total_btc,
            platform_fee_btc: total_btc * Decimal::from_str_exact("0.025").expect("\"0.025\" is a valid Decimal string"),
            issuer_royalty_btc: total_btc * Decimal::from_str_exact("0.005").expect("\"0.005\" is a valid Decimal string"),
            executed_at: Utc::now(),
        }
    }
}

impl TradeFactory {
    /// Build a trade with specific buyer and seller
    pub fn with_parties(buyer_id: Uuid, seller_id: Uuid) -> Trade {
        let mut trade = Self::build();
        trade.buyer_user_id = buyer_id;
        trade.seller_user_id = Some(seller_id);
        trade
    }

    /// Build a trade with a specific token
    pub fn with_token(token_id: Uuid) -> Trade {
        let mut trade = Self::build();
        trade.token_id = token_id;
        trade
    }
}

/// Balance factory
pub struct BalanceFactory;

impl Factory<Balance> for BalanceFactory {
    fn build() -> Balance {
        let mut rng = rand::rng();

        Balance {
            balance_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: Decimal::from(rng.random_range(0..10000)),
            locked_amount: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }
}

impl BalanceFactory {
    /// Build a balance with a specific amount
    pub fn with_amount(amount: Decimal) -> Balance {
        let mut balance = Self::build();
        balance.amount = amount;
        balance
    }

    /// Build a balance with locked amount
    pub fn with_locked(amount: Decimal, locked: Decimal) -> Balance {
        let mut balance = Self::build();
        balance.amount = amount;
        balance.locked_amount = locked;
        balance
    }
}

/// Mock data generator
pub struct MockDataGenerator;

impl MockDataGenerator {
    /// Generate a random UUID
    pub fn uuid() -> Uuid {
        Uuid::new_v4()
    }

    /// Generate a random username
    pub fn username() -> String {
        let mut rng = rand::rng();
        format!("user{}", rng.random_range(1000..9999))
    }

    /// Generate a random email
    pub fn email() -> String {
        let mut rng = rand::rng();
        format!("user{}@example.com", rng.random_range(1000..9999))
    }

    /// Generate a random token symbol
    pub fn token_symbol() -> String {
        let mut rng = rand::rng();
        format!("$TOK{}", rng.random_range(1000..9999))
    }

    /// Generate a random decimal in a range
    pub fn decimal(min: i64, max: i64) -> Decimal {
        let mut rng = rand::rng();
        Decimal::from(rng.random_range(min..max))
    }

    /// Generate a random timestamp within the last N days
    pub fn recent_timestamp(days: i64) -> DateTime<Utc> {
        let mut rng = rand::rng();
        let seconds_ago = rng.random_range(0..(days * 24 * 60 * 60));
        Utc::now() - chrono::Duration::seconds(seconds_ago)
    }

    /// Generate a random future timestamp within N days
    pub fn future_timestamp(days: i64) -> DateTime<Utc> {
        let mut rng = rand::rng();
        let seconds_ahead = rng.random_range(0..(days * 24 * 60 * 60));
        Utc::now() + chrono::Duration::seconds(seconds_ahead)
    }

    /// Generate a random bitcoin address (simplified)
    pub fn btc_address() -> String {
        let mut rng = rand::rng();
        let prefix = if rng.random_range(0..2) == 0 {
            "1"
        } else {
            "3"
        };
        let random_chars: String = (0..33)
            .map(|_| {
                let chars = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
                chars.chars().nth(rng.random_range(0..chars.len())).expect("index is within bounds of chars.len()")
            })
            .collect();
        format!("{}{}", prefix, random_chars)
    }
}

/// Test fixture builder
pub struct FixtureBuilder {
    users: Vec<User>,
    tokens: Vec<Token>,
    orders: Vec<Order>,
    trades: Vec<Trade>,
    balances: Vec<Balance>,
}

impl FixtureBuilder {
    /// Create a new fixture builder
    pub fn new() -> Self {
        Self {
            users: Vec::new(),
            tokens: Vec::new(),
            orders: Vec::new(),
            trades: Vec::new(),
            balances: Vec::new(),
        }
    }

    /// Add users to the fixture
    pub fn with_users(mut self, count: usize) -> Self {
        self.users = UserFactory::build_many(count);
        self
    }

    /// Add tokens to the fixture
    pub fn with_tokens(mut self, count: usize) -> Self {
        self.tokens = TokenFactory::build_many(count);
        self
    }

    /// Add orders to the fixture
    pub fn with_orders(mut self, count: usize) -> Self {
        self.orders = OrderFactory::build_many(count);
        self
    }

    /// Add trades to the fixture
    pub fn with_trades(mut self, count: usize) -> Self {
        self.trades = TradeFactory::build_many(count);
        self
    }

    /// Add balances to the fixture
    pub fn with_balances(mut self, count: usize) -> Self {
        self.balances = BalanceFactory::build_many(count);
        self
    }

    /// Build the fixture
    pub fn build(self) -> Fixture {
        Fixture {
            users: self.users,
            tokens: self.tokens,
            orders: self.orders,
            trades: self.trades,
            balances: self.balances,
        }
    }
}

impl Default for FixtureBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test fixture
pub struct Fixture {
    /// Sample users created for the test.
    pub users: Vec<User>,
    /// Sample tokens created for the test.
    pub tokens: Vec<Token>,
    /// Sample orders created for the test.
    pub orders: Vec<Order>,
    /// Sample trades recorded for the test.
    pub trades: Vec<Trade>,
    /// Sample balances set up for the test.
    pub balances: Vec<Balance>,
}

impl Fixture {
    /// Get the first user
    pub fn first_user(&self) -> Option<&User> {
        self.users.first()
    }

    /// Get the first token
    pub fn first_token(&self) -> Option<&Token> {
        self.tokens.first()
    }

    /// Get the first order
    pub fn first_order(&self) -> Option<&Order> {
        self.orders.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_factory() {
        let user = UserFactory::build();
        assert!(user.username.starts_with("user"));
        assert!(user.email.contains("@example.com"));
    }

    #[test]
    fn test_user_factory_with_username() {
        let user = UserFactory::with_username("testuser");
        assert_eq!(user.username, "testuser");
    }

    #[test]
    fn test_token_factory() {
        let token = TokenFactory::build();
        assert!(token.symbol.starts_with("$TOK"));
        assert_eq!(token.total_supply, Decimal::from(1000000));
    }

    #[test]
    fn test_token_factory_with_issuer() {
        let issuer_user_id = Uuid::new_v4();
        let token = TokenFactory::with_issuer(issuer_user_id);
        assert_eq!(token.issuer_user_id, issuer_user_id);
    }

    #[test]
    fn test_order_factory() {
        let order = OrderFactory::build();
        assert_eq!(order.status, OrderStatus::Pending);
        assert_eq!(order.total_btc, order.amount * order.price_btc);
    }

    #[test]
    fn test_order_factory_buy_sell() {
        let buy_order = OrderFactory::buy();
        assert_eq!(buy_order.order_type, OrderType::Buy);

        let sell_order = OrderFactory::sell();
        assert_eq!(sell_order.order_type, OrderType::Sell);
    }

    #[test]
    fn test_trade_factory() {
        let trade = TradeFactory::build();
        assert_eq!(trade.total_btc, trade.amount * trade.price_btc);
    }

    #[test]
    fn test_balance_factory() {
        let balance = BalanceFactory::build();
        assert_eq!(balance.locked_amount, Decimal::ZERO);
    }

    #[test]
    fn test_mock_data_generator() {
        let username = MockDataGenerator::username();
        assert!(username.starts_with("user"));

        let email = MockDataGenerator::email();
        assert!(email.contains("@example.com"));

        let symbol = MockDataGenerator::token_symbol();
        assert!(symbol.starts_with("$TOK"));
    }

    #[test]
    fn test_fixture_builder() {
        let fixture = FixtureBuilder::new()
            .with_users(3)
            .with_tokens(2)
            .with_orders(5)
            .build();

        assert_eq!(fixture.users.len(), 3);
        assert_eq!(fixture.tokens.len(), 2);
        assert_eq!(fixture.orders.len(), 5);
    }

    #[test]
    fn test_fixture_first_helpers() {
        let fixture = FixtureBuilder::new().with_users(1).with_tokens(1).build();

        assert!(fixture.first_user().is_some());
        assert!(fixture.first_token().is_some());
    }
}

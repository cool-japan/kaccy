//! Synthetic assets system
//!
//! This module provides:
//! - Collateral-backed synthetic assets
//! - Price oracle integration for tracking real-world assets
//! - Minting and burning mechanisms
//! - Redemption protocols

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Type of synthetic asset
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SyntheticAssetType {
    /// Fiat currency (e.g., USD, EUR)
    Fiat,
    /// Commodity (e.g., Gold, Silver)
    Commodity,
    /// Stock (e.g., AAPL, TSLA)
    Stock,
    /// Index (e.g., S&P500)
    Index,
    /// Cryptocurrency (e.g., BTC, ETH)
    Crypto,
    /// Custom asset
    Custom,
}

/// Synthetic asset definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticAsset {
    /// Unique identifier for this synthetic asset
    pub asset_id: Uuid,
    /// Short ticker symbol (e.g. `sUSD`)
    pub symbol: String,
    /// Human-readable name
    pub name: String,
    /// Category of the underlying asset
    pub asset_type: SyntheticAssetType,
    /// Collateral ratio required (e.g., 1.5 = 150% collateralization)
    pub collateral_ratio: Decimal,
    /// Liquidation ratio (e.g., 1.2 = liquidate below 120%)
    pub liquidation_ratio: Decimal,
    /// Fee for minting (e.g., 0.003 = 0.3%)
    pub mint_fee: Decimal,
    /// Fee for burning (e.g., 0.003 = 0.3%)
    pub burn_fee: Decimal,
    /// Total supply of synthetic asset
    pub total_supply: Decimal,
    /// UNIX timestamp when this asset was created
    pub created_at: i64,
}

impl SyntheticAsset {
    /// Create and validate a new synthetic asset definition
    pub fn new(
        symbol: String,
        name: String,
        asset_type: SyntheticAssetType,
        collateral_ratio: Decimal,
        liquidation_ratio: Decimal,
        mint_fee: Decimal,
        burn_fee: Decimal,
    ) -> Result<Self, &'static str> {
        if collateral_ratio < dec!(1) {
            return Err("Collateral ratio must be at least 1.0");
        }

        if liquidation_ratio >= collateral_ratio {
            return Err("Liquidation ratio must be less than collateral ratio");
        }

        if mint_fee < Decimal::ZERO || mint_fee > dec!(0.1) {
            return Err("Mint fee must be between 0 and 10%");
        }

        if burn_fee < Decimal::ZERO || burn_fee > dec!(0.1) {
            return Err("Burn fee must be between 0 and 10%");
        }

        Ok(Self {
            asset_id: Uuid::new_v4(),
            symbol,
            name,
            asset_type,
            collateral_ratio,
            liquidation_ratio,
            mint_fee,
            burn_fee,
            total_supply: Decimal::ZERO,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Calculate required collateral for minting
    pub fn required_collateral(&self, amount: Decimal, price: Decimal) -> Decimal {
        amount * price * self.collateral_ratio
    }

    /// Calculate collateral ratio for a position
    pub fn calculate_ratio(&self, collateral: Decimal, debt: Decimal, price: Decimal) -> Decimal {
        if debt == Decimal::ZERO {
            return Decimal::MAX;
        }
        collateral / (debt * price)
    }
}

/// Collateral deposit record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralDeposit {
    /// Unique identifier for this deposit
    pub deposit_id: Uuid,
    /// User who made the deposit
    pub user_id: Uuid,
    /// Asset used as collateral
    pub collateral_asset_id: Uuid,
    /// Amount of collateral deposited
    pub amount: Decimal,
    /// UNIX timestamp of the deposit
    pub timestamp: i64,
}

/// Synthetic position (CDP - Collateralized Debt Position)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticPosition {
    /// Unique identifier for this position
    pub position_id: Uuid,
    /// Owner of this position
    pub user_id: Uuid,
    /// Synthetic asset that was minted
    pub synthetic_asset_id: Uuid,
    /// Collateral deposited
    pub collateral_amount: Decimal,
    /// Asset used as collateral for this position.
    pub collateral_asset_id: Uuid,
    /// Synthetic asset minted (debt)
    pub minted_amount: Decimal,
    /// UNIX timestamp when this position was opened
    pub created_at: i64,
    /// UNIX timestamp of the most recent position update
    pub last_update: i64,
}

impl SyntheticPosition {
    /// Create a new empty synthetic position
    pub fn new(user_id: Uuid, synthetic_asset_id: Uuid, collateral_asset_id: Uuid) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            position_id: Uuid::new_v4(),
            user_id,
            synthetic_asset_id,
            collateral_amount: Decimal::ZERO,
            collateral_asset_id,
            minted_amount: Decimal::ZERO,
            created_at: now,
            last_update: now,
        }
    }

    /// Calculate current collateralization ratio
    pub fn collateral_ratio(&self, collateral_price: Decimal, synthetic_price: Decimal) -> Decimal {
        if self.minted_amount == Decimal::ZERO {
            return Decimal::MAX;
        }

        let collateral_value = self.collateral_amount * collateral_price;
        let debt_value = self.minted_amount * synthetic_price;

        collateral_value / debt_value
    }

    /// Check if position is safe (above liquidation threshold)
    pub fn is_safe(
        &self,
        collateral_price: Decimal,
        synthetic_price: Decimal,
        liquidation_ratio: Decimal,
    ) -> bool {
        self.collateral_ratio(collateral_price, synthetic_price) >= liquidation_ratio
    }
}

/// Liquidation event for synthetic position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticLiquidation {
    /// Unique identifier for this liquidation
    pub liquidation_id: Uuid,
    /// Position that was liquidated
    pub position_id: Uuid,
    /// User who triggered the liquidation
    pub liquidator_id: Uuid,
    /// Amount of synthetic debt that was covered
    pub debt_covered: Decimal,
    /// Amount of collateral seized by the liquidator
    pub collateral_seized: Decimal,
    /// Penalty amount charged on the liquidated collateral
    pub liquidation_penalty: Decimal,
    /// UNIX timestamp of the liquidation
    pub timestamp: i64,
}

/// Oracle price feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFeed {
    /// Asset whose price is reported
    pub asset_id: Uuid,
    /// Current price of the asset
    pub price: Decimal,
    /// UNIX timestamp of this price observation
    pub timestamp: i64,
    /// Oracle confidence score (0.0 to 1.0)
    pub confidence: Decimal,
}

/// Synthetic asset manager
pub struct SyntheticAssetManager {
    /// Registered synthetic assets indexed by asset ID
    assets: Arc<RwLock<HashMap<Uuid, SyntheticAsset>>>,
    /// Open CDP positions indexed by position ID
    positions: Arc<RwLock<HashMap<Uuid, SyntheticPosition>>>,
    /// Latest oracle price feeds indexed by asset ID
    price_feeds: Arc<RwLock<HashMap<Uuid, PriceFeed>>>,
    /// Position IDs grouped by user ID
    user_positions: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
}

impl SyntheticAssetManager {
    /// Create a new synthetic asset manager
    pub fn new() -> Self {
        Self {
            assets: Arc::new(RwLock::new(HashMap::new())),
            positions: Arc::new(RwLock::new(HashMap::new())),
            price_feeds: Arc::new(RwLock::new(HashMap::new())),
            user_positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new synthetic asset
    pub async fn create_asset(&self, asset: SyntheticAsset) -> Result<Uuid, &'static str> {
        let asset_id = asset.asset_id;
        self.assets.write().await.insert(asset_id, asset);
        Ok(asset_id)
    }

    /// Update price feed (oracle integration)
    pub async fn update_price(&self, feed: PriceFeed) {
        self.price_feeds.write().await.insert(feed.asset_id, feed);
    }

    /// Get current price for an asset
    pub async fn get_price(&self, asset_id: Uuid) -> Option<Decimal> {
        self.price_feeds
            .read()
            .await
            .get(&asset_id)
            .map(|feed| feed.price)
    }

    /// Mint synthetic asset (create CDP)
    pub async fn mint(
        &self,
        user_id: Uuid,
        synthetic_asset_id: Uuid,
        collateral_asset_id: Uuid,
        collateral_amount: Decimal,
        mint_amount: Decimal,
    ) -> Result<Uuid, &'static str> {
        if collateral_amount <= Decimal::ZERO {
            return Err("Collateral amount must be positive");
        }

        if mint_amount <= Decimal::ZERO {
            return Err("Mint amount must be positive");
        }

        // Get asset info
        let assets = self.assets.read().await;
        let asset = assets
            .get(&synthetic_asset_id)
            .ok_or("Synthetic asset not found")?;
        let required_ratio = asset.collateral_ratio;
        let mint_fee = asset.mint_fee;
        drop(assets);

        // Get prices
        let collateral_price = self
            .get_price(collateral_asset_id)
            .await
            .ok_or("Collateral price not available")?;
        let synthetic_price = self
            .get_price(synthetic_asset_id)
            .await
            .ok_or("Synthetic price not available")?;

        // Check collateralization
        let collateral_value = collateral_amount * collateral_price;
        let mint_value = mint_amount * synthetic_price;
        let ratio = collateral_value / mint_value;

        if ratio < required_ratio {
            return Err("Insufficient collateralization");
        }

        // Create position
        let mut position = SyntheticPosition::new(user_id, synthetic_asset_id, collateral_asset_id);
        position.collateral_amount = collateral_amount;
        position.minted_amount = mint_amount;

        let position_id = position.position_id;

        // Store position
        self.positions.write().await.insert(position_id, position);

        // Track user position
        self.user_positions
            .write()
            .await
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push(position_id);

        // Update total supply (deduct fee)
        let net_mint = mint_amount * (dec!(1) - mint_fee);
        self.assets
            .write()
            .await
            .get_mut(&synthetic_asset_id)
            .unwrap()
            .total_supply += net_mint;

        Ok(position_id)
    }

    /// Burn synthetic asset (reduce debt)
    pub async fn burn(
        &self,
        position_id: Uuid,
        burn_amount: Decimal,
    ) -> Result<Decimal, &'static str> {
        if burn_amount <= Decimal::ZERO {
            return Err("Burn amount must be positive");
        }

        let mut positions = self.positions.write().await;
        let position = positions
            .get_mut(&position_id)
            .ok_or("Position not found")?;

        if burn_amount > position.minted_amount {
            return Err("Burn amount exceeds debt");
        }

        let synthetic_asset_id = position.synthetic_asset_id;
        position.minted_amount -= burn_amount;
        position.last_update = chrono::Utc::now().timestamp();

        drop(positions);

        // Get burn fee
        let assets = self.assets.read().await;
        let asset = assets.get(&synthetic_asset_id).unwrap();
        let burn_fee = asset.burn_fee;
        drop(assets);

        // Update total supply (add back fee)
        let fee_amount = burn_amount * burn_fee;
        self.assets
            .write()
            .await
            .get_mut(&synthetic_asset_id)
            .unwrap()
            .total_supply -= burn_amount;

        Ok(fee_amount)
    }

    /// Add collateral to existing position
    pub async fn add_collateral(
        &self,
        position_id: Uuid,
        amount: Decimal,
    ) -> Result<(), &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Amount must be positive");
        }

        let mut positions = self.positions.write().await;
        let position = positions
            .get_mut(&position_id)
            .ok_or("Position not found")?;

        position.collateral_amount += amount;
        position.last_update = chrono::Utc::now().timestamp();

        Ok(())
    }

    /// Withdraw collateral (if position remains safe)
    pub async fn withdraw_collateral(
        &self,
        position_id: Uuid,
        amount: Decimal,
    ) -> Result<(), &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Amount must be positive");
        }

        let positions = self.positions.read().await;
        let position = positions.get(&position_id).ok_or("Position not found")?;

        if amount > position.collateral_amount {
            return Err("Insufficient collateral");
        }

        let synthetic_asset_id = position.synthetic_asset_id;
        let collateral_asset_id = position.collateral_asset_id;
        drop(positions);

        // Get asset info
        let assets = self.assets.read().await;
        let asset = assets.get(&synthetic_asset_id).unwrap();
        let required_ratio = asset.collateral_ratio;
        drop(assets);

        // Check if withdrawal is safe
        let collateral_price = self
            .get_price(collateral_asset_id)
            .await
            .ok_or("Collateral price not available")?;
        let synthetic_price = self
            .get_price(synthetic_asset_id)
            .await
            .ok_or("Synthetic price not available")?;

        let positions = self.positions.read().await;
        let position = positions.get(&position_id).unwrap();

        let new_collateral = position.collateral_amount - amount;
        let collateral_value = new_collateral * collateral_price;
        let debt_value = position.minted_amount * synthetic_price;

        if debt_value > Decimal::ZERO {
            let ratio = collateral_value / debt_value;
            if ratio < required_ratio {
                return Err("Withdrawal would under-collateralize position");
            }
        }

        drop(positions);

        // Update position
        let mut positions = self.positions.write().await;
        let position = positions.get_mut(&position_id).unwrap();
        position.collateral_amount -= amount;
        position.last_update = chrono::Utc::now().timestamp();

        Ok(())
    }

    /// Liquidate under-collateralized position
    pub async fn liquidate(
        &self,
        liquidator_id: Uuid,
        position_id: Uuid,
        debt_to_cover: Decimal,
    ) -> Result<SyntheticLiquidation, &'static str> {
        let positions = self.positions.read().await;
        let position = positions.get(&position_id).ok_or("Position not found")?;

        let synthetic_asset_id = position.synthetic_asset_id;
        let collateral_asset_id = position.collateral_asset_id;
        drop(positions);

        // Get asset info
        let assets = self.assets.read().await;
        let asset = assets.get(&synthetic_asset_id).unwrap();
        let liquidation_ratio = asset.liquidation_ratio;
        drop(assets);

        // Get prices
        let collateral_price = self
            .get_price(collateral_asset_id)
            .await
            .ok_or("Collateral price not available")?;
        let synthetic_price = self
            .get_price(synthetic_asset_id)
            .await
            .ok_or("Synthetic price not available")?;

        // Check if liquidatable
        let positions = self.positions.read().await;
        let position = positions.get(&position_id).unwrap();

        if position.is_safe(collateral_price, synthetic_price, liquidation_ratio) {
            return Err("Position is not liquidatable");
        }

        if debt_to_cover > position.minted_amount {
            return Err("Debt to cover exceeds position debt");
        }

        drop(positions);

        // Calculate collateral to seize (with 5% liquidation penalty)
        let liquidation_penalty = dec!(0.05);
        let debt_value = debt_to_cover * synthetic_price;
        let collateral_seized = (debt_value * (dec!(1) + liquidation_penalty)) / collateral_price;

        // Update position
        let mut positions = self.positions.write().await;
        let position = positions.get_mut(&position_id).unwrap();
        position.collateral_amount -= collateral_seized;
        position.minted_amount -= debt_to_cover;
        position.last_update = chrono::Utc::now().timestamp();

        // Update total supply
        self.assets
            .write()
            .await
            .get_mut(&synthetic_asset_id)
            .unwrap()
            .total_supply -= debt_to_cover;

        Ok(SyntheticLiquidation {
            liquidation_id: Uuid::new_v4(),
            position_id,
            liquidator_id,
            debt_covered: debt_to_cover,
            collateral_seized,
            liquidation_penalty: debt_value * liquidation_penalty,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    /// Get synthetic asset info
    pub async fn get_asset(&self, asset_id: Uuid) -> Option<SyntheticAsset> {
        self.assets.read().await.get(&asset_id).cloned()
    }

    /// Get position info
    pub async fn get_position(&self, position_id: Uuid) -> Option<SyntheticPosition> {
        self.positions.read().await.get(&position_id).cloned()
    }

    /// Get all positions for a user
    pub async fn get_user_positions(&self, user_id: Uuid) -> Vec<SyntheticPosition> {
        let user_positions = self.user_positions.read().await;
        let position_ids = user_positions.get(&user_id);

        if let Some(ids) = position_ids {
            let positions = self.positions.read().await;
            ids.iter()
                .filter_map(|id| positions.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for SyntheticAssetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_asset_creation() {
        let asset = SyntheticAsset::new(
            "sUSD".to_string(),
            "Synthetic USD".to_string(),
            SyntheticAssetType::Fiat,
            dec!(1.5),
            dec!(1.2),
            dec!(0.003),
            dec!(0.003),
        );
        assert!(asset.is_ok());
    }

    #[test]
    fn test_required_collateral() {
        let asset = SyntheticAsset::new(
            "sUSD".to_string(),
            "Synthetic USD".to_string(),
            SyntheticAssetType::Fiat,
            dec!(1.5),
            dec!(1.2),
            dec!(0.003),
            dec!(0.003),
        )
        .unwrap();

        // To mint 100 sUSD at $1, need $150 collateral (1.5x)
        let required = asset.required_collateral(dec!(100), dec!(1));
        assert_eq!(required, dec!(150));
    }

    #[test]
    fn test_collateral_ratio_calculation() {
        let asset = SyntheticAsset::new(
            "sUSD".to_string(),
            "Synthetic USD".to_string(),
            SyntheticAssetType::Fiat,
            dec!(1.5),
            dec!(1.2),
            dec!(0.003),
            dec!(0.003),
        )
        .unwrap();

        // $200 collateral, 100 sUSD debt at $1 each
        let ratio = asset.calculate_ratio(dec!(200), dec!(100), dec!(1));
        assert_eq!(ratio, dec!(2)); // 200% collateralized
    }

    #[test]
    fn test_position_safety() {
        let position = SyntheticPosition::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

        // Safe position: $200 collateral, 100 debt, both at $1
        let mut test_pos = position.clone();
        test_pos.collateral_amount = dec!(200);
        test_pos.minted_amount = dec!(100);

        assert!(test_pos.is_safe(dec!(1), dec!(1), dec!(1.5)));

        // Unsafe position: $140 collateral, 100 debt, both at $1 (ratio 1.4 < 1.5)
        test_pos.collateral_amount = dec!(140);
        assert!(!test_pos.is_safe(dec!(1), dec!(1), dec!(1.5)));
    }

    #[tokio::test]
    async fn test_mint_synthetic() {
        let manager = SyntheticAssetManager::new();

        // Create synthetic USD
        let asset = SyntheticAsset::new(
            "sUSD".to_string(),
            "Synthetic USD".to_string(),
            SyntheticAssetType::Fiat,
            dec!(1.5),
            dec!(1.2),
            dec!(0.003),
            dec!(0.003),
        )
        .unwrap();
        let asset_id = asset.asset_id;
        manager.create_asset(asset).await.unwrap();

        // Set up prices
        let collateral_id = Uuid::new_v4();
        manager
            .update_price(PriceFeed {
                asset_id: collateral_id,
                price: dec!(1),
                timestamp: chrono::Utc::now().timestamp(),
                confidence: dec!(0.99),
            })
            .await;

        manager
            .update_price(PriceFeed {
                asset_id,
                price: dec!(1),
                timestamp: chrono::Utc::now().timestamp(),
                confidence: dec!(0.99),
            })
            .await;

        // Mint 100 sUSD with $200 collateral (2x ratio, above 1.5x requirement)
        let user_id = Uuid::new_v4();
        let position_id = manager
            .mint(user_id, asset_id, collateral_id, dec!(200), dec!(100))
            .await
            .unwrap();

        let position = manager.get_position(position_id).await.unwrap();
        assert_eq!(position.collateral_amount, dec!(200));
        assert_eq!(position.minted_amount, dec!(100));
    }

    #[tokio::test]
    async fn test_burn_synthetic() {
        let manager = SyntheticAssetManager::new();

        // Create and mint
        let asset = SyntheticAsset::new(
            "sUSD".to_string(),
            "Synthetic USD".to_string(),
            SyntheticAssetType::Fiat,
            dec!(1.5),
            dec!(1.2),
            dec!(0.003),
            dec!(0.003),
        )
        .unwrap();
        let asset_id = asset.asset_id;
        manager.create_asset(asset).await.unwrap();

        let collateral_id = Uuid::new_v4();
        manager
            .update_price(PriceFeed {
                asset_id: collateral_id,
                price: dec!(1),
                timestamp: chrono::Utc::now().timestamp(),
                confidence: dec!(0.99),
            })
            .await;

        manager
            .update_price(PriceFeed {
                asset_id,
                price: dec!(1),
                timestamp: chrono::Utc::now().timestamp(),
                confidence: dec!(0.99),
            })
            .await;

        let user_id = Uuid::new_v4();
        let position_id = manager
            .mint(user_id, asset_id, collateral_id, dec!(200), dec!(100))
            .await
            .unwrap();

        // Burn 50 sUSD
        manager.burn(position_id, dec!(50)).await.unwrap();

        let position = manager.get_position(position_id).await.unwrap();
        assert_eq!(position.minted_amount, dec!(50));
    }
}

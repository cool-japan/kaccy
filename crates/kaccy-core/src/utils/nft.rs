// NFT integration for gated access and trading benefits
use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// NFT rarity tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NftRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl NftRarity {
    /// Get the fee discount percentage for this rarity
    pub fn fee_discount(&self) -> Decimal {
        match self {
            NftRarity::Common => Decimal::new(5, 2),     // 0.05 (5%)
            NftRarity::Uncommon => Decimal::new(10, 2),  // 0.10 (10%)
            NftRarity::Rare => Decimal::new(15, 2),      // 0.15 (15%)
            NftRarity::Epic => Decimal::new(20, 2),      // 0.20 (20%)
            NftRarity::Legendary => Decimal::new(30, 2), // 0.30 (30%)
            NftRarity::Mythic => Decimal::new(50, 2),    // 0.50 (50%)
        }
    }

    /// Get the priority level for this rarity (higher = better)
    pub fn priority_level(&self) -> u8 {
        match self {
            NftRarity::Common => 1,
            NftRarity::Uncommon => 2,
            NftRarity::Rare => 3,
            NftRarity::Epic => 4,
            NftRarity::Legendary => 5,
            NftRarity::Mythic => 6,
        }
    }
}

/// NFT collection metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftCollection {
    pub id: Uuid,
    pub name: String,
    pub symbol: String,
    pub contract_address: String,
    pub chain: String,
    pub total_supply: u64,
    pub verified: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl NftCollection {
    /// Create a new NFT collection
    pub fn new(
        name: String,
        symbol: String,
        contract_address: String,
        chain: String,
        total_supply: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            symbol,
            contract_address,
            chain,
            total_supply,
            verified: false,
            created_at: chrono::Utc::now(),
        }
    }

    /// Verify the collection
    pub fn verify(&mut self) {
        self.verified = true;
    }
}

/// NFT ownership record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftOwnership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub collection_id: Uuid,
    pub token_id: String,
    pub rarity: NftRarity,
    pub acquired_at: chrono::DateTime<chrono::Utc>,
    pub verified: bool,
}

impl NftOwnership {
    /// Create a new NFT ownership record
    pub fn new(user_id: Uuid, collection_id: Uuid, token_id: String, rarity: NftRarity) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            collection_id,
            token_id,
            rarity,
            acquired_at: chrono::Utc::now(),
            verified: false,
        }
    }

    /// Verify NFT ownership
    pub fn verify(&mut self) {
        self.verified = true;
    }
}

/// NFT-gated access levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftAccessLevel {
    pub collection_id: Uuid,
    pub required_rarity: NftRarity,
    pub access_type: AccessType,
    pub benefits: Vec<AccessBenefit>,
}

/// Types of gated access
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessType {
    PrivatePool,       // Access to exclusive liquidity pools
    EarlyTokenAccess,  // Early access to new token launches
    PremiumFeatures,   // Access to premium platform features
    ExclusiveEvents,   // Access to exclusive events/airdrops
    ReducedFees,       // Reduced trading fees
    PrioritySupport,   // Priority customer support
    AdvancedAnalytics, // Advanced analytics and data
}

/// Specific benefits provided by NFT access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessBenefit {
    pub benefit_type: String,
    pub description: String,
    pub value: Decimal, // Quantified value (e.g., % discount, priority level)
}

impl NftAccessLevel {
    /// Create a new NFT access level
    pub fn new(collection_id: Uuid, required_rarity: NftRarity, access_type: AccessType) -> Self {
        let benefits = match access_type {
            AccessType::ReducedFees => vec![AccessBenefit {
                benefit_type: "fee_discount".to_string(),
                description: "Trading fee discount".to_string(),
                value: required_rarity.fee_discount(),
            }],
            AccessType::PrioritySupport => vec![AccessBenefit {
                benefit_type: "priority_level".to_string(),
                description: "Support priority level".to_string(),
                value: Decimal::from(required_rarity.priority_level()),
            }],
            _ => vec![],
        };

        Self {
            collection_id,
            required_rarity,
            access_type,
            benefits,
        }
    }

    /// Check if a user has the required NFT
    pub fn has_access(&self, user_nfts: &[NftOwnership]) -> bool {
        user_nfts.iter().any(|nft| {
            nft.collection_id == self.collection_id
                && nft.rarity.priority_level() >= self.required_rarity.priority_level()
                && nft.verified
        })
    }
}

/// NFT verification system
#[derive(Debug, Clone)]
pub struct NftVerifier {
    verified_collections: HashMap<Uuid, NftCollection>,
}

impl NftVerifier {
    /// Create a new NFT verifier
    pub fn new() -> Self {
        Self {
            verified_collections: HashMap::new(),
        }
    }

    /// Add a verified collection
    pub fn add_verified_collection(&mut self, collection: NftCollection) {
        if collection.verified {
            self.verified_collections.insert(collection.id, collection);
        }
    }

    /// Verify NFT ownership
    pub async fn verify_ownership(
        &self,
        collection_id: Uuid,
        token_id: &str,
        owner_address: &str,
    ) -> Result<bool, CoreError> {
        // In a real implementation, this would:
        // 1. Check if collection is verified
        // 2. Query the blockchain to verify ownership
        // 3. Return the verification result

        if !self.verified_collections.contains_key(&collection_id) {
            return Err(CoreError::Validation("Collection not verified".to_string()));
        }

        // Simplified verification for demonstration
        // In production, this would make actual blockchain queries
        Ok(!owner_address.is_empty() && !token_id.is_empty())
    }

    /// Get collection by ID
    pub fn get_collection(&self, collection_id: &Uuid) -> Option<&NftCollection> {
        self.verified_collections.get(collection_id)
    }
}

impl Default for NftVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// NFT marketplace integration models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftListing {
    pub id: Uuid,
    pub collection_id: Uuid,
    pub token_id: String,
    pub seller_id: Uuid,
    pub price: Decimal,
    pub currency: String,
    pub status: ListingStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
    Expired,
}

impl NftListing {
    /// Create a new NFT listing
    pub fn new(
        collection_id: Uuid,
        token_id: String,
        seller_id: Uuid,
        price: Decimal,
        currency: String,
        duration_hours: Option<u64>,
    ) -> Self {
        let expires_at =
            duration_hours.map(|hours| chrono::Utc::now() + chrono::Duration::hours(hours as i64));

        Self {
            id: Uuid::new_v4(),
            collection_id,
            token_id,
            seller_id,
            price,
            currency,
            status: ListingStatus::Active,
            created_at: chrono::Utc::now(),
            expires_at,
        }
    }

    /// Check if listing is still active
    pub fn is_active(&self) -> bool {
        if self.status != ListingStatus::Active {
            return false;
        }

        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() < expires_at
        } else {
            true
        }
    }

    /// Mark listing as sold
    pub fn mark_sold(&mut self) {
        self.status = ListingStatus::Sold;
    }

    /// Cancel listing
    pub fn cancel(&mut self) {
        self.status = ListingStatus::Cancelled;
    }
}

/// NFT-based fee discount calculator
#[derive(Debug, Clone)]
pub struct NftFeeDiscountCalculator {
    access_levels: Vec<NftAccessLevel>,
}

impl NftFeeDiscountCalculator {
    /// Create a new calculator
    pub fn new() -> Self {
        Self {
            access_levels: Vec::new(),
        }
    }

    /// Add an access level
    pub fn add_access_level(&mut self, access_level: NftAccessLevel) {
        self.access_levels.push(access_level);
    }

    /// Calculate total fee discount for a user
    pub fn calculate_discount(&self, user_nfts: &[NftOwnership]) -> Decimal {
        let mut max_discount = Decimal::ZERO;

        for access_level in &self.access_levels {
            if access_level.access_type == AccessType::ReducedFees
                && access_level.has_access(user_nfts)
            {
                for benefit in &access_level.benefits {
                    if benefit.benefit_type == "fee_discount" {
                        max_discount = max_discount.max(benefit.value);
                    }
                }
            }
        }

        max_discount
    }

    /// Get all benefits available to a user
    pub fn get_user_benefits(&self, user_nfts: &[NftOwnership]) -> Vec<AccessBenefit> {
        let mut benefits = Vec::new();

        for access_level in &self.access_levels {
            if access_level.has_access(user_nfts) {
                benefits.extend(access_level.benefits.clone());
            }
        }

        // Deduplicate and keep highest values
        let mut unique_benefits: HashMap<String, AccessBenefit> = HashMap::new();
        for benefit in benefits {
            unique_benefits
                .entry(benefit.benefit_type.clone())
                .and_modify(|existing| {
                    if benefit.value > existing.value {
                        *existing = benefit.clone();
                    }
                })
                .or_insert(benefit);
        }

        unique_benefits.into_values().collect()
    }
}

impl Default for NftFeeDiscountCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Exclusive trading pool for NFT holders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftGatedPool {
    pub id: Uuid,
    pub name: String,
    pub required_collection: Uuid,
    pub required_rarity: NftRarity,
    pub token_pairs: Vec<(Uuid, Uuid)>,
    pub total_liquidity: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl NftGatedPool {
    /// Create a new NFT-gated pool
    pub fn new(name: String, required_collection: Uuid, required_rarity: NftRarity) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            required_collection,
            required_rarity,
            token_pairs: Vec::new(),
            total_liquidity: Decimal::ZERO,
            created_at: chrono::Utc::now(),
        }
    }

    /// Add a token pair to the pool
    pub fn add_token_pair(&mut self, token_a: Uuid, token_b: Uuid) {
        self.token_pairs.push((token_a, token_b));
    }

    /// Check if user can access this pool
    pub fn can_access(&self, user_nfts: &[NftOwnership]) -> bool {
        user_nfts.iter().any(|nft| {
            nft.collection_id == self.required_collection
                && nft.rarity.priority_level() >= self.required_rarity.priority_level()
                && nft.verified
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nft_rarity_discounts() {
        assert_eq!(NftRarity::Common.fee_discount(), Decimal::new(5, 2));
        assert_eq!(NftRarity::Legendary.fee_discount(), Decimal::new(30, 2));
        assert_eq!(NftRarity::Mythic.fee_discount(), Decimal::new(50, 2));
    }

    #[test]
    fn test_nft_rarity_priority() {
        assert!(NftRarity::Legendary.priority_level() > NftRarity::Rare.priority_level());
        assert_eq!(NftRarity::Mythic.priority_level(), 6);
    }

    #[test]
    fn test_nft_collection_creation() {
        let collection = NftCollection::new(
            "Test Collection".to_string(),
            "TEST".to_string(),
            "0x123".to_string(),
            "Ethereum".to_string(),
            10000,
        );

        assert_eq!(collection.name, "Test Collection");
        assert!(!collection.verified);
    }

    #[test]
    fn test_nft_collection_verification() {
        let mut collection = NftCollection::new(
            "Test".to_string(),
            "TEST".to_string(),
            "0x123".to_string(),
            "Ethereum".to_string(),
            1000,
        );

        assert!(!collection.verified);
        collection.verify();
        assert!(collection.verified);
    }

    #[test]
    fn test_nft_ownership() {
        let ownership = NftOwnership::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "1".to_string(),
            NftRarity::Rare,
        );

        assert_eq!(ownership.rarity, NftRarity::Rare);
        assert!(!ownership.verified);
    }

    #[test]
    fn test_access_level_benefits() {
        let collection_id = Uuid::new_v4();
        let access_level =
            NftAccessLevel::new(collection_id, NftRarity::Epic, AccessType::ReducedFees);

        assert_eq!(access_level.access_type, AccessType::ReducedFees);
        assert!(!access_level.benefits.is_empty());
    }

    #[test]
    fn test_has_access() {
        let collection_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let access_level =
            NftAccessLevel::new(collection_id, NftRarity::Rare, AccessType::PrivatePool);

        let mut nft = NftOwnership::new(user_id, collection_id, "1".to_string(), NftRarity::Epic);
        nft.verify();

        assert!(access_level.has_access(&[nft.clone()]));

        // Lower rarity should not grant access
        let mut nft_common =
            NftOwnership::new(user_id, collection_id, "2".to_string(), NftRarity::Common);
        nft_common.verify();
        assert!(!access_level.has_access(&[nft_common]));
    }

    #[test]
    fn test_nft_listing() {
        let listing = NftListing::new(
            Uuid::new_v4(),
            "1".to_string(),
            Uuid::new_v4(),
            Decimal::from(100),
            "BTC".to_string(),
            Some(24),
        );

        assert!(listing.is_active());
        assert_eq!(listing.status, ListingStatus::Active);
    }

    #[test]
    fn test_listing_expiration() {
        let mut listing = NftListing::new(
            Uuid::new_v4(),
            "1".to_string(),
            Uuid::new_v4(),
            Decimal::from(100),
            "BTC".to_string(),
            Some(0), // Expires immediately
        );

        // Should be expired
        assert!(!listing.is_active());

        listing.status = ListingStatus::Expired;
        assert!(!listing.is_active());
    }

    #[test]
    fn test_fee_discount_calculator() {
        let collection_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let mut calculator = NftFeeDiscountCalculator::new();
        let access_level =
            NftAccessLevel::new(collection_id, NftRarity::Epic, AccessType::ReducedFees);
        calculator.add_access_level(access_level);

        let mut nft = NftOwnership::new(user_id, collection_id, "1".to_string(), NftRarity::Epic);
        nft.verify();

        let discount = calculator.calculate_discount(&[nft]);
        assert_eq!(discount, NftRarity::Epic.fee_discount());
    }

    #[test]
    fn test_nft_gated_pool() {
        let collection_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let mut pool = NftGatedPool::new(
            "Exclusive Pool".to_string(),
            collection_id,
            NftRarity::Legendary,
        );

        pool.add_token_pair(Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(pool.token_pairs.len(), 1);

        let mut nft = NftOwnership::new(
            user_id,
            collection_id,
            "1".to_string(),
            NftRarity::Legendary,
        );
        nft.verify();

        assert!(pool.can_access(&[nft]));
    }

    #[tokio::test]
    async fn test_nft_verifier() {
        let mut verifier = NftVerifier::new();
        let mut collection = NftCollection::new(
            "Test".to_string(),
            "TEST".to_string(),
            "0x123".to_string(),
            "Ethereum".to_string(),
            1000,
        );
        collection.verify();

        let collection_id = collection.id;
        verifier.add_verified_collection(collection);

        let result = verifier.verify_ownership(collection_id, "1", "0xabc").await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

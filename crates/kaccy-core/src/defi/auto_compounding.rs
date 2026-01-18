//! Auto-compounding staking implementation
//!
//! Automatically reinvests staking rewards to maximize returns through compound interest.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Auto-compounding staking vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCompoundingVault {
    /// Vault ID
    pub id: Uuid,
    /// Token ID being staked
    pub token_id: Uuid,
    /// Total staked amount (including compounded rewards)
    pub total_staked: Decimal,
    /// Base APR (annual percentage rate)
    pub base_apr: Decimal,
    /// Compound frequency (times per year)
    pub compounds_per_year: u32,
    /// Effective APY (including compounding)
    pub effective_apy: Decimal,
    /// Last compound time
    pub last_compound_time: DateTime<Utc>,
    /// Total rewards distributed
    pub total_rewards_distributed: Decimal,
    /// Minimum gas cost for compounding
    pub min_gas_cost: Decimal,
    /// Created at
    pub created_at: DateTime<Utc>,
}

/// User's auto-compounding position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundingPosition {
    /// Position ID
    pub id: Uuid,
    /// Vault ID
    pub vault_id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// Principal amount (original deposit)
    pub principal: Decimal,
    /// Current value (including compounded rewards)
    pub current_value: Decimal,
    /// Total rewards earned
    pub total_rewards_earned: Decimal,
    /// Number of compounds
    pub compound_count: u64,
    /// Share of vault (percentage)
    pub share_percentage: Decimal,
    /// Staked at
    pub staked_at: DateTime<Utc>,
    /// Last compound time
    pub last_compound_at: DateTime<Utc>,
}

/// Compound event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundEvent {
    /// Event ID
    pub id: Uuid,
    /// Vault ID
    pub vault_id: Uuid,
    /// Reward amount compounded
    pub reward_amount: Decimal,
    /// Gas cost (if applicable)
    pub gas_cost: Decimal,
    /// Net reward (after gas)
    pub net_reward: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Compound strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompoundStrategy {
    /// Fixed frequency (e.g., daily, weekly)
    FixedFrequency,
    /// Threshold-based (compound when rewards exceed threshold)
    ThresholdBased,
    /// Gas-optimized (compound when gas cost is favorable)
    GasOptimized,
    /// Hybrid (combine multiple strategies)
    Hybrid,
}

impl AutoCompoundingVault {
    /// Create a new auto-compounding vault
    pub fn new(
        token_id: Uuid,
        base_apr: Decimal,
        compounds_per_year: u32,
        min_gas_cost: Decimal,
    ) -> Self {
        let effective_apy = Self::calculate_apy(base_apr, compounds_per_year);

        Self {
            id: Uuid::new_v4(),
            token_id,
            total_staked: Decimal::ZERO,
            base_apr,
            compounds_per_year,
            effective_apy,
            last_compound_time: Utc::now(),
            total_rewards_distributed: Decimal::ZERO,
            min_gas_cost,
            created_at: Utc::now(),
        }
    }

    /// Calculate APY from APR and compound frequency
    /// Formula: APY = (1 + APR/n)^n - 1
    pub fn calculate_apy(apr: Decimal, compounds_per_year: u32) -> Decimal {
        if compounds_per_year == 0 {
            return apr;
        }

        let rate_per_period = apr / Decimal::from(compounds_per_year) / dec!(100);
        let mut result = dec!(1.0);

        for _ in 0..compounds_per_year {
            result *= dec!(1.0) + rate_per_period;
        }

        (result - dec!(1.0)) * dec!(100)
    }

    /// Stake and create a compounding position
    pub fn stake(&mut self, user_id: Uuid, amount: Decimal) -> Result<CompoundingPosition> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Stake amount must be positive".to_string(),
            ));
        }

        let now = Utc::now();
        self.total_staked += amount;

        // Calculate share percentage
        let share_percentage = if self.total_staked > Decimal::ZERO {
            (amount / self.total_staked) * dec!(100)
        } else {
            dec!(100)
        };

        Ok(CompoundingPosition {
            id: Uuid::new_v4(),
            vault_id: self.id,
            user_id,
            principal: amount,
            current_value: amount,
            total_rewards_earned: Decimal::ZERO,
            compound_count: 0,
            share_percentage,
            staked_at: now,
            last_compound_at: now,
        })
    }

    /// Compound rewards for a position
    pub fn compound(
        &mut self,
        position: &mut CompoundingPosition,
        gas_cost: Decimal,
    ) -> Result<CompoundEvent> {
        // Calculate time since last compound
        let now = Utc::now();
        let time_diff = (now - position.last_compound_at).num_seconds() as f64;
        let year_in_seconds = 365.25 * 24.0 * 3600.0;
        let time_fraction =
            Decimal::from_f64_retain(time_diff / year_in_seconds).unwrap_or(Decimal::ZERO);

        // Calculate rewards: current_value * (APR / 100) * time_fraction
        let reward_amount = position.current_value * (self.base_apr / dec!(100)) * time_fraction;

        // Check if reward is worth compounding (gas cost consideration)
        let net_reward = reward_amount - gas_cost;
        if net_reward <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Reward too small to cover gas cost".to_string(),
            ));
        }

        // Add reward to current value (compounding)
        position.current_value += net_reward;
        position.total_rewards_earned += net_reward;
        position.compound_count += 1;
        position.last_compound_at = now;

        // Update vault
        self.total_staked += net_reward;
        self.total_rewards_distributed += reward_amount;
        self.last_compound_time = now;

        Ok(CompoundEvent {
            id: Uuid::new_v4(),
            vault_id: self.id,
            reward_amount,
            gas_cost,
            net_reward,
            timestamp: now,
        })
    }

    /// Calculate optimal compound frequency based on gas costs
    pub fn calculate_optimal_frequency(&self, position_size: Decimal, gas_price: Decimal) -> u32 {
        // Simplified model: compound when rewards > 2x gas cost
        let target_reward = gas_price * dec!(2);
        let time_to_target =
            target_reward / (position_size * self.base_apr / dec!(100) / dec!(365));

        // Convert to compounds per year
        let days_between_compounds = time_to_target.max(dec!(1));
        let compounds_per_year = (dec!(365) / days_between_compounds).to_u32().unwrap_or(365);

        compounds_per_year.clamp(1, 365)
    }

    /// Withdraw from position
    pub fn withdraw(
        &mut self,
        position: &mut CompoundingPosition,
        amount: Decimal,
    ) -> Result<Decimal> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Withdrawal amount must be positive".to_string(),
            ));
        }

        if amount > position.current_value {
            return Err(CoreError::InsufficientBalance {
                required: amount,
                available: position.current_value,
            });
        }

        position.current_value -= amount;
        self.total_staked -= amount;

        Ok(amount)
    }

    /// Get vault statistics
    pub fn get_stats(&self) -> VaultStats {
        VaultStats {
            total_staked: self.total_staked,
            base_apr: self.base_apr,
            effective_apy: self.effective_apy,
            compounds_per_year: self.compounds_per_year,
            total_rewards_distributed: self.total_rewards_distributed,
        }
    }
}

/// Vault statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    pub total_staked: Decimal,
    pub base_apr: Decimal,
    pub effective_apy: Decimal,
    pub compounds_per_year: u32,
    pub total_rewards_distributed: Decimal,
}

/// Manager for auto-compounding vaults
#[derive(Debug, Clone, Default)]
pub struct AutoCompoundingManager {
    vaults: HashMap<Uuid, AutoCompoundingVault>,
    positions: HashMap<Uuid, Vec<CompoundingPosition>>,
    compound_events: Vec<CompoundEvent>,
}

impl AutoCompoundingManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a vault
    pub fn create_vault(
        &mut self,
        token_id: Uuid,
        base_apr: Decimal,
        compounds_per_year: u32,
        min_gas_cost: Decimal,
    ) -> Uuid {
        let vault = AutoCompoundingVault::new(token_id, base_apr, compounds_per_year, min_gas_cost);
        let vault_id = vault.id;
        self.vaults.insert(vault_id, vault);
        vault_id
    }

    /// Stake in a vault
    pub fn stake(
        &mut self,
        vault_id: Uuid,
        user_id: Uuid,
        amount: Decimal,
    ) -> Result<CompoundingPosition> {
        let vault = self
            .vaults
            .get_mut(&vault_id)
            .ok_or(CoreError::NotFound("Vault not found".to_string()))?;

        let position = vault.stake(user_id, amount)?;
        self.positions
            .entry(user_id)
            .or_default()
            .push(position.clone());

        Ok(position)
    }

    /// Compound rewards for a user's position
    pub fn compound_position(
        &mut self,
        vault_id: Uuid,
        user_id: Uuid,
        position_id: Uuid,
        gas_cost: Decimal,
    ) -> Result<CompoundEvent> {
        let vault = self
            .vaults
            .get_mut(&vault_id)
            .ok_or(CoreError::NotFound("Vault not found".to_string()))?;

        let user_positions = self
            .positions
            .get_mut(&user_id)
            .ok_or(CoreError::NotFound("No positions found".to_string()))?;

        let position = user_positions
            .iter_mut()
            .find(|p| p.id == position_id)
            .ok_or(CoreError::NotFound("Position not found".to_string()))?;

        let event = vault.compound(position, gas_cost)?;
        self.compound_events.push(event.clone());

        Ok(event)
    }

    /// Compound all positions in a vault (batch operation)
    pub fn compound_all(
        &mut self,
        vault_id: Uuid,
        gas_cost_per_position: Decimal,
    ) -> Result<Vec<CompoundEvent>> {
        let mut events = Vec::new();

        // Collect all user IDs that have positions in this vault
        let user_ids: Vec<Uuid> = self
            .positions
            .iter()
            .filter_map(|(user_id, positions)| {
                if positions.iter().any(|p| p.vault_id == vault_id) {
                    Some(*user_id)
                } else {
                    None
                }
            })
            .collect();

        for user_id in user_ids {
            let position_ids: Vec<Uuid> = self
                .positions
                .get(&user_id)
                .map(|positions| {
                    positions
                        .iter()
                        .filter(|p| p.vault_id == vault_id)
                        .map(|p| p.id)
                        .collect()
                })
                .unwrap_or_default();

            for position_id in position_ids {
                if let Ok(event) =
                    self.compound_position(vault_id, user_id, position_id, gas_cost_per_position)
                {
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    /// Get user's positions
    pub fn get_user_positions(&self, user_id: Uuid) -> Vec<&CompoundingPosition> {
        self.positions
            .get(&user_id)
            .map(|positions| positions.iter().collect())
            .unwrap_or_default()
    }

    /// Get vault
    pub fn get_vault(&self, vault_id: Uuid) -> Option<&AutoCompoundingVault> {
        self.vaults.get(&vault_id)
    }

    /// Calculate projected returns
    pub fn calculate_projection(
        &self,
        vault_id: Uuid,
        principal: Decimal,
        years: u32,
    ) -> Result<ProjectedReturns> {
        let vault = self
            .vaults
            .get(&vault_id)
            .ok_or(CoreError::NotFound("Vault not found".to_string()))?;

        let rate_per_period = vault.base_apr / Decimal::from(vault.compounds_per_year) / dec!(100);
        let total_periods = vault.compounds_per_year * years;

        let mut final_value = principal;
        for _ in 0..total_periods {
            final_value *= dec!(1.0) + rate_per_period;
        }

        let total_return = final_value - principal;
        let total_return_percentage = (total_return / principal) * dec!(100);

        Ok(ProjectedReturns {
            principal,
            final_value,
            total_return,
            total_return_percentage,
            years,
            effective_apy: vault.effective_apy,
        })
    }
}

/// Projected returns for a vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedReturns {
    pub principal: Decimal,
    pub final_value: Decimal,
    pub total_return: Decimal,
    pub total_return_percentage: Decimal,
    pub years: u32,
    pub effective_apy: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apy_calculation() {
        // 10% APR compounded daily should give ~10.52% APY
        let apy = AutoCompoundingVault::calculate_apy(dec!(10), 365);
        assert!(apy > dec!(10.5) && apy < dec!(10.6));
    }

    #[test]
    fn test_vault_creation() {
        let vault = AutoCompoundingVault::new(Uuid::new_v4(), dec!(10), 365, dec!(0.01));
        assert_eq!(vault.base_apr, dec!(10));
        assert_eq!(vault.compounds_per_year, 365);
        assert!(vault.effective_apy > dec!(10));
    }

    #[test]
    fn test_stake_and_compound() {
        let mut vault = AutoCompoundingVault::new(Uuid::new_v4(), dec!(10), 365, dec!(0.01));
        let user_id = Uuid::new_v4();

        // Stake
        let mut position = vault.stake(user_id, dec!(1000)).unwrap();
        assert_eq!(position.principal, dec!(1000));
        assert_eq!(position.current_value, dec!(1000));

        // Simulate time passing by setting last_compound_at to 1 day ago
        position.last_compound_at = chrono::Utc::now() - chrono::Duration::days(1);
        let event = vault.compound(&mut position, dec!(0.01)).unwrap();

        // Should have earned some rewards (approx 1000 * 10% / 365 = ~0.27)
        assert!(event.net_reward > Decimal::ZERO);
        assert!(position.current_value > dec!(1000));
        assert_eq!(position.compound_count, 1);
    }

    #[test]
    fn test_gas_cost_prevents_small_compounds() {
        let mut vault = AutoCompoundingVault::new(Uuid::new_v4(), dec!(10), 365, dec!(0.01));
        let user_id = Uuid::new_v4();
        let mut position = vault.stake(user_id, dec!(10)).unwrap(); // Small amount

        // Try to compound immediately with high gas cost
        let result = vault.compound(&mut position, dec!(10));
        assert!(result.is_err()); // Should fail due to gas cost
    }

    #[test]
    fn test_optimal_frequency_calculation() {
        let vault = AutoCompoundingVault::new(Uuid::new_v4(), dec!(10), 365, dec!(0.01));

        // For a very large position with low gas, very frequent compounding is optimal
        let frequency_large = vault.calculate_optimal_frequency(dec!(1000000), dec!(0.01));

        // For a smaller position with same gas, less frequent compounding is optimal
        let frequency_medium = vault.calculate_optimal_frequency(dec!(10000), dec!(0.01));

        // For a very small position, even less frequent
        let frequency_small = vault.calculate_optimal_frequency(dec!(100), dec!(0.01));

        // Verify the relationship: larger positions should compound more frequently
        assert!(frequency_large >= frequency_medium);
        assert!(frequency_medium >= frequency_small);
    }

    #[test]
    fn test_manager_operations() {
        let mut manager = AutoCompoundingManager::new();
        let token_id = Uuid::new_v4();

        // Create vault
        let vault_id = manager.create_vault(token_id, dec!(10), 365, dec!(0.01));

        // Stake
        let user_id = Uuid::new_v4();
        let _position = manager.stake(vault_id, user_id, dec!(1000)).unwrap();

        // Check positions
        let positions = manager.get_user_positions(user_id);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].principal, dec!(1000));
    }

    #[test]
    fn test_projection_calculation() {
        let mut manager = AutoCompoundingManager::new();
        let vault_id = manager.create_vault(Uuid::new_v4(), dec!(10), 365, dec!(0.01));

        // Project 5 years
        let projection = manager
            .calculate_projection(vault_id, dec!(1000), 5)
            .unwrap();

        assert_eq!(projection.principal, dec!(1000));
        assert!(projection.final_value > dec!(1600)); // Should be ~1648 with 10% APR compounded
        assert!(projection.total_return_percentage > dec!(60));
    }
}

//! Flash loan system
//!
//! Flash loans are uncollateralized loans that must be borrowed and repaid within
//! a single transaction. This is a key DeFi primitive that enables:
//!
//! - Arbitrage opportunities
//! - Collateral swaps
//! - Debt refinancing
//! - Liquidations
//!
//! # How It Works
//!
//! 1. **Borrow**: User borrows tokens from the flash loan pool
//! 2. **Execute**: User executes custom logic with the borrowed funds
//! 3. **Repay**: User repays the loan plus fee in the same transaction
//! 4. **Validation**: If repayment fails, the entire transaction reverts
//!
//! # Security
//!
//! - Reentrancy protection
//! - Maximum borrow limits
//! - Fee validation
//! - Balance verification

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Flash loan configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanConfig {
    /// Flash loan fee (as percentage, e.g., 0.09 for 0.09%)
    pub fee_percentage: Decimal,
    /// Maximum amount that can be borrowed in a single flash loan
    pub max_loan_amount: Decimal,
    /// Minimum amount for a flash loan
    pub min_loan_amount: Decimal,
    /// Whether flash loans are enabled
    pub enabled: bool,
}

impl Default for FlashLoanConfig {
    fn default() -> Self {
        Self {
            fee_percentage: dec!(0.09),       // 0.09% standard fee
            max_loan_amount: dec!(1000000.0), // 1M tokens max
            min_loan_amount: dec!(1.0),
            enabled: true,
        }
    }
}

/// Flash loan request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanRequest {
    /// Unique identifier for this loan
    pub loan_id: Uuid,
    /// User who initiated the loan
    pub borrower_id: Uuid,
    /// Token being borrowed
    pub token_id: Uuid,
    /// Amount to borrow
    pub amount: Decimal,
    /// Calculated fee
    pub fee: Decimal,
    /// Total amount to repay (amount + fee)
    pub repay_amount: Decimal,
    /// Timestamp when the loan was initiated
    pub timestamp: DateTime<Utc>,
    /// Status of the loan
    pub status: FlashLoanStatus,
}

/// Flash loan status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashLoanStatus {
    /// Loan initiated
    Initiated,
    /// Loan executed successfully
    Executed,
    /// Loan repaid successfully
    Repaid,
    /// Loan failed (transaction should revert)
    Failed,
}

/// Flash loan execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanResult {
    /// Identifier of the loan this result belongs to
    pub loan_id: Uuid,
    /// Whether the loan was repaid successfully
    pub success: bool,
    /// Amount that was borrowed
    pub amount_borrowed: Decimal,
    /// Fee paid to the pool
    pub fee_paid: Decimal,
    /// Net profit after repayment, if successful
    pub profit: Option<Decimal>,
    /// Error message if the loan failed
    pub error: Option<String>,
}

/// Flash loan executor interface
///
/// Implementers of this trait can execute custom logic with borrowed funds
pub trait FlashLoanExecutor {
    /// Execute custom logic with borrowed funds
    ///
    /// # Arguments
    ///
    /// * `token_id` - The token being borrowed
    /// * `amount` - The amount borrowed
    /// * `fee` - The fee that must be paid
    /// * `params` - Custom parameters for the execution
    ///
    /// # Returns
    ///
    /// Returns Ok(true) if execution was successful and loan can be repaid,
    /// Ok(false) if execution failed, or Err if there was an error
    fn execute(
        &mut self,
        token_id: Uuid,
        amount: Decimal,
        fee: Decimal,
        params: &[u8],
    ) -> Result<bool>;
}

/// Flash loan pool
///
/// Manages available liquidity for flash loans
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanPool {
    /// Pool ID
    pub pool_id: Uuid,
    /// Token ID
    pub token_id: Uuid,
    /// Available liquidity
    pub available_liquidity: Decimal,
    /// Total amount borrowed (lifetime)
    pub total_borrowed: Decimal,
    /// Total fees collected
    pub total_fees_collected: Decimal,
    /// Number of successful loans
    pub successful_loans: u64,
    /// Number of failed loans
    pub failed_loans: u64,
    /// Configuration
    pub config: FlashLoanConfig,
    /// Active loans keyed by borrower ID (for reentrancy protection)
    active_loans: HashMap<Uuid, FlashLoanRequest>,
}

impl FlashLoanPool {
    /// Create a new flash loan pool
    pub fn new(token_id: Uuid, initial_liquidity: Decimal, config: FlashLoanConfig) -> Self {
        Self {
            pool_id: Uuid::new_v4(),
            token_id,
            available_liquidity: initial_liquidity,
            total_borrowed: Decimal::ZERO,
            total_fees_collected: Decimal::ZERO,
            successful_loans: 0,
            failed_loans: 0,
            config,
            active_loans: HashMap::new(),
        }
    }

    /// Add liquidity to the pool
    pub fn add_liquidity(&mut self, amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        self.available_liquidity += amount;
        Ok(())
    }

    /// Remove liquidity from the pool
    pub fn remove_liquidity(&mut self, amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        if amount > self.available_liquidity {
            return Err(CoreError::InsufficientLiquidity(
                "Insufficient liquidity in pool".to_string(),
            ));
        }

        self.available_liquidity -= amount;
        Ok(())
    }

    /// Calculate flash loan fee
    pub fn calculate_fee(&self, amount: Decimal) -> Decimal {
        amount * self.config.fee_percentage / dec!(100.0)
    }

    /// Initiate a flash loan
    ///
    /// This is the first step of the flash loan process
    pub fn initiate_loan(
        &mut self,
        borrower_id: Uuid,
        amount: Decimal,
    ) -> Result<FlashLoanRequest> {
        // Check if flash loans are enabled
        if !self.config.enabled {
            return Err(CoreError::Validation(
                "Flash loans are disabled".to_string(),
            ));
        }

        // Validate amount
        if amount < self.config.min_loan_amount {
            return Err(CoreError::Validation(format!(
                "Amount below minimum: {}",
                self.config.min_loan_amount
            )));
        }

        if amount > self.config.max_loan_amount {
            return Err(CoreError::Validation(format!(
                "Amount exceeds maximum: {}",
                self.config.max_loan_amount
            )));
        }

        // Check available liquidity
        if amount > self.available_liquidity {
            return Err(CoreError::InsufficientLiquidity(
                "Insufficient liquidity for flash loan".to_string(),
            ));
        }

        // Check for reentrancy
        if self.active_loans.contains_key(&borrower_id) {
            return Err(CoreError::Validation(
                "Reentrancy detected: borrower has active loan".to_string(),
            ));
        }

        // Calculate fee
        let fee = self.calculate_fee(amount);
        let repay_amount = amount + fee;

        let loan = FlashLoanRequest {
            loan_id: Uuid::new_v4(),
            borrower_id,
            token_id: self.token_id,
            amount,
            fee,
            repay_amount,
            timestamp: Utc::now(),
            status: FlashLoanStatus::Initiated,
        };

        // Mark loan as active (reentrancy protection)
        self.active_loans.insert(borrower_id, loan.clone());

        // Decrease available liquidity
        self.available_liquidity -= amount;

        Ok(loan)
    }

    /// Complete a flash loan (repayment)
    ///
    /// This is the final step of the flash loan process
    pub fn complete_loan(
        &mut self,
        loan_id: Uuid,
        borrower_id: Uuid,
        repaid_amount: Decimal,
    ) -> Result<FlashLoanResult> {
        // Get active loan and clone it
        let loan = self
            .active_loans
            .get(&borrower_id)
            .ok_or_else(|| CoreError::NotFound("No active loan found".to_string()))?
            .clone();

        // Verify loan ID
        if loan.loan_id != loan_id {
            return Err(CoreError::Validation("Loan ID mismatch".to_string()));
        }

        // Verify repayment amount
        if repaid_amount < loan.repay_amount {
            // Insufficient repayment - loan failed
            self.active_loans.remove(&borrower_id);
            self.available_liquidity += loan.amount; // Return borrowed amount
            self.failed_loans += 1;

            return Ok(FlashLoanResult {
                loan_id,
                success: false,
                amount_borrowed: loan.amount,
                fee_paid: Decimal::ZERO,
                profit: None,
                error: Some("Insufficient repayment".to_string()),
            });
        }

        // Loan successful
        self.active_loans.remove(&borrower_id);
        self.available_liquidity += loan.repay_amount;
        self.total_borrowed += loan.amount;
        self.total_fees_collected += loan.fee;
        self.successful_loans += 1;

        Ok(FlashLoanResult {
            loan_id,
            success: true,
            amount_borrowed: loan.amount,
            fee_paid: loan.fee,
            profit: Some(repaid_amount - loan.repay_amount),
            error: None,
        })
    }

    /// Cancel an active loan (used when execution fails)
    pub fn cancel_loan(&mut self, borrower_id: Uuid) -> Result<()> {
        if let Some(loan) = self.active_loans.remove(&borrower_id) {
            // Return borrowed amount to pool
            self.available_liquidity += loan.amount;
            self.failed_loans += 1;
            Ok(())
        } else {
            Err(CoreError::NotFound("No active loan found".to_string()))
        }
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> FlashLoanPoolStats {
        let total_loans = self.successful_loans + self.failed_loans;
        let success_rate = if total_loans > 0 {
            (self.successful_loans as f64 / total_loans as f64) * 100.0
        } else {
            0.0
        };

        FlashLoanPoolStats {
            pool_id: self.pool_id,
            token_id: self.token_id,
            available_liquidity: self.available_liquidity,
            total_borrowed: self.total_borrowed,
            total_fees_collected: self.total_fees_collected,
            successful_loans: self.successful_loans,
            failed_loans: self.failed_loans,
            success_rate,
            active_loan_count: self.active_loans.len() as u64,
        }
    }
}

/// Flash loan pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashLoanPoolStats {
    /// Pool identifier
    pub pool_id: Uuid,
    /// Token this pool lends
    pub token_id: Uuid,
    /// Currently available liquidity
    pub available_liquidity: Decimal,
    /// Cumulative lifetime borrowed amount
    pub total_borrowed: Decimal,
    /// Cumulative fees collected
    pub total_fees_collected: Decimal,
    /// Number of successfully completed loans
    pub successful_loans: u64,
    /// Number of failed loans
    pub failed_loans: u64,
    /// Percentage of loans that succeeded
    pub success_rate: f64,
    /// Number of currently in-flight loans
    pub active_loan_count: u64,
}

/// Flash loan manager for multiple tokens
#[derive(Debug)]
pub struct FlashLoanManager {
    /// Pools keyed by token ID
    pools: HashMap<Uuid, FlashLoanPool>,
}

impl FlashLoanManager {
    /// Create a new flash loan manager
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Create a pool for a token
    pub fn create_pool(
        &mut self,
        token_id: Uuid,
        initial_liquidity: Decimal,
        config: FlashLoanConfig,
    ) -> Result<Uuid> {
        if self.pools.contains_key(&token_id) {
            return Err(CoreError::Validation(
                "Pool already exists for this token".to_string(),
            ));
        }

        let pool = FlashLoanPool::new(token_id, initial_liquidity, config);
        let pool_id = pool.pool_id;
        self.pools.insert(token_id, pool);
        Ok(pool_id)
    }

    /// Get pool for a token
    pub fn get_pool(&self, token_id: Uuid) -> Result<&FlashLoanPool> {
        self.pools
            .get(&token_id)
            .ok_or_else(|| CoreError::NotFound("Pool not found".to_string()))
    }

    /// Get mutable pool for a token
    pub fn get_pool_mut(&mut self, token_id: Uuid) -> Result<&mut FlashLoanPool> {
        self.pools
            .get_mut(&token_id)
            .ok_or_else(|| CoreError::NotFound("Pool not found".to_string()))
    }

    /// Execute a flash loan with custom executor
    #[allow(clippy::too_many_arguments)]
    pub fn execute_flash_loan(
        &mut self,
        token_id: Uuid,
        borrower_id: Uuid,
        amount: Decimal,
        executor: &mut dyn FlashLoanExecutor,
        params: &[u8],
    ) -> Result<FlashLoanResult> {
        // Step 1: Initiate the loan
        let pool = self.get_pool_mut(token_id)?;
        let loan = pool.initiate_loan(borrower_id, amount)?;

        // Step 2: Execute custom logic
        let execution_result = executor.execute(token_id, amount, loan.fee, params);

        match execution_result {
            Ok(true) => {
                // Step 3: Complete the loan (repayment)
                let pool = self.get_pool_mut(token_id)?;
                pool.complete_loan(loan.loan_id, borrower_id, loan.repay_amount)
            }
            Ok(false) | Err(_) => {
                // Execution failed, cancel the loan
                let pool = self.get_pool_mut(token_id)?;
                pool.cancel_loan(borrower_id)?;

                Ok(FlashLoanResult {
                    loan_id: loan.loan_id,
                    success: false,
                    amount_borrowed: loan.amount,
                    fee_paid: Decimal::ZERO,
                    profit: None,
                    error: Some("Execution failed".to_string()),
                })
            }
        }
    }

    /// Get statistics for all pools
    pub fn get_all_stats(&self) -> Vec<FlashLoanPoolStats> {
        self.pools.values().map(|pool| pool.get_stats()).collect()
    }
}

impl Default for FlashLoanManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockExecutor {
        should_succeed: bool,
    }

    impl FlashLoanExecutor for MockExecutor {
        fn execute(
            &mut self,
            _token_id: Uuid,
            _amount: Decimal,
            _fee: Decimal,
            _params: &[u8],
        ) -> Result<bool> {
            Ok(self.should_succeed)
        }
    }

    #[test]
    fn test_flash_loan_pool_creation() {
        let token_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        assert_eq!(pool.token_id, token_id);
        assert_eq!(pool.available_liquidity, dec!(10000.0));
        assert_eq!(pool.successful_loans, 0);
    }

    #[test]
    fn test_add_liquidity() {
        let token_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let mut pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        pool.add_liquidity(dec!(5000.0)).unwrap();
        assert_eq!(pool.available_liquidity, dec!(15000.0));
    }

    #[test]
    fn test_calculate_fee() {
        let token_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        let fee = pool.calculate_fee(dec!(1000.0));
        assert_eq!(fee, dec!(0.9)); // 0.09% of 1000
    }

    #[test]
    fn test_successful_flash_loan() {
        let token_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let mut pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        // Initiate loan
        let loan = pool.initiate_loan(borrower_id, dec!(1000.0)).unwrap();
        assert_eq!(loan.amount, dec!(1000.0));
        assert_eq!(loan.fee, dec!(0.9));
        assert_eq!(pool.available_liquidity, dec!(9000.0));

        // Complete loan
        let result = pool
            .complete_loan(loan.loan_id, borrower_id, loan.repay_amount)
            .unwrap();
        assert!(result.success);
        assert_eq!(result.fee_paid, dec!(0.9));
        assert_eq!(pool.available_liquidity, dec!(10000.9));
        assert_eq!(pool.successful_loans, 1);
    }

    #[test]
    fn test_failed_flash_loan_insufficient_repayment() {
        let token_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let mut pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        let loan = pool.initiate_loan(borrower_id, dec!(1000.0)).unwrap();

        // Try to repay less than required
        let result = pool
            .complete_loan(loan.loan_id, borrower_id, dec!(1000.0))
            .unwrap();
        assert!(!result.success);
        assert_eq!(pool.failed_loans, 1);
        assert_eq!(pool.available_liquidity, dec!(10000.0)); // Liquidity restored
    }

    #[test]
    fn test_reentrancy_protection() {
        let token_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let mut pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        pool.initiate_loan(borrower_id, dec!(1000.0)).unwrap();

        // Try to initiate another loan while first is active
        let result = pool.initiate_loan(borrower_id, dec!(500.0));
        assert!(result.is_err());
    }

    #[test]
    fn test_flash_loan_manager() {
        let token_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let mut manager = FlashLoanManager::new();

        // Create pool
        manager
            .create_pool(token_id, dec!(10000.0), FlashLoanConfig::default())
            .unwrap();

        // Execute flash loan with mock executor
        let mut executor = MockExecutor {
            should_succeed: true,
        };
        let result = manager
            .execute_flash_loan(token_id, borrower_id, dec!(1000.0), &mut executor, &[])
            .unwrap();

        assert!(result.success);
        assert_eq!(result.amount_borrowed, dec!(1000.0));
        assert_eq!(result.fee_paid, dec!(0.9));
    }

    #[test]
    fn test_flash_loan_execution_failure() {
        let token_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let mut manager = FlashLoanManager::new();

        manager
            .create_pool(token_id, dec!(10000.0), FlashLoanConfig::default())
            .unwrap();

        // Execute flash loan with failing executor
        let mut executor = MockExecutor {
            should_succeed: false,
        };
        let result = manager
            .execute_flash_loan(token_id, borrower_id, dec!(1000.0), &mut executor, &[])
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.fee_paid, Decimal::ZERO);

        // Verify liquidity is restored
        let pool = manager.get_pool(token_id).unwrap();
        assert_eq!(pool.available_liquidity, dec!(10000.0));
    }

    #[test]
    fn test_pool_stats() {
        let token_id = Uuid::new_v4();
        let borrower_id = Uuid::new_v4();
        let config = FlashLoanConfig::default();
        let mut pool = FlashLoanPool::new(token_id, dec!(10000.0), config);

        // Execute successful loan
        let loan = pool.initiate_loan(borrower_id, dec!(1000.0)).unwrap();
        pool.complete_loan(loan.loan_id, borrower_id, loan.repay_amount)
            .unwrap();

        let stats = pool.get_stats();
        assert_eq!(stats.successful_loans, 1);
        assert_eq!(stats.total_borrowed, dec!(1000.0));
        assert_eq!(stats.total_fees_collected, dec!(0.9));
        assert_eq!(stats.success_rate, 100.0);
    }
}

//! Institutional Features
//!
//! This module provides institutional-grade features including prime brokerage services,
//! algorithmic trading interfaces, and custodial services.

use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Prime brokerage account
///
/// Manages multiple sub-accounts under a single institutional account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimeBrokerageAccount {
    pub account_id: String,
    pub institution_name: String,
    pub sub_accounts: HashMap<String, SubAccount>,
    pub credit_line: CreditLine,
    pub created_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAccount {
    pub sub_account_id: String,
    pub account_name: String,
    pub balances: HashMap<String, Decimal>,
    pub trading_enabled: bool,
    pub withdrawal_enabled: bool,
    pub daily_limits: TradingLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditLine {
    pub total_limit: Decimal,
    pub used_amount: Decimal,
    pub interest_rate: Decimal,
    pub collateral_required: Decimal,
    pub margin_call_threshold: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingLimits {
    pub max_order_size: Decimal,
    pub daily_volume_limit: Decimal,
    pub daily_volume_used: Decimal,
    pub max_open_positions: usize,
}

impl PrimeBrokerageAccount {
    pub fn new(account_id: String, institution_name: String, credit_limit: Decimal) -> Self {
        Self {
            account_id,
            institution_name,
            sub_accounts: HashMap::new(),
            credit_line: CreditLine {
                total_limit: credit_limit,
                used_amount: Decimal::ZERO,
                interest_rate: Decimal::new(5, 2), // 0.05 = 5% annual
                collateral_required: Decimal::new(120, 2), // 120%
                margin_call_threshold: Decimal::new(110, 2), // 110%
            },
            created_at: SystemTime::now(),
        }
    }

    /// Adds a new sub-account
    pub fn add_sub_account(&mut self, sub_account: SubAccount) -> Result<(), CoreError> {
        if self.sub_accounts.contains_key(&sub_account.sub_account_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Sub-account {} already exists",
                sub_account.sub_account_id
            )));
        }

        self.sub_accounts
            .insert(sub_account.sub_account_id.clone(), sub_account);
        Ok(())
    }

    /// Gets consolidated balance across all sub-accounts
    pub fn get_consolidated_balance(&self, asset: &str) -> Decimal {
        self.sub_accounts
            .values()
            .filter_map(|sa| sa.balances.get(asset))
            .sum()
    }

    /// Checks if account has available credit
    pub fn has_available_credit(&self, required: Decimal) -> bool {
        let available = self.credit_line.total_limit - self.credit_line.used_amount;
        available >= required
    }

    /// Draws from credit line
    pub fn draw_credit(&mut self, amount: Decimal) -> Result<(), CoreError> {
        if !self.has_available_credit(amount) {
            return Err(CoreError::InsufficientBalance {
                required: amount,
                available: self.credit_line.total_limit - self.credit_line.used_amount,
            });
        }

        self.credit_line.used_amount += amount;
        Ok(())
    }

    /// Repays credit
    pub fn repay_credit(&mut self, amount: Decimal) -> Result<(), CoreError> {
        if amount > self.credit_line.used_amount {
            return Err(CoreError::Validation(
                "Repayment amount exceeds used credit".to_string(),
            ));
        }

        self.credit_line.used_amount -= amount;
        Ok(())
    }
}

/// Consolidated reporting for institutional accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidatedReport {
    pub account_id: String,
    pub report_date: SystemTime,
    pub total_assets: HashMap<String, Decimal>,
    pub total_liabilities: Decimal,
    pub net_asset_value: Decimal,
    pub period_pnl: Decimal,
    pub sub_account_summaries: Vec<SubAccountSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAccountSummary {
    pub sub_account_id: String,
    pub account_name: String,
    pub total_value: Decimal,
    pub period_pnl: Decimal,
    pub position_count: usize,
}

/// Prime brokerage manager
pub struct PrimeBrokerageManager {
    accounts: HashMap<String, PrimeBrokerageAccount>,
}

impl PrimeBrokerageManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    /// Creates a new prime brokerage account
    pub fn create_account(
        &mut self,
        account_id: String,
        institution_name: String,
        credit_limit: Decimal,
    ) -> Result<(), CoreError> {
        if self.accounts.contains_key(&account_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Account {} already exists",
                account_id
            )));
        }

        let account =
            PrimeBrokerageAccount::new(account_id.clone(), institution_name, credit_limit);
        self.accounts.insert(account_id, account);
        Ok(())
    }

    /// Gets an account
    pub fn get_account(&self, account_id: &str) -> Option<&PrimeBrokerageAccount> {
        self.accounts.get(account_id)
    }

    /// Gets a mutable account
    pub fn get_account_mut(&mut self, account_id: &str) -> Option<&mut PrimeBrokerageAccount> {
        self.accounts.get_mut(account_id)
    }

    /// Generates consolidated report
    pub fn generate_report(&self, account_id: &str) -> Result<ConsolidatedReport, CoreError> {
        let account = self
            .get_account(account_id)
            .ok_or_else(|| CoreError::NotFound(format!("Account {} not found", account_id)))?;

        let mut total_assets: HashMap<String, Decimal> = HashMap::new();
        let mut sub_account_summaries = Vec::new();

        for sub_account in account.sub_accounts.values() {
            let mut sub_total = Decimal::ZERO;

            for (asset, balance) in &sub_account.balances {
                *total_assets.entry(asset.clone()).or_insert(Decimal::ZERO) += *balance;
                sub_total += *balance; // Simplified - would need pricing in reality
            }

            sub_account_summaries.push(SubAccountSummary {
                sub_account_id: sub_account.sub_account_id.clone(),
                account_name: sub_account.account_name.clone(),
                total_value: sub_total,
                period_pnl: Decimal::ZERO, // Would calculate from historical data
                position_count: sub_account.balances.len(),
            });
        }

        let total_value: Decimal = total_assets.values().sum();
        let total_liabilities = account.credit_line.used_amount;
        let nav = total_value - total_liabilities;

        Ok(ConsolidatedReport {
            account_id: account_id.to_string(),
            report_date: SystemTime::now(),
            total_assets,
            total_liabilities,
            net_asset_value: nav,
            period_pnl: Decimal::ZERO, // Would calculate from historical data
            sub_account_summaries,
        })
    }
}

impl Default for PrimeBrokerageManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Algorithmic trading strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgoStrategy {
    pub strategy_id: String,
    pub strategy_name: String,
    pub strategy_type: StrategyType,
    pub parameters: HashMap<String, String>,
    pub risk_limits: RiskLimits,
    pub status: StrategyStatus,
    pub performance: StrategyPerformance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyType {
    MarketMaking,
    Arbitrage,
    TrendFollowing,
    MeanReversion,
    Statistical,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyStatus {
    Active,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: Decimal,
    pub max_daily_loss: Decimal,
    pub max_drawdown: Decimal,
    pub stop_loss_threshold: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub total_pnl: Decimal,
    pub daily_pnl: Decimal,
    pub win_rate: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown: Decimal,
    pub trade_count: usize,
}

/// Algorithmic trading framework
pub struct AlgoTradingFramework {
    strategies: HashMap<String, AlgoStrategy>,
    execution_log: Vec<StrategyExecution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyExecution {
    pub execution_id: String,
    pub strategy_id: String,
    pub timestamp: SystemTime,
    pub action: String,
    pub result: ExecutionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    Success { pnl: Decimal },
    Failure { reason: String },
}

impl AlgoTradingFramework {
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
            execution_log: Vec::new(),
        }
    }

    /// Registers a new strategy
    pub fn register_strategy(&mut self, strategy: AlgoStrategy) -> Result<(), CoreError> {
        if self.strategies.contains_key(&strategy.strategy_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Strategy {} already exists",
                strategy.strategy_id
            )));
        }

        self.strategies
            .insert(strategy.strategy_id.clone(), strategy);
        Ok(())
    }

    /// Starts a strategy
    pub fn start_strategy(&mut self, strategy_id: &str) -> Result<(), CoreError> {
        let strategy = self
            .strategies
            .get_mut(strategy_id)
            .ok_or_else(|| CoreError::NotFound(format!("Strategy {} not found", strategy_id)))?;

        strategy.status = StrategyStatus::Active;
        Ok(())
    }

    /// Stops a strategy
    pub fn stop_strategy(&mut self, strategy_id: &str) -> Result<(), CoreError> {
        let strategy = self
            .strategies
            .get_mut(strategy_id)
            .ok_or_else(|| CoreError::NotFound(format!("Strategy {} not found", strategy_id)))?;

        strategy.status = StrategyStatus::Stopped;
        Ok(())
    }

    /// Gets strategy performance
    pub fn get_performance(&self, strategy_id: &str) -> Option<&StrategyPerformance> {
        self.strategies.get(strategy_id).map(|s| &s.performance)
    }

    /// Records strategy execution
    pub fn record_execution(&mut self, execution: StrategyExecution) {
        self.execution_log.push(execution);

        // Keep only last 10000 executions
        if self.execution_log.len() > 10000 {
            self.execution_log.remove(0);
        }
    }
}

impl Default for AlgoTradingFramework {
    fn default() -> Self {
        Self::new()
    }
}

/// Custodial wallet system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodialWallet {
    pub wallet_id: String,
    pub wallet_type: WalletType,
    pub multi_sig_config: Option<MultiSigConfig>,
    pub balances: HashMap<String, Decimal>,
    pub status: WalletStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletType {
    /// Hot wallet for immediate access
    Hot,
    /// Cold wallet for secure storage
    Cold,
    /// Warm wallet - semi-offline
    Warm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletStatus {
    Active,
    Locked,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSigConfig {
    pub required_signatures: usize,
    pub total_signers: usize,
    pub signer_ids: Vec<String>,
}

/// Withdrawal request for custodial services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    pub request_id: String,
    pub wallet_id: String,
    pub asset: String,
    pub amount: Decimal,
    pub destination: String,
    pub requester_id: String,
    pub created_at: SystemTime,
    pub status: WithdrawalStatus,
    pub approvals: Vec<Approval>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalStatus {
    Pending,
    AwaitingApproval,
    Approved,
    Rejected,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub approver_id: String,
    pub approved_at: SystemTime,
    pub signature: String,
}

/// Custodial services manager
pub struct CustodialManager {
    wallets: HashMap<String, CustodialWallet>,
    withdrawal_requests: HashMap<String, WithdrawalRequest>,
    #[allow(dead_code)]
    approval_workflows: HashMap<String, ApprovalWorkflow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalWorkflow {
    pub workflow_id: String,
    pub required_approvals: usize,
    pub approvers: Vec<String>,
    pub auto_approve_threshold: Option<Decimal>,
}

impl CustodialManager {
    pub fn new() -> Self {
        Self {
            wallets: HashMap::new(),
            withdrawal_requests: HashMap::new(),
            approval_workflows: HashMap::new(),
        }
    }

    /// Creates a new custodial wallet
    pub fn create_wallet(
        &mut self,
        wallet_id: String,
        wallet_type: WalletType,
        multi_sig_config: Option<MultiSigConfig>,
    ) -> Result<(), CoreError> {
        if self.wallets.contains_key(&wallet_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Wallet {} already exists",
                wallet_id
            )));
        }

        let wallet = CustodialWallet {
            wallet_id: wallet_id.clone(),
            wallet_type,
            multi_sig_config,
            balances: HashMap::new(),
            status: WalletStatus::Active,
        };

        self.wallets.insert(wallet_id, wallet);
        Ok(())
    }

    /// Submits a withdrawal request
    pub fn submit_withdrawal(&mut self, request: WithdrawalRequest) -> Result<(), CoreError> {
        // Validate wallet exists and is active
        let wallet = self.wallets.get(&request.wallet_id).ok_or_else(|| {
            CoreError::NotFound(format!("Wallet {} not found", request.wallet_id))
        })?;

        if wallet.status != WalletStatus::Active {
            return Err(CoreError::InvalidState("Wallet not active".to_string()));
        }

        // Check balance
        let balance = wallet
            .balances
            .get(&request.asset)
            .copied()
            .unwrap_or(Decimal::ZERO);

        if balance < request.amount {
            return Err(CoreError::InsufficientBalance {
                required: request.amount,
                available: balance,
            });
        }

        self.withdrawal_requests
            .insert(request.request_id.clone(), request);
        Ok(())
    }

    /// Approves a withdrawal request
    pub fn approve_withdrawal(
        &mut self,
        request_id: &str,
        approver_id: String,
        signature: String,
    ) -> Result<(), CoreError> {
        let request = self
            .withdrawal_requests
            .get_mut(request_id)
            .ok_or_else(|| CoreError::NotFound(format!("Request {} not found", request_id)))?;

        if request.status != WithdrawalStatus::Pending
            && request.status != WithdrawalStatus::AwaitingApproval
        {
            return Err(CoreError::InvalidState(
                "Request not in approvable state".to_string(),
            ));
        }

        let approval = Approval {
            approver_id,
            approved_at: SystemTime::now(),
            signature,
        };

        request.approvals.push(approval);
        request.status = WithdrawalStatus::AwaitingApproval;

        // Check if we have enough approvals
        let wallet = self.wallets.get(&request.wallet_id).unwrap();
        if let Some(multi_sig) = &wallet.multi_sig_config {
            if request.approvals.len() >= multi_sig.required_signatures {
                request.status = WithdrawalStatus::Approved;
            }
        } else {
            // Single-sig wallet
            request.status = WithdrawalStatus::Approved;
        }

        Ok(())
    }

    /// Gets pending withdrawal requests for a wallet
    pub fn get_pending_withdrawals(&self, wallet_id: &str) -> Vec<&WithdrawalRequest> {
        self.withdrawal_requests
            .values()
            .filter(|r| r.wallet_id == wallet_id && r.status == WithdrawalStatus::Pending)
            .collect()
    }
}

impl Default for CustodialManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prime_brokerage_account() {
        let mut account = PrimeBrokerageAccount::new(
            "PB001".to_string(),
            "Test Institution".to_string(),
            Decimal::new(1000000, 0),
        );

        let sub_account = SubAccount {
            sub_account_id: "SA001".to_string(),
            account_name: "Trading Desk A".to_string(),
            balances: HashMap::from([("BTC".to_string(), Decimal::new(10, 0))]),
            trading_enabled: true,
            withdrawal_enabled: true,
            daily_limits: TradingLimits {
                max_order_size: Decimal::new(100, 0),
                daily_volume_limit: Decimal::new(1000, 0),
                daily_volume_used: Decimal::ZERO,
                max_open_positions: 50,
            },
        };

        account.add_sub_account(sub_account).unwrap();
        assert_eq!(account.sub_accounts.len(), 1);

        let balance = account.get_consolidated_balance("BTC");
        assert_eq!(balance, Decimal::new(10, 0));
    }

    #[test]
    fn test_credit_line() {
        let mut account = PrimeBrokerageAccount::new(
            "PB001".to_string(),
            "Test Institution".to_string(),
            Decimal::new(1000, 0),
        );

        assert!(account.draw_credit(Decimal::new(500, 0)).is_ok());
        assert_eq!(account.credit_line.used_amount, Decimal::new(500, 0));

        assert!(account.draw_credit(Decimal::new(600, 0)).is_err()); // Exceeds limit

        assert!(account.repay_credit(Decimal::new(200, 0)).is_ok());
        assert_eq!(account.credit_line.used_amount, Decimal::new(300, 0));
    }

    #[test]
    fn test_algo_trading_framework() {
        let mut framework = AlgoTradingFramework::new();

        let strategy = AlgoStrategy {
            strategy_id: "STRAT001".to_string(),
            strategy_name: "Market Making Bot".to_string(),
            strategy_type: StrategyType::MarketMaking,
            parameters: HashMap::new(),
            risk_limits: RiskLimits {
                max_position_size: Decimal::new(100, 0),
                max_daily_loss: Decimal::new(10, 0),
                max_drawdown: Decimal::new(20, 0),
                stop_loss_threshold: Decimal::new(5, 0),
            },
            status: StrategyStatus::Stopped,
            performance: StrategyPerformance {
                total_pnl: Decimal::ZERO,
                daily_pnl: Decimal::ZERO,
                win_rate: Decimal::ZERO,
                sharpe_ratio: Decimal::ZERO,
                max_drawdown: Decimal::ZERO,
                trade_count: 0,
            },
        };

        framework.register_strategy(strategy).unwrap();
        framework.start_strategy("STRAT001").unwrap();

        let strategy = framework.strategies.get("STRAT001").unwrap();
        assert_eq!(strategy.status, StrategyStatus::Active);
    }

    #[test]
    fn test_custodial_wallet() {
        let mut manager = CustodialManager::new();

        let multi_sig = MultiSigConfig {
            required_signatures: 2,
            total_signers: 3,
            signer_ids: vec![
                "signer1".to_string(),
                "signer2".to_string(),
                "signer3".to_string(),
            ],
        };

        manager
            .create_wallet("WALLET001".to_string(), WalletType::Cold, Some(multi_sig))
            .unwrap();

        let wallet = manager.wallets.get_mut("WALLET001").unwrap();
        wallet
            .balances
            .insert("BTC".to_string(), Decimal::new(100, 0));

        let request = WithdrawalRequest {
            request_id: "WD001".to_string(),
            wallet_id: "WALLET001".to_string(),
            asset: "BTC".to_string(),
            amount: Decimal::new(10, 0),
            destination: "external_address".to_string(),
            requester_id: "user1".to_string(),
            created_at: SystemTime::now(),
            status: WithdrawalStatus::Pending,
            approvals: Vec::new(),
        };

        manager.submit_withdrawal(request).unwrap();

        // First approval
        manager
            .approve_withdrawal("WD001", "signer1".to_string(), "sig1".to_string())
            .unwrap();

        let request = manager.withdrawal_requests.get("WD001").unwrap();
        assert_eq!(request.status, WithdrawalStatus::AwaitingApproval);

        // Second approval - should be approved now
        manager
            .approve_withdrawal("WD001", "signer2".to_string(), "sig2".to_string())
            .unwrap();

        let request = manager.withdrawal_requests.get("WD001").unwrap();
        assert_eq!(request.status, WithdrawalStatus::Approved);
    }
}

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
    /// Unique identifier for this prime brokerage account
    pub account_id: String,
    /// Legal name of the institution
    pub institution_name: String,
    /// Sub-accounts indexed by sub-account ID
    pub sub_accounts: HashMap<String, SubAccount>,
    /// Credit facility attached to this account
    pub credit_line: CreditLine,
    /// Timestamp when the account was created
    pub created_at: SystemTime,
}

/// A sub-account under a prime brokerage account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAccount {
    /// Unique identifier for this sub-account
    pub sub_account_id: String,
    /// Human-readable name for this sub-account
    pub account_name: String,
    /// Asset balances keyed by asset symbol
    pub balances: HashMap<String, Decimal>,
    /// Whether trading is permitted from this sub-account
    pub trading_enabled: bool,
    /// Whether withdrawals are permitted from this sub-account
    pub withdrawal_enabled: bool,
    /// Trading volume and size limits for this sub-account
    pub daily_limits: TradingLimits,
}

/// Credit line extended to an institutional client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditLine {
    /// Maximum credit available
    pub total_limit: Decimal,
    /// Amount of credit currently drawn
    pub used_amount: Decimal,
    /// Annual interest rate on drawn credit
    pub interest_rate: Decimal,
    /// Required collateral as a percentage of drawn credit
    pub collateral_required: Decimal,
    /// Collateral ratio below which a margin call is triggered
    pub margin_call_threshold: Decimal,
}

/// Trading limits for a sub-account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingLimits {
    /// Maximum size of a single order
    pub max_order_size: Decimal,
    /// Maximum cumulative trading volume per day
    pub daily_volume_limit: Decimal,
    /// Cumulative volume traded today
    pub daily_volume_used: Decimal,
    /// Maximum number of simultaneously open positions
    pub max_open_positions: usize,
}

impl PrimeBrokerageAccount {
    /// Create a new prime brokerage account with the given credit limit
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
    /// Identifier of the prime brokerage account being reported on
    pub account_id: String,
    /// Timestamp when the report was generated
    pub report_date: SystemTime,
    /// Total asset balances aggregated across all sub-accounts
    pub total_assets: HashMap<String, Decimal>,
    /// Total outstanding liabilities (drawn credit)
    pub total_liabilities: Decimal,
    /// Net asset value (total assets minus liabilities)
    pub net_asset_value: Decimal,
    /// Profit and loss for the reporting period
    pub period_pnl: Decimal,
    /// Per-sub-account summaries
    pub sub_account_summaries: Vec<SubAccountSummary>,
}

/// Summary of a single sub-account for consolidated reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAccountSummary {
    /// Unique identifier of the sub-account
    pub sub_account_id: String,
    /// Human-readable name
    pub account_name: String,
    /// Total market value of assets in this sub-account
    pub total_value: Decimal,
    /// Profit and loss for the reporting period
    pub period_pnl: Decimal,
    /// Number of open positions
    pub position_count: usize,
}

/// Prime brokerage manager
pub struct PrimeBrokerageManager {
    accounts: HashMap<String, PrimeBrokerageAccount>,
}

impl PrimeBrokerageManager {
    /// Create a new prime brokerage manager
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
    /// Unique identifier for this strategy
    pub strategy_id: String,
    /// Human-readable name for this strategy
    pub strategy_name: String,
    /// Classification of the strategy type
    pub strategy_type: StrategyType,
    /// Strategy-specific configuration parameters
    pub parameters: HashMap<String, String>,
    /// Risk limits applied to this strategy
    pub risk_limits: RiskLimits,
    /// Current lifecycle state of the strategy
    pub status: StrategyStatus,
    /// Cumulative performance metrics
    pub performance: StrategyPerformance,
}

/// Classification of algorithmic trading strategy type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyType {
    /// Provides bid and ask quotes to earn the spread
    MarketMaking,
    /// Exploits price differences across venues
    Arbitrage,
    /// Follows market momentum and trends
    TrendFollowing,
    /// Trades on price reversion to a historical mean
    MeanReversion,
    /// Uses statistical models for signal generation
    Statistical,
    /// Custom strategy type
    Custom,
}

/// Lifecycle status of an algorithmic strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyStatus {
    /// Strategy is running and generating orders
    Active,
    /// Strategy is temporarily suspended
    Paused,
    /// Strategy has been stopped
    Stopped,
    /// Strategy encountered an error
    Error,
}

/// Risk limits applied to an algorithmic strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    /// Maximum allowed position size
    pub max_position_size: Decimal,
    /// Maximum loss allowed per day before the strategy is halted
    pub max_daily_loss: Decimal,
    /// Maximum peak-to-trough drawdown before the strategy is halted
    pub max_drawdown: Decimal,
    /// Stop-loss threshold for individual positions
    pub stop_loss_threshold: Decimal,
}

/// Cumulative performance metrics for an algorithmic strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    /// Total profit and loss since inception
    pub total_pnl: Decimal,
    /// Profit and loss for today
    pub daily_pnl: Decimal,
    /// Fraction of trades that were profitable
    pub win_rate: Decimal,
    /// Risk-adjusted return metric
    pub sharpe_ratio: Decimal,
    /// Maximum peak-to-trough drawdown observed
    pub max_drawdown: Decimal,
    /// Total number of trades executed
    pub trade_count: usize,
}

/// Algorithmic trading framework
pub struct AlgoTradingFramework {
    /// Registered strategies indexed by strategy ID
    strategies: HashMap<String, AlgoStrategy>,
    /// Chronological log of strategy execution events
    execution_log: Vec<StrategyExecution>,
}

/// Record of a single strategy execution event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyExecution {
    /// Unique identifier for this execution record
    pub execution_id: String,
    /// Strategy that was executed
    pub strategy_id: String,
    /// When the execution occurred
    pub timestamp: SystemTime,
    /// Description of the action taken
    pub action: String,
    /// Outcome of the execution
    pub result: ExecutionResult,
}

/// Outcome of a strategy execution attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// Execution succeeded with a recorded profit/loss
    Success {
        /// Profit or loss from this execution
        pnl: Decimal,
    },
    /// Execution failed
    Failure {
        /// Human-readable reason for failure
        reason: String,
    },
}

impl AlgoTradingFramework {
    /// Create a new algorithmic trading framework
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
    /// Unique identifier for this wallet
    pub wallet_id: String,
    /// Storage tier of this wallet
    pub wallet_type: WalletType,
    /// Multi-signature configuration, if enabled
    pub multi_sig_config: Option<MultiSigConfig>,
    /// Asset balances held in this wallet indexed by asset symbol
    pub balances: HashMap<String, Decimal>,
    /// Operational status of this wallet
    pub status: WalletStatus,
}

/// Storage tier of a custodial wallet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletType {
    /// Hot wallet for immediate access
    Hot,
    /// Cold wallet for secure storage
    Cold,
    /// Warm wallet - semi-offline
    Warm,
}

/// Operational status of a custodial wallet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletStatus {
    /// Wallet is operational
    Active,
    /// Wallet is locked and cannot be used
    Locked,
    /// Wallet is undergoing maintenance
    Maintenance,
}

/// Multi-signature configuration for a custodial wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSigConfig {
    /// Number of signatures required to authorise a transaction
    pub required_signatures: usize,
    /// Total number of signers in the group
    pub total_signers: usize,
    /// Identifiers of all authorised signers
    pub signer_ids: Vec<String>,
}

/// Withdrawal request for custodial services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRequest {
    /// Unique identifier for this withdrawal request
    pub request_id: String,
    /// Wallet from which funds are to be withdrawn
    pub wallet_id: String,
    /// Asset symbol being withdrawn
    pub asset: String,
    /// Amount to withdraw
    pub amount: Decimal,
    /// Destination address or account for the withdrawal
    pub destination: String,
    /// User or system that initiated the request
    pub requester_id: String,
    /// Timestamp when the request was created
    pub created_at: SystemTime,
    /// Current status of the withdrawal request
    pub status: WithdrawalStatus,
    /// Approvals collected so far
    pub approvals: Vec<Approval>,
}

/// Status of a custodial withdrawal request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WithdrawalStatus {
    /// Request submitted and awaiting initial processing
    Pending,
    /// Request requires additional approvals
    AwaitingApproval,
    /// Request has received sufficient approvals
    Approved,
    /// Request was denied
    Rejected,
    /// Funds have been transferred
    Completed,
    /// Transfer attempt failed
    Failed,
}

/// An individual approval on a withdrawal request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    /// Identifier of the approving signer
    pub approver_id: String,
    /// Timestamp when the approval was granted
    pub approved_at: SystemTime,
    /// Cryptographic signature from the approver
    pub signature: String,
}

/// Custodial services manager
pub struct CustodialManager {
    /// Custodial wallets indexed by wallet ID
    wallets: HashMap<String, CustodialWallet>,
    /// Withdrawal requests indexed by request ID
    withdrawal_requests: HashMap<String, WithdrawalRequest>,
    /// Approval workflow configurations indexed by workflow ID
    #[allow(dead_code)]
    approval_workflows: HashMap<String, ApprovalWorkflow>,
}

/// Workflow configuration for multi-party withdrawal approvals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalWorkflow {
    /// Unique identifier for this workflow
    pub workflow_id: String,
    /// Number of approvals required to proceed
    pub required_approvals: usize,
    /// List of approver identifiers
    pub approvers: Vec<String>,
    /// Amount below which the withdrawal is auto-approved
    pub auto_approve_threshold: Option<Decimal>,
}

impl CustodialManager {
    /// Create a new custodial manager
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

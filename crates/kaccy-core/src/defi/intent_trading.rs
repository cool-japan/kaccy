//! Intent-Based Trading System
//!
//! This module implements an intent-based trading system where users express their trading
//! intentions and solvers compete to fulfill them. This is inspired by systems like CoWSwap
//! and UniswapX.
//!
//! # Architecture
//!
//! 1. Users create signed intents expressing their desired trades
//! 2. Solvers compete to find the best execution path
//! 3. Solutions are verified and executed
//! 4. Solvers are rewarded based on their performance

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Types of trading intents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentType {
    /// Simple swap from one token to another
    Swap,
    /// Limit order intent
    Limit,
    /// Time-weighted average price order
    Twap,
    /// Cross-chain swap intent
    CrossChain,
    /// Batch of multiple intents
    Batch,
}

/// Intent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentStatus {
    /// Intent is pending solver execution
    Pending,
    /// Intent is being executed
    Executing,
    /// Intent has been fulfilled
    Fulfilled,
    /// Intent was cancelled by user
    Cancelled,
    /// Intent expired without fulfillment
    Expired,
    /// Intent failed to execute
    Failed,
}

/// User trading intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Unique intent identifier
    pub id: Uuid,
    /// User who created the intent
    pub user_id: Uuid,
    /// Type of intent
    pub intent_type: IntentType,
    /// Token to sell
    pub sell_token_id: Uuid,
    /// Amount to sell
    pub sell_amount: Decimal,
    /// Token to buy
    pub buy_token_id: Uuid,
    /// Minimum amount to receive
    pub min_buy_amount: Decimal,
    /// Maximum slippage tolerance (percentage)
    pub max_slippage: Decimal,
    /// Intent expiration timestamp
    pub expiration: DateTime<Utc>,
    /// Current status
    pub status: IntentStatus,
    /// User signature for verification
    pub signature: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Fulfilled amount (if partially filled)
    pub filled_amount: Decimal,
    /// Assigned solver ID (if any)
    pub solver_id: Option<Uuid>,
    /// Additional constraints (JSON)
    pub constraints: HashMap<String, String>,
}

impl Intent {
    /// Create a new intent
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: Uuid,
        intent_type: IntentType,
        sell_token_id: Uuid,
        sell_amount: Decimal,
        buy_token_id: Uuid,
        min_buy_amount: Decimal,
        max_slippage: Decimal,
        expiration: DateTime<Utc>,
        signature: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            intent_type,
            sell_token_id,
            sell_amount,
            buy_token_id,
            min_buy_amount,
            max_slippage,
            expiration,
            status: IntentStatus::Pending,
            signature,
            created_at: now,
            updated_at: now,
            filled_amount: Decimal::ZERO,
            solver_id: None,
            constraints: HashMap::new(),
        }
    }

    /// Check if intent is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expiration
    }

    /// Check if intent can be cancelled
    pub fn can_cancel(&self) -> bool {
        matches!(self.status, IntentStatus::Pending | IntentStatus::Executing)
    }

    /// Cancel the intent
    pub fn cancel(&mut self) -> Result<()> {
        if !self.can_cancel() {
            return Err(CoreError::Validation(
                "Intent cannot be cancelled in current state".to_string(),
            ));
        }

        self.status = IntentStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark intent as executing
    pub fn mark_executing(&mut self, solver_id: Uuid) -> Result<()> {
        if self.status != IntentStatus::Pending {
            return Err(CoreError::Validation(
                "Intent must be pending to start execution".to_string(),
            ));
        }

        self.status = IntentStatus::Executing;
        self.solver_id = Some(solver_id);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark intent as fulfilled
    pub fn mark_fulfilled(&mut self, filled_amount: Decimal) -> Result<()> {
        if self.status != IntentStatus::Executing {
            return Err(CoreError::Validation(
                "Intent must be executing to mark as fulfilled".to_string(),
            ));
        }

        self.status = IntentStatus::Fulfilled;
        self.filled_amount = filled_amount;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Verify intent signature
    pub fn verify_signature(&self) -> Result<bool> {
        // In production, this would verify the cryptographic signature
        // For now, we just check that signature is not empty
        Ok(!self.signature.is_empty())
    }

    /// Add constraint to intent
    pub fn add_constraint(&mut self, key: String, value: String) {
        self.constraints.insert(key, value);
        self.updated_at = Utc::now();
    }

    /// Calculate unfilled amount
    pub fn unfilled_amount(&self) -> Decimal {
        self.sell_amount - self.filled_amount
    }

    /// Check if intent is fully filled
    pub fn is_fully_filled(&self) -> bool {
        self.filled_amount >= self.sell_amount
    }
}

/// Solver reputation and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solver {
    /// Unique solver identifier
    pub id: Uuid,
    /// Solver name/address
    pub name: String,
    /// Total intents solved
    pub total_solved: u64,
    /// Total intents failed
    pub total_failed: u64,
    /// Average execution time (seconds)
    pub avg_execution_time: Decimal,
    /// Reputation score (0-100)
    pub reputation_score: Decimal,
    /// Total rewards earned
    pub total_rewards: Decimal,
    /// Is solver active?
    pub is_active: bool,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_active_at: DateTime<Utc>,
}

impl Solver {
    /// Create a new solver
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            total_solved: 0,
            total_failed: 0,
            avg_execution_time: Decimal::ZERO,
            reputation_score: Decimal::from(50), // Start at 50/100
            total_rewards: Decimal::ZERO,
            is_active: true,
            registered_at: now,
            last_active_at: now,
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> Decimal {
        let total = self.total_solved + self.total_failed;
        if total == 0 {
            return Decimal::ZERO;
        }
        Decimal::from(self.total_solved) / Decimal::from(total) * Decimal::from(100)
    }

    /// Record successful solve
    pub fn record_success(&mut self, execution_time: Decimal, reward: Decimal) {
        self.total_solved += 1;

        // Update average execution time
        let total_time = self.avg_execution_time * Decimal::from(self.total_solved - 1);
        self.avg_execution_time = (total_time + execution_time) / Decimal::from(self.total_solved);

        // Update reputation (increase by 1, max 100)
        self.reputation_score = (self.reputation_score + Decimal::ONE).min(Decimal::from(100));

        self.total_rewards += reward;
        self.last_active_at = Utc::now();
    }

    /// Record failed solve
    pub fn record_failure(&mut self) {
        self.total_failed += 1;

        // Update reputation (decrease by 2, min 0)
        self.reputation_score = (self.reputation_score - Decimal::from(2)).max(Decimal::ZERO);

        self.last_active_at = Utc::now();
    }

    /// Check if solver is eligible to solve intents
    pub fn is_eligible(&self) -> bool {
        self.is_active && self.reputation_score >= Decimal::from(20)
    }
}

/// Solution proposed by a solver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    /// Unique solution identifier
    pub id: Uuid,
    /// Intent being solved
    pub intent_id: Uuid,
    /// Solver proposing solution
    pub solver_id: Uuid,
    /// Execution path (list of trades)
    pub execution_path: Vec<ExecutionStep>,
    /// Expected output amount
    pub expected_output: Decimal,
    /// Estimated gas cost
    pub estimated_gas: Decimal,
    /// Quality score (higher is better)
    pub quality_score: Decimal,
    /// Timestamp when solution was proposed
    pub proposed_at: DateTime<Utc>,
}

/// Step in execution path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Trading venue/protocol
    pub venue: String,
    /// Input token
    pub input_token: Uuid,
    /// Input amount
    pub input_amount: Decimal,
    /// Output token
    pub output_token: Uuid,
    /// Expected output amount
    pub expected_output: Decimal,
    /// Step index in path
    pub step_index: u32,
}

impl Solution {
    /// Create a new solution
    pub fn new(
        intent_id: Uuid,
        solver_id: Uuid,
        execution_path: Vec<ExecutionStep>,
        expected_output: Decimal,
        estimated_gas: Decimal,
    ) -> Self {
        // Calculate quality score based on output and gas
        let quality_score = expected_output - (estimated_gas * Decimal::from(100));

        Self {
            id: Uuid::new_v4(),
            intent_id,
            solver_id,
            execution_path,
            expected_output,
            estimated_gas,
            quality_score,
            proposed_at: Utc::now(),
        }
    }

    /// Verify solution meets intent requirements
    pub fn verify(&self, intent: &Intent) -> Result<bool> {
        // Check if expected output meets minimum buy amount
        if self.expected_output < intent.min_buy_amount {
            return Ok(false);
        }

        // Check if execution path is valid
        if self.execution_path.is_empty() {
            return Ok(false);
        }

        // Verify path continuity
        for i in 0..self.execution_path.len() - 1 {
            if self.execution_path[i].output_token != self.execution_path[i + 1].input_token {
                return Ok(false);
            }
        }

        // Verify first step matches intent sell token
        if let Some(first_step) = self.execution_path.first() {
            if first_step.input_token != intent.sell_token_id {
                return Ok(false);
            }
        }

        // Verify last step matches intent buy token
        if let Some(last_step) = self.execution_path.last() {
            if last_step.output_token != intent.buy_token_id {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// Intent matching engine
pub struct IntentMatcher {
    /// Pending intents
    pending_intents: Vec<Intent>,
    /// Active solvers
    solvers: HashMap<Uuid, Solver>,
    /// Proposed solutions
    solutions: HashMap<Uuid, Vec<Solution>>,
}

impl IntentMatcher {
    /// Create a new intent matcher
    pub fn new() -> Self {
        Self {
            pending_intents: Vec::new(),
            solvers: HashMap::new(),
            solutions: HashMap::new(),
        }
    }

    /// Submit an intent
    pub fn submit_intent(&mut self, intent: Intent) -> Result<Uuid> {
        // Verify signature
        if !intent.verify_signature()? {
            return Err(CoreError::Validation(
                "Invalid intent signature".to_string(),
            ));
        }

        // Check expiration
        if intent.is_expired() {
            return Err(CoreError::Validation(
                "Intent is already expired".to_string(),
            ));
        }

        let intent_id = intent.id;
        self.pending_intents.push(intent);
        Ok(intent_id)
    }

    /// Register a solver
    pub fn register_solver(&mut self, solver: Solver) -> Result<Uuid> {
        let solver_id = solver.id;
        self.solvers.insert(solver_id, solver);
        Ok(solver_id)
    }

    /// Submit a solution
    pub fn submit_solution(&mut self, solution: Solution) -> Result<Uuid> {
        // Find the intent
        let intent = self
            .pending_intents
            .iter()
            .find(|i| i.id == solution.intent_id)
            .ok_or_else(|| CoreError::NotFound("Intent not found".to_string()))?
            .clone();

        // Verify solution
        if !solution.verify(&intent)? {
            return Err(CoreError::Validation("Invalid solution".to_string()));
        }

        // Check solver eligibility
        let solver = self
            .solvers
            .get(&solution.solver_id)
            .ok_or_else(|| CoreError::NotFound("Solver not found".to_string()))?;

        if !solver.is_eligible() {
            return Err(CoreError::Validation("Solver is not eligible".to_string()));
        }

        let solution_id = solution.id;
        self.solutions
            .entry(solution.intent_id)
            .or_default()
            .push(solution);

        Ok(solution_id)
    }

    /// Select best solution for an intent
    pub fn select_best_solution(&self, intent_id: Uuid) -> Option<&Solution> {
        self.solutions.get(&intent_id).and_then(|solutions| {
            solutions
                .iter()
                .max_by(|a, b| a.quality_score.cmp(&b.quality_score))
        })
    }

    /// Execute best solution for an intent
    pub fn execute_intent(&mut self, intent_id: Uuid) -> Result<Decimal> {
        // Find and remove the intent
        let intent_idx = self
            .pending_intents
            .iter()
            .position(|i| i.id == intent_id)
            .ok_or_else(|| CoreError::NotFound("Intent not found".to_string()))?;

        let mut intent = self.pending_intents.remove(intent_idx);

        // Get best solution
        let solution = self
            .select_best_solution(intent_id)
            .ok_or_else(|| CoreError::NotFound("No solutions found".to_string()))?
            .clone();

        // Mark intent as executing
        intent.mark_executing(solution.solver_id)?;

        // Execute the solution (in production, this would actually execute trades)
        let output_amount = solution.expected_output;

        // Mark intent as fulfilled
        intent.mark_fulfilled(output_amount)?;

        // Update solver statistics
        if let Some(solver) = self.solvers.get_mut(&solution.solver_id) {
            let execution_time = Decimal::from(5); // Mock execution time
            let reward = solution.estimated_gas * Decimal::from(2); // Mock reward
            solver.record_success(execution_time, reward);
        }

        // Clean up solutions
        self.solutions.remove(&intent_id);

        Ok(output_amount)
    }

    /// Cancel an intent
    pub fn cancel_intent(&mut self, intent_id: Uuid, user_id: Uuid) -> Result<()> {
        let intent = self
            .pending_intents
            .iter_mut()
            .find(|i| i.id == intent_id)
            .ok_or_else(|| CoreError::NotFound("Intent not found".to_string()))?;

        // Verify ownership
        if intent.user_id != user_id {
            return Err(CoreError::Unauthorized);
        }

        intent.cancel()?;
        Ok(())
    }

    /// Clean up expired intents
    pub fn cleanup_expired(&mut self) -> usize {
        let initial_count = self.pending_intents.len();
        self.pending_intents.retain(|intent| !intent.is_expired());

        // Clean up solutions for expired intents
        let expired_ids: Vec<Uuid> = self
            .pending_intents
            .iter()
            .filter(|i| i.is_expired())
            .map(|i| i.id)
            .collect();

        for intent_id in expired_ids {
            self.solutions.remove(&intent_id);
        }

        initial_count - self.pending_intents.len()
    }

    /// Get pending intents count
    pub fn pending_count(&self) -> usize {
        self.pending_intents.len()
    }

    /// Get solver by ID
    pub fn get_solver(&self, solver_id: Uuid) -> Option<&Solver> {
        self.solvers.get(&solver_id)
    }

    /// Get all active solvers
    pub fn active_solvers(&self) -> Vec<&Solver> {
        self.solvers
            .values()
            .filter(|s| s.is_active && s.is_eligible())
            .collect()
    }
}

impl Default for IntentMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;

    #[test]
    fn test_intent_creation() {
        let intent = Intent::new(
            Uuid::new_v4(),
            IntentType::Swap,
            Uuid::new_v4(),
            dec!(100),
            Uuid::new_v4(),
            dec!(95),
            dec!(5),
            Utc::now() + Duration::hours(1),
            "signature".to_string(),
        );

        assert_eq!(intent.status, IntentStatus::Pending);
        assert_eq!(intent.filled_amount, Decimal::ZERO);
        assert!(!intent.is_expired());
    }

    #[test]
    fn test_intent_cancel() {
        let mut intent = Intent::new(
            Uuid::new_v4(),
            IntentType::Swap,
            Uuid::new_v4(),
            dec!(100),
            Uuid::new_v4(),
            dec!(95),
            dec!(5),
            Utc::now() + Duration::hours(1),
            "signature".to_string(),
        );

        assert!(intent.cancel().is_ok());
        assert_eq!(intent.status, IntentStatus::Cancelled);

        // Cannot cancel again
        assert!(intent.cancel().is_err());
    }

    #[test]
    fn test_solver_reputation() {
        let mut solver = Solver::new("TestSolver".to_string());
        assert_eq!(solver.reputation_score, dec!(50));
        assert!(solver.is_eligible());

        // Record success
        solver.record_success(dec!(5), dec!(10));
        assert_eq!(solver.total_solved, 1);
        assert_eq!(solver.reputation_score, dec!(51));

        // Record failure
        solver.record_failure();
        assert_eq!(solver.total_failed, 1);
        assert_eq!(solver.reputation_score, dec!(49));
    }

    #[test]
    fn test_solution_verification() {
        let sell_token = Uuid::new_v4();
        let buy_token = Uuid::new_v4();

        let intent = Intent::new(
            Uuid::new_v4(),
            IntentType::Swap,
            sell_token,
            dec!(100),
            buy_token,
            dec!(95),
            dec!(5),
            Utc::now() + Duration::hours(1),
            "signature".to_string(),
        );

        let execution_path = vec![ExecutionStep {
            venue: "UniswapV3".to_string(),
            input_token: sell_token,
            input_amount: dec!(100),
            output_token: buy_token,
            expected_output: dec!(98),
            step_index: 0,
        }];

        let solution = Solution::new(
            intent.id,
            Uuid::new_v4(),
            execution_path,
            dec!(98),
            dec!(0.1),
        );

        assert!(solution.verify(&intent).unwrap());
    }

    #[test]
    fn test_intent_matcher() {
        let mut matcher = IntentMatcher::new();

        // Register solver
        let solver = Solver::new("TestSolver".to_string());
        let solver_id = solver.id;
        matcher.register_solver(solver).unwrap();

        // Submit intent
        let sell_token = Uuid::new_v4();
        let buy_token = Uuid::new_v4();

        let intent = Intent::new(
            Uuid::new_v4(),
            IntentType::Swap,
            sell_token,
            dec!(100),
            buy_token,
            dec!(95),
            dec!(5),
            Utc::now() + Duration::hours(1),
            "signature".to_string(),
        );

        let intent_id = intent.id;
        matcher.submit_intent(intent).unwrap();

        assert_eq!(matcher.pending_count(), 1);

        // Submit solution
        let execution_path = vec![ExecutionStep {
            venue: "UniswapV3".to_string(),
            input_token: sell_token,
            input_amount: dec!(100),
            output_token: buy_token,
            expected_output: dec!(98),
            step_index: 0,
        }];

        let solution = Solution::new(intent_id, solver_id, execution_path, dec!(98), dec!(0.1));

        matcher.submit_solution(solution).unwrap();

        // Execute intent
        let output = matcher.execute_intent(intent_id).unwrap();
        assert_eq!(output, dec!(98));
        assert_eq!(matcher.pending_count(), 0);
    }

    #[test]
    fn test_cleanup_expired() {
        let mut matcher = IntentMatcher::new();

        // Submit expired intent
        let intent = Intent::new(
            Uuid::new_v4(),
            IntentType::Swap,
            Uuid::new_v4(),
            dec!(100),
            Uuid::new_v4(),
            dec!(95),
            dec!(5),
            Utc::now() - Duration::hours(1), // Already expired
            "signature".to_string(),
        );

        matcher.pending_intents.push(intent);

        let cleaned = matcher.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert_eq!(matcher.pending_count(), 0);
    }
}

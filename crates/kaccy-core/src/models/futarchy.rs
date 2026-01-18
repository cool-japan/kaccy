/// Futarchy: Prediction Market Governance
///
/// Futarchy is a governance model where decisions are made based on prediction markets.
/// The idea: "Vote on values, but bet on beliefs."
///
/// # How it works
/// 1. A proposal is created with a measurable success metric
/// 2. Two conditional prediction markets are created:
///    - Market A: "What will the metric be IF the proposal passes?"
///    - Market B: "What will the metric be IF the proposal does NOT pass?"
/// 3. Traders bet on both markets based on their beliefs
/// 4. The proposal passes if Market A predicts a better outcome than Market B
/// 5. After a decision period, only the correct market settles
///
/// # Example
/// Proposal: "Reduce trading fees by 50%"
/// Metric: "Total 30-day trading volume"
/// - Market A traders bet on volume if fees are reduced
/// - Market B traders bet on volume if fees stay the same
/// - If Market A predicts higher volume, the proposal passes
use crate::error::{CoreError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Metric type for measuring proposal success
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuccessMetric {
    /// Trading volume in BTC
    TradingVolume,
    /// Total value locked (TVL) in BTC
    TotalValueLocked,
    /// Number of active users
    ActiveUsers,
    /// Average transaction value
    AvgTransactionValue,
    /// Platform revenue
    PlatformRevenue,
    /// Token price
    TokenPrice,
}

/// Conditional market for a specific outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalMarket {
    /// Market ID
    pub market_id: String,
    /// Proposal this market is for
    pub proposal_id: String,
    /// Condition: true = proposal passes, false = proposal fails
    pub condition: bool,
    /// Current prediction (in metric units)
    pub predicted_value: Decimal,
    /// Total liquidity in the market
    pub liquidity: Decimal,
    /// Individual positions: user_id -> position
    pub positions: HashMap<String, MarketPosition>,
    /// Market resolved
    pub resolved: bool,
    /// Actual outcome (only set after resolution)
    pub actual_value: Option<Decimal>,
}

/// User position in a conditional market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPosition {
    /// User ID
    pub user_id: String,
    /// Amount staked
    pub stake: Decimal,
    /// Predicted value when position opened
    pub predicted_value: Decimal,
    /// Timestamp of position
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Futarchy proposal with dual conditional markets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FutarchyProposal {
    /// Proposal ID
    pub proposal_id: String,
    /// Title of the proposal
    pub title: String,
    /// Description
    pub description: String,
    /// Success metric to be predicted
    pub metric: SuccessMetric,
    /// Market if proposal passes (condition = true)
    pub pass_market: ConditionalMarket,
    /// Market if proposal does NOT pass (condition = false)
    pub fail_market: ConditionalMarket,
    /// Prediction period end time
    pub prediction_end: chrono::DateTime<chrono::Utc>,
    /// Measurement period end (when actual outcome is known)
    pub measurement_end: chrono::DateTime<chrono::Utc>,
    /// Proposal status
    pub status: FutarchyProposalStatus,
    /// Decision made (if status = Decided)
    pub decision: Option<bool>,
    /// Creator
    pub creator: String,
    /// Created at
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Status of a futarchy proposal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FutarchyProposalStatus {
    /// Markets are open for prediction
    PredictionPhase,
    /// Prediction phase ended, decision made
    Decided,
    /// Waiting for actual outcome to measure
    AwaitingOutcome,
    /// Resolved with actual outcome
    Resolved,
    /// Cancelled
    Cancelled,
}

/// Futarchy governance manager
pub struct FutarchyGovernor {
    /// Minimum liquidity required per market
    pub min_liquidity: Decimal,
    /// Minimum prediction period (hours)
    pub min_prediction_period: i64,
    /// Measurement period after decision (days)
    pub measurement_period_days: i64,
}

impl FutarchyGovernor {
    /// Create a new futarchy governor
    pub fn new(
        min_liquidity: Decimal,
        min_prediction_period: i64,
        measurement_period_days: i64,
    ) -> Self {
        Self {
            min_liquidity,
            min_prediction_period,
            measurement_period_days,
        }
    }

    /// Create a futarchy proposal with conditional markets
    pub fn create_proposal(
        &self,
        title: String,
        description: String,
        metric: SuccessMetric,
        creator: String,
        prediction_period_hours: i64,
    ) -> Result<FutarchyProposal> {
        if prediction_period_hours < self.min_prediction_period {
            return Err(CoreError::Validation(format!(
                "Prediction period must be at least {} hours",
                self.min_prediction_period
            )));
        }

        let now = chrono::Utc::now();
        let proposal_id = Uuid::new_v4().to_string();

        let pass_market = ConditionalMarket {
            market_id: format!("{}-pass", proposal_id),
            proposal_id: proposal_id.clone(),
            condition: true,
            predicted_value: Decimal::ZERO,
            liquidity: Decimal::ZERO,
            positions: HashMap::new(),
            resolved: false,
            actual_value: None,
        };

        let fail_market = ConditionalMarket {
            market_id: format!("{}-fail", proposal_id),
            proposal_id: proposal_id.clone(),
            condition: false,
            predicted_value: Decimal::ZERO,
            liquidity: Decimal::ZERO,
            positions: HashMap::new(),
            resolved: false,
            actual_value: None,
        };

        Ok(FutarchyProposal {
            proposal_id,
            title,
            description,
            metric,
            pass_market,
            fail_market,
            prediction_end: now + chrono::Duration::hours(prediction_period_hours),
            measurement_end: now
                + chrono::Duration::hours(prediction_period_hours)
                + chrono::Duration::days(self.measurement_period_days),
            status: FutarchyProposalStatus::PredictionPhase,
            decision: None,
            creator,
            created_at: now,
        })
    }

    /// Place a bet on a conditional market
    pub fn place_bet(
        &self,
        proposal: &mut FutarchyProposal,
        user_id: String,
        condition: bool,
        stake: Decimal,
        predicted_value: Decimal,
    ) -> Result<()> {
        // Check proposal status
        if proposal.status != FutarchyProposalStatus::PredictionPhase {
            return Err(CoreError::InvalidState(
                "Proposal is not in prediction phase".to_string(),
            ));
        }

        // Check if prediction period has ended
        if chrono::Utc::now() > proposal.prediction_end {
            return Err(CoreError::InvalidState(
                "Prediction period has ended".to_string(),
            ));
        }

        // Validate stake
        if stake <= Decimal::ZERO {
            return Err(CoreError::Validation("Stake must be positive".to_string()));
        }

        // Select the market
        let market = if condition {
            &mut proposal.pass_market
        } else {
            &mut proposal.fail_market
        };

        // Update market liquidity
        market.liquidity += stake;

        // Update predicted value (weighted average)
        let total_stake = market.positions.values().map(|p| p.stake).sum::<Decimal>() + stake;
        let weighted_sum = market
            .positions
            .values()
            .map(|p| p.stake * p.predicted_value)
            .sum::<Decimal>()
            + stake * predicted_value;
        market.predicted_value = weighted_sum / total_stake;

        // Add position
        let position = MarketPosition {
            user_id: user_id.clone(),
            stake,
            predicted_value,
            timestamp: chrono::Utc::now(),
        };

        market.positions.insert(user_id, position);

        Ok(())
    }

    /// Make decision based on prediction markets
    pub fn make_decision(&self, proposal: &mut FutarchyProposal) -> Result<bool> {
        // Check status
        if proposal.status != FutarchyProposalStatus::PredictionPhase {
            return Err(CoreError::InvalidState(
                "Proposal is not in prediction phase".to_string(),
            ));
        }

        // Check if prediction period has ended
        if chrono::Utc::now() < proposal.prediction_end {
            return Err(CoreError::InvalidState(
                "Prediction period has not ended yet".to_string(),
            ));
        }

        // Check minimum liquidity
        if proposal.pass_market.liquidity < self.min_liquidity
            || proposal.fail_market.liquidity < self.min_liquidity
        {
            proposal.status = FutarchyProposalStatus::Cancelled;
            return Err(CoreError::InvalidState(
                "Insufficient liquidity in markets".to_string(),
            ));
        }

        // Decision: proposal passes if pass_market predicts better outcome
        let decision = proposal.pass_market.predicted_value > proposal.fail_market.predicted_value;

        proposal.decision = Some(decision);
        proposal.status = FutarchyProposalStatus::Decided;

        Ok(decision)
    }

    /// Resolve markets after actual outcome is measured
    pub fn resolve_markets(
        &self,
        proposal: &mut FutarchyProposal,
        actual_value: Decimal,
    ) -> Result<()> {
        // Check status
        if proposal.status != FutarchyProposalStatus::Decided
            && proposal.status != FutarchyProposalStatus::AwaitingOutcome
        {
            return Err(CoreError::InvalidState(
                "Proposal must be decided before resolution".to_string(),
            ));
        }

        // Check if measurement period has ended
        if chrono::Utc::now() < proposal.measurement_end {
            proposal.status = FutarchyProposalStatus::AwaitingOutcome;
            return Err(CoreError::InvalidState(
                "Measurement period has not ended yet".to_string(),
            ));
        }

        // Resolve only the correct market (based on actual decision)
        let decision = proposal
            .decision
            .ok_or_else(|| CoreError::InvalidState("No decision made".to_string()))?;

        if decision {
            // Proposal passed, resolve pass_market
            proposal.pass_market.resolved = true;
            proposal.pass_market.actual_value = Some(actual_value);
        } else {
            // Proposal failed, resolve fail_market
            proposal.fail_market.resolved = true;
            proposal.fail_market.actual_value = Some(actual_value);
        }

        proposal.status = FutarchyProposalStatus::Resolved;

        Ok(())
    }

    /// Calculate payout for a user in a resolved market
    pub fn calculate_payout(&self, market: &ConditionalMarket, user_id: &str) -> Result<Decimal> {
        // Check if market is resolved
        if !market.resolved {
            return Err(CoreError::InvalidState(
                "Market not resolved yet".to_string(),
            ));
        }

        let actual_value = market
            .actual_value
            .ok_or_else(|| CoreError::InvalidState("Actual value not set".to_string()))?;

        // Get user position
        let position = market
            .positions
            .get(user_id)
            .ok_or_else(|| CoreError::NotFound("User has no position".to_string()))?;

        // Simple payout model: accuracy-weighted distribution
        // Users who predicted closer to actual value get more payout

        // Calculate prediction error for this user
        let user_error = (position.predicted_value - actual_value).abs();

        // Calculate total weighted stakes (inverse of errors)
        let mut total_weighted_stake = Decimal::ZERO;
        for pos in market.positions.values() {
            let error = (pos.predicted_value - actual_value).abs();
            let weight = if error == Decimal::ZERO {
                Decimal::from(1000) // High weight for exact predictions
            } else {
                Decimal::ONE / error
            };
            total_weighted_stake += pos.stake * weight;
        }

        // Calculate user's weight
        let user_weight = if user_error == Decimal::ZERO {
            Decimal::from(1000)
        } else {
            Decimal::ONE / user_error
        };

        let user_weighted_stake = position.stake * user_weight;

        // User's share of total liquidity
        let payout = (user_weighted_stake / total_weighted_stake) * market.liquidity;

        Ok(payout)
    }

    /// Get market statistics
    pub fn get_market_stats(&self, market: &ConditionalMarket) -> MarketStats {
        let num_participants = market.positions.len();
        let avg_prediction = if num_participants > 0 {
            market.predicted_value
        } else {
            Decimal::ZERO
        };

        // Calculate prediction variance
        let predictions: Vec<Decimal> = market
            .positions
            .values()
            .map(|p| p.predicted_value)
            .collect();
        let variance = if predictions.len() > 1 {
            let mean = avg_prediction;
            let sum_squared_diff: Decimal =
                predictions.iter().map(|&p| (p - mean) * (p - mean)).sum();
            sum_squared_diff / Decimal::from(predictions.len())
        } else {
            Decimal::ZERO
        };

        MarketStats {
            liquidity: market.liquidity,
            num_participants,
            predicted_value: market.predicted_value,
            prediction_variance: variance,
            resolved: market.resolved,
            actual_value: market.actual_value,
        }
    }
}

/// Market statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub liquidity: Decimal,
    pub num_participants: usize,
    pub predicted_value: Decimal,
    pub prediction_variance: Decimal,
    pub resolved: bool,
    pub actual_value: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_proposal() {
        let governor = FutarchyGovernor::new(
            Decimal::from(100), // min liquidity
            24,                 // min prediction period (hours)
            30,                 // measurement period (days)
        );

        let proposal = governor
            .create_proposal(
                "Reduce trading fees".to_string(),
                "Reduce trading fees by 50%".to_string(),
                SuccessMetric::TradingVolume,
                "creator123".to_string(),
                48, // 48 hours prediction period
            )
            .unwrap();

        assert_eq!(proposal.title, "Reduce trading fees");
        assert_eq!(proposal.status, FutarchyProposalStatus::PredictionPhase);
        assert!(proposal.pass_market.liquidity == Decimal::ZERO);
        assert!(proposal.fail_market.liquidity == Decimal::ZERO);
    }

    #[test]
    fn test_place_bet() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 24, 30);

        let mut proposal = governor
            .create_proposal(
                "Test proposal".to_string(),
                "Test".to_string(),
                SuccessMetric::TradingVolume,
                "creator".to_string(),
                48,
            )
            .unwrap();

        // Alice bets on pass market
        governor
            .place_bet(
                &mut proposal,
                "alice".to_string(),
                true, // pass condition
                Decimal::from(50),
                Decimal::from(1000), // predicts volume of 1000
            )
            .unwrap();

        assert_eq!(proposal.pass_market.liquidity, Decimal::from(50));
        assert_eq!(proposal.pass_market.predicted_value, Decimal::from(1000));

        // Bob bets on pass market with different prediction
        governor
            .place_bet(
                &mut proposal,
                "bob".to_string(),
                true,
                Decimal::from(50),
                Decimal::from(1200), // predicts volume of 1200
            )
            .unwrap();

        // Average: (50*1000 + 50*1200) / 100 = 1100
        assert_eq!(proposal.pass_market.liquidity, Decimal::from(100));
        assert_eq!(proposal.pass_market.predicted_value, Decimal::from(1100));
    }

    #[test]
    fn test_make_decision() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 0, 30); // 0 min period for testing

        let mut proposal = governor
            .create_proposal(
                "Test proposal".to_string(),
                "Test".to_string(),
                SuccessMetric::TradingVolume,
                "creator".to_string(),
                1, // 1 hour
            )
            .unwrap();

        // Set prediction end to past
        proposal.prediction_end = chrono::Utc::now() - chrono::Duration::hours(1);

        // Add liquidity to both markets
        proposal.pass_market.liquidity = Decimal::from(150);
        proposal.pass_market.predicted_value = Decimal::from(1200); // Higher prediction

        proposal.fail_market.liquidity = Decimal::from(150);
        proposal.fail_market.predicted_value = Decimal::from(1000); // Lower prediction

        let decision = governor.make_decision(&mut proposal).unwrap();

        assert!(decision); // Should pass because pass_market predicts better outcome
        assert_eq!(proposal.status, FutarchyProposalStatus::Decided);
        assert_eq!(proposal.decision, Some(true));
    }

    #[test]
    fn test_resolve_markets() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 0, 30);

        let mut proposal = governor
            .create_proposal(
                "Test proposal".to_string(),
                "Test".to_string(),
                SuccessMetric::TradingVolume,
                "creator".to_string(),
                1,
            )
            .unwrap();

        // Setup for resolution
        proposal.status = FutarchyProposalStatus::Decided;
        proposal.decision = Some(true); // Proposal passed
        proposal.measurement_end = chrono::Utc::now() - chrono::Duration::hours(1);

        governor
            .resolve_markets(&mut proposal, Decimal::from(1150))
            .unwrap();

        assert_eq!(proposal.status, FutarchyProposalStatus::Resolved);
        assert!(proposal.pass_market.resolved);
        assert_eq!(proposal.pass_market.actual_value, Some(Decimal::from(1150)));
        assert!(!proposal.fail_market.resolved); // Fail market not resolved (proposal passed)
    }

    #[test]
    fn test_calculate_payout() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 0, 30);

        let mut market = ConditionalMarket {
            market_id: "test".to_string(),
            proposal_id: "test".to_string(),
            condition: true,
            predicted_value: Decimal::from(1100),
            liquidity: Decimal::from(200),
            positions: HashMap::new(),
            resolved: true,
            actual_value: Some(Decimal::from(1150)), // Actual outcome
        };

        // Alice predicted 1200 (error: 50)
        market.positions.insert(
            "alice".to_string(),
            MarketPosition {
                user_id: "alice".to_string(),
                stake: Decimal::from(100),
                predicted_value: Decimal::from(1200),
                timestamp: chrono::Utc::now(),
            },
        );

        // Bob predicted 1100 (error: 50)
        market.positions.insert(
            "bob".to_string(),
            MarketPosition {
                user_id: "bob".to_string(),
                stake: Decimal::from(100),
                predicted_value: Decimal::from(1100),
                timestamp: chrono::Utc::now(),
            },
        );

        // Both have same error, should split evenly
        let alice_payout = governor.calculate_payout(&market, "alice").unwrap();
        let bob_payout = governor.calculate_payout(&market, "bob").unwrap();

        assert_eq!(alice_payout, Decimal::from(100));
        assert_eq!(bob_payout, Decimal::from(100));
        assert_eq!(alice_payout + bob_payout, market.liquidity);
    }

    #[test]
    fn test_market_stats() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 0, 30);

        let mut market = ConditionalMarket {
            market_id: "test".to_string(),
            proposal_id: "test".to_string(),
            condition: true,
            predicted_value: Decimal::from(1100),
            liquidity: Decimal::from(200),
            positions: HashMap::new(),
            resolved: false,
            actual_value: None,
        };

        market.positions.insert(
            "alice".to_string(),
            MarketPosition {
                user_id: "alice".to_string(),
                stake: Decimal::from(100),
                predicted_value: Decimal::from(1200),
                timestamp: chrono::Utc::now(),
            },
        );

        market.positions.insert(
            "bob".to_string(),
            MarketPosition {
                user_id: "bob".to_string(),
                stake: Decimal::from(100),
                predicted_value: Decimal::from(1000),
                timestamp: chrono::Utc::now(),
            },
        );

        let stats = governor.get_market_stats(&market);

        assert_eq!(stats.liquidity, Decimal::from(200));
        assert_eq!(stats.num_participants, 2);
        assert_eq!(stats.predicted_value, Decimal::from(1100));
        assert!(!stats.resolved);
        assert_eq!(stats.actual_value, None);
    }

    #[test]
    fn test_insufficient_liquidity() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 0, 30);

        let mut proposal = governor
            .create_proposal(
                "Test proposal".to_string(),
                "Test".to_string(),
                SuccessMetric::TradingVolume,
                "creator".to_string(),
                1,
            )
            .unwrap();

        proposal.prediction_end = chrono::Utc::now() - chrono::Duration::hours(1);

        // Insufficient liquidity
        proposal.pass_market.liquidity = Decimal::from(50); // Less than min 100
        proposal.fail_market.liquidity = Decimal::from(150);

        let result = governor.make_decision(&mut proposal);

        assert!(result.is_err());
        assert_eq!(proposal.status, FutarchyProposalStatus::Cancelled);
    }

    #[test]
    fn test_decision_based_on_better_prediction() {
        let governor = FutarchyGovernor::new(Decimal::from(100), 0, 30);

        let mut proposal = governor
            .create_proposal(
                "Test proposal".to_string(),
                "Test".to_string(),
                SuccessMetric::TradingVolume,
                "creator".to_string(),
                1,
            )
            .unwrap();

        proposal.prediction_end = chrono::Utc::now() - chrono::Duration::hours(1);

        // Fail market predicts better outcome
        proposal.pass_market.liquidity = Decimal::from(150);
        proposal.pass_market.predicted_value = Decimal::from(1000);

        proposal.fail_market.liquidity = Decimal::from(150);
        proposal.fail_market.predicted_value = Decimal::from(1500); // Higher is better

        let decision = governor.make_decision(&mut proposal).unwrap();

        assert!(!decision); // Should NOT pass because fail_market predicts better outcome
        assert_eq!(proposal.decision, Some(false));
    }
}

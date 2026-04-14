//! Economic Security Modeling
//!
//! This module provides tools for modeling economic security, including cost of attack
//! analysis, game-theoretic equilibrium, and incentive alignment verification.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Attack vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackVector {
    /// Attack name
    pub name: String,
    /// Attack type
    pub attack_type: AttackType,
    /// Cost to execute attack
    pub cost: Decimal,
    /// Potential profit from attack
    pub potential_profit: Decimal,
    /// Probability of success
    pub success_probability: Decimal,
    /// Probability of detection
    pub detection_probability: Decimal,
    /// Penalty if detected
    pub penalty: Decimal,
}

/// Category of economic attack against the protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackType {
    /// 51% attack (consensus)
    Consensus51,
    /// Front-running / MEV
    FrontRunning,
    /// Oracle manipulation
    OracleManipulation,
    /// Flash loan attack
    FlashLoan,
    /// Sybil attack
    Sybil,
    /// Economic exploit
    EconomicExploit,
}

impl AttackVector {
    /// Calculate expected value of attack
    pub fn expected_value(&self) -> Decimal {
        let expected_profit = self.potential_profit * self.success_probability;
        let expected_penalty = self.penalty * self.detection_probability;

        expected_profit - expected_penalty - self.cost
    }

    /// Check if attack is economically rational
    pub fn is_rational(&self) -> bool {
        self.expected_value() > dec!(0)
    }

    /// Calculate profit-to-cost ratio
    pub fn profit_cost_ratio(&self) -> Decimal {
        if self.cost > dec!(0) {
            (self.potential_profit * self.success_probability) / self.cost
        } else {
            dec!(0)
        }
    }
}

/// Economic security analyzer
pub struct EconomicSecurityAnalyzer {
    /// System total value locked (TVL)
    tvl: Decimal,
    /// Attack vectors
    attack_vectors: Vec<AttackVector>,
    /// Security budget
    #[allow(dead_code)]
    security_budget: Decimal,
}

impl EconomicSecurityAnalyzer {
    /// Create a new economic security analyzer
    pub fn new(tvl: Decimal, security_budget: Decimal) -> Self {
        Self {
            tvl,
            attack_vectors: Vec::new(),
            security_budget,
        }
    }

    /// Add an attack vector
    pub fn add_attack_vector(&mut self, vector: AttackVector) {
        self.attack_vectors.push(vector);
    }

    /// Calculate minimum cost of attack
    pub fn minimum_attack_cost(&self) -> Decimal {
        self.attack_vectors
            .iter()
            .map(|v| v.cost)
            .min()
            .unwrap_or(dec!(0))
    }

    /// Calculate Byzantine fault tolerance threshold
    pub fn byzantine_threshold(&self, validator_stake: Decimal, total_stake: Decimal) -> Decimal {
        if total_stake > dec!(0) {
            (validator_stake / total_stake) * dec!(100)
        } else {
            dec!(0)
        }
    }

    /// Calculate Nakamoto coefficient (decentralization metric)
    pub fn nakamoto_coefficient(&self, stakes: &[Decimal]) -> usize {
        if stakes.is_empty() {
            return 0;
        }

        let total: Decimal = stakes.iter().sum();
        let target = total / dec!(2); // 50%+ control
        let mut sorted = stakes.to_vec();
        sorted.sort_by(|a, b| b.cmp(a));

        let mut cumulative = dec!(0);
        for (i, stake) in sorted.iter().enumerate() {
            cumulative += stake;
            if cumulative > target {
                return i + 1;
            }
        }

        stakes.len()
    }

    /// Identify economically rational attacks
    pub fn identify_rational_attacks(&self) -> Vec<&AttackVector> {
        self.attack_vectors
            .iter()
            .filter(|v| v.is_rational())
            .collect()
    }

    /// Calculate security factor (cost to attack / TVL)
    pub fn security_factor(&self) -> Decimal {
        let min_cost = self.minimum_attack_cost();
        if self.tvl > dec!(0) {
            min_cost / self.tvl
        } else {
            dec!(0)
        }
    }

    /// Assess overall security level
    pub fn security_level(&self) -> SecurityLevel {
        let factor = self.security_factor();
        let rational_attacks = self.identify_rational_attacks().len();

        if rational_attacks > 0 {
            return SecurityLevel::Critical;
        }

        if factor >= dec!(2.0) {
            SecurityLevel::High
        } else if factor >= dec!(1.0) {
            SecurityLevel::Medium
        } else if factor >= dec!(0.5) {
            SecurityLevel::Low
        } else {
            SecurityLevel::Critical
        }
    }

    /// Recommend security improvements
    pub fn recommend_improvements(&self) -> Vec<SecurityRecommendation> {
        let mut recommendations = Vec::new();
        let rational_attacks = self.identify_rational_attacks();

        for attack in rational_attacks {
            let rec = SecurityRecommendation {
                priority: Priority::Critical,
                description: format!("Mitigate {} attack", attack.name),
                estimated_cost: attack.cost * dec!(0.1), // 10% of attack cost
                expected_benefit: attack.potential_profit,
                action: match attack.attack_type {
                    AttackType::Consensus51 => {
                        "Increase validator requirements or implement slashing".to_string()
                    }
                    AttackType::FrontRunning => "Implement MEV protection mechanisms".to_string(),
                    AttackType::OracleManipulation => {
                        "Use decentralized oracles with multiple sources".to_string()
                    }
                    AttackType::FlashLoan => "Implement flash loan attack prevention".to_string(),
                    AttackType::Sybil => "Require economic stake for participation".to_string(),
                    AttackType::EconomicExploit => "Audit economic model".to_string(),
                },
            };
            recommendations.push(rec);
        }

        recommendations
    }
}

/// Security posture classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Critical security issues
    Critical,
    /// Low security
    Low,
    /// Medium security
    Medium,
    /// High security
    High,
}

/// Execution priority for security recommendations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    /// Low priority
    Low,
    /// Medium priority
    Medium,
    /// High priority
    High,
    /// Critical priority
    Critical,
}

/// Actionable security recommendation with cost-benefit analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRecommendation {
    /// Priority level
    pub priority: Priority,
    /// Description
    pub description: String,
    /// Estimated cost to implement
    pub estimated_cost: Decimal,
    /// Expected benefit
    pub expected_benefit: Decimal,
    /// Recommended action
    pub action: String,
}

/// Game-theoretic model for mechanism design
pub struct GameTheoreticModel {
    /// Players
    players: HashMap<String, Player>,
    /// Strategies
    strategies: Vec<Strategy>,
}

/// A participant in the game-theoretic model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    /// Player ID
    pub id: String,
    /// Player type (validator, trader, liquidity provider, etc.)
    pub player_type: PlayerType,
    /// Stake or capital
    pub stake: Decimal,
    /// Current strategy
    pub current_strategy: Option<usize>,
}

/// Role a player takes in the game-theoretic model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerType {
    /// Validator
    Validator,
    /// Trader
    Trader,
    /// Liquidity Provider
    LiquidityProvider,
    /// Attacker
    Attacker,
}

/// A strategy available to players in the game-theoretic model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    /// Strategy name
    pub name: String,
    /// Payoff function (player stake → expected payoff)
    pub base_payoff: Decimal,
    /// Risk factor
    pub risk: Decimal,
    /// Cost to execute
    pub cost: Decimal,
}

impl GameTheoreticModel {
    /// Create a new game-theoretic model
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            strategies: Vec::new(),
        }
    }

    /// Add a player
    pub fn add_player(&mut self, player: Player) {
        self.players.insert(player.id.clone(), player);
    }

    /// Add a strategy
    pub fn add_strategy(&mut self, strategy: Strategy) {
        self.strategies.push(strategy);
    }

    /// Calculate payoff for a player using a strategy
    pub fn calculate_payoff(&self, player_id: &str, strategy_idx: usize) -> Decimal {
        let player = match self.players.get(player_id) {
            Some(p) => p,
            None => return dec!(0),
        };

        let strategy = match self.strategies.get(strategy_idx) {
            Some(s) => s,
            None => return dec!(0),
        };

        // Payoff = base_payoff × (stake factor) - cost - (risk × variance)
        let stake_factor = (player.stake / dec!(1000)).min(dec!(10)); // Cap scaling
        strategy.base_payoff * stake_factor - strategy.cost
    }

    /// Find Nash equilibrium (simplified)
    pub fn find_nash_equilibrium(&self) -> HashMap<String, usize> {
        let mut equilibrium = HashMap::new();

        for player_id in self.players.keys() {
            let mut best_strategy = 0;
            let mut best_payoff = dec!(0);

            for (idx, _) in self.strategies.iter().enumerate() {
                let payoff = self.calculate_payoff(player_id, idx);
                if payoff > best_payoff {
                    best_payoff = payoff;
                    best_strategy = idx;
                }
            }

            equilibrium.insert(player_id.clone(), best_strategy);
        }

        equilibrium
    }

    /// Check if honest behavior is incentivized
    pub fn is_honest_incentivized(&self, honest_strategy_idx: usize) -> bool {
        for player_id in self.players.keys() {
            let honest_payoff = self.calculate_payoff(player_id, honest_strategy_idx);

            for (idx, _) in self.strategies.iter().enumerate() {
                if idx != honest_strategy_idx {
                    let other_payoff = self.calculate_payoff(player_id, idx);
                    if other_payoff > honest_payoff {
                        return false; // Dishonest strategy is better
                    }
                }
            }
        }

        true
    }
}

impl Default for GameTheoreticModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Incentive alignment verifier
pub struct IncentiveAlignmentVerifier {
    /// Reward structure
    rewards: HashMap<String, Decimal>,
    /// Penalty structure
    penalties: HashMap<String, Decimal>,
}

impl IncentiveAlignmentVerifier {
    /// Create a new incentive alignment verifier
    pub fn new() -> Self {
        Self {
            rewards: HashMap::new(),
            penalties: HashMap::new(),
        }
    }

    /// Set reward for an action
    pub fn set_reward(&mut self, action: String, reward: Decimal) {
        self.rewards.insert(action, reward);
    }

    /// Set penalty for an action
    pub fn set_penalty(&mut self, action: String, penalty: Decimal) {
        self.penalties.insert(action, penalty);
    }

    /// Calculate net incentive for an action
    pub fn net_incentive(&self, action: &str) -> Decimal {
        let reward = self.rewards.get(action).copied().unwrap_or(dec!(0));
        let penalty = self.penalties.get(action).copied().unwrap_or(dec!(0));
        reward - penalty
    }

    /// Check if incentive structure is aligned
    pub fn check_alignment(&self, desired_action: &str, undesired_actions: &[String]) -> bool {
        let desired_incentive = self.net_incentive(desired_action);

        for action in undesired_actions {
            let undesired_incentive = self.net_incentive(action);
            if undesired_incentive >= desired_incentive {
                return false; // Undesired action has equal or better incentive
            }
        }

        true
    }

    /// Suggest incentive adjustments
    pub fn suggest_adjustments(
        &self,
        desired_action: &str,
        undesired_actions: &[String],
    ) -> Vec<IncentiveAdjustment> {
        let mut suggestions = Vec::new();
        let desired_incentive = self.net_incentive(desired_action);

        for action in undesired_actions {
            let undesired_incentive = self.net_incentive(action);

            if undesired_incentive >= desired_incentive {
                let gap = undesired_incentive - desired_incentive;

                suggestions.push(IncentiveAdjustment {
                    action: action.clone(),
                    current_incentive: undesired_incentive,
                    adjustment_type: AdjustmentType::IncreasePenalty,
                    recommended_change: gap + dec!(100), // Add margin
                    reason: format!(
                        "Undesired action {} has higher incentive than desired",
                        action
                    ),
                });
            }
        }

        suggestions
    }
}

impl Default for IncentiveAlignmentVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Recommendation to adjust a protocol incentive parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncentiveAdjustment {
    /// Action to adjust
    pub action: String,
    /// Current net incentive
    pub current_incentive: Decimal,
    /// Type of adjustment
    pub adjustment_type: AdjustmentType,
    /// Recommended change amount
    pub recommended_change: Decimal,
    /// Reason for adjustment
    pub reason: String,
}

/// Direction of an incentive adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdjustmentType {
    /// Increase reward
    IncreaseReward,
    /// Decrease reward
    DecreaseReward,
    /// Increase penalty
    IncreasePenalty,
    /// Decrease penalty
    DecreasePenalty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attack_vector() {
        let attack = AttackVector {
            name: "51% Attack".to_string(),
            attack_type: AttackType::Consensus51,
            cost: dec!(1000000),
            potential_profit: dec!(500000),
            success_probability: dec!(0.5),
            detection_probability: dec!(0.8),
            penalty: dec!(2000000),
        };

        // EV = (500k × 0.5) - (2M × 0.8) - 1M = 250k - 1.6M - 1M = -2.35M
        let ev = attack.expected_value();
        assert!(ev < dec!(0)); // Not rational

        assert!(!attack.is_rational());
    }

    #[test]
    fn test_rational_attack() {
        let attack = AttackVector {
            name: "Profitable Attack".to_string(),
            attack_type: AttackType::EconomicExploit,
            cost: dec!(10000),
            potential_profit: dec!(100000),
            success_probability: dec!(0.9),
            detection_probability: dec!(0.1),
            penalty: dec!(50000),
        };

        // EV = (100k × 0.9) - (50k × 0.1) - 10k = 90k - 5k - 10k = 75k
        let ev = attack.expected_value();
        assert!(ev > dec!(70000));
        assert!(attack.is_rational());
    }

    #[test]
    fn test_economic_security_analyzer() {
        let mut analyzer = EconomicSecurityAnalyzer::new(dec!(10000000), dec!(500000));

        analyzer.add_attack_vector(AttackVector {
            name: "Attack A".to_string(),
            attack_type: AttackType::FlashLoan,
            cost: dec!(50000),
            potential_profit: dec!(30000),
            success_probability: dec!(0.5),
            detection_probability: dec!(0.9),
            penalty: dec!(100000),
        });

        assert_eq!(analyzer.minimum_attack_cost(), dec!(50000));

        let factor = analyzer.security_factor();
        // 50000 / 10000000 = 0.005
        assert!(factor > dec!(0.004) && factor < dec!(0.006));
    }

    #[test]
    fn test_nakamoto_coefficient() {
        let analyzer = EconomicSecurityAnalyzer::new(dec!(1000000), dec!(50000));

        // 3 validators with 40%, 35%, 25% stake
        let stakes = vec![dec!(400000), dec!(350000), dec!(250000)];

        let coef = analyzer.nakamoto_coefficient(&stakes);
        // Need 2 validators to control >50%
        assert_eq!(coef, 2);
    }

    #[test]
    fn test_security_level() {
        let mut analyzer = EconomicSecurityAnalyzer::new(dec!(1000000), dec!(100000));

        // Add expensive attack
        analyzer.add_attack_vector(AttackVector {
            name: "Expensive Attack".to_string(),
            attack_type: AttackType::Consensus51,
            cost: dec!(5000000), // 5x TVL
            potential_profit: dec!(1000000),
            success_probability: dec!(0.3),
            detection_probability: dec!(0.9),
            penalty: dec!(10000000),
        });

        assert_eq!(analyzer.security_level(), SecurityLevel::High);
    }

    #[test]
    fn test_game_theoretic_model() {
        let mut model = GameTheoreticModel::new();

        model.add_player(Player {
            id: "validator1".to_string(),
            player_type: PlayerType::Validator,
            stake: dec!(100000),
            current_strategy: None,
        });

        model.add_strategy(Strategy {
            name: "Honest Validation".to_string(),
            base_payoff: dec!(100),
            risk: dec!(0.1),
            cost: dec!(10),
        });

        model.add_strategy(Strategy {
            name: "Dishonest Validation".to_string(),
            base_payoff: dec!(50),
            risk: dec!(0.5),
            cost: dec!(20),
        });

        let payoff = model.calculate_payoff("validator1", 0);
        assert!(payoff > dec!(0));
    }

    #[test]
    fn test_nash_equilibrium() {
        let mut model = GameTheoreticModel::new();

        model.add_player(Player {
            id: "player1".to_string(),
            player_type: PlayerType::Trader,
            stake: dec!(10000),
            current_strategy: None,
        });

        model.add_strategy(Strategy {
            name: "Strategy A".to_string(),
            base_payoff: dec!(100),
            risk: dec!(0.1),
            cost: dec!(10),
        });

        model.add_strategy(Strategy {
            name: "Strategy B".to_string(),
            base_payoff: dec!(80),
            risk: dec!(0.05),
            cost: dec!(5),
        });

        let equilibrium = model.find_nash_equilibrium();
        assert!(equilibrium.contains_key("player1"));
    }

    #[test]
    fn test_honest_incentivization() {
        let mut model = GameTheoreticModel::new();

        model.add_player(Player {
            id: "validator".to_string(),
            player_type: PlayerType::Validator,
            stake: dec!(100000),
            current_strategy: None,
        });

        model.add_strategy(Strategy {
            name: "Honest".to_string(),
            base_payoff: dec!(200),
            risk: dec!(0.1),
            cost: dec!(10),
        });

        model.add_strategy(Strategy {
            name: "Dishonest".to_string(),
            base_payoff: dec!(100), // Lower payoff
            risk: dec!(0.5),
            cost: dec!(20),
        });

        assert!(model.is_honest_incentivized(0));
    }

    #[test]
    fn test_incentive_alignment() {
        let mut verifier = IncentiveAlignmentVerifier::new();

        verifier.set_reward("stake".to_string(), dec!(1000));
        verifier.set_penalty("attack".to_string(), dec!(5000));

        let stake_incentive = verifier.net_incentive("stake");
        let attack_incentive = verifier.net_incentive("attack");

        assert_eq!(stake_incentive, dec!(1000));
        assert_eq!(attack_incentive, dec!(-5000));
    }

    #[test]
    fn test_alignment_check() {
        let mut verifier = IncentiveAlignmentVerifier::new();

        verifier.set_reward("honest_behavior".to_string(), dec!(1000));
        verifier.set_reward("malicious_behavior".to_string(), dec!(500));
        verifier.set_penalty("malicious_behavior".to_string(), dec!(2000));

        let undesired = vec!["malicious_behavior".to_string()];

        assert!(verifier.check_alignment("honest_behavior", &undesired));
    }

    #[test]
    fn test_misaligned_incentives() {
        let mut verifier = IncentiveAlignmentVerifier::new();

        verifier.set_reward("desired".to_string(), dec!(100));
        verifier.set_reward("undesired".to_string(), dec!(200)); // Higher reward!

        let undesired = vec!["undesired".to_string()];

        assert!(!verifier.check_alignment("desired", &undesired));
    }

    #[test]
    fn test_incentive_adjustment_suggestions() {
        let mut verifier = IncentiveAlignmentVerifier::new();

        verifier.set_reward("desired".to_string(), dec!(100));
        verifier.set_reward("undesired".to_string(), dec!(200));

        let undesired = vec!["undesired".to_string()];
        let suggestions = verifier.suggest_adjustments("desired", &undesired);

        assert!(!suggestions.is_empty());
        assert_eq!(
            suggestions[0].adjustment_type,
            AdjustmentType::IncreasePenalty
        );
    }
}

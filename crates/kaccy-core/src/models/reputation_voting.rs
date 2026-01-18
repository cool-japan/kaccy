//! Reputation-weighted voting system
//!
//! Extends the governance system with reputation-based vote weighting
//! to give more influence to proven contributors and domain experts.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::Result;

/// Domain of expertise for reputation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExpertiseDomain {
    /// Technical development and protocol design
    Technical,
    /// Economic and tokenomics
    Economic,
    /// Security and auditing
    Security,
    /// Community and governance
    Governance,
    /// Marketing and growth
    Marketing,
    /// General platform usage
    General,
}

/// Historical participation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipationHistory {
    /// User ID
    pub user_id: Uuid,

    /// Total proposals voted on
    pub total_votes: u64,

    /// Proposals where user voted with majority
    pub aligned_votes: u64,

    /// Proposals created by user
    pub proposals_created: u64,

    /// Proposals created that passed
    pub proposals_passed: u64,

    /// Consecutive votes cast
    pub voting_streak: u64,

    /// Quality score (0-100) based on vote alignment
    pub quality_score: Decimal,
}

impl ParticipationHistory {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            total_votes: 0,
            aligned_votes: 0,
            proposals_created: 0,
            proposals_passed: 0,
            voting_streak: 0,
            quality_score: dec!(50), // Start at median
        }
    }

    /// Update participation after a vote
    pub fn record_vote(&mut self, voted_with_majority: bool) {
        self.total_votes += 1;
        if voted_with_majority {
            self.aligned_votes += 1;
            self.voting_streak += 1;
        } else {
            self.voting_streak = 0;
        }

        // Update quality score
        self.update_quality_score();
    }

    /// Update participation after creating a proposal
    pub fn record_proposal(&mut self, passed: bool) {
        self.proposals_created += 1;
        if passed {
            self.proposals_passed += 1;
        }
    }

    /// Calculate quality score based on participation
    fn update_quality_score(&mut self) {
        if self.total_votes == 0 {
            self.quality_score = dec!(50);
            return;
        }

        // Base score from alignment rate
        let alignment_rate = Decimal::from(self.aligned_votes) / Decimal::from(self.total_votes);
        let mut score = alignment_rate * dec!(60); // 0-60 points

        // Bonus for proposal success rate
        if self.proposals_created > 0 {
            let proposal_rate =
                Decimal::from(self.proposals_passed) / Decimal::from(self.proposals_created);
            score += proposal_rate * dec!(20); // 0-20 points
        }

        // Bonus for voting streak
        let streak_bonus = Decimal::from(self.voting_streak.min(10)) * dec!(2); // 0-20 points
        score += streak_bonus;

        self.quality_score = score.min(dec!(100));
    }

    /// Calculate participation rate
    pub fn participation_rate(&self, total_proposals: u64) -> Decimal {
        if total_proposals == 0 {
            return Decimal::ZERO;
        }
        Decimal::from(self.total_votes) / Decimal::from(total_proposals)
    }
}

/// Domain-specific reputation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainReputation {
    /// Domain
    pub domain: ExpertiseDomain,

    /// Reputation score (0-100)
    pub score: Decimal,

    /// Number of contributions in this domain
    pub contributions: u64,

    /// Verified expert status
    pub is_verified: bool,
}

impl DomainReputation {
    pub fn new(domain: ExpertiseDomain) -> Self {
        Self {
            domain,
            score: dec!(0),
            contributions: 0,
            is_verified: false,
        }
    }

    /// Add a contribution and update score
    pub fn add_contribution(&mut self, quality: Decimal) {
        self.contributions += 1;

        // Weighted average: recent contributions matter more
        let weight = dec!(0.2); // 20% from new, 80% from existing
        self.score = self.score * (dec!(1) - weight) + quality * weight;
        self.score = self.score.min(dec!(100));
    }
}

/// Reputation-weighted voter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationVoter {
    pub user_id: Uuid,

    /// Base voting power (from token holdings)
    pub base_voting_power: Decimal,

    /// Overall reputation score (0-100)
    pub overall_reputation: Decimal,

    /// Domain-specific reputations
    pub domain_reputations: HashMap<ExpertiseDomain, DomainReputation>,

    /// Historical participation
    pub participation_history: ParticipationHistory,
}

impl ReputationVoter {
    pub fn new(user_id: Uuid, base_voting_power: Decimal) -> Self {
        Self {
            user_id,
            base_voting_power,
            overall_reputation: dec!(50), // Start at median
            domain_reputations: HashMap::new(),
            participation_history: ParticipationHistory::new(user_id),
        }
    }

    /// Get domain reputation, creating if doesn't exist
    pub fn get_or_create_domain_reputation(
        &mut self,
        domain: ExpertiseDomain,
    ) -> &mut DomainReputation {
        self.domain_reputations
            .entry(domain)
            .or_insert_with(|| DomainReputation::new(domain))
    }

    /// Calculate overall reputation from domain scores
    pub fn update_overall_reputation(&mut self) {
        if self.domain_reputations.is_empty() {
            self.overall_reputation = dec!(50);
            return;
        }

        let total: Decimal = self.domain_reputations.values().map(|r| r.score).sum();
        let avg = total / Decimal::from(self.domain_reputations.len());

        // Combine with quality score from participation
        self.overall_reputation =
            (avg * dec!(0.7)) + (self.participation_history.quality_score * dec!(0.3));
    }
}

/// Reputation-weighted voting calculator
#[derive(Debug)]
pub struct ReputationWeightedVoting {
    /// Base weight for token voting power (default 0.5)
    pub token_weight: Decimal,

    /// Weight for reputation (default 0.3)
    pub reputation_weight: Decimal,

    /// Weight for participation history (default 0.2)
    pub participation_weight: Decimal,

    /// Minimum reputation to get bonus
    pub min_reputation_threshold: Decimal,

    /// Maximum voting power multiplier from reputation
    pub max_reputation_multiplier: Decimal,
}

impl Default for ReputationWeightedVoting {
    fn default() -> Self {
        Self {
            token_weight: dec!(0.5),
            reputation_weight: dec!(0.3),
            participation_weight: dec!(0.2),
            min_reputation_threshold: dec!(60),
            max_reputation_multiplier: dec!(2.0), // 2x max boost
        }
    }
}

impl ReputationWeightedVoting {
    /// Calculate effective voting power for a voter
    pub fn calculate_voting_power(&self, voter: &ReputationVoter) -> Result<Decimal> {
        // Component 1: Token-based voting power
        let token_component = voter.base_voting_power * self.token_weight;

        // Component 2: Reputation-based multiplier
        let reputation_multiplier = if voter.overall_reputation >= self.min_reputation_threshold {
            let reputation_boost =
                (voter.overall_reputation - self.min_reputation_threshold) / dec!(100);
            dec!(1.0) + (reputation_boost * self.max_reputation_multiplier)
        } else {
            dec!(1.0)
        };

        // Component 3: Participation bonus
        let participation_bonus =
            self.calculate_participation_bonus(&voter.participation_history)?;

        // Combined voting power
        let reputation_component =
            voter.base_voting_power * self.reputation_weight * reputation_multiplier;
        let participation_component =
            voter.base_voting_power * self.participation_weight * participation_bonus;

        let total_voting_power = token_component + reputation_component + participation_component;

        Ok(total_voting_power)
    }

    /// Calculate domain-specific voting power
    pub fn calculate_domain_voting_power(
        &self,
        voter: &ReputationVoter,
        domain: ExpertiseDomain,
    ) -> Result<Decimal> {
        let base_power = self.calculate_voting_power(voter)?;

        // Apply domain expertise multiplier
        if let Some(domain_rep) = voter.domain_reputations.get(&domain) {
            if domain_rep.is_verified {
                // Verified experts get extra boost
                let domain_multiplier = dec!(1.0) + (domain_rep.score / dec!(100)) * dec!(0.5); // Up to 1.5x
                return Ok(base_power * domain_multiplier);
            } else if domain_rep.score >= dec!(70) {
                // High domain score gets moderate boost
                let domain_multiplier = dec!(1.0) + (domain_rep.score / dec!(100)) * dec!(0.25); // Up to 1.25x
                return Ok(base_power * domain_multiplier);
            }
        }

        Ok(base_power)
    }

    /// Calculate participation bonus multiplier
    fn calculate_participation_bonus(&self, history: &ParticipationHistory) -> Result<Decimal> {
        let mut bonus = dec!(1.0);

        // Bonus for high quality score
        if history.quality_score >= dec!(70) {
            bonus += dec!(0.2);
        } else if history.quality_score >= dec!(60) {
            bonus += dec!(0.1);
        }

        // Bonus for voting streak
        if history.voting_streak >= 10 {
            bonus += dec!(0.15);
        } else if history.voting_streak >= 5 {
            bonus += dec!(0.05);
        }

        // Bonus for successful proposals
        if history.proposals_passed >= 3 {
            bonus += dec!(0.1);
        }

        Ok(bonus)
    }

    /// Calculate vote quality score after proposal resolution
    pub fn score_vote_quality(
        &self,
        voted_for: bool,
        proposal_passed: bool,
        voter_reputation: Decimal,
    ) -> Result<Decimal> {
        // Did the voter vote with the majority?
        let voted_with_majority = voted_for == proposal_passed;

        let mut quality = if voted_with_majority {
            dec!(70) // Base score for voting with majority
        } else {
            dec!(30) // Lower score for voting against majority
        };

        // Adjust based on voter reputation
        // High reputation voters making contrarian calls might be right for other reasons
        if !voted_with_majority && voter_reputation > dec!(80) {
            quality += dec!(20); // Expert dissent is valuable
        }

        Ok(quality.min(dec!(100)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_participation_history() {
        let mut history = ParticipationHistory::new(Uuid::new_v4());

        // Record some votes
        history.record_vote(true);
        history.record_vote(true);
        history.record_vote(false);
        history.record_vote(true);

        assert_eq!(history.total_votes, 4);
        assert_eq!(history.aligned_votes, 3);
        assert_eq!(history.voting_streak, 1); // Reset after the false vote

        // Quality score should be reasonable (3/4 = 75% alignment)
        assert!(history.quality_score > Decimal::ZERO);
        assert!(history.quality_score <= dec!(100));
    }

    #[test]
    fn test_domain_reputation() {
        let mut domain_rep = DomainReputation::new(ExpertiseDomain::Technical);

        domain_rep.add_contribution(dec!(80));
        domain_rep.add_contribution(dec!(90));
        domain_rep.add_contribution(dec!(70));

        assert_eq!(domain_rep.contributions, 3);
        assert!(domain_rep.score > dec!(0));
        assert!(domain_rep.score <= dec!(100));
    }

    #[test]
    fn test_basic_voting_power() {
        let calculator = ReputationWeightedVoting::default();
        let voter = ReputationVoter::new(Uuid::new_v4(), dec!(100));

        let voting_power = calculator.calculate_voting_power(&voter).unwrap();

        // Should have base voting power from tokens
        assert!(voting_power > Decimal::ZERO);
        assert!(voting_power <= dec!(150)); // Shouldn't exceed reasonable bounds
    }

    #[test]
    fn test_reputation_boost() {
        let calculator = ReputationWeightedVoting::default();
        let mut voter = ReputationVoter::new(Uuid::new_v4(), dec!(100));

        // Low reputation voter
        voter.overall_reputation = dec!(40);
        let low_power = calculator.calculate_voting_power(&voter).unwrap();

        // High reputation voter
        voter.overall_reputation = dec!(90);
        let high_power = calculator.calculate_voting_power(&voter).unwrap();

        // High reputation should have more voting power
        assert!(high_power > low_power);
    }

    #[test]
    fn test_domain_expertise_boost() {
        let calculator = ReputationWeightedVoting::default();
        let mut voter = ReputationVoter::new(Uuid::new_v4(), dec!(100));
        voter.overall_reputation = dec!(70);

        // Add technical domain expertise
        let mut tech_rep = DomainReputation::new(ExpertiseDomain::Technical);
        tech_rep.score = dec!(90);
        tech_rep.is_verified = true;
        voter
            .domain_reputations
            .insert(ExpertiseDomain::Technical, tech_rep);

        // Calculate domain-specific voting power
        let tech_power = calculator
            .calculate_domain_voting_power(&voter, ExpertiseDomain::Technical)
            .unwrap();

        let general_power = calculator
            .calculate_domain_voting_power(&voter, ExpertiseDomain::General)
            .unwrap();

        // Technical domain should have higher power
        assert!(tech_power > general_power);
    }

    #[test]
    fn test_participation_bonus() {
        let calculator = ReputationWeightedVoting::default();
        let mut voter = ReputationVoter::new(Uuid::new_v4(), dec!(100));

        // Low participation
        let low_power = calculator.calculate_voting_power(&voter).unwrap();

        // High participation with quality
        voter.participation_history.total_votes = 50;
        voter.participation_history.aligned_votes = 40;
        voter.participation_history.voting_streak = 10;
        voter.participation_history.proposals_created = 5;
        voter.participation_history.proposals_passed = 4;
        voter.participation_history.update_quality_score();

        let high_power = calculator.calculate_voting_power(&voter).unwrap();

        // High participation should increase voting power
        assert!(high_power > low_power);
    }

    #[test]
    fn test_vote_quality_scoring() {
        let calculator = ReputationWeightedVoting::default();

        // Voting with majority
        let quality_majority = calculator.score_vote_quality(true, true, dec!(50)).unwrap();
        assert!(quality_majority >= dec!(70));

        // Voting against majority
        let quality_minority = calculator
            .score_vote_quality(true, false, dec!(50))
            .unwrap();
        assert!(quality_minority < quality_majority);

        // Expert dissent
        let quality_expert_dissent = calculator
            .score_vote_quality(true, false, dec!(90))
            .unwrap();
        assert!(quality_expert_dissent > quality_minority);
    }

    #[test]
    fn test_update_overall_reputation() {
        let mut voter = ReputationVoter::new(Uuid::new_v4(), dec!(100));

        let mut tech_rep = DomainReputation::new(ExpertiseDomain::Technical);
        tech_rep.score = dec!(80);
        voter
            .domain_reputations
            .insert(ExpertiseDomain::Technical, tech_rep);

        let mut econ_rep = DomainReputation::new(ExpertiseDomain::Economic);
        econ_rep.score = dec!(60);
        voter
            .domain_reputations
            .insert(ExpertiseDomain::Economic, econ_rep);

        voter.update_overall_reputation();

        // Overall reputation should be weighted average of domain scores and quality
        assert!(voter.overall_reputation > dec!(50));
        assert!(voter.overall_reputation < dec!(80));
    }
}

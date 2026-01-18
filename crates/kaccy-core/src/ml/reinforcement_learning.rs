//! Reinforcement Learning Framework for Trading
//!
//! This module provides RL abstractions for:
//! - RL-based trading agents
//! - Multi-agent trading systems
//! - Adversarial training
//! - Transfer learning across markets

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RL Algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RLAlgorithm {
    /// Deep Q-Network
    DQN,
    /// Proximal Policy Optimization
    PPO,
    /// Actor-Critic
    A3C,
    /// Soft Actor-Critic
    SAC,
    /// Twin Delayed DDPG
    TD3,
}

/// Trading action in RL environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
    BuyAmount(u32),  // Buy specific amount (discretized)
    SellAmount(u32), // Sell specific amount (discretized)
}

/// Market state observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    /// Current price
    pub price: f64,
    /// Price history (normalized)
    pub price_history: Vec<f64>,
    /// Volume history
    pub volume_history: Vec<f64>,
    /// Technical indicators
    pub indicators: HashMap<String, f64>,
    /// Current position
    pub position: f64,
    /// Available capital
    pub capital: f64,
}

impl MarketState {
    pub fn to_feature_vector(&self) -> Vec<f64> {
        let mut features = vec![self.price, self.position, self.capital];
        features.extend(&self.price_history);
        features.extend(&self.volume_history);
        features.extend(self.indicators.values());
        features
    }
}

/// RL Agent for trading
pub struct RLTradingAgent {
    pub algorithm: RLAlgorithm,
    pub state_dim: usize,
    pub action_dim: usize,
    pub learning_rate: f64,
    pub discount_factor: f64,
    pub epsilon: f64, // Exploration rate
    /// Experience replay buffer
    pub replay_buffer: Vec<Experience>,
    pub total_reward: f64,
}

/// Experience tuple for replay buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    pub state: Vec<f64>,
    pub action: TradingAction,
    pub reward: f64,
    pub next_state: Vec<f64>,
    pub done: bool,
}

impl RLTradingAgent {
    pub fn new(algorithm: RLAlgorithm, state_dim: usize, action_dim: usize) -> Self {
        Self {
            algorithm,
            state_dim,
            action_dim,
            learning_rate: 0.001,
            discount_factor: 0.99,
            epsilon: 0.1,
            replay_buffer: Vec::new(),
            total_reward: 0.0,
        }
    }

    /// Select action based on current policy (epsilon-greedy)
    pub fn select_action(&self, state: &MarketState) -> TradingAction {
        // NOTE: In production, this would use the trained neural network
        // For now, simple epsilon-greedy strategy
        if rand::random::<f64>() < self.epsilon {
            // Explore: random action
            match rand::random::<u8>() % 3 {
                0 => TradingAction::Buy,
                1 => TradingAction::Sell,
                _ => TradingAction::Hold,
            }
        } else {
            // Exploit: use policy (simplified - would use network prediction)
            if state.price < state.price_history.last().copied().unwrap_or(state.price) {
                TradingAction::Buy
            } else if state.price > state.price_history.last().copied().unwrap_or(state.price) {
                TradingAction::Sell
            } else {
                TradingAction::Hold
            }
        }
    }

    /// Store experience in replay buffer
    pub fn store_experience(&mut self, experience: Experience) {
        self.replay_buffer.push(experience);

        // Limit buffer size
        if self.replay_buffer.len() > 10000 {
            self.replay_buffer.remove(0);
        }
    }

    /// Train the agent (would update network weights)
    pub fn train(&mut self, batch_size: usize) -> Result<f64> {
        if self.replay_buffer.len() < batch_size {
            return Ok(0.0); // Not enough experiences
        }

        // NOTE: In production, this would:
        // 1. Sample mini-batch from replay buffer
        // 2. Calculate Q-values/advantage
        // 3. Update network weights via backpropagation
        // For now, placeholder returns average reward
        let avg_reward = self.total_reward / self.replay_buffer.len() as f64;
        Ok(avg_reward)
    }

    /// Update exploration rate (decay epsilon)
    pub fn decay_epsilon(&mut self, decay_rate: f64) {
        self.epsilon = (self.epsilon * decay_rate).max(0.01);
    }
}

/// Multi-agent trading system
pub struct MultiAgentSystem {
    pub agents: Vec<RLTradingAgent>,
    pub cooperation_mode: CooperationMode,
}

/// Cooperation mode for multi-agent systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CooperationMode {
    /// Agents compete against each other
    Competitive,
    /// Agents cooperate for shared goal
    Cooperative,
    /// Mix of competition and cooperation
    Mixed,
}

impl MultiAgentSystem {
    pub fn new(
        num_agents: usize,
        algorithm: RLAlgorithm,
        cooperation_mode: CooperationMode,
    ) -> Self {
        let agents = (0..num_agents)
            .map(|_| RLTradingAgent::new(algorithm, 10, 3))
            .collect();

        Self {
            agents,
            cooperation_mode,
        }
    }

    /// Coordinate actions among agents
    pub fn coordinate_actions(&self, states: &[MarketState]) -> Vec<TradingAction> {
        match self.cooperation_mode {
            CooperationMode::Competitive => {
                // Each agent acts independently
                self.agents
                    .iter()
                    .zip(states.iter())
                    .map(|(agent, state)| agent.select_action(state))
                    .collect()
            }
            CooperationMode::Cooperative => {
                // Agents share information and coordinate
                // Simplified: use majority voting
                let actions: Vec<_> = self
                    .agents
                    .iter()
                    .zip(states.iter())
                    .map(|(agent, state)| agent.select_action(state))
                    .collect();

                // Return same action for all (simplified cooperation)
                vec![actions[0]; actions.len()]
            }
            CooperationMode::Mixed => {
                // Some cooperation, some competition
                self.agents
                    .iter()
                    .zip(states.iter())
                    .map(|(agent, state)| agent.select_action(state))
                    .collect()
            }
        }
    }
}

/// Adversarial training setup
pub struct AdversarialTraining {
    pub generator_agent: RLTradingAgent,
    pub discriminator_agent: RLTradingAgent,
    pub num_iterations: usize,
}

impl AdversarialTraining {
    pub fn new(state_dim: usize, action_dim: usize) -> Self {
        Self {
            generator_agent: RLTradingAgent::new(RLAlgorithm::PPO, state_dim, action_dim),
            discriminator_agent: RLTradingAgent::new(RLAlgorithm::DQN, state_dim, action_dim),
            num_iterations: 0,
        }
    }

    /// Train both agents adversarially
    pub fn train_adversarial(&mut self, batch_size: usize) -> Result<(f64, f64)> {
        // Train generator to fool discriminator
        let gen_loss = self.generator_agent.train(batch_size)?;

        // Train discriminator to detect generator's actions
        let disc_loss = self.discriminator_agent.train(batch_size)?;

        self.num_iterations += 1;

        Ok((gen_loss, disc_loss))
    }
}

/// Transfer learning across markets
pub struct TransferLearning {
    pub source_agent: RLTradingAgent,
    pub target_agent: RLTradingAgent,
    pub transfer_method: TransferMethod,
}

/// Transfer learning methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferMethod {
    /// Transfer all weights
    FullTransfer,
    /// Transfer only feature extraction layers
    FeatureTransfer,
    /// Fine-tune on target market
    FineTune,
    /// Progressive neural network
    Progressive,
}

impl TransferLearning {
    pub fn new(
        source_algorithm: RLAlgorithm,
        state_dim: usize,
        action_dim: usize,
        method: TransferMethod,
    ) -> Self {
        Self {
            source_agent: RLTradingAgent::new(source_algorithm, state_dim, action_dim),
            target_agent: RLTradingAgent::new(source_algorithm, state_dim, action_dim),
            transfer_method: method,
        }
    }

    /// Transfer knowledge from source to target
    pub fn transfer_knowledge(&mut self) -> Result<()> {
        match self.transfer_method {
            TransferMethod::FullTransfer => {
                // Copy all learned parameters
                // NOTE: Would copy network weights in production
                self.target_agent.total_reward = self.source_agent.total_reward;
                Ok(())
            }
            TransferMethod::FeatureTransfer => {
                // Transfer feature extraction layers only
                // NOTE: Would copy only early layers in production
                Ok(())
            }
            TransferMethod::FineTune => {
                // Copy weights and allow fine-tuning
                self.target_agent.learning_rate = self.source_agent.learning_rate * 0.1;
                Ok(())
            }
            TransferMethod::Progressive => {
                // Add new columns to network
                // NOTE: Would implement progressive neural network in production
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rl_agent_creation() {
        let agent = RLTradingAgent::new(RLAlgorithm::DQN, 10, 3);
        assert_eq!(agent.algorithm, RLAlgorithm::DQN);
        assert_eq!(agent.state_dim, 10);
        assert_eq!(agent.action_dim, 3);
    }

    #[test]
    fn test_action_selection() {
        let agent = RLTradingAgent::new(RLAlgorithm::PPO, 10, 3);
        let state = MarketState {
            price: 100.0,
            price_history: vec![95.0, 98.0, 99.0],
            volume_history: vec![1000.0, 1100.0, 1050.0],
            indicators: HashMap::new(),
            position: 0.0,
            capital: 10000.0,
        };

        let action = agent.select_action(&state);
        assert!(matches!(
            action,
            TradingAction::Buy | TradingAction::Sell | TradingAction::Hold
        ));
    }

    #[test]
    fn test_experience_storage() {
        let mut agent = RLTradingAgent::new(RLAlgorithm::DQN, 10, 3);

        let exp = Experience {
            state: vec![1.0; 10],
            action: TradingAction::Buy,
            reward: 1.5,
            next_state: vec![1.1; 10],
            done: false,
        };

        agent.store_experience(exp);
        assert_eq!(agent.replay_buffer.len(), 1);
    }

    #[test]
    fn test_epsilon_decay() {
        let mut agent = RLTradingAgent::new(RLAlgorithm::SAC, 10, 3);
        let initial_epsilon = agent.epsilon;

        agent.decay_epsilon(0.95);
        assert!(agent.epsilon < initial_epsilon);
        assert!(agent.epsilon >= 0.01); // Minimum bound
    }

    #[test]
    fn test_multi_agent_system() {
        let mas = MultiAgentSystem::new(3, RLAlgorithm::PPO, CooperationMode::Competitive);
        assert_eq!(mas.agents.len(), 3);
        assert_eq!(mas.cooperation_mode, CooperationMode::Competitive);
    }

    #[test]
    fn test_multi_agent_coordination() {
        let mas = MultiAgentSystem::new(2, RLAlgorithm::DQN, CooperationMode::Cooperative);

        let states = vec![
            MarketState {
                price: 100.0,
                price_history: vec![95.0],
                volume_history: vec![1000.0],
                indicators: HashMap::new(),
                position: 0.0,
                capital: 10000.0,
            },
            MarketState {
                price: 101.0,
                price_history: vec![96.0],
                volume_history: vec![1100.0],
                indicators: HashMap::new(),
                position: 0.0,
                capital: 10000.0,
            },
        ];

        let actions = mas.coordinate_actions(&states);
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_adversarial_training() {
        let adv_training = AdversarialTraining::new(10, 3);
        assert_eq!(adv_training.num_iterations, 0);

        // Would train in production, placeholder test
        assert_eq!(adv_training.generator_agent.state_dim, 10);
        assert_eq!(adv_training.discriminator_agent.state_dim, 10);
    }

    #[test]
    fn test_transfer_learning() {
        let mut transfer =
            TransferLearning::new(RLAlgorithm::PPO, 10, 3, TransferMethod::FullTransfer);

        transfer.source_agent.total_reward = 100.0;
        transfer.transfer_knowledge().unwrap();

        // In full transfer mode, target should inherit source reward
        assert_eq!(transfer.target_agent.total_reward, 100.0);
    }

    #[test]
    fn test_market_state_feature_vector() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 65.0);
        indicators.insert("macd".to_string(), 0.5);

        let state = MarketState {
            price: 100.0,
            price_history: vec![95.0, 98.0],
            volume_history: vec![1000.0, 1100.0],
            indicators,
            position: 10.0,
            capital: 5000.0,
        };

        let features = state.to_feature_vector();
        assert!(features.len() >= 7); // price + position + capital + histories + indicators
    }
}

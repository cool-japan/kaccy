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
    /// Buy at market price
    Buy,
    /// Sell at market price
    Sell,
    /// Take no action this step
    Hold,
    /// Buy a discretised quantity
    BuyAmount(u32),
    /// Sell a discretised quantity
    SellAmount(u32),
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
    /// Flatten all state fields into a single feature vector for the network
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
    /// Which RL algorithm this agent uses
    pub algorithm: RLAlgorithm,
    /// Dimensionality of the input state vector
    pub state_dim: usize,
    /// Number of discrete actions available
    pub action_dim: usize,
    /// Optimizer learning rate
    pub learning_rate: f64,
    /// Reward discount factor (gamma)
    pub discount_factor: f64,
    /// Epsilon for epsilon-greedy exploration
    pub epsilon: f64,
    /// Experience replay buffer
    pub replay_buffer: Vec<Experience>,
    /// Cumulative reward across all steps
    pub total_reward: f64,
    /// Learned action-value function `Q(s, a)`.
    pub q_function: LinearQFunction,
}

/// Linear action-value function approximator: `Q(s, a) = w_a . s + b_a`.
///
/// This is a genuine parameterised Q-function trained with semi-gradient
/// temporal-difference (Q-learning) updates. Each action `a` owns a weight
/// vector `w_a` (length `state_dim`) and a bias `b_a`; the action-value is the
/// affine score of the state features. The [`td_update`](LinearQFunction::td_update)
/// method performs a real gradient step that moves `Q(s, a)` toward the Bellman
/// target `r + gamma * max_a' Q(s', a')`, which is exactly linear-function-
/// approximation Q-learning (Sutton & Barto, ch. 10-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearQFunction {
    /// Per-action weight vectors, shape `[action_dim][state_dim]`.
    weights: Vec<Vec<f64>>,
    /// Per-action bias terms, length `action_dim`.
    bias: Vec<f64>,
    /// Input feature dimension.
    state_dim: usize,
    /// Number of discrete actions.
    action_dim: usize,
}

impl LinearQFunction {
    /// Create a new approximator with small random weights and zero biases.
    pub fn new(state_dim: usize, action_dim: usize) -> Self {
        use rand::RngExt;
        let mut rng = rand::rng();
        let scale = (1.0 / (state_dim.max(1) as f64)).sqrt();

        let weights = (0..action_dim)
            .map(|_| {
                (0..state_dim)
                    .map(|_| rng.random_range(-scale..scale))
                    .collect()
            })
            .collect();

        Self {
            weights,
            bias: vec![0.0; action_dim],
            state_dim,
            action_dim,
        }
    }

    /// Create an approximator with all weights and biases set to zero.
    ///
    /// Every action-value starts at exactly zero, giving a deterministic,
    /// reproducible initial policy (useful for warm-starting and for tests that
    /// must reason about the pre-training behaviour).
    pub fn zeros(state_dim: usize, action_dim: usize) -> Self {
        Self {
            weights: vec![vec![0.0; state_dim]; action_dim],
            bias: vec![0.0; action_dim],
            state_dim,
            action_dim,
        }
    }

    /// Coerce an arbitrary feature vector to the fixed input width.
    fn fit_features(&self, features: &[f64]) -> Vec<f64> {
        let mut input = vec![0.0; self.state_dim];
        for (slot, &value) in input.iter_mut().zip(features.iter()) {
            *slot = value;
        }
        input
    }

    /// Action-value of a single action for the given features.
    pub fn q_value(&self, features: &[f64], action: usize) -> f64 {
        if action >= self.action_dim {
            return f64::NEG_INFINITY;
        }
        let input = self.fit_features(features);
        let mut sum = self.bias[action];
        for (w, x) in self.weights[action].iter().zip(input.iter()) {
            sum += w * x;
        }
        sum
    }

    /// Action-values for every action.
    pub fn q_values(&self, features: &[f64]) -> Vec<f64> {
        let input = self.fit_features(features);
        self.weights
            .iter()
            .zip(self.bias.iter())
            .map(|(w, &b)| {
                b + w
                    .iter()
                    .zip(input.iter())
                    .map(|(wi, xi)| wi * xi)
                    .sum::<f64>()
            })
            .collect()
    }

    /// Greedy action: `argmax_a Q(s, a)`.
    pub fn greedy_action(&self, features: &[f64]) -> usize {
        let values = self.q_values(features);
        let mut best = 0usize;
        let mut best_val = f64::NEG_INFINITY;
        for (idx, &v) in values.iter().enumerate() {
            if v > best_val {
                best_val = v;
                best = idx;
            }
        }
        best
    }

    /// Maximum action-value over all actions (the bootstrap term).
    pub fn max_q_value(&self, features: &[f64]) -> f64 {
        self.q_values(features)
            .into_iter()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Perform one semi-gradient Q-learning update for `(s, a, target)`.
    ///
    /// Moves `Q(s, a)` toward `target` by descending the squared TD error:
    /// `w_a += lr * (target - Q(s,a)) * s` and `b_a += lr * (target - Q(s,a))`.
    /// Returns the TD error `target - Q(s, a)` measured before the update.
    pub fn td_update(
        &mut self,
        features: &[f64],
        action: usize,
        target: f64,
        learning_rate: f64,
    ) -> f64 {
        if action >= self.action_dim {
            return 0.0;
        }
        let input = self.fit_features(features);
        let predicted = self.q_value(&input, action);
        let td_error = target - predicted;
        let step = learning_rate * td_error;

        for (w, &x) in self.weights[action].iter_mut().zip(input.iter()) {
            *w += step * x;
        }
        self.bias[action] += step;

        td_error
    }

    /// Number of discrete actions represented.
    pub fn action_dim(&self) -> usize {
        self.action_dim
    }
}

/// Experience tuple for replay buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    /// State observation at time t
    pub state: Vec<f64>,
    /// Action taken at time t
    pub action: TradingAction,
    /// Reward received after taking the action
    pub reward: f64,
    /// State observation at time t+1
    pub next_state: Vec<f64>,
    /// Whether this transition ended the episode
    pub done: bool,
}

impl RLTradingAgent {
    /// Create a new untrained RL trading agent
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
            q_function: LinearQFunction::new(state_dim, action_dim),
        }
    }

    /// Map a discrete action index to its [`TradingAction`].
    ///
    /// Index `0 -> Buy`, `1 -> Sell`, `2 -> Hold`; any further indices map to
    /// discretised `BuyAmount` / `SellAmount` actions so the whole `action_dim`
    /// is representable.
    pub fn action_from_index(index: usize) -> TradingAction {
        match index {
            0 => TradingAction::Buy,
            1 => TradingAction::Sell,
            2 => TradingAction::Hold,
            n if n % 2 == 1 => TradingAction::BuyAmount(((n - 1) / 2) as u32),
            n => TradingAction::SellAmount(((n - 2) / 2) as u32),
        }
    }

    /// Map a [`TradingAction`] back to its discrete index (inverse of
    /// [`action_from_index`](Self::action_from_index)).
    pub fn action_to_index(action: TradingAction) -> usize {
        match action {
            TradingAction::Buy => 0,
            TradingAction::Sell => 1,
            TradingAction::Hold => 2,
            TradingAction::BuyAmount(q) => 2 * q as usize + 1,
            TradingAction::SellAmount(q) => 2 * q as usize + 2,
        }
    }

    /// Build the fixed-width feature vector consumed by the Q-function.
    fn state_features(state: &MarketState) -> Vec<f64> {
        state.to_feature_vector()
    }

    /// Greedy action under the current learned Q-function (no exploration).
    pub fn greedy_action(&self, state: &MarketState) -> TradingAction {
        let features = Self::state_features(state);
        Self::action_from_index(self.q_function.greedy_action(&features))
    }

    /// Select action with an epsilon-greedy policy over the **learned** Q-values.
    pub fn select_action(&self, state: &MarketState) -> TradingAction {
        if rand::random::<f64>() < self.epsilon {
            // Explore: uniform random discrete action.
            let index = (rand::random::<u32>() as usize) % self.action_dim.max(1);
            Self::action_from_index(index)
        } else {
            // Exploit: act greedily with respect to the learned Q-function.
            self.greedy_action(state)
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

    /// Train the agent with temporal-difference Q-learning over the replay buffer.
    ///
    /// Samples a mini-batch of stored transitions and, for each, performs a real
    /// semi-gradient Q-learning update of the learned Q-function toward the
    /// Bellman target `r + gamma * max_a' Q(s', a')` (just `r` for terminal
    /// transitions). This genuinely changes the Q-parameters and therefore the
    /// induced greedy policy. Returns the mean absolute TD error over the batch
    /// (a learning signal that shrinks as the value estimates converge).
    pub fn train(&mut self, batch_size: usize) -> Result<f64> {
        if self.replay_buffer.len() < batch_size {
            return Ok(0.0); // Not enough experiences
        }

        // Sample a mini-batch (uniform, without replacement) from the buffer.
        let batch = self.sample_batch(batch_size);

        let gamma = self.discount_factor;
        let lr = self.learning_rate;
        let mut total_abs_td = 0.0;

        for experience in &batch {
            let action_index = Self::action_to_index(experience.action);
            if action_index >= self.q_function.action_dim() {
                continue;
            }

            let reward = experience.reward;
            let target = if experience.done {
                reward
            } else {
                reward + gamma * self.q_function.max_q_value(&experience.next_state)
            };

            let td_error = self
                .q_function
                .td_update(&experience.state, action_index, target, lr);
            total_abs_td += td_error.abs();
        }

        Ok(total_abs_td / batch.len() as f64)
    }

    /// Uniformly sample (without replacement) up to `batch_size` experiences.
    fn sample_batch(&self, batch_size: usize) -> Vec<Experience> {
        use rand::RngExt;
        let mut rng = rand::rng();

        let buffer_len = self.replay_buffer.len();
        let sample_size = batch_size.min(buffer_len);
        let mut indices: Vec<usize> = (0..buffer_len).collect();
        let mut sampled = Vec::with_capacity(sample_size);

        for _ in 0..sample_size {
            if indices.is_empty() {
                break;
            }
            let pick = rng.random_range(0..indices.len());
            let buffer_idx = indices.swap_remove(pick);
            sampled.push(self.replay_buffer[buffer_idx].clone());
        }

        sampled
    }

    /// Update exploration rate (decay epsilon)
    pub fn decay_epsilon(&mut self, decay_rate: f64) {
        self.epsilon = (self.epsilon * decay_rate).max(0.01);
    }
}

/// Multi-agent trading system
pub struct MultiAgentSystem {
    /// Individual RL agents in this system
    pub agents: Vec<RLTradingAgent>,
    /// How agents interact with one another
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
    /// Create a new multi-agent system with identical agents
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
    /// Agent trying to generate realistic-looking trading behaviour
    pub generator_agent: RLTradingAgent,
    /// Agent trying to distinguish generated from real behaviour
    pub discriminator_agent: RLTradingAgent,
    /// Number of adversarial training rounds completed
    pub num_iterations: usize,
}

impl AdversarialTraining {
    /// Create a new adversarial training setup
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
    /// Agent pre-trained on the source market
    pub source_agent: RLTradingAgent,
    /// Agent being adapted to the target market
    pub target_agent: RLTradingAgent,
    /// Which transfer learning strategy to use
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
    /// Create a new transfer learning setup
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

    /// Build a `MarketState` whose `to_feature_vector()` is exactly
    /// `[price, position, capital]` (no history / indicators), so the stored
    /// experience features and the inference features coincide.
    fn context_state(price: f64, position: f64) -> MarketState {
        MarketState {
            price,
            price_history: Vec::new(),
            volume_history: Vec::new(),
            indicators: HashMap::new(),
            position,
            capital: 1.0,
        }
    }

    #[test]
    fn test_linear_q_update_moves_toward_target() {
        // A single semi-gradient TD update must move Q(s, a) strictly closer to
        // the target value than it was before.
        let mut q = LinearQFunction::new(3, 2);
        let features = vec![0.7, -0.3, 1.0];
        let action = 1usize;
        let target = 5.0;

        let q_old = q.q_value(&features, action);
        let td_error = q.td_update(&features, action, target, 0.05);
        let q_new = q.q_value(&features, action);

        assert!(
            td_error.abs() > 0.0,
            "expected a non-zero TD error to drive learning"
        );
        let err_old = (q_old - target).abs();
        let err_new = (q_new - target).abs();
        assert!(
            err_new < err_old,
            "|Q_new - target|={err_new} must be < |Q_old - target|={err_old}"
        );
    }

    #[test]
    fn test_td_update_only_touches_selected_action() {
        // Q-learning updates the value of the taken action only; the other
        // action-value for the same state must be unchanged.
        let mut q = LinearQFunction::new(3, 2);
        let features = vec![1.0, 0.0, 1.0];

        let other_before = q.q_value(&features, 1);
        let _ = q.td_update(&features, 0, 10.0, 0.1);
        let other_after = q.q_value(&features, 1);

        assert!(
            (other_before - other_after).abs() < 1e-12,
            "updating action 0 must not change action 1's value"
        );
    }

    #[test]
    fn test_policy_converges_to_optimal_action() {
        // Deterministic contextual bandit (one-step terminal MDP):
        //   context A = [1, 0, 1]: action Buy(0) -> +1, action Sell(1) -> -1
        //   context B = [0, 1, 1]: action Buy(0) -> -1, action Sell(1) -> +1
        // The optimal greedy policy is Buy in A and Sell in B. A learned linear
        // Q-function must recover this; the untrained network must not already
        // know it for both contexts.
        let mut agent = RLTradingAgent::new(RLAlgorithm::DQN, 3, 2);
        agent.learning_rate = 0.1;
        agent.epsilon = 0.0; // evaluate the greedy (exploitation) policy
        // Deterministic zero start: all Q-values equal, so the untrained greedy
        // policy degenerates to action index 0 (Buy) for every context.
        agent.q_function = LinearQFunction::zeros(3, 2);

        let state_a = context_state(1.0, 0.0); // [1, 0, 1]
        let state_b = context_state(0.0, 1.0); // [0, 1, 1]

        // Greedy policy BEFORE any learning.
        let pre_a = agent.greedy_action(&state_a);
        let pre_b = agent.greedy_action(&state_b);
        let optimal_before = pre_a == TradingAction::Buy && pre_b == TradingAction::Sell;

        // Generate the deterministic reward signal as replay experiences.
        let feat_a = state_a.to_feature_vector();
        let feat_b = state_b.to_feature_vector();
        let reward = |context_a: bool, action: TradingAction| -> f64 {
            let buy = action == TradingAction::Buy;
            if context_a == buy { 1.0 } else { -1.0 }
        };
        for _ in 0..200 {
            for &action in &[TradingAction::Buy, TradingAction::Sell] {
                agent.store_experience(Experience {
                    state: feat_a.clone(),
                    action,
                    reward: reward(true, action),
                    next_state: feat_a.clone(),
                    done: true,
                });
                agent.store_experience(Experience {
                    state: feat_b.clone(),
                    action,
                    reward: reward(false, action),
                    next_state: feat_b.clone(),
                    done: true,
                });
            }
        }

        // Train with real temporal-difference updates.
        let first_td = agent.train(64).expect("train should succeed");
        for _ in 0..600 {
            let _ = agent.train(64).expect("train should succeed");
        }
        let last_td = agent.train(64).expect("train should succeed");

        // The TD error must shrink as the value estimates converge.
        assert!(
            last_td < first_td,
            "mean |TD error| should decrease: first={first_td}, last={last_td}"
        );

        // Greedy policy AFTER learning must be optimal in BOTH contexts.
        let post_a = agent.greedy_action(&state_a);
        let post_b = agent.greedy_action(&state_b);
        assert_eq!(
            post_a,
            TradingAction::Buy,
            "context A should choose Buy after training"
        );
        assert_eq!(
            post_b,
            TradingAction::Sell,
            "context B should choose Sell after training"
        );

        // Learning must have actually changed the policy: the trained policy is
        // optimal whereas the untrained policy was not jointly optimal.
        assert!(
            !optimal_before,
            "untrained greedy policy unexpectedly already optimal (pre_a={pre_a:?}, pre_b={pre_b:?})"
        );

        // And the learned Q-values must order the optimal action on top.
        let qa = agent.q_function.q_values(&feat_a);
        let qb = agent.q_function.q_values(&feat_b);
        assert!(qa[0] > qa[1], "Q(A, Buy) should exceed Q(A, Sell)");
        assert!(qb[1] > qb[0], "Q(B, Sell) should exceed Q(B, Buy)");
    }

    #[test]
    fn test_trained_policy_beats_random_action_value() {
        // After learning, the greedy policy's expected reward must beat a random
        // policy's expected reward on the same deterministic bandit.
        let mut agent = RLTradingAgent::new(RLAlgorithm::DQN, 3, 2);
        agent.learning_rate = 0.1;
        agent.epsilon = 0.0;

        let state_a = context_state(1.0, 0.0);
        let state_b = context_state(0.0, 1.0);
        let feat_a = state_a.to_feature_vector();
        let feat_b = state_b.to_feature_vector();

        let reward =
            |context_a: bool, buy: bool| -> f64 { if context_a == buy { 1.0 } else { -1.0 } };
        for _ in 0..200 {
            for &action in &[TradingAction::Buy, TradingAction::Sell] {
                let buy = action == TradingAction::Buy;
                agent.store_experience(Experience {
                    state: feat_a.clone(),
                    action,
                    reward: reward(true, buy),
                    next_state: feat_a.clone(),
                    done: true,
                });
                agent.store_experience(Experience {
                    state: feat_b.clone(),
                    action,
                    reward: reward(false, buy),
                    next_state: feat_b.clone(),
                    done: true,
                });
            }
        }
        for _ in 0..600 {
            let _ = agent.train(64).expect("train should succeed");
        }

        // Greedy expected reward across both contexts.
        let greedy_reward = |state: &MarketState, context_a: bool| -> f64 {
            let buy = agent.greedy_action(state) == TradingAction::Buy;
            reward(context_a, buy)
        };
        let greedy_total = greedy_reward(&state_a, true) + greedy_reward(&state_b, false);

        // Random policy expected reward: 0.5 * (+1) + 0.5 * (-1) = 0 per context.
        let random_total = 0.0;

        assert!(
            greedy_total > random_total,
            "trained greedy reward ({greedy_total}) should beat random ({random_total})"
        );
        // Both contexts solved => maximum achievable total reward of +2.
        assert!(
            (greedy_total - 2.0).abs() < 1e-9,
            "trained policy should achieve the optimal total reward of 2.0, got {greedy_total}"
        );
    }
}

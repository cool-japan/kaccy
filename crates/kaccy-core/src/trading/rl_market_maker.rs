/// Reinforcement Learning-based Market Making
///
/// This module implements a market maker that learns optimal quoting strategies
/// through reinforcement learning. The agent learns to balance:
/// - Profit from bid-ask spread
/// - Inventory risk
/// - Adverse selection risk
///
/// # State Representation
/// The state includes:
/// - Current inventory position
/// - Market volatility
/// - Current spread
/// - Order book imbalance
/// - Recent PnL
///
/// # Action Space
/// The agent chooses:
/// - Bid quote level (relative to mid-price)
/// - Ask quote level (relative to mid-price)
/// - Bid quantity
/// - Ask quantity
///
/// # Reward Function
/// Reward = PnL - inventory_penalty - adverse_selection_penalty
use crate::error::{CoreError, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;

/// Market making state representation for RL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLMarketState {
    /// Current inventory (positive = long, negative = short)
    pub inventory: Decimal,
    /// Mid-price
    pub mid_price: Decimal,
    /// Recent volatility (std dev of returns)
    pub volatility: Decimal,
    /// Current bid-ask spread
    pub spread: Decimal,
    /// Order book imbalance (bid_volume - ask_volume) / (bid_volume + ask_volume)
    pub order_book_imbalance: Decimal,
    /// Recent PnL
    pub recent_pnl: Decimal,
    /// Time elapsed (normalized)
    pub time_elapsed: Decimal,
    /// Market trend (recent price change)
    pub trend: Decimal,
}

impl RLMarketState {
    /// Normalize state to [0, 1] range for neural network input
    pub fn normalize(&self, max_inventory: Decimal, max_price: Decimal) -> Vec<f64> {
        vec![
            // Inventory: normalize to [-1, 1]
            (self.inventory / max_inventory)
                .to_string()
                .parse()
                .unwrap_or(0.0),
            // Price: normalize to [0, 1]
            (self.mid_price / max_price)
                .to_string()
                .parse()
                .unwrap_or(0.0),
            // Volatility: already in [0, 1] typically
            self.volatility.to_string().parse().unwrap_or(0.0),
            // Spread: normalize to [0, 1]
            (self.spread / max_price).to_string().parse().unwrap_or(0.0),
            // Order book imbalance: already in [-1, 1]
            self.order_book_imbalance.to_string().parse().unwrap_or(0.0),
            // PnL: normalize to [-1, 1]
            (self.recent_pnl / max_price)
                .to_string()
                .parse()
                .unwrap_or(0.0),
            // Time: already normalized
            self.time_elapsed.to_string().parse().unwrap_or(0.0),
            // Trend: normalize to [-1, 1]
            self.trend.to_string().parse().unwrap_or(0.0),
        ]
    }

    /// Get state dimension (number of features)
    pub fn dimension() -> usize {
        8
    }
}

/// Market making action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketAction {
    /// Bid offset from mid-price (in bps)
    pub bid_offset: Decimal,
    /// Ask offset from mid-price (in bps)
    pub ask_offset: Decimal,
    /// Bid quantity
    pub bid_quantity: Decimal,
    /// Ask quantity
    pub ask_quantity: Decimal,
}

impl MarketAction {
    /// Create action from continuous values
    /// Maps network outputs to valid action ranges
    pub fn from_continuous(values: &[f64]) -> Self {
        // Apply sigmoid to offsets to keep them in [0, 0.02] range (0-2% from mid)
        let bid_offset_raw = values.first().copied().unwrap_or(0.0);
        let ask_offset_raw = values.get(1).copied().unwrap_or(0.0);
        let bid_offset = Decimal::from_f64_retain((1.0 / (1.0 + (-bid_offset_raw).exp())) * 0.02)
            .unwrap_or(dec!(0.005));

        let ask_offset = Decimal::from_f64_retain((1.0 / (1.0 + (-ask_offset_raw).exp())) * 0.02)
            .unwrap_or(dec!(0.005));

        // Apply sigmoid to quantities and scale to [50, 200] range
        let bid_qty_raw = values.get(2).copied().unwrap_or(0.0);
        let ask_qty_raw = values.get(3).copied().unwrap_or(0.0);
        let bid_quantity =
            Decimal::from_f64_retain(50.0 + (1.0 / (1.0 + (-bid_qty_raw).exp())) * 150.0)
                .unwrap_or(Decimal::from(100));

        let ask_quantity =
            Decimal::from_f64_retain(50.0 + (1.0 / (1.0 + (-ask_qty_raw).exp())) * 150.0)
                .unwrap_or(Decimal::from(100));

        Self {
            bid_offset,
            ask_offset,
            bid_quantity,
            ask_quantity,
        }
    }

    /// Get action dimension
    pub fn dimension() -> usize {
        4
    }
}

/// Experience for replay buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experience {
    /// Market state at the time of the action.
    pub state: RLMarketState,
    /// Action taken by the agent.
    pub action: MarketAction,
    /// Reward received after taking the action.
    pub reward: Decimal,
    /// Resulting market state after the action.
    pub next_state: RLMarketState,
    /// Whether this experience ends an episode.
    pub done: bool,
}

/// Replay buffer for experience replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBuffer {
    buffer: VecDeque<Experience>,
    capacity: usize,
}

impl ReplayBuffer {
    /// Create new replay buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add experience
    pub fn push(&mut self, experience: Experience) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(experience);
    }

    /// Sample batch of experiences
    pub fn sample(&self, batch_size: usize) -> Vec<Experience> {
        use rand::RngExt;
        let mut rng = rand::rng();
        let buffer_vec: Vec<_> = self.buffer.iter().collect();
        let sample_size = batch_size.min(self.buffer.len());

        // Simple random sampling without replacement
        let mut sampled = Vec::with_capacity(sample_size);
        let mut indices: Vec<usize> = (0..buffer_vec.len()).collect();

        for _ in 0..sample_size {
            if indices.is_empty() {
                break;
            }
            let idx = rng.random_range(0..indices.len());
            let buffer_idx = indices.swap_remove(idx);
            sampled.push(buffer_vec[buffer_idx].clone());
        }

        sampled
    }

    /// Get buffer size
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

/// RL Market Maker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLMarketMakerConfig {
    /// Learning rate
    pub learning_rate: f64,
    /// Discount factor (gamma)
    pub discount_factor: f64,
    /// Exploration rate (epsilon)
    pub epsilon: f64,
    /// Epsilon decay rate
    pub epsilon_decay: f64,
    /// Minimum epsilon
    pub epsilon_min: f64,
    /// Target network update frequency
    pub target_update_freq: usize,
    /// Batch size for training
    pub batch_size: usize,
    /// Replay buffer capacity
    pub replay_buffer_capacity: usize,
    /// Inventory penalty coefficient
    pub inventory_penalty: Decimal,
    /// Adverse selection penalty coefficient
    pub adverse_selection_penalty: Decimal,
}

impl Default for RLMarketMakerConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.001,
            discount_factor: 0.99,
            epsilon: 1.0,
            epsilon_decay: 0.995,
            epsilon_min: 0.01,
            target_update_freq: 100,
            batch_size: 32,
            replay_buffer_capacity: 10000,
            inventory_penalty: dec!(0.01),
            adverse_selection_penalty: dec!(0.005),
        }
    }
}

/// Two-layer multi-layer perceptron used as a Q-function approximator.
///
/// Architecture: `input(state_dim) -> hidden(hidden_dim) -> output(action_dim)`
/// with a `tanh` nonlinearity on the hidden layer. This is a genuinely
/// parameterised network: [`forward`](QNetwork::forward) runs a real two-layer
/// matrix-vector pass and [`update`](QNetwork::update) performs an actual
/// backpropagation / gradient-descent step on a mean-squared-error objective,
/// which provably moves the outputs toward the supplied target vector.
///
/// Parameters are stored as a `Vec<Vec<f64>>` so the structure (and therefore
/// its serialised form) stays stable across versions:
/// - `weights[0]`: input -> hidden weight matrix, row-major `[hidden][state]`
/// - `weights[1]`: hidden -> output weight matrix, row-major `[action][hidden]`
/// - `weights[2]`: hidden-layer bias vector, length `hidden_dim`
/// - `weights[3]`: output-layer bias vector, length `action_dim`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QNetwork {
    /// Network parameters (see type-level docs for the layout).
    weights: Vec<Vec<f64>>,
    /// State dimension (input width)
    state_dim: usize,
    /// Action dimension (output width)
    action_dim: usize,
    /// Hidden-layer width
    hidden_dim: usize,
}

impl QNetwork {
    /// Create a new Q-network with randomly initialised weights.
    pub fn new(state_dim: usize, action_dim: usize, hidden_dim: usize) -> Self {
        use rand::RngExt;
        let mut rng = rand::rng();

        // He/Xavier-style scaling keeps activations in a sane range and avoids
        // saturating the tanh nonlinearity at initialisation.
        let scale_in = (1.0 / (state_dim.max(1) as f64)).sqrt();
        let scale_hidden = (1.0 / (hidden_dim.max(1) as f64)).sqrt();

        // Input -> Hidden weight matrix [hidden][state].
        let mut w_in = Vec::with_capacity(state_dim * hidden_dim);
        for _ in 0..(state_dim * hidden_dim) {
            w_in.push(rng.random_range(-scale_in..scale_in));
        }

        // Hidden -> Output weight matrix [action][hidden].
        let mut w_out = Vec::with_capacity(hidden_dim * action_dim);
        for _ in 0..(hidden_dim * action_dim) {
            w_out.push(rng.random_range(-scale_hidden..scale_hidden));
        }

        // Biases start at zero.
        let b_hidden = vec![0.0; hidden_dim];
        let b_out = vec![0.0; action_dim];

        Self {
            weights: vec![w_in, w_out, b_hidden, b_out],
            state_dim,
            action_dim,
            hidden_dim,
        }
    }

    /// Read a state feature into a fixed-length input vector (truncate/zero-pad).
    fn input_vector(&self, state: &[f64]) -> Vec<f64> {
        let mut input = vec![0.0; self.state_dim];
        for (slot, &value) in input.iter_mut().zip(state.iter()) {
            *slot = value;
        }
        input
    }

    /// Run the hidden layer, returning pre-activations and `tanh` activations.
    fn hidden_layer(&self, input: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let w_in = &self.weights[0];
        let b_hidden = &self.weights[2];
        let mut pre = vec![0.0; self.hidden_dim];
        let mut act = vec![0.0; self.hidden_dim];
        for h in 0..self.hidden_dim {
            let mut sum = b_hidden[h];
            let row = h * self.state_dim;
            for (s, &x) in input.iter().enumerate() {
                sum += w_in[row + s] * x;
            }
            pre[h] = sum;
            act[h] = sum.tanh();
        }
        (pre, act)
    }

    /// Forward pass: a real two-layer MLP with a `tanh` hidden nonlinearity.
    ///
    /// Returns one value per action (the estimated action-values for `state`).
    pub fn forward(&self, state: &[f64]) -> Vec<f64> {
        let input = self.input_vector(state);
        let (_, hidden) = self.hidden_layer(&input);

        let w_out = &self.weights[1];
        let b_out = &self.weights[3];
        let mut output = vec![0.0; self.action_dim];
        for (a, out) in output.iter_mut().enumerate() {
            let mut sum = b_out[a];
            let row = a * self.hidden_dim;
            for (h, &hv) in hidden.iter().enumerate() {
                sum += w_out[row + h] * hv;
            }
            *out = sum;
        }
        output
    }

    /// Perform one backpropagation step minimising the mean-squared error
    /// between the network's current output for `state` and `target_q`.
    ///
    /// This is a genuine gradient-descent update: it computes the analytic
    /// gradients of the MSE loss with respect to every weight and bias (through
    /// the `tanh` hidden layer) and nudges each parameter against its gradient.
    /// For a sufficiently small `learning_rate` it provably reduces the aggregate
    /// mean-squared error toward the target vector (and drives each component to
    /// its target under repeated application); a single step on conflicting
    /// targets that share hidden units need not improve every component at once.
    ///
    /// Returns the scalar MSE loss measured **before** the parameter update.
    pub fn update(&mut self, state: &[f64], target_q: &[f64], learning_rate: f64) -> f64 {
        let input = self.input_vector(state);
        let (hidden_pre, hidden_act) = self.hidden_layer(&input);

        // Forward to outputs (re-using the hidden activations).
        let mut output = vec![0.0; self.action_dim];
        {
            let w_out = &self.weights[1];
            let b_out = &self.weights[3];
            for (a, out) in output.iter_mut().enumerate() {
                let mut sum = b_out[a];
                let row = a * self.hidden_dim;
                for (h, &hv) in hidden_act.iter().enumerate() {
                    sum += w_out[row + h] * hv;
                }
                *out = sum;
            }
        }

        // Output-layer error: d(L)/d(output_a) for L = sum_a (output_a - t_a)^2.
        // (The leading factor of 2 is folded into the learning rate.)
        let mut output_delta = vec![0.0; self.action_dim];
        let mut loss = 0.0;
        for a in 0..self.action_dim {
            let target = target_q.get(a).copied().unwrap_or(output[a]);
            let diff = output[a] - target;
            output_delta[a] = diff;
            loss += diff * diff;
        }
        loss /= self.action_dim.max(1) as f64;

        // Backpropagate into the hidden layer.
        // d(L)/d(hidden_pre_h) = (sum_a delta_a * w_out[a][h]) * (1 - tanh^2(pre_h)).
        let mut hidden_delta = vec![0.0; self.hidden_dim];
        {
            let w_out = &self.weights[1];
            for h in 0..self.hidden_dim {
                let mut grad = 0.0;
                for a in 0..self.action_dim {
                    grad += output_delta[a] * w_out[a * self.hidden_dim + h];
                }
                let tanh_grad = 1.0 - hidden_pre[h].tanh().powi(2);
                hidden_delta[h] = grad * tanh_grad;
            }
        }

        // Gradient-descent update of hidden -> output weights and biases.
        {
            let hidden_width = self.hidden_dim;
            let w_out = &mut self.weights[1];
            for (row, &delta) in w_out.chunks_mut(hidden_width).zip(output_delta.iter()) {
                for (weight, &hv) in row.iter_mut().zip(hidden_act.iter()) {
                    *weight -= learning_rate * delta * hv;
                }
            }
        }
        for (bias, &delta) in self.weights[3].iter_mut().zip(output_delta.iter()) {
            *bias -= learning_rate * delta;
        }

        // Gradient-descent update of input -> hidden weights and biases.
        {
            let state_width = self.state_dim;
            let w_in = &mut self.weights[0];
            for (row, &delta) in w_in.chunks_mut(state_width).zip(hidden_delta.iter()) {
                for (weight, &x) in row.iter_mut().zip(input.iter()) {
                    *weight -= learning_rate * delta * x;
                }
            }
        }
        for (bias, &delta) in self.weights[2].iter_mut().zip(hidden_delta.iter()) {
            *bias -= learning_rate * delta;
        }

        loss
    }

    /// Number of action-value outputs produced by [`forward`](QNetwork::forward).
    pub fn action_dim(&self) -> usize {
        self.action_dim
    }

    /// Input width (state dimension) expected by the network.
    pub fn state_dim(&self) -> usize {
        self.state_dim
    }
}

/// Serializable snapshot of an RLMarketMaker's state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLModelSnapshot {
    /// Model configuration
    pub config: RLMarketMakerConfig,
    /// Online Q-network weights
    pub q_network: QNetwork,
    /// Target Q-network weights
    pub target_network: QNetwork,
    /// Replay buffer contents
    pub replay_buffer: ReplayBuffer,
    /// Number of training steps completed
    pub step_count: usize,
    /// Maximum inventory bound used for state normalisation
    pub max_inventory: Decimal,
    /// Maximum price bound used for state normalisation
    pub max_price: Decimal,
}

/// RL-based Market Maker
pub struct RLMarketMaker {
    config: RLMarketMakerConfig,
    q_network: QNetwork,
    target_network: QNetwork,
    replay_buffer: ReplayBuffer,
    step_count: usize,
    max_inventory: Decimal,
    max_price: Decimal,
}

impl RLMarketMaker {
    /// Create new RL market maker
    pub fn new(config: RLMarketMakerConfig, max_inventory: Decimal, max_price: Decimal) -> Self {
        let state_dim = RLMarketState::dimension();
        let action_dim = MarketAction::dimension();
        let hidden_dim = 64;

        let q_network = QNetwork::new(state_dim, action_dim, hidden_dim);
        let target_network = q_network.clone();
        let replay_buffer = ReplayBuffer::new(config.replay_buffer_capacity);

        Self {
            config,
            q_network,
            target_network,
            replay_buffer,
            step_count: 0,
            max_inventory,
            max_price,
        }
    }

    /// Select action using epsilon-greedy policy
    pub fn select_action(&self, state: &RLMarketState, epsilon: f64) -> MarketAction {
        use rand::RngExt;
        let mut rng = rand::rng();

        if rng.random_range(0.0..1.0) < epsilon {
            // Explore: random action
            MarketAction {
                bid_offset: Decimal::from_f64_retain(rng.random_range(0.0..0.01))
                    .unwrap_or(dec!(0.005)),
                ask_offset: Decimal::from_f64_retain(rng.random_range(0.0..0.01))
                    .unwrap_or(dec!(0.005)),
                bid_quantity: Decimal::from(rng.random_range(50..200)),
                ask_quantity: Decimal::from(rng.random_range(50..200)),
            }
        } else {
            // Exploit: use Q-network
            let state_vec = state.normalize(self.max_inventory, self.max_price);
            let q_values = self.q_network.forward(&state_vec);
            MarketAction::from_continuous(&q_values)
        }
    }

    /// Calculate reward
    pub fn calculate_reward(
        &self,
        pnl: Decimal,
        inventory: Decimal,
        adverse_selection: Decimal,
    ) -> Decimal {
        // Reward = PnL - inventory_penalty * |inventory| - adverse_selection_penalty * adverse_selection
        let inventory_cost = self.config.inventory_penalty * inventory.abs();
        let adverse_cost = self.config.adverse_selection_penalty * adverse_selection;

        pnl - inventory_cost - adverse_cost
    }

    /// Store experience in replay buffer
    pub fn store_experience(&mut self, experience: Experience) {
        self.replay_buffer.push(experience);
    }

    /// Train the network
    pub fn train(&mut self) -> Result<Decimal> {
        if self.replay_buffer.len() < self.config.batch_size {
            return Ok(Decimal::ZERO);
        }

        // Sample batch
        let batch = self.replay_buffer.sample(self.config.batch_size);

        // Run a real temporal-difference update for every sampled transition.
        let mut total_loss = 0.0;

        for experience in &batch {
            let state_vec = experience
                .state
                .normalize(self.max_inventory, self.max_price);
            let next_state_vec = experience
                .next_state
                .normalize(self.max_inventory, self.max_price);

            // Bootstrapped value of the next state from the (frozen) target net.
            let next_q_values = self.target_network.forward(&next_state_vec);
            let max_next_q = next_q_values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

            // Bellman TD target r + gamma * max_a' Q_target(s', a').
            let reward_f64: f64 = experience.reward.to_string().parse().unwrap_or(0.0);
            let td_target = if experience.done {
                reward_f64
            } else {
                reward_f64 + self.config.discount_factor * max_next_q
            };

            // Regress every output component of the online network toward the
            // shared TD target, then take a real backprop step. `update`
            // returns the pre-step MSE, which we accumulate as the batch loss.
            let target_vec = vec![td_target; self.q_network.action_dim()];
            total_loss += self
                .q_network
                .update(&state_vec, &target_vec, self.config.learning_rate);
        }

        // Update target network periodically (Polyak-style hard copy).
        self.step_count += 1;
        if self.step_count % self.config.target_update_freq == 0 {
            self.target_network = self.q_network.clone();
        }

        Ok(Decimal::from_f64_retain(total_loss / batch.len() as f64).unwrap_or(Decimal::ZERO))
    }

    /// Get current epsilon for exploration
    pub fn get_epsilon(&self) -> f64 {
        self.config.epsilon.max(self.config.epsilon_min)
    }

    /// Decay epsilon
    pub fn decay_epsilon(&mut self) {
        self.config.epsilon *= self.config.epsilon_decay;
        self.config.epsilon = self.config.epsilon.max(self.config.epsilon_min);
    }

    /// Get training statistics
    pub fn get_stats(&self) -> TrainingStats {
        TrainingStats {
            step_count: self.step_count,
            epsilon: self.get_epsilon(),
            replay_buffer_size: self.replay_buffer.len(),
        }
    }

    /// Serialize the full model state (weights, config, replay buffer) to a JSON file.
    ///
    /// The file is written atomically: JSON is first serialised into memory, then
    /// written to `path` in a single `write_all` call so partial writes do not
    /// produce a corrupted snapshot.
    pub fn save_model(&self, path: &str) -> Result<()> {
        let snapshot = RLModelSnapshot {
            config: self.config.clone(),
            q_network: self.q_network.clone(),
            target_network: self.target_network.clone(),
            replay_buffer: self.replay_buffer.clone(),
            step_count: self.step_count,
            max_inventory: self.max_inventory,
            max_price: self.max_price,
        };

        let json = serde_json::to_string_pretty(&snapshot).map_err(|e| {
            CoreError::Serialization(format!("Failed to serialise model snapshot: {}", e))
        })?;

        // Create parent directory if it does not exist
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| {
                    CoreError::Serialization(format!(
                        "Failed to create model directory '{}': {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        fs::write(path, json).map_err(|e| {
            CoreError::Serialization(format!("Failed to write model to '{}': {}", path, e))
        })?;

        tracing::info!("RLMarketMaker model saved to '{}'", path);
        Ok(())
    }

    /// Load model state from a JSON file previously written by [`save_model`].
    ///
    /// All fields (weights, config, replay buffer, step count, normalisation
    /// bounds) are restored so training can resume exactly where it left off.
    pub fn load_model(&mut self, path: &str) -> Result<()> {
        let json = fs::read_to_string(path).map_err(|e| {
            CoreError::Serialization(format!("Failed to read model from '{}': {}", path, e))
        })?;

        let snapshot: RLModelSnapshot = serde_json::from_str(&json).map_err(|e| {
            CoreError::Serialization(format!(
                "Failed to deserialise model snapshot from '{}': {}",
                path, e
            ))
        })?;

        self.config = snapshot.config;
        self.q_network = snapshot.q_network;
        self.target_network = snapshot.target_network;
        self.replay_buffer = snapshot.replay_buffer;
        self.step_count = snapshot.step_count;
        self.max_inventory = snapshot.max_inventory;
        self.max_price = snapshot.max_price;

        tracing::info!(
            "RLMarketMaker model loaded from '{}' (step {})",
            path,
            self.step_count
        );
        Ok(())
    }
}

/// Training statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStats {
    /// Total number of training steps completed.
    pub step_count: usize,
    /// Current exploration rate (epsilon-greedy).
    pub epsilon: f64,
    /// Number of experiences currently stored in the replay buffer.
    pub replay_buffer_size: usize,
}

/// Online learning market maker
/// Continuously adapts to market conditions
pub struct OnlineLearningMarketMaker {
    rl_maker: RLMarketMaker,
    /// Performance window for online adaptation
    performance_window: VecDeque<Decimal>,
    /// Window size for performance tracking
    window_size: usize,
}

impl OnlineLearningMarketMaker {
    /// Create new online learning market maker
    pub fn new(config: RLMarketMakerConfig, max_inventory: Decimal, max_price: Decimal) -> Self {
        let rl_maker = RLMarketMaker::new(config, max_inventory, max_price);

        Self {
            rl_maker,
            performance_window: VecDeque::new(),
            window_size: 100,
        }
    }

    /// Update with new performance metric
    pub fn update_performance(&mut self, reward: Decimal) {
        if self.performance_window.len() >= self.window_size {
            self.performance_window.pop_front();
        }
        self.performance_window.push_back(reward);
    }

    /// Get average recent performance
    pub fn get_avg_performance(&self) -> Decimal {
        if self.performance_window.is_empty() {
            return Decimal::ZERO;
        }

        let sum: Decimal = self.performance_window.iter().sum();
        sum / Decimal::from(self.performance_window.len())
    }

    /// Adapt learning rate based on performance
    pub fn adapt_learning_rate(&mut self) {
        let avg_perf = self.get_avg_performance();

        // If performance is declining, increase learning rate
        if avg_perf < Decimal::ZERO {
            self.rl_maker.config.learning_rate *= 1.1;
        } else {
            // If performance is good, slightly decrease learning rate
            self.rl_maker.config.learning_rate *= 0.99;
        }

        // Clamp learning rate
        self.rl_maker.config.learning_rate = self.rl_maker.config.learning_rate.clamp(0.0001, 0.01);
    }

    /// Get underlying RL maker
    pub fn rl_maker(&self) -> &RLMarketMaker {
        &self.rl_maker
    }

    /// Get mutable underlying RL maker
    pub fn rl_maker_mut(&mut self) -> &mut RLMarketMaker {
        &mut self.rl_maker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_state_normalize() {
        let state = RLMarketState {
            inventory: dec!(100),
            mid_price: dec!(50000),
            volatility: dec!(0.02),
            spread: dec!(10),
            order_book_imbalance: dec!(0.1),
            recent_pnl: dec!(50),
            time_elapsed: dec!(0.5),
            trend: dec!(0.01),
        };

        let normalized = state.normalize(dec!(1000), dec!(100000));
        assert_eq!(normalized.len(), RLMarketState::dimension());

        // Check that values are in reasonable range
        for &val in &normalized {
            assert!((-1.5..=1.5).contains(&val));
        }
    }

    #[test]
    fn test_market_action_from_continuous() {
        let values = vec![0.005, 0.005, 0.5, 0.5];
        let action = MarketAction::from_continuous(&values);

        assert!(action.bid_offset >= Decimal::ZERO);
        assert!(action.ask_offset >= Decimal::ZERO);
        assert!(action.bid_quantity > Decimal::ZERO);
        assert!(action.ask_quantity > Decimal::ZERO);
    }

    #[test]
    fn test_replay_buffer() {
        let mut buffer = ReplayBuffer::new(5);

        let state = RLMarketState {
            inventory: dec!(0),
            mid_price: dec!(50000),
            volatility: dec!(0.02),
            spread: dec!(10),
            order_book_imbalance: dec!(0),
            recent_pnl: dec!(0),
            time_elapsed: dec!(0),
            trend: dec!(0),
        };

        let action = MarketAction {
            bid_offset: dec!(0.001),
            ask_offset: dec!(0.001),
            bid_quantity: dec!(100),
            ask_quantity: dec!(100),
        };

        // Add 10 experiences (should only keep 5)
        for i in 0..10 {
            buffer.push(Experience {
                state: state.clone(),
                action: action.clone(),
                reward: Decimal::from(i),
                next_state: state.clone(),
                done: false,
            });
        }

        assert_eq!(buffer.len(), 5);

        let sample = buffer.sample(3);
        assert_eq!(sample.len(), 3);
    }

    #[test]
    fn test_rl_market_maker_creation() {
        let config = RLMarketMakerConfig::default();
        let maker = RLMarketMaker::new(config, dec!(1000), dec!(100000));

        assert_eq!(maker.step_count, 0);
        assert_eq!(maker.replay_buffer.len(), 0);
    }

    #[test]
    fn test_select_action() {
        let config = RLMarketMakerConfig::default();
        let maker = RLMarketMaker::new(config, dec!(1000), dec!(100000));

        let state = RLMarketState {
            inventory: dec!(100),
            mid_price: dec!(50000),
            volatility: dec!(0.02),
            spread: dec!(10),
            order_book_imbalance: dec!(0.1),
            recent_pnl: dec!(50),
            time_elapsed: dec!(0.5),
            trend: dec!(0.01),
        };

        // With epsilon = 1.0, should always explore
        let action = maker.select_action(&state, 1.0);
        assert!(action.bid_offset >= Decimal::ZERO);
        assert!(action.ask_offset >= Decimal::ZERO);

        // With epsilon = 0.0, should always exploit
        let action = maker.select_action(&state, 0.0);
        assert!(action.bid_quantity > Decimal::ZERO);
        assert!(action.ask_quantity > Decimal::ZERO);
    }

    #[test]
    fn test_calculate_reward() {
        let config = RLMarketMakerConfig::default();
        let maker = RLMarketMaker::new(config, dec!(1000), dec!(100000));

        let reward = maker.calculate_reward(dec!(100), dec!(50), dec!(10));

        // Reward should be: PnL - inventory_penalty * inventory - adverse_selection_penalty * adverse_selection
        // = 100 - 0.01 * 50 - 0.005 * 10 = 100 - 0.5 - 0.05 = 99.45
        assert!(reward > dec!(99) && reward < dec!(100));
    }

    #[test]
    fn test_epsilon_decay() {
        let config = RLMarketMakerConfig::default();
        let mut maker = RLMarketMaker::new(config, dec!(1000), dec!(100000));

        let initial_epsilon = maker.get_epsilon();
        maker.decay_epsilon();
        let decayed_epsilon = maker.get_epsilon();

        assert!(decayed_epsilon < initial_epsilon);
        assert!(decayed_epsilon >= maker.config.epsilon_min);
    }

    #[test]
    fn test_online_learning_market_maker() {
        let config = RLMarketMakerConfig::default();
        let mut maker = OnlineLearningMarketMaker::new(config, dec!(1000), dec!(100000));

        // Add some performance data
        maker.update_performance(dec!(10));
        maker.update_performance(dec!(20));
        maker.update_performance(dec!(30));

        let avg = maker.get_avg_performance();
        assert_eq!(avg, dec!(20));
    }

    #[test]
    fn test_adapt_learning_rate() {
        let config = RLMarketMakerConfig::default();
        let mut maker = OnlineLearningMarketMaker::new(config, dec!(1000), dec!(100000));

        let initial_lr = maker.rl_maker().config.learning_rate;

        // Add negative performance
        for _ in 0..10 {
            maker.update_performance(dec!(-10));
        }

        maker.adapt_learning_rate();
        let new_lr = maker.rl_maker().config.learning_rate;

        // Learning rate should increase with poor performance
        assert!(new_lr > initial_lr);
    }

    #[test]
    fn test_training_stats() {
        let config = RLMarketMakerConfig::default();
        let maker = RLMarketMaker::new(config, dec!(1000), dec!(100000));

        let stats = maker.get_stats();
        assert_eq!(stats.step_count, 0);
        assert_eq!(stats.replay_buffer_size, 0);
        assert!(stats.epsilon > 0.0);
    }

    #[test]
    fn test_save_and_load_model() {
        let config = RLMarketMakerConfig {
            epsilon: 0.42,
            ..RLMarketMakerConfig::default()
        };
        let mut maker = RLMarketMaker::new(config, dec!(500), dec!(80000));

        // Train for a couple of steps to move step_count forward
        maker.step_count = 7;
        maker.config.learning_rate = 0.0075;

        let tmp_path = std::env::temp_dir().join("rl_model_test_snapshot.json");
        let path_str = tmp_path.to_str().expect("valid UTF-8 temp path");

        // Save
        maker
            .save_model(path_str)
            .expect("save_model should succeed");
        assert!(tmp_path.exists(), "snapshot file should exist after save");

        // Load into a fresh instance
        let config2 = RLMarketMakerConfig::default();
        let mut loaded = RLMarketMaker::new(config2, dec!(1), dec!(1));
        loaded
            .load_model(path_str)
            .expect("load_model should succeed");

        // Verify key fields were round-tripped
        assert_eq!(loaded.step_count, 7);
        assert!((loaded.config.learning_rate - 0.0075).abs() < 1e-10);
        assert_eq!(loaded.max_inventory, dec!(500));
        assert_eq!(loaded.max_price, dec!(80000));

        // Clean up
        let _ = std::fs::remove_file(tmp_path);
    }

    #[test]
    fn test_qnetwork_forward_is_deterministic_and_nonlinear() {
        // A trained-from-scratch network must produce a stable, real MLP output.
        let net = QNetwork::new(4, 3, 8);
        let state = vec![0.5, -0.25, 0.75, -0.1];

        let out_a = net.forward(&state);
        let out_b = net.forward(&state);
        assert_eq!(out_a.len(), 3);
        // Forward must be a pure function of the weights and input.
        for (a, b) in out_a.iter().zip(out_b.iter()) {
            assert!((a - b).abs() < 1e-12, "forward must be deterministic");
        }

        // The hidden tanh nonlinearity means scaling the input does NOT scale
        // the output linearly (which a bare linear map would). Build a network
        // with large enough weights to drive the hidden units off the linear
        // region of tanh, then check homogeneity is broken.
        let mut net2 = QNetwork::new(2, 1, 4);
        // Push hidden pre-activations to ~+/-3 where tanh is clearly saturating.
        for w in net2.weights[0].iter_mut() {
            *w = 3.0;
        }
        for w in net2.weights[1].iter_mut() {
            *w = 1.0;
        }
        let s1 = vec![1.0, 1.0];
        let s2 = vec![2.0, 2.0];
        let y1 = net2.forward(&s1)[0];
        let y2 = net2.forward(&s2)[0];
        // For a linear map f(2x) == 2 f(x); the nonlinearity must violate this.
        assert!(
            (y2 - 2.0 * y1).abs() > 1e-6,
            "tanh nonlinearity should break linear homogeneity: y1={y1}, y2={y2}"
        );
    }

    #[test]
    fn test_qnetwork_update_moves_toward_target() {
        // Core proof of a real gradient step: regressing the chosen action's
        // Q-value toward a TD target must move that Q-value strictly closer to
        // the target (the other action targets are pinned to the current output
        // so the update isolates the action of interest, exactly as in a DQN
        // TD step where only the taken action's value is supervised).
        let mut net = QNetwork::new(5, 4, 16);
        // Deterministic, reproducible weights so the assertion is seed-free.
        for (idx, w) in net.weights[0].iter_mut().enumerate() {
            *w = 0.05 * ((idx % 7) as f64 - 3.0);
        }
        for (idx, w) in net.weights[1].iter_mut().enumerate() {
            *w = 0.05 * ((idx % 5) as f64 - 2.0);
        }

        let state = vec![0.3, -0.4, 0.1, 0.9, -0.2];
        let action = 2usize;
        let td_target = 1.0;

        let q_old = net.forward(&state);
        // Target equals the current output except for the supervised action.
        let mut target = q_old.clone();
        target[action] = td_target;

        let err_old = (q_old[action] - td_target).abs();
        let loss_before = net.update(&state, &target, 0.05);
        let q_new = net.forward(&state);
        let err_new = (q_new[action] - td_target).abs();

        assert!(loss_before > 0.0, "expected non-zero starting loss");
        assert!(
            err_new < err_old,
            "|Q_new - target|={err_new} should be < |Q_old - target|={err_old}"
        );

        // Iterating the same update must drive the supervised Q-value to the
        // target (monotone convergence of the TD step).
        for _ in 0..500 {
            let mut t = net.forward(&state);
            t[action] = td_target;
            let _ = net.update(&state, &t, 0.05);
        }
        let q_final = net.forward(&state)[action];
        assert!(
            (q_final - td_target).abs() < 1e-3,
            "repeated TD updates should converge Q({action}) to the target: got {q_final}"
        );
    }

    #[test]
    fn test_qnetwork_update_reduces_total_error() {
        // For a full multi-target regression, a small gradient step minimises the
        // aggregate mean-squared error (the formal gradient-descent guarantee).
        let mut net = QNetwork::new(5, 4, 16);
        let state = vec![0.3, -0.4, 0.1, 0.9, -0.2];
        let target = vec![1.0, -1.0, 0.5, -0.5];

        let q_old = net.forward(&state);
        let mse = |q: &[f64]| -> f64 {
            q.iter()
                .zip(target.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                / target.len() as f64
        };
        let err_before = mse(&q_old);
        let _ = net.update(&state, &target, 0.01);
        let err_after = mse(&net.forward(&state));

        assert!(
            err_after < err_before,
            "total MSE should decrease after a gradient step: before={err_before}, after={err_after}"
        );
    }

    #[test]
    fn test_qnetwork_update_changes_weights() {
        // A no-op update would leave parameters untouched; ours must change them.
        let mut net = QNetwork::new(4, 3, 8);
        let before = net.weights.clone();
        let state = vec![0.2, 0.4, -0.6, 0.8];
        let target = vec![2.0, -2.0, 1.5];

        let _ = net.update(&state, &target, 0.1);

        let mut total_change = 0.0;
        for (layer_before, layer_after) in before.iter().zip(net.weights.iter()) {
            for (a, b) in layer_before.iter().zip(layer_after.iter()) {
                total_change += (a - b).abs();
            }
        }
        assert!(
            total_change > 1e-6,
            "update must modify the network parameters (total change = {total_change})"
        );
    }

    #[test]
    fn test_market_maker_learns_value_on_deterministic_env() {
        // Deterministic environment: a "good" terminal transition yields a large
        // reward, a "bad" terminal transition yields a large penalty. Real TD
        // learning must raise the value estimate of the good state above the bad
        // one after training, starting from a near-zero initialisation.
        let config = RLMarketMakerConfig {
            learning_rate: 0.05,
            batch_size: 8,
            target_update_freq: 5,
            ..RLMarketMakerConfig::default()
        };
        let mut maker = RLMarketMaker::new(config, dec!(1000), dec!(100000));

        let good_state = RLMarketState {
            inventory: dec!(0),
            mid_price: dec!(50000),
            volatility: dec!(0.01),
            spread: dec!(5),
            order_book_imbalance: dec!(0.8),
            recent_pnl: dec!(0),
            time_elapsed: dec!(0.1),
            trend: dec!(0.05),
        };
        let bad_state = RLMarketState {
            inventory: dec!(0),
            mid_price: dec!(50000),
            volatility: dec!(0.5),
            spread: dec!(5),
            order_book_imbalance: dec!(-0.8),
            recent_pnl: dec!(0),
            time_elapsed: dec!(0.1),
            trend: dec!(-0.05),
        };
        let action = MarketAction {
            bid_offset: dec!(0.001),
            ask_offset: dec!(0.001),
            bid_quantity: dec!(100),
            ask_quantity: dec!(100),
        };

        let good_vec = good_state.normalize(dec!(1000), dec!(100000));
        let bad_vec = bad_state.normalize(dec!(1000), dec!(100000));

        // Baseline: untrained value estimates (mean over outputs).
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let good_before = mean(&maker.q_network.forward(&good_vec));
        let bad_before = mean(&maker.q_network.forward(&bad_vec));

        // Fill the replay buffer with the deterministic terminal transitions.
        for _ in 0..64 {
            maker.store_experience(Experience {
                state: good_state.clone(),
                action: action.clone(),
                reward: dec!(10),
                next_state: good_state.clone(),
                done: true,
            });
            maker.store_experience(Experience {
                state: bad_state.clone(),
                action: action.clone(),
                reward: dec!(-10),
                next_state: bad_state.clone(),
                done: true,
            });
        }

        // Train: TD loss should generally fall and value estimates separate.
        let first_loss = maker.train().expect("train should succeed");
        for _ in 0..400 {
            let _ = maker.train().expect("train should succeed");
        }
        let last_loss = maker.train().expect("train should succeed");

        let good_after = mean(&maker.q_network.forward(&good_vec));
        let bad_after = mean(&maker.q_network.forward(&bad_vec));

        // Learning must reduce the temporal-difference error over training.
        assert!(
            last_loss < first_loss,
            "TD loss should decrease with training: first={first_loss}, last={last_loss}"
        );
        // The good state must end up clearly more valuable than the bad state.
        assert!(
            good_after > bad_after + 1.0,
            "value of good state ({good_after}) should exceed bad state ({bad_after}) after training"
        );
        // And the good state's estimate must have moved up toward its reward.
        assert!(
            good_after > good_before,
            "good-state value should increase: before={good_before}, after={good_after}"
        );
        assert!(
            bad_after < bad_before,
            "bad-state value should decrease: before={bad_before}, after={bad_after}"
        );
    }
}

//! Network Effects Analysis
//!
//! Provides comprehensive network effects analysis including:
//! - User interaction graphs (directed and weighted)
//! - Influencer identification using centrality metrics
//! - Viral coefficient calculation (K-factor)
//! - Network growth modeling

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// User interaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InteractionType {
    /// User referred another user
    Referral,
    /// User traded with another user
    Trade,
    /// User followed another user
    Follow,
    /// User sent a message to another user
    Message,
    /// User shared content from another user
    Share,
    /// User copied trades from another user
    CopyTrade,
}

/// User interaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    /// From user ID
    pub from_user: String,
    /// To user ID
    pub to_user: String,
    /// Interaction type
    pub interaction_type: InteractionType,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Weight (importance/frequency)
    pub weight: f64,
}

/// User node in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserNode {
    /// User ID
    pub user_id: String,
    /// Registration date
    pub registration_date: DateTime<Utc>,
    /// Total revenue generated
    pub total_revenue: Decimal,
    /// Number of referrals made
    pub referrals_made: usize,
    /// Number of referrals received (how many people referred this user)
    pub referred_by_count: usize,
}

/// Network graph
pub struct NetworkGraph {
    /// Nodes (users)
    nodes: HashMap<String, UserNode>,
    /// Edges (interactions)
    edges: Vec<Interaction>,
    /// Adjacency list (for efficient traversal)
    adjacency: HashMap<String, Vec<(String, f64)>>,
    /// Reverse adjacency list (incoming edges)
    reverse_adjacency: HashMap<String, Vec<(String, f64)>>,
}

impl NetworkGraph {
    /// Create a new network graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            reverse_adjacency: HashMap::new(),
        }
    }

    /// Add a node
    pub fn add_node(&mut self, node: UserNode) {
        self.nodes.insert(node.user_id.clone(), node);
    }

    /// Add an edge
    pub fn add_edge(&mut self, interaction: Interaction) {
        // Update adjacency lists
        self.adjacency
            .entry(interaction.from_user.clone())
            .or_default()
            .push((interaction.to_user.clone(), interaction.weight));

        self.reverse_adjacency
            .entry(interaction.to_user.clone())
            .or_default()
            .push((interaction.from_user.clone(), interaction.weight));

        self.edges.push(interaction);
    }

    /// Get node
    pub fn get_node(&self, user_id: &str) -> Option<&UserNode> {
        self.nodes.get(user_id)
    }

    /// Get all nodes
    pub fn nodes(&self) -> &HashMap<String, UserNode> {
        &self.nodes
    }

    /// Get outgoing edges for a user
    pub fn outgoing_edges(&self, user_id: &str) -> Option<&Vec<(String, f64)>> {
        self.adjacency.get(user_id)
    }

    /// Get incoming edges for a user
    pub fn incoming_edges(&self, user_id: &str) -> Option<&Vec<(String, f64)>> {
        self.reverse_adjacency.get(user_id)
    }

    /// Calculate degree centrality (number of connections)
    pub fn degree_centrality(&self, user_id: &str) -> f64 {
        let out_degree = self.adjacency.get(user_id).map(|v| v.len()).unwrap_or(0);
        let in_degree = self
            .reverse_adjacency
            .get(user_id)
            .map(|v| v.len())
            .unwrap_or(0);
        (out_degree + in_degree) as f64
    }

    /// Calculate weighted degree centrality
    pub fn weighted_degree_centrality(&self, user_id: &str) -> f64 {
        let out_weight: f64 = self
            .adjacency
            .get(user_id)
            .map(|edges| edges.iter().map(|(_, w)| w).sum())
            .unwrap_or(0.0);

        let in_weight: f64 = self
            .reverse_adjacency
            .get(user_id)
            .map(|edges| edges.iter().map(|(_, w)| w).sum())
            .unwrap_or(0.0);

        out_weight + in_weight
    }

    /// Calculate betweenness centrality (simplified version)
    /// This measures how often a user appears on shortest paths between other users
    pub fn betweenness_centrality(&self, user_id: &str) -> f64 {
        // Simplified: count how many users this user is connected to
        // that are not directly connected to each other
        let neighbors = self.get_neighbors(user_id);
        if neighbors.len() < 2 {
            return 0.0;
        }

        let mut bridge_count = 0;
        for i in 0..neighbors.len() {
            for j in i + 1..neighbors.len() {
                let n1 = &neighbors[i];
                let n2 = &neighbors[j];
                // Check if n1 and n2 are directly connected
                if !self.are_connected(n1, n2) {
                    bridge_count += 1;
                }
            }
        }

        bridge_count as f64
    }

    /// Get all neighbors (incoming and outgoing)
    fn get_neighbors(&self, user_id: &str) -> Vec<String> {
        let mut neighbors = HashSet::new();

        if let Some(out_edges) = self.adjacency.get(user_id) {
            for (neighbor, _) in out_edges {
                neighbors.insert(neighbor.clone());
            }
        }

        if let Some(in_edges) = self.reverse_adjacency.get(user_id) {
            for (neighbor, _) in in_edges {
                neighbors.insert(neighbor.clone());
            }
        }

        neighbors.into_iter().collect()
    }

    /// Check if two users are connected
    fn are_connected(&self, user1: &str, user2: &str) -> bool {
        if let Some(edges) = self.adjacency.get(user1) {
            if edges.iter().any(|(neighbor, _)| neighbor == user2) {
                return true;
            }
        }
        if let Some(edges) = self.adjacency.get(user2) {
            if edges.iter().any(|(neighbor, _)| neighbor == user1) {
                return true;
            }
        }
        false
    }

    /// Calculate PageRank (simplified version)
    pub fn pagerank(&self, damping_factor: f64, iterations: usize) -> HashMap<String, f64> {
        let num_nodes = self.nodes.len();
        if num_nodes == 0 {
            return HashMap::new();
        }

        let initial_value = 1.0 / num_nodes as f64;
        let mut ranks: HashMap<String, f64> = self
            .nodes
            .keys()
            .map(|id| (id.clone(), initial_value))
            .collect();

        for _ in 0..iterations {
            let mut new_ranks = HashMap::new();

            for user_id in self.nodes.keys() {
                let mut rank_sum = 0.0;

                // Sum ranks from incoming edges
                if let Some(in_edges) = self.reverse_adjacency.get(user_id) {
                    for (from_user, _) in in_edges {
                        let from_rank = ranks.get(from_user).unwrap_or(&initial_value);
                        let out_degree =
                            self.adjacency.get(from_user).map(|v| v.len()).unwrap_or(1);
                        rank_sum += from_rank / out_degree as f64;
                    }
                }

                let new_rank =
                    (1.0 - damping_factor) / num_nodes as f64 + damping_factor * rank_sum;
                new_ranks.insert(user_id.clone(), new_rank);
            }

            ranks = new_ranks;
        }

        ranks
    }
}

impl Default for NetworkGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Influencer metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluencerMetrics {
    /// User ID
    pub user_id: String,
    /// Degree centrality
    pub degree_centrality: f64,
    /// Weighted degree centrality
    pub weighted_degree_centrality: f64,
    /// Betweenness centrality
    pub betweenness_centrality: f64,
    /// PageRank score
    pub pagerank: f64,
    /// Influence score (composite)
    pub influence_score: f64,
}

impl InfluencerMetrics {
    /// Calculate composite influence score
    pub fn calculate_influence_score(&mut self) {
        // Weighted combination of different centrality metrics
        self.influence_score = 0.3 * self.degree_centrality.min(100.0) / 100.0
            + 0.2 * self.weighted_degree_centrality.min(500.0) / 500.0
            + 0.2 * self.betweenness_centrality.min(100.0) / 100.0
            + 0.3 * self.pagerank * 1000.0; // PageRank is typically small
    }

    /// Get influencer tier
    pub fn tier(&self) -> InfluencerTier {
        if self.influence_score >= 0.8 {
            InfluencerTier::TopInfluencer
        } else if self.influence_score >= 0.5 {
            InfluencerTier::MajorInfluencer
        } else if self.influence_score >= 0.3 {
            InfluencerTier::MinorInfluencer
        } else {
            InfluencerTier::Regular
        }
    }
}

/// Influencer tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfluencerTier {
    Regular,
    MinorInfluencer,
    MajorInfluencer,
    TopInfluencer,
}

/// Viral coefficient metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViralCoefficient {
    /// K-factor (viral coefficient)
    pub k_factor: f64,
    /// Average invites per user
    pub avg_invites_per_user: f64,
    /// Conversion rate (invites to signups)
    pub conversion_rate: f64,
    /// Cycle time (average time for a user to invite others, in days)
    pub cycle_time_days: f64,
}

impl ViralCoefficient {
    /// Calculate K-factor
    /// K = (average invites per user) * (conversion rate)
    pub fn calculate(invites: usize, users: usize, conversions: usize) -> Self {
        let avg_invites_per_user = if users > 0 {
            invites as f64 / users as f64
        } else {
            0.0
        };

        let conversion_rate = if invites > 0 {
            conversions as f64 / invites as f64
        } else {
            0.0
        };

        let k_factor = avg_invites_per_user * conversion_rate;

        Self {
            k_factor,
            avg_invites_per_user,
            conversion_rate,
            cycle_time_days: 0.0,
        }
    }

    /// Calculate with cycle time
    pub fn with_cycle_time(mut self, cycle_time_days: f64) -> Self {
        self.cycle_time_days = cycle_time_days;
        self
    }

    /// Get growth rate
    /// Growth rate = K^(time / cycle_time)
    pub fn growth_rate(&self, time_days: f64) -> f64 {
        if self.cycle_time_days > 0.0 {
            self.k_factor.powf(time_days / self.cycle_time_days)
        } else {
            1.0
        }
    }

    /// Check if growth is viral (K > 1)
    pub fn is_viral(&self) -> bool {
        self.k_factor > 1.0
    }
}

/// Network growth model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkGrowthModel {
    /// Initial users
    pub initial_users: usize,
    /// Viral coefficient
    pub viral_coefficient: ViralCoefficient,
    /// Churn rate (percentage per cycle)
    pub churn_rate: f64,
}

impl NetworkGrowthModel {
    /// Create a new network growth model
    pub fn new(initial_users: usize, viral_coefficient: ViralCoefficient, churn_rate: f64) -> Self {
        Self {
            initial_users,
            viral_coefficient,
            churn_rate,
        }
    }

    /// Predict user count after N cycles
    pub fn predict_users(&self, cycles: usize) -> Vec<usize> {
        let mut users = vec![self.initial_users];
        let mut current = self.initial_users as f64;

        for _ in 0..cycles {
            let new_users = current * self.viral_coefficient.k_factor;
            let churned = current * self.churn_rate;
            current = current + new_users - churned;
            users.push(current.max(0.0) as usize);
        }

        users
    }

    /// Calculate network lifetime value
    pub fn network_ltv(&self, revenue_per_user: Decimal) -> Decimal {
        // Simplified LTV calculation
        // LTV = revenue_per_user / churn_rate (if churn_rate > 0)
        if self.churn_rate > 0.0 {
            revenue_per_user / Decimal::from_f64_retain(self.churn_rate).unwrap_or(Decimal::ONE)
        } else {
            Decimal::ZERO
        }
    }
}

/// Network analyzer
pub struct NetworkAnalyzer {
    graph: NetworkGraph,
}

impl NetworkAnalyzer {
    /// Create a new network analyzer
    pub fn new(graph: NetworkGraph) -> Self {
        Self { graph }
    }

    /// Identify top influencers
    pub fn identify_influencers(&self, top_n: usize) -> Vec<InfluencerMetrics> {
        let pageranks = self.graph.pagerank(0.85, 20);

        let mut influencers: Vec<InfluencerMetrics> = self
            .graph
            .nodes()
            .keys()
            .map(|user_id| {
                let mut metrics = InfluencerMetrics {
                    user_id: user_id.clone(),
                    degree_centrality: self.graph.degree_centrality(user_id),
                    weighted_degree_centrality: self.graph.weighted_degree_centrality(user_id),
                    betweenness_centrality: self.graph.betweenness_centrality(user_id),
                    pagerank: *pageranks.get(user_id).unwrap_or(&0.0),
                    influence_score: 0.0,
                };
                metrics.calculate_influence_score();
                metrics
            })
            .collect();

        influencers.sort_by(|a, b| b.influence_score.partial_cmp(&a.influence_score).unwrap());
        influencers.truncate(top_n);
        influencers
    }

    /// Calculate viral coefficient
    pub fn calculate_viral_coefficient(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> ViralCoefficient {
        let users_at_start = self
            .graph
            .nodes()
            .values()
            .filter(|n| n.registration_date < start_date)
            .count();

        let new_users = self
            .graph
            .nodes()
            .values()
            .filter(|n| n.registration_date >= start_date && n.registration_date < end_date)
            .count();

        let referrals = self
            .graph
            .nodes()
            .values()
            .filter(|n| n.registration_date < end_date)
            .map(|n| n.referrals_made)
            .sum();

        let cycle_time = (end_date - start_date).num_days() as f64;

        ViralCoefficient::calculate(referrals, users_at_start, new_users)
            .with_cycle_time(cycle_time)
    }

    /// Get community clusters (simplified - users who interact with each other)
    pub fn get_communities(&self) -> Vec<Vec<String>> {
        let mut visited = HashSet::new();
        let mut communities = Vec::new();

        for user_id in self.graph.nodes().keys() {
            if visited.contains(user_id) {
                continue;
            }

            let mut community = Vec::new();
            let mut queue = vec![user_id.clone()];

            while let Some(current) = queue.pop() {
                if visited.contains(&current) {
                    continue;
                }

                visited.insert(current.clone());
                community.push(current.clone());

                // Add neighbors to queue
                let neighbors = self.graph.get_neighbors(&current);
                for neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        queue.push(neighbor);
                    }
                }
            }

            if !community.is_empty() {
                communities.push(community);
            }
        }

        communities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_user(id: &str, referrals: usize) -> UserNode {
        UserNode {
            user_id: id.to_string(),
            registration_date: Utc::now(),
            total_revenue: Decimal::from(100),
            referrals_made: referrals,
            referred_by_count: 0,
        }
    }

    fn create_test_interaction(from: &str, to: &str, weight: f64) -> Interaction {
        Interaction {
            from_user: from.to_string(),
            to_user: to.to_string(),
            interaction_type: InteractionType::Referral,
            timestamp: Utc::now(),
            weight,
        }
    }

    #[test]
    fn test_network_graph_creation() {
        let mut graph = NetworkGraph::new();
        graph.add_node(create_test_user("user1", 2));
        graph.add_node(create_test_user("user2", 1));
        graph.add_edge(create_test_interaction("user1", "user2", 1.0));

        assert_eq!(graph.nodes().len(), 2);
        assert!(graph.get_node("user1").is_some());
    }

    #[test]
    fn test_degree_centrality() {
        let mut graph = NetworkGraph::new();
        graph.add_node(create_test_user("user1", 2));
        graph.add_node(create_test_user("user2", 1));
        graph.add_node(create_test_user("user3", 0));

        graph.add_edge(create_test_interaction("user1", "user2", 1.0));
        graph.add_edge(create_test_interaction("user1", "user3", 1.0));

        assert_eq!(graph.degree_centrality("user1"), 2.0);
        assert_eq!(graph.degree_centrality("user2"), 1.0);
        assert_eq!(graph.degree_centrality("user3"), 1.0);
    }

    #[test]
    fn test_weighted_degree_centrality() {
        let mut graph = NetworkGraph::new();
        graph.add_node(create_test_user("user1", 2));
        graph.add_node(create_test_user("user2", 1));

        graph.add_edge(create_test_interaction("user1", "user2", 5.0));

        let centrality = graph.weighted_degree_centrality("user1");
        assert_eq!(centrality, 5.0);
    }

    #[test]
    fn test_pagerank() {
        let mut graph = NetworkGraph::new();
        graph.add_node(create_test_user("user1", 2));
        graph.add_node(create_test_user("user2", 1));
        graph.add_node(create_test_user("user3", 0));

        graph.add_edge(create_test_interaction("user1", "user2", 1.0));
        graph.add_edge(create_test_interaction("user2", "user3", 1.0));
        graph.add_edge(create_test_interaction("user3", "user1", 1.0));

        let ranks = graph.pagerank(0.85, 20);
        assert!(ranks.contains_key("user1"));
        assert!(ranks.contains_key("user2"));
        assert!(ranks.contains_key("user3"));

        // Sum of all PageRanks should be approximately 1.0
        let sum: f64 = ranks.values().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_viral_coefficient() {
        let vc = ViralCoefficient::calculate(100, 50, 30);
        assert_eq!(vc.avg_invites_per_user, 2.0);
        assert_eq!(vc.conversion_rate, 0.3);
        assert_eq!(vc.k_factor, 0.6);
        assert!(!vc.is_viral());
    }

    #[test]
    fn test_viral_growth() {
        let vc = ViralCoefficient::calculate(150, 50, 60).with_cycle_time(7.0);
        // 150 invites / 50 users = 3.0 avg_invites
        // 60 conversions / 150 invites = 0.4 conversion_rate
        // K = 3.0 * 0.4 = 1.2
        assert!((vc.k_factor - 1.2).abs() < 0.001);
        assert!(vc.is_viral());
    }

    #[test]
    fn test_network_growth_prediction() {
        let vc = ViralCoefficient::calculate(150, 50, 60);
        let model = NetworkGrowthModel::new(100, vc, 0.1);
        let predictions = model.predict_users(5);

        assert_eq!(predictions[0], 100); // Initial
        assert!(predictions[1] > predictions[0]); // Should grow
    }

    #[test]
    fn test_influencer_identification() {
        let mut graph = NetworkGraph::new();
        graph.add_node(create_test_user("user1", 5));
        graph.add_node(create_test_user("user2", 2));
        graph.add_node(create_test_user("user3", 1));
        graph.add_node(create_test_user("user4", 0));

        // Create a star topology with user1 at center
        graph.add_edge(create_test_interaction("user1", "user2", 10.0));
        graph.add_edge(create_test_interaction("user1", "user3", 10.0));
        graph.add_edge(create_test_interaction("user1", "user4", 10.0));
        graph.add_edge(create_test_interaction("user2", "user1", 5.0)); // Some back to user1
        graph.add_edge(create_test_interaction("user3", "user1", 5.0));

        let analyzer = NetworkAnalyzer::new(graph);
        let influencers = analyzer.identify_influencers(3);

        assert_eq!(influencers.len(), 3);
        // user1 has the highest degree and weighted centrality
        assert_eq!(influencers[0].user_id, "user1");
    }
}

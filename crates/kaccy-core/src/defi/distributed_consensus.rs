//! Distributed Consensus Framework
//!
//! This module provides abstractions for distributed consensus including:
//! - Byzantine Fault Tolerance (BFT)
//! - Leader election algorithms
//! - State synchronization
//! - Conflict resolution

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Consensus algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusAlgorithm {
    /// Practical Byzantine Fault Tolerance
    PBFT,
    /// Raft consensus
    Raft,
    /// Paxos
    Paxos,
    /// Proof of Stake
    PoS,
}

/// Node in distributed system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub address: String,
    pub is_leader: bool,
    pub term: u64,
    pub last_heartbeat: DateTime<Utc>,
    pub vote_count: u32,
}

impl Node {
    pub fn new(address: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            address,
            is_leader: false,
            term: 0,
            last_heartbeat: Utc::now(),
            vote_count: 0,
        }
    }
}

/// Leader election coordinator
pub struct LeaderElection {
    pub nodes: HashMap<Uuid, Node>,
    pub current_leader: Option<Uuid>,
    pub election_timeout_ms: u64,
    pub algorithm: ConsensusAlgorithm,
}

impl LeaderElection {
    pub fn new(algorithm: ConsensusAlgorithm) -> Self {
        Self {
            nodes: HashMap::new(),
            current_leader: None,
            election_timeout_ms: 5000,
            algorithm,
        }
    }

    /// Add node to cluster
    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id, node);
    }

    /// Initiate leader election
    pub fn elect_leader(&mut self) -> Option<Uuid> {
        match self.algorithm {
            ConsensusAlgorithm::Raft => self.elect_leader_raft(),
            ConsensusAlgorithm::PBFT => self.elect_leader_pbft(),
            _ => self.elect_leader_simple(),
        }
    }

    fn elect_leader_raft(&mut self) -> Option<Uuid> {
        // Simplified Raft election
        let majority = (self.nodes.len() / 2) + 1;

        for node in self.nodes.values_mut() {
            node.term += 1;
            node.vote_count = 0;
        }

        // Request votes (simplified)
        let node_ids: Vec<_> = self.nodes.keys().copied().collect();
        let total_nodes = self.nodes.len() as u32;
        if let Some(candidate_id) = node_ids.first() {
            if let Some(candidate) = self.nodes.get_mut(candidate_id) {
                candidate.vote_count = total_nodes; // Self-vote + others

                if candidate.vote_count as usize >= majority {
                    candidate.is_leader = true;
                    self.current_leader = Some(*candidate_id);
                    return Some(*candidate_id);
                }
            }
        }

        None
    }

    fn elect_leader_pbft(&mut self) -> Option<Uuid> {
        // Byzantine fault tolerance: need 2f+1 agreement
        let n = self.nodes.len();
        let f = (n - 1) / 3; // Max faulty nodes
        let _min_agreement = 2 * f + 1;

        // Primary selection (simplified: by lowest ID)
        if let Some((id, node)) = self.nodes.iter_mut().min_by_key(|(id, _)| *id) {
            node.is_leader = true;
            self.current_leader = Some(*id);
            return Some(*id);
        }

        None
    }

    fn elect_leader_simple(&mut self) -> Option<Uuid> {
        // Simple election: highest vote count
        if let Some((id, node)) = self.nodes.iter_mut().max_by_key(|(_, n)| n.vote_count) {
            node.is_leader = true;
            self.current_leader = Some(*id);
            return Some(*id);
        }

        None
    }

    /// Check if current leader is alive
    pub fn is_leader_alive(&self) -> bool {
        if let Some(leader_id) = self.current_leader {
            if let Some(leader) = self.nodes.get(&leader_id) {
                let now = Utc::now();
                let elapsed = (now - leader.last_heartbeat).num_milliseconds() as u64;
                return elapsed < self.election_timeout_ms;
            }
        }
        false
    }
}

/// State synchronization manager
pub struct StateSynchronization {
    pub local_state: HashMap<String, Vec<u8>>,
    pub version: u64,
    pub sync_status: SyncStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Synced,
    Syncing,
    OutOfSync,
}

impl StateSynchronization {
    pub fn new() -> Self {
        Self {
            local_state: HashMap::new(),
            version: 0,
            sync_status: SyncStatus::Synced,
        }
    }

    /// Update local state
    pub fn update_state(&mut self, key: String, value: Vec<u8>) {
        self.local_state.insert(key, value);
        self.version += 1;
    }

    /// Sync with remote state
    pub fn sync_with_remote(
        &mut self,
        remote_state: HashMap<String, Vec<u8>>,
        remote_version: u64,
    ) {
        self.sync_status = SyncStatus::Syncing;

        if remote_version > self.version {
            // Remote is ahead, merge states
            for (key, value) in remote_state {
                self.local_state.insert(key, value);
            }
            self.version = remote_version;
        }

        self.sync_status = SyncStatus::Synced;
    }

    /// Check if state is consistent
    pub fn is_consistent_with(
        &self,
        other_state: &HashMap<String, Vec<u8>>,
        other_version: u64,
    ) -> bool {
        self.version == other_version && self.local_state == *other_state
    }
}

impl Default for StateSynchronization {
    fn default() -> Self {
        Self::new()
    }
}

/// Conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Last Write Wins
    LWW,
    /// First Write Wins
    FWW,
    /// Merge (custom logic)
    Merge,
    /// Vote-based
    Voting,
}

/// Conflict resolver
pub struct ConflictResolver {
    pub strategy: ConflictResolution,
}

impl ConflictResolver {
    pub fn new(strategy: ConflictResolution) -> Self {
        Self { strategy }
    }

    /// Resolve conflict between two values
    pub fn resolve(
        &self,
        local: &[u8],
        remote: &[u8],
        local_ts: DateTime<Utc>,
        remote_ts: DateTime<Utc>,
    ) -> Vec<u8> {
        match self.strategy {
            ConflictResolution::LWW => {
                if remote_ts > local_ts {
                    remote.to_vec()
                } else {
                    local.to_vec()
                }
            }
            ConflictResolution::FWW => {
                if local_ts < remote_ts {
                    local.to_vec()
                } else {
                    remote.to_vec()
                }
            }
            ConflictResolution::Merge => {
                // Simplified merge: concatenate
                let mut merged = local.to_vec();
                merged.extend_from_slice(remote);
                merged
            }
            ConflictResolution::Voting => {
                // Would implement voting logic in production
                local.to_vec()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = Node::new("127.0.0.1:8080".to_string());
        assert!(!node.is_leader);
        assert_eq!(node.term, 0);
    }

    #[test]
    fn test_leader_election_raft() {
        let mut election = LeaderElection::new(ConsensusAlgorithm::Raft);

        election.add_node(Node::new("node1".to_string()));
        election.add_node(Node::new("node2".to_string()));
        election.add_node(Node::new("node3".to_string()));

        let leader_id = election.elect_leader();
        assert!(leader_id.is_some());
        assert_eq!(election.current_leader, leader_id);
    }

    #[test]
    fn test_leader_election_pbft() {
        let mut election = LeaderElection::new(ConsensusAlgorithm::PBFT);

        election.add_node(Node::new("node1".to_string()));
        election.add_node(Node::new("node2".to_string()));
        election.add_node(Node::new("node3".to_string()));
        election.add_node(Node::new("node4".to_string()));

        let leader_id = election.elect_leader();
        assert!(leader_id.is_some());
    }

    #[test]
    fn test_state_synchronization() {
        let mut sync = StateSynchronization::new();

        sync.update_state("key1".to_string(), vec![1, 2, 3]);
        assert_eq!(sync.version, 1);

        let mut remote_state = HashMap::new();
        remote_state.insert("key2".to_string(), vec![4, 5, 6]);

        sync.sync_with_remote(remote_state, 2);
        assert_eq!(sync.version, 2);
        assert!(sync.local_state.contains_key("key2"));
    }

    #[test]
    fn test_conflict_resolution_lww() {
        let resolver = ConflictResolver::new(ConflictResolution::LWW);

        let local = vec![1, 2, 3];
        let remote = vec![4, 5, 6];
        let now = Utc::now();
        let earlier = now - chrono::Duration::seconds(10);

        let result = resolver.resolve(&local, &remote, earlier, now);
        assert_eq!(result, remote); // Remote is newer, should win
    }

    #[test]
    fn test_conflict_resolution_fww() {
        let resolver = ConflictResolver::new(ConflictResolution::FWW);

        let local = vec![1, 2, 3];
        let remote = vec![4, 5, 6];
        let now = Utc::now();
        let earlier = now - chrono::Duration::seconds(10);

        let result = resolver.resolve(&local, &remote, earlier, now);
        assert_eq!(result, local); // Local is older, should win
    }

    #[test]
    fn test_leader_alive_check() {
        let mut election = LeaderElection::new(ConsensusAlgorithm::Raft);
        let mut node = Node::new("node1".to_string());
        node.is_leader = true;
        let node_id = node.id;

        election.add_node(node);
        election.current_leader = Some(node_id);

        assert!(election.is_leader_alive());
    }

    #[test]
    fn test_state_consistency_check() {
        let mut sync = StateSynchronization::new();
        sync.update_state("key1".to_string(), vec![1, 2, 3]);

        let mut other_state = HashMap::new();
        other_state.insert("key1".to_string(), vec![1, 2, 3]);

        assert!(sync.is_consistent_with(&other_state, 1));
        assert!(!sync.is_consistent_with(&other_state, 2)); // Different version
    }
}

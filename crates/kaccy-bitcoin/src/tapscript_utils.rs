//! Tapscript utilities for advanced Taproot features
//!
//! Provides tools for building complex script trees, generating Merkle proofs,
//! and optimizing script paths using Huffman coding.

use crate::error::BitcoinError;
use bitcoin::hashes::Hash;
use bitcoin::{Script, TapLeafHash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A leaf in the Tapscript tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapscriptLeaf {
    /// The script
    pub script: Vec<u8>,
    /// Leaf version (typically 0xc0 for Tapscript)
    pub version: u8,
    /// Weight (for Huffman optimization)
    pub weight: u32,
}

impl TapscriptLeaf {
    /// Create a new tapscript leaf
    pub fn new(script: Vec<u8>, weight: u32) -> Self {
        Self {
            script,
            version: 0xc0, // Tapscript version
            weight,
        }
    }

    /// Calculate the leaf hash
    pub fn leaf_hash(&self) -> Result<TapLeafHash, BitcoinError> {
        use bitcoin::hashes::{HashEngine, sha256};
        let script = Script::from_bytes(&self.script);

        // Manually compute TapLeaf hash: SHA256(SHA256(0xc0 || script))
        let mut engine = sha256::Hash::engine();
        engine.input(&[self.version]);
        engine.input(script.as_bytes());
        let hash = sha256::Hash::from_engine(engine);

        Ok(TapLeafHash::from_byte_array(hash.to_byte_array()))
    }
}

/// A node in the Tapscript tree
#[derive(Debug, Clone)]
pub enum TapscriptNode {
    /// Leaf node containing a script
    Leaf(TapscriptLeaf),
    /// Branch node with left and right children
    Branch {
        /// Left child of the branch
        left: Box<TapscriptNode>,
        /// Right child of the branch
        right: Box<TapscriptNode>,
    },
}

impl TapscriptNode {
    /// Calculate the hash of this node
    pub fn node_hash(&self) -> Result<[u8; 32], BitcoinError> {
        match self {
            TapscriptNode::Leaf(leaf) => {
                let hash = leaf.leaf_hash()?;
                Ok(*hash.as_byte_array())
            }
            TapscriptNode::Branch { left, right } => {
                let left_hash = left.node_hash()?;
                let right_hash = right.node_hash()?;

                // Lexicographic ordering for branches
                let (first, second) = if left_hash < right_hash {
                    (left_hash, right_hash)
                } else {
                    (right_hash, left_hash)
                };

                // TapBranch hash = SHA256(SHA256(0x01 || first || second))
                use bitcoin::hashes::{Hash, HashEngine, sha256};
                let mut engine = sha256::Hash::engine();
                engine.input(&[0x01]); // Branch tag
                engine.input(&first);
                engine.input(&second);
                let hash = sha256::Hash::from_engine(engine);

                Ok(hash.to_byte_array())
            }
        }
    }

    /// Get total weight of the tree
    pub fn total_weight(&self) -> u32 {
        match self {
            TapscriptNode::Leaf(leaf) => leaf.weight,
            TapscriptNode::Branch { left, right } => left.total_weight() + right.total_weight(),
        }
    }

    /// Get tree depth
    pub fn depth(&self) -> usize {
        match self {
            TapscriptNode::Leaf(_) => 1,
            TapscriptNode::Branch { left, right } => 1 + std::cmp::max(left.depth(), right.depth()),
        }
    }
}

/// Merkle proof for a specific script path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The script being proven
    pub script: Vec<u8>,
    /// Proof hashes (path from leaf to root)
    pub proof_hashes: Vec<[u8; 32]>,
    /// Leaf version
    pub leaf_version: u8,
}

impl MerkleProof {
    /// Create a new Merkle proof
    pub fn new(script: Vec<u8>, proof_hashes: Vec<[u8; 32]>, leaf_version: u8) -> Self {
        Self {
            script,
            proof_hashes,
            leaf_version,
        }
    }

    /// Verify the proof against a root hash
    pub fn verify(&self, root_hash: &[u8; 32]) -> Result<bool, BitcoinError> {
        let leaf = TapscriptLeaf {
            script: self.script.clone(),
            version: self.leaf_version,
            weight: 1,
        };

        let mut current_hash = *leaf.leaf_hash()?.as_byte_array();

        for proof_hash in &self.proof_hashes {
            let (first, second) = if current_hash < *proof_hash {
                (current_hash, *proof_hash)
            } else {
                (*proof_hash, current_hash)
            };

            use bitcoin::hashes::{Hash, HashEngine, sha256};
            let mut engine = sha256::Hash::engine();
            engine.input(&[0x01]);
            engine.input(&first);
            engine.input(&second);
            current_hash = sha256::Hash::from_engine(engine).to_byte_array();
        }

        Ok(current_hash == *root_hash)
    }
}

/// Builder for complex Tapscript trees
pub struct TapscriptTreeBuilder {
    leaves: Vec<TapscriptLeaf>,
}

impl TapscriptTreeBuilder {
    /// Create a new tree builder
    pub fn new() -> Self {
        Self { leaves: Vec::new() }
    }

    /// Add a script leaf with weight
    pub fn add_leaf(&mut self, script: Vec<u8>, weight: u32) {
        self.leaves.push(TapscriptLeaf::new(script, weight));
    }

    /// Add a simple script leaf with default weight
    pub fn add_simple_leaf(&mut self, script: Vec<u8>) {
        self.add_leaf(script, 1);
    }

    /// Build a balanced tree (not optimized)
    pub fn build_balanced(&self) -> Result<TapscriptNode, BitcoinError> {
        if self.leaves.is_empty() {
            return Err(BitcoinError::InvalidTransaction(
                "No leaves to build tree".to_string(),
            ));
        }

        let mut nodes: Vec<TapscriptNode> = self
            .leaves
            .iter()
            .map(|leaf| TapscriptNode::Leaf(leaf.clone()))
            .collect();

        while nodes.len() > 1 {
            let mut new_nodes = Vec::new();

            for chunk in nodes.chunks(2) {
                if chunk.len() == 2 {
                    new_nodes.push(TapscriptNode::Branch {
                        left: Box::new(chunk[0].clone()),
                        right: Box::new(chunk[1].clone()),
                    });
                } else {
                    new_nodes.push(chunk[0].clone());
                }
            }

            nodes = new_nodes;
        }

        Ok(nodes.into_iter().next().unwrap())
    }

    /// Build a Huffman-coded tree (optimized for weights)
    ///
    /// Creates a tree where more frequently used scripts (higher weight)
    /// have shorter paths, reducing witness size.
    pub fn build_huffman(&self) -> Result<TapscriptNode, BitcoinError> {
        if self.leaves.is_empty() {
            return Err(BitcoinError::InvalidTransaction(
                "No leaves to build tree".to_string(),
            ));
        }

        if self.leaves.len() == 1 {
            return Ok(TapscriptNode::Leaf(self.leaves[0].clone()));
        }

        // Create initial nodes
        let mut nodes: Vec<TapscriptNode> = self
            .leaves
            .iter()
            .map(|leaf| TapscriptNode::Leaf(leaf.clone()))
            .collect();

        // Build Huffman tree by repeatedly combining lowest weight nodes
        while nodes.len() > 1 {
            // Sort by weight (ascending)
            nodes.sort_by_key(|n| n.total_weight());

            // Take two nodes with lowest weight
            let left = nodes.remove(0);
            let right = nodes.remove(0);

            // Create branch node
            let branch = TapscriptNode::Branch {
                left: Box::new(left),
                right: Box::new(right),
            };

            // Insert back into list
            nodes.push(branch);
        }

        Ok(nodes.into_iter().next().unwrap())
    }

    /// Generate Merkle proof for a specific script
    pub fn generate_proof(
        &self,
        tree: &TapscriptNode,
        target_script: &[u8],
    ) -> Result<MerkleProof, BitcoinError> {
        let mut proof_hashes = Vec::new();

        if !self.find_proof(tree, target_script, &mut proof_hashes)? {
            return Err(BitcoinError::InvalidTransaction(
                "Script not found in tree".to_string(),
            ));
        }

        Ok(MerkleProof::new(target_script.to_vec(), proof_hashes, 0xc0))
    }

    /// Helper to recursively find proof path
    fn find_proof(
        &self,
        node: &TapscriptNode,
        target: &[u8],
        proof: &mut Vec<[u8; 32]>,
    ) -> Result<bool, BitcoinError> {
        match node {
            TapscriptNode::Leaf(leaf) => Ok(leaf.script == target),
            TapscriptNode::Branch { left, right } => {
                if self.find_proof(left, target, proof)? {
                    proof.push(right.node_hash()?);
                    Ok(true)
                } else if self.find_proof(right, target, proof)? {
                    proof.push(left.node_hash()?);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

impl Default for TapscriptTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyzer for script path optimization
pub struct ScriptPathOptimizer {
    /// Script usage frequencies
    script_frequencies: HashMap<Vec<u8>, u32>,
}

impl ScriptPathOptimizer {
    /// Create a new optimizer
    pub fn new() -> Self {
        Self {
            script_frequencies: HashMap::new(),
        }
    }

    /// Record script usage
    pub fn record_usage(&mut self, script: Vec<u8>) {
        *self.script_frequencies.entry(script).or_insert(0) += 1;
    }

    /// Build optimized tree based on usage patterns
    pub fn build_optimized_tree(&self) -> Result<TapscriptNode, BitcoinError> {
        let mut builder = TapscriptTreeBuilder::new();

        for (script, frequency) in &self.script_frequencies {
            builder.add_leaf(script.clone(), *frequency);
        }

        builder.build_huffman()
    }

    /// Calculate expected witness size for a tree
    pub fn calculate_expected_witness_size(&self, tree: &TapscriptNode) -> f64 {
        let total_frequency: u32 = self.script_frequencies.values().sum();
        if total_frequency == 0 {
            return 0.0;
        }

        let mut expected_size = 0.0;

        for (script, frequency) in &self.script_frequencies {
            let depth = self.get_script_depth(tree, script).unwrap_or(0);
            let probability = *frequency as f64 / total_frequency as f64;
            expected_size += probability * (script.len() + depth * 32) as f64;
        }

        expected_size
    }

    /// Get depth of a script in the tree
    fn get_script_depth(&self, node: &TapscriptNode, target: &[u8]) -> Option<usize> {
        match node {
            TapscriptNode::Leaf(leaf) => {
                if leaf.script == target {
                    Some(0)
                } else {
                    None
                }
            }
            TapscriptNode::Branch { left, right } => {
                if let Some(depth) = self.get_script_depth(left, target) {
                    Some(depth + 1)
                } else {
                    self.get_script_depth(right, target).map(|depth| depth + 1)
                }
            }
        }
    }
}

impl Default for ScriptPathOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tapscript_leaf() {
        let script = vec![0x51]; // OP_1
        let leaf = TapscriptLeaf::new(script.clone(), 1);

        assert_eq!(leaf.script, script);
        assert_eq!(leaf.version, 0xc0);
        assert_eq!(leaf.weight, 1);
    }

    #[test]
    fn test_tree_builder_balanced() {
        let mut builder = TapscriptTreeBuilder::new();
        builder.add_simple_leaf(vec![0x51]); // OP_1
        builder.add_simple_leaf(vec![0x52]); // OP_2

        let tree = builder.build_balanced();
        assert!(tree.is_ok());
    }

    #[test]
    fn test_tree_builder_huffman() {
        let mut builder = TapscriptTreeBuilder::new();
        builder.add_leaf(vec![0x51], 10); // High frequency
        builder.add_leaf(vec![0x52], 1); // Low frequency

        let tree = builder.build_huffman();
        assert!(tree.is_ok());
    }

    #[test]
    fn test_script_path_optimizer() {
        let mut optimizer = ScriptPathOptimizer::new();
        optimizer.record_usage(vec![0x51]);
        optimizer.record_usage(vec![0x51]);
        optimizer.record_usage(vec![0x52]);

        let tree = optimizer.build_optimized_tree();
        assert!(tree.is_ok());
    }

    #[test]
    fn test_merkle_proof_creation() {
        let mut builder = TapscriptTreeBuilder::new();
        let script1 = vec![0x51];
        let script2 = vec![0x52];

        builder.add_simple_leaf(script1.clone());
        builder.add_simple_leaf(script2.clone());

        let tree = builder.build_balanced().unwrap();
        let proof = builder.generate_proof(&tree, &script1);

        assert!(proof.is_ok());
    }

    #[test]
    fn test_node_depth() {
        let leaf = TapscriptNode::Leaf(TapscriptLeaf::new(vec![0x51], 1));
        assert_eq!(leaf.depth(), 1);

        let branch = TapscriptNode::Branch {
            left: Box::new(leaf.clone()),
            right: Box::new(leaf),
        };
        assert_eq!(branch.depth(), 2);
    }

    #[test]
    fn test_node_total_weight() {
        let leaf1 = TapscriptNode::Leaf(TapscriptLeaf::new(vec![0x51], 5));
        let leaf2 = TapscriptNode::Leaf(TapscriptLeaf::new(vec![0x52], 3));

        let branch = TapscriptNode::Branch {
            left: Box::new(leaf1),
            right: Box::new(leaf2),
        };

        assert_eq!(branch.total_weight(), 8);
    }
}

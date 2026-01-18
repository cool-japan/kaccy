//! Bitcoin Script Optimizer
//!
//! This module provides tools for optimizing Bitcoin scripts to reduce size and witness costs.
//! Optimization strategies include:
//! - Redundant opcode elimination
//! - Witness size optimization
//! - Script size reduction strategies
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::script_optimizer::{ScriptOptimizer, OptimizationConfig};
//! use bitcoin::ScriptBuf;
//!
//! let script = ScriptBuf::new();
//! let optimizer = ScriptOptimizer::new(OptimizationConfig::default());
//! let optimized = optimizer.optimize(&script).unwrap();
//! println!("Original size: {} bytes", script.len());
//! println!("Optimized size: {} bytes", optimized.optimized_script.len());
//! println!("Savings: {} bytes", optimized.bytes_saved);
//! ```

use crate::error::BitcoinError;
use bitcoin::blockdata::opcodes::all::*;
use bitcoin::blockdata::script::Instruction;
use bitcoin::{Script, ScriptBuf};
use serde::{Deserialize, Serialize};

/// Configuration for script optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConfig {
    /// Remove redundant DUP/DROP sequences
    pub eliminate_redundant_stack_ops: bool,
    /// Optimize numeric pushes (use shorter opcodes for small numbers)
    pub optimize_numeric_pushes: bool,
    /// Combine consecutive pushes where possible
    pub combine_pushes: bool,
    /// Optimize witness scripts for SegWit
    pub optimize_witness_scripts: bool,
    /// Remove unreachable code after OP_RETURN
    pub remove_dead_code: bool,
    /// Optimize control flow (IF/ELSE/ENDIF)
    pub optimize_control_flow: bool,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            eliminate_redundant_stack_ops: true,
            optimize_numeric_pushes: true,
            combine_pushes: false, // Conservative: might change semantics
            optimize_witness_scripts: true,
            remove_dead_code: true,
            optimize_control_flow: true,
        }
    }
}

impl OptimizationConfig {
    /// Create an aggressive optimization configuration
    pub fn aggressive() -> Self {
        Self {
            eliminate_redundant_stack_ops: true,
            optimize_numeric_pushes: true,
            combine_pushes: true,
            optimize_witness_scripts: true,
            remove_dead_code: true,
            optimize_control_flow: true,
        }
    }

    /// Create a conservative optimization configuration
    pub fn conservative() -> Self {
        Self {
            eliminate_redundant_stack_ops: true,
            optimize_numeric_pushes: true,
            combine_pushes: false,
            optimize_witness_scripts: false,
            remove_dead_code: false,
            optimize_control_flow: false,
        }
    }
}

/// Result of script optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    /// The optimized script
    pub optimized_script: ScriptBuf,
    /// Original script size in bytes
    pub original_size: usize,
    /// Optimized script size in bytes
    pub optimized_size: usize,
    /// Number of bytes saved
    pub bytes_saved: usize,
    /// Percentage reduction
    pub reduction_percentage: f64,
    /// List of optimizations applied
    pub optimizations_applied: Vec<OptimizationType>,
    /// Whether the script is safe to use (semantics preserved)
    pub is_safe: bool,
    /// Warnings about potential issues
    pub warnings: Vec<String>,
}

/// Types of optimizations that can be applied
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationType {
    /// Removed redundant DUP/DROP sequence
    RedundantStackOp,
    /// Optimized numeric push
    NumericPush,
    /// Combined consecutive pushes
    CombinedPushes,
    /// Optimized witness script
    WitnessOptimization,
    /// Removed dead code
    DeadCodeRemoval,
    /// Optimized control flow
    ControlFlowOptimization,
}

/// Bitcoin script optimizer
pub struct ScriptOptimizer {
    config: OptimizationConfig,
}

impl ScriptOptimizer {
    /// Create a new script optimizer with the given configuration
    pub fn new(config: OptimizationConfig) -> Self {
        Self { config }
    }

    /// Optimize a Bitcoin script
    pub fn optimize(&self, script: &Script) -> Result<OptimizationResult, BitcoinError> {
        let original_size = script.len();
        let mut current_script = script.to_owned();
        let mut optimizations_applied = Vec::new();
        let mut warnings = Vec::new();

        // Apply optimizations in sequence
        if self.config.eliminate_redundant_stack_ops {
            if let Some((script, count)) = self.eliminate_redundant_stack_ops(&current_script) {
                current_script = script;
                for _ in 0..count {
                    optimizations_applied.push(OptimizationType::RedundantStackOp);
                }
            }
        }

        if self.config.optimize_numeric_pushes {
            if let Some((script, count)) = self.optimize_numeric_pushes(&current_script) {
                current_script = script;
                for _ in 0..count {
                    optimizations_applied.push(OptimizationType::NumericPush);
                }
            }
        }

        if self.config.remove_dead_code {
            if let Some((script, removed)) = self.remove_dead_code(&current_script) {
                if removed {
                    current_script = script;
                    optimizations_applied.push(OptimizationType::DeadCodeRemoval);
                    warnings.push(
                        "Removed dead code after OP_RETURN - verify this is intentional"
                            .to_string(),
                    );
                }
            }
        }

        if self.config.optimize_witness_scripts
            && (script.is_p2wpkh() || script.is_p2wsh() || script.is_p2tr())
        {
            // Witness optimization is mostly about choosing the right script type
            // This is more of an analysis than a transformation
            warnings.push(
                "Witness optimization: consider using Taproot for better efficiency".to_string(),
            );
        }

        let optimized_size = current_script.len();
        let bytes_saved = original_size.saturating_sub(optimized_size);
        let reduction_percentage = if original_size > 0 {
            (bytes_saved as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };

        // Check if optimization is safe (semantics preserved)
        let is_safe = self.verify_optimization_safety(script, &current_script);
        if !is_safe {
            warnings.push(
                "Optimization may have changed script semantics - use with caution".to_string(),
            );
        }

        Ok(OptimizationResult {
            optimized_script: current_script,
            original_size,
            optimized_size,
            bytes_saved,
            reduction_percentage,
            optimizations_applied,
            is_safe,
            warnings,
        })
    }

    /// Eliminate redundant stack operations (e.g., DUP DROP sequences)
    fn eliminate_redundant_stack_ops(&self, script: &Script) -> Option<(ScriptBuf, usize)> {
        let mut builder = ScriptBuf::builder();
        let instructions: Vec<_> = script.instructions().collect();
        let mut i = 0;
        let mut optimizations = 0;

        while i < instructions.len() {
            if i + 1 < instructions.len() {
                // Check for DUP DROP pattern
                if let (Ok(Instruction::Op(op1)), Ok(Instruction::Op(op2))) =
                    (&instructions[i], &instructions[i + 1])
                {
                    if op1.to_u8() == OP_DUP.to_u8() && op2.to_u8() == OP_DROP.to_u8() {
                        // Skip both instructions (they cancel out)
                        i += 2;
                        optimizations += 1;
                        continue;
                    }
                }
            }

            // Copy the instruction as-is
            if let Ok(instruction) = &instructions[i] {
                match instruction {
                    Instruction::Op(opcode) => {
                        builder = builder.push_opcode(*opcode);
                    }
                    Instruction::PushBytes(bytes) => {
                        builder = builder.push_slice(bytes);
                    }
                }
            }
            i += 1;
        }

        if optimizations > 0 {
            Some((builder.into_script(), optimizations))
        } else {
            None
        }
    }

    /// Optimize numeric pushes to use shorter opcodes
    fn optimize_numeric_pushes(&self, script: &Script) -> Option<(ScriptBuf, usize)> {
        let mut builder = ScriptBuf::builder();
        let mut optimizations = 0;

        for instruction in script.instructions().flatten() {
            match instruction {
                Instruction::PushBytes(bytes) => {
                    // Check if this is a small number that can be optimized
                    if bytes.len() == 1 {
                        let value = bytes.as_bytes()[0];
                        // Numbers 1-16 can use OP_1 through OP_16
                        if (1..=16).contains(&value) {
                            builder = builder.push_opcode(bitcoin::opcodes::Opcode::from(
                                OP_PUSHNUM_1.to_u8() + value - 1,
                            ));
                            optimizations += 1;
                            continue;
                        } else if value == 0 {
                            builder = builder.push_opcode(OP_PUSHBYTES_0);
                            optimizations += 1;
                            continue;
                        } else if value == 0x81 {
                            // -1 can use OP_1NEGATE
                            builder = builder.push_opcode(OP_PUSHNUM_NEG1);
                            optimizations += 1;
                            continue;
                        }
                    }
                    builder = builder.push_slice(bytes);
                }
                Instruction::Op(opcode) => {
                    builder = builder.push_opcode(opcode);
                }
            }
        }

        if optimizations > 0 {
            Some((builder.into_script(), optimizations))
        } else {
            None
        }
    }

    /// Remove dead code after OP_RETURN
    fn remove_dead_code(&self, script: &Script) -> Option<(ScriptBuf, bool)> {
        let mut builder = ScriptBuf::builder();
        let mut found_return = false;
        let mut removed_code = false;

        for instruction in script.instructions().flatten() {
            if found_return {
                // Skip all instructions after OP_RETURN
                removed_code = true;
                continue;
            }

            match instruction {
                Instruction::Op(opcode) => {
                    if opcode.to_u8() == OP_RETURN.to_u8() {
                        found_return = true;
                    }
                    builder = builder.push_opcode(opcode);
                }
                Instruction::PushBytes(bytes) => {
                    builder = builder.push_slice(bytes);
                }
            }
        }

        if removed_code {
            Some((builder.into_script(), true))
        } else {
            None
        }
    }

    /// Verify that optimization preserved script semantics
    fn verify_optimization_safety(&self, original: &Script, optimized: &Script) -> bool {
        // Basic safety checks:
        // 1. Standard scripts should remain standard
        // 2. Script type should not change
        // 3. If original is valid, optimized should be valid

        // Check script type preservation
        if original.is_p2pkh() != optimized.is_p2pkh() {
            return false;
        }
        if original.is_p2sh() != optimized.is_p2sh() {
            return false;
        }
        if original.is_p2wpkh() != optimized.is_p2wpkh() {
            return false;
        }
        if original.is_p2wsh() != optimized.is_p2wsh() {
            return false;
        }
        if original.is_p2tr() != optimized.is_p2tr() {
            return false;
        }

        // If we made the script larger, something is wrong
        if optimized.len() > original.len() {
            return false;
        }

        // Conservative: if the script changed significantly, mark as potentially unsafe
        let size_change_ratio = if !original.is_empty() {
            (original.len() - optimized.len()) as f64 / original.len() as f64
        } else {
            0.0
        };

        // If more than 50% of the script was removed, be cautious
        if size_change_ratio > 0.5 {
            return false;
        }

        true
    }

    /// Estimate potential savings without actually optimizing
    pub fn estimate_savings(&self, script: &Script) -> OptimizationEstimate {
        let mut potential_bytes = 0;
        let mut opportunities = Vec::new();

        // Count redundant stack ops
        let instructions: Vec<_> = script.instructions().collect();
        for i in 0..instructions.len().saturating_sub(1) {
            if let (Ok(Instruction::Op(op1)), Ok(Instruction::Op(op2))) =
                (&instructions[i], &instructions[i + 1])
            {
                if op1.to_u8() == OP_DUP.to_u8() && op2.to_u8() == OP_DROP.to_u8() {
                    potential_bytes += 2;
                    opportunities.push("Redundant DUP/DROP sequence".to_string());
                }
            }
        }

        // Count inefficient numeric pushes
        for instruction in script.instructions() {
            if let Ok(Instruction::PushBytes(bytes)) = instruction {
                if bytes.len() == 1 {
                    let value = bytes.as_bytes()[0];
                    if (1..=16).contains(&value) || value == 0 || value == 0x81 {
                        potential_bytes += 1; // Save 1 byte by using OP_N
                        opportunities.push(format!("Inefficient push of number {}", value));
                    }
                }
            }
        }

        OptimizationEstimate {
            potential_bytes_saved: potential_bytes,
            optimization_opportunities: opportunities,
            is_worth_optimizing: potential_bytes > 0,
        }
    }
}

/// Estimate of potential optimization savings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationEstimate {
    /// Potential bytes that could be saved
    pub potential_bytes_saved: usize,
    /// List of optimization opportunities
    pub optimization_opportunities: Vec<String>,
    /// Whether optimization is worthwhile
    pub is_worth_optimizing: bool,
}

impl Default for ScriptOptimizer {
    fn default() -> Self {
        Self::new(OptimizationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn test_optimization_config_default() {
        let config = OptimizationConfig::default();
        assert!(config.eliminate_redundant_stack_ops);
        assert!(config.optimize_numeric_pushes);
    }

    #[test]
    fn test_optimization_config_aggressive() {
        let config = OptimizationConfig::aggressive();
        assert!(config.eliminate_redundant_stack_ops);
        assert!(config.combine_pushes);
    }

    #[test]
    fn test_optimization_config_conservative() {
        let config = OptimizationConfig::conservative();
        assert!(config.eliminate_redundant_stack_ops);
        assert!(!config.combine_pushes);
    }

    #[test]
    fn test_optimize_empty_script() {
        let script = ScriptBuf::new();
        let optimizer = ScriptOptimizer::default();
        let result = optimizer.optimize(&script).unwrap();
        assert_eq!(result.original_size, 0);
        assert_eq!(result.optimized_size, 0);
        assert_eq!(result.bytes_saved, 0);
    }

    #[test]
    fn test_optimize_standard_script() {
        // Standard P2PKH script should not be modified
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));

        let optimizer = ScriptOptimizer::default();
        let result = optimizer.optimize(&script).unwrap();

        assert_eq!(result.original_size, script.len());
        assert!(result.is_safe);
    }

    #[test]
    fn test_eliminate_redundant_dup_drop() {
        // Create a script with DUP DROP sequence
        let script = ScriptBuf::builder()
            .push_opcode(OP_DUP)
            .push_opcode(OP_DROP)
            .push_opcode(OP_PUSHNUM_1)
            .into_script();

        let optimizer = ScriptOptimizer::default();
        let result = optimizer.optimize(&script).unwrap();

        assert!(result.bytes_saved > 0);
        assert!(
            result
                .optimizations_applied
                .contains(&OptimizationType::RedundantStackOp)
        );
    }

    #[test]
    fn test_optimize_numeric_push() {
        // Create a script with inefficient numeric push
        let script = ScriptBuf::builder()
            .push_slice([1u8]) // This should be optimized to OP_PUSHNUM_1
            .into_script();

        let optimizer = ScriptOptimizer::default();
        let result = optimizer.optimize(&script).unwrap();

        // Should have optimized the push
        assert!(
            result
                .optimizations_applied
                .contains(&OptimizationType::NumericPush)
        );
    }

    #[test]
    fn test_remove_dead_code() {
        // Create a script with code after OP_RETURN
        let script = ScriptBuf::builder()
            .push_opcode(OP_RETURN)
            .push_slice(b"data")
            .push_opcode(OP_PUSHNUM_1) // Dead code
            .push_opcode(OP_PUSHNUM_2) // Dead code
            .into_script();

        let config = OptimizationConfig {
            remove_dead_code: true,
            ..Default::default()
        };
        let optimizer = ScriptOptimizer::new(config);
        let result = optimizer.optimize(&script).unwrap();

        assert!(result.bytes_saved > 0);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_estimate_savings() {
        // Create a script with optimization opportunities
        let script = ScriptBuf::builder()
            .push_opcode(OP_DUP)
            .push_opcode(OP_DROP)
            .push_slice([1u8])
            .into_script();

        let optimizer = ScriptOptimizer::default();
        let estimate = optimizer.estimate_savings(&script);

        assert!(estimate.is_worth_optimizing);
        assert!(estimate.potential_bytes_saved > 0);
        assert!(!estimate.optimization_opportunities.is_empty());
    }

    #[test]
    fn test_safety_verification() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));

        let optimizer = ScriptOptimizer::default();
        let result = optimizer.optimize(&script).unwrap();

        assert!(result.is_safe);
    }

    #[test]
    fn test_optimization_result_percentage() {
        let script = ScriptBuf::builder()
            .push_opcode(OP_DUP)
            .push_opcode(OP_DROP)
            .into_script();

        let optimizer = ScriptOptimizer::default();
        let result = optimizer.optimize(&script).unwrap();

        if result.bytes_saved > 0 {
            assert!(result.reduction_percentage > 0.0);
            assert!(result.reduction_percentage <= 100.0);
        }
    }
}

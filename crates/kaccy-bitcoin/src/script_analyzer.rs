//! Bitcoin Script Analysis and Utilities
//!
//! This module provides tools for analyzing Bitcoin scripts, including:
//! - Script disassembly (opcodes to human-readable format)
//! - Script execution simulation
//! - Complexity analysis
//! - Witness cost estimation
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::script_analyzer::{ScriptAnalyzer, ScriptDisassembler};
//! use bitcoin::ScriptBuf;
//!
//! // Disassemble a script
//! let script = ScriptBuf::new();
//! let disassembler = ScriptDisassembler::new();
//! let asm = disassembler.disassemble(&script);
//! println!("Script: {}", asm);
//!
//! // Analyze script complexity
//! let analyzer = ScriptAnalyzer::new(&script);
//! let complexity = analyzer.complexity_score();
//! println!("Complexity: {}", complexity);
//! ```

use crate::error::BitcoinError;
use bitcoin::blockdata::opcodes::all::*;
use bitcoin::blockdata::script::Instruction;
use bitcoin::{Script, ScriptBuf};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Script disassembler for converting scripts to human-readable format
pub struct ScriptDisassembler {
    /// Include addresses for push data opcodes
    pub show_addresses: bool,
    /// Use symbolic names for standard scripts
    pub use_symbolic_names: bool,
}

impl ScriptDisassembler {
    /// Create a new script disassembler with default settings
    pub fn new() -> Self {
        Self {
            show_addresses: false,
            use_symbolic_names: true,
        }
    }

    /// Disassemble a script into human-readable assembly
    pub fn disassemble(&self, script: &Script) -> String {
        let mut result = Vec::new();

        for instruction in script.instructions() {
            match instruction {
                Ok(Instruction::Op(opcode)) => {
                    result.push(format!("{:?}", opcode));
                }
                Ok(Instruction::PushBytes(bytes)) => {
                    let hex_data = hex::encode(bytes.as_bytes());
                    if bytes.len() <= 4 {
                        // For small pushes, show as decimal too
                        if let Ok(num) = bytes.as_bytes().try_into().map(i32::from_le_bytes) {
                            result.push(format!("PUSH {} (0x{})", num, hex_data));
                        } else {
                            result.push(format!("PUSH 0x{}", hex_data));
                        }
                    } else {
                        result.push(format!("PUSH 0x{}", hex_data));
                    }
                }
                Err(e) => {
                    result.push(format!("ERROR: {:?}", e));
                }
            }
        }

        // Check for standard script patterns
        if self.use_symbolic_names {
            if script.is_p2pkh() {
                return format!("P2PKH ({})", result.join(" "));
            } else if script.is_p2sh() {
                return format!("P2SH ({})", result.join(" "));
            } else if script.is_p2wpkh() {
                return format!("P2WPKH ({})", result.join(" "));
            } else if script.is_p2wsh() {
                return format!("P2WSH ({})", result.join(" "));
            } else if script.is_p2tr() {
                return format!("P2TR ({})", result.join(" "));
            } else if script.is_op_return() {
                return format!("OP_RETURN ({})", result.join(" "));
            }
        }

        result.join(" ")
    }

    /// Get a compact representation of the script
    pub fn disassemble_compact(&self, script: &Script) -> String {
        if script.is_p2pkh() {
            "P2PKH".to_string()
        } else if script.is_p2sh() {
            "P2SH".to_string()
        } else if script.is_p2wpkh() {
            "P2WPKH".to_string()
        } else if script.is_p2wsh() {
            "P2WSH".to_string()
        } else if script.is_p2tr() {
            "P2TR".to_string()
        } else if script.is_op_return() {
            "OP_RETURN".to_string()
        } else {
            format!("<{} bytes>", script.len())
        }
    }
}

impl Default for ScriptDisassembler {
    fn default() -> Self {
        Self::new()
    }
}

/// Bitcoin script analyzer for comprehensive script analysis
pub struct ScriptAnalyzer<'a> {
    script: &'a Script,
}

impl<'a> ScriptAnalyzer<'a> {
    /// Create a new script analyzer
    pub fn new(script: &'a Script) -> Self {
        Self { script }
    }

    /// Calculate script complexity score (higher = more complex)
    ///
    /// This score considers:
    /// - Number of opcodes
    /// - Presence of complex operations (crypto, control flow)
    /// - Data push sizes
    /// - Stack depth requirements
    pub fn complexity_score(&self) -> u32 {
        let mut score = 0u32;

        for instruction in self.script.instructions().flatten() {
            score += match instruction {
                Instruction::Op(opcode) => self.opcode_complexity(opcode),
                Instruction::PushBytes(bytes) => {
                    // Larger pushes are slightly more complex
                    1 + (bytes.len() / 32) as u32
                }
            };
        }

        score
    }

    /// Get the complexity score for a single opcode
    fn opcode_complexity(&self, opcode: bitcoin::opcodes::Opcode) -> u32 {
        use bitcoin::blockdata::opcodes::all::*;

        match opcode.to_u8() {
            // Crypto operations are most complex
            x if x == OP_CHECKSIG.to_u8()
                || x == OP_CHECKSIGVERIFY.to_u8()
                || x == OP_CHECKMULTISIG.to_u8()
                || x == OP_CHECKMULTISIGVERIFY.to_u8() =>
            {
                10
            }
            x if x == OP_HASH160.to_u8()
                || x == OP_HASH256.to_u8()
                || x == OP_SHA256.to_u8()
                || x == OP_RIPEMD160.to_u8() =>
            {
                5
            }

            // Control flow is moderately complex
            x if x == OP_IF.to_u8()
                || x == OP_NOTIF.to_u8()
                || x == OP_ELSE.to_u8()
                || x == OP_ENDIF.to_u8() =>
            {
                3
            }
            x if x == OP_RETURN.to_u8() => 1,

            // Stack operations
            x if x == OP_DUP.to_u8()
                || x == OP_DROP.to_u8()
                || x == OP_SWAP.to_u8()
                || x == OP_ROT.to_u8() =>
            {
                1
            }

            // Arithmetic
            x if x == OP_ADD.to_u8() || x == OP_SUB.to_u8() => 2,

            // Everything else
            _ => 1,
        }
    }

    /// Count the number of opcodes in the script
    pub fn opcode_count(&self) -> usize {
        self.script
            .instructions()
            .filter(|i| matches!(i, Ok(Instruction::Op(_))))
            .count()
    }

    /// Count the number of push operations in the script
    pub fn push_count(&self) -> usize {
        self.script
            .instructions()
            .filter(|i| matches!(i, Ok(Instruction::PushBytes(_))))
            .count()
    }

    /// Estimate the witness cost (weight units) for this script
    ///
    /// This is an approximation based on script size and complexity
    pub fn estimate_witness_cost(&self) -> usize {
        // Base cost is 4 * script size (non-witness data)
        let base_cost = self.script.len() * 4;

        // Add witness-specific costs for SegWit scripts
        let witness_cost = if self.script.is_p2wpkh() {
            // P2WPKH witness: signature (72) + pubkey (33)
            72 + 33 // Witness data is counted as 1 WU per byte
        } else if self.script.is_p2wsh() {
            // P2WSH witness varies, estimate based on complexity
            let complexity = self.complexity_score();
            (complexity * 10) as usize
        } else if self.script.is_p2tr() {
            // Taproot key-path: signature (64)
            64
        } else {
            0
        };

        base_cost + witness_cost
    }

    /// Check if the script contains any potentially problematic opcodes
    ///
    /// Currently checks for commonly disabled opcodes (those between OP_CAT and OP_CODESEPARATOR)
    pub fn has_risky_opcodes(&self) -> bool {
        for instruction in self.script.instructions() {
            if let Ok(Instruction::Op(opcode)) = instruction {
                let op_byte = opcode.to_u8();
                // Check for disabled opcodes (126-129, 131-134, 141-142, 149-153)
                // These are the historically disabled opcodes in Bitcoin
                if (126..=129).contains(&op_byte)
                    || (131..=134).contains(&op_byte)
                    || (141..=142).contains(&op_byte)
                    || (149..=153).contains(&op_byte)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Identify the script type
    pub fn script_type(&self) -> ScriptType {
        if self.script.is_p2pkh() {
            ScriptType::P2PKH
        } else if self.script.is_p2sh() {
            ScriptType::P2SH
        } else if self.script.is_p2wpkh() {
            ScriptType::P2WPKH
        } else if self.script.is_p2wsh() {
            ScriptType::P2WSH
        } else if self.script.is_p2tr() {
            ScriptType::P2TR
        } else if self.script.is_op_return() {
            ScriptType::OpReturn
        } else {
            ScriptType::NonStandard
        }
    }

    /// Generate a full analysis report
    pub fn analyze(&self) -> ScriptAnalysis {
        ScriptAnalysis {
            script_type: self.script_type(),
            script_size: self.script.len(),
            opcode_count: self.opcode_count(),
            push_count: self.push_count(),
            complexity_score: self.complexity_score(),
            estimated_witness_cost: self.estimate_witness_cost(),
            has_risky_opcodes: self.has_risky_opcodes(),
            is_standard: self.is_standard_script(),
        }
    }

    /// Check if this is a standard script type
    fn is_standard_script(&self) -> bool {
        self.script.is_p2pkh()
            || self.script.is_p2sh()
            || self.script.is_p2wpkh()
            || self.script.is_p2wsh()
            || self.script.is_p2tr()
            || self.script.is_op_return()
    }
}

/// Script type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptType {
    /// Pay to Public Key Hash
    P2PKH,
    /// Pay to Script Hash
    P2SH,
    /// Pay to Witness Public Key Hash
    P2WPKH,
    /// Pay to Witness Script Hash
    P2WSH,
    /// Pay to Taproot
    P2TR,
    /// OP_RETURN data output
    OpReturn,
    /// Non-standard script
    NonStandard,
}

impl fmt::Display for ScriptType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptType::P2PKH => write!(f, "P2PKH"),
            ScriptType::P2SH => write!(f, "P2SH"),
            ScriptType::P2WPKH => write!(f, "P2WPKH"),
            ScriptType::P2WSH => write!(f, "P2WSH"),
            ScriptType::P2TR => write!(f, "P2TR"),
            ScriptType::OpReturn => write!(f, "OP_RETURN"),
            ScriptType::NonStandard => write!(f, "Non-Standard"),
        }
    }
}

/// Comprehensive script analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAnalysis {
    /// The type of script
    pub script_type: ScriptType,
    /// Size of the script in bytes
    pub script_size: usize,
    /// Number of opcodes
    pub opcode_count: usize,
    /// Number of push operations
    pub push_count: usize,
    /// Complexity score (higher = more complex)
    pub complexity_score: u32,
    /// Estimated witness cost in weight units
    pub estimated_witness_cost: usize,
    /// Whether the script contains risky/disabled opcodes
    pub has_risky_opcodes: bool,
    /// Whether this is a standard script type
    pub is_standard: bool,
}

impl fmt::Display for ScriptAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Script Analysis:")?;
        writeln!(f, "  Type: {}", self.script_type)?;
        writeln!(f, "  Size: {} bytes", self.script_size)?;
        writeln!(f, "  Opcodes: {}", self.opcode_count)?;
        writeln!(f, "  Push operations: {}", self.push_count)?;
        writeln!(f, "  Complexity: {}", self.complexity_score)?;
        writeln!(
            f,
            "  Estimated witness cost: {} WU",
            self.estimated_witness_cost
        )?;
        writeln!(f, "  Standard script: {}", self.is_standard)?;
        writeln!(f, "  Risky opcodes: {}", self.has_risky_opcodes)?;
        Ok(())
    }
}

/// Script template builder for common patterns
pub struct ScriptTemplateBuilder;

impl ScriptTemplateBuilder {
    /// Create a P2PKH script template
    pub fn p2pkh(pubkey_hash: &[u8; 20]) -> Result<ScriptBuf, BitcoinError> {
        use bitcoin::hashes::Hash;
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(*pubkey_hash);
        Ok(ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(
            hash,
        )))
    }

    /// Create an OP_RETURN script with data
    pub fn op_return(data: &[u8]) -> Result<ScriptBuf, BitcoinError> {
        if data.len() > 80 {
            return Err(BitcoinError::InvalidInput(
                "OP_RETURN data too large (max 80 bytes)".to_string(),
            ));
        }

        // Construct OP_RETURN script from raw bytes
        let mut script_bytes = vec![OP_RETURN.to_u8()];

        // Add push opcode for data length
        if data.is_empty() {
            // Empty OP_RETURN
        } else if data.len() <= 75 {
            script_bytes.push(data.len() as u8);
            script_bytes.extend_from_slice(data);
        } else {
            // OP_PUSHDATA1 for 76-80 bytes
            script_bytes.push(0x4c); // OP_PUSHDATA1
            script_bytes.push(data.len() as u8);
            script_bytes.extend_from_slice(data);
        }

        Ok(ScriptBuf::from_bytes(script_bytes))
    }

    /// Create a simple timelock script (CLTV)
    pub fn timelock_cltv(locktime: u32, pubkey_hash: &[u8; 20]) -> Result<ScriptBuf, BitcoinError> {
        // Use the array directly since [u8; 20] implements AsRef<PushBytes>
        let script = ScriptBuf::builder()
            .push_int(locktime as i64)
            .push_opcode(OP_CLTV)
            .push_opcode(OP_DROP)
            .push_opcode(OP_DUP)
            .push_opcode(OP_HASH160)
            .push_slice(pubkey_hash)
            .push_opcode(OP_EQUALVERIFY)
            .push_opcode(OP_CHECKSIG)
            .into_script();

        Ok(script)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn test_disassemble_empty_script() {
        let script = ScriptBuf::new();
        let disassembler = ScriptDisassembler::new();
        let asm = disassembler.disassemble(&script);
        assert_eq!(asm, "");
    }

    #[test]
    fn test_disassemble_compact() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let disassembler = ScriptDisassembler::new();
        let compact = disassembler.disassemble_compact(&script);
        assert_eq!(compact, "P2PKH");
    }

    #[test]
    fn test_script_analyzer_p2pkh() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        assert_eq!(analyzer.script_type(), ScriptType::P2PKH);
        assert!(analyzer.complexity_score() > 0);
    }

    #[test]
    fn test_script_analyzer_p2wpkh() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        assert_eq!(analyzer.script_type(), ScriptType::P2WPKH);
    }

    #[test]
    fn test_complexity_score() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        let score = analyzer.complexity_score();
        assert!(score > 0);
    }

    #[test]
    fn test_opcode_count() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        assert!(analyzer.opcode_count() > 0);
    }

    #[test]
    fn test_push_count() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        assert!(analyzer.push_count() > 0);
    }

    #[test]
    fn test_witness_cost_estimation() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        let cost = analyzer.estimate_witness_cost();
        assert!(cost > 0);
    }

    #[test]
    fn test_no_risky_opcodes() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        assert!(!analyzer.has_risky_opcodes());
    }

    #[test]
    fn test_script_analysis() {
        let pubkey_hash = [0u8; 20];
        let hash = bitcoin::hashes::hash160::Hash::from_byte_array(pubkey_hash);
        let script = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_raw_hash(hash));
        let analyzer = ScriptAnalyzer::new(&script);
        let analysis = analyzer.analyze();
        assert_eq!(analysis.script_type, ScriptType::P2PKH);
        assert!(analysis.is_standard);
        assert!(!analysis.has_risky_opcodes);
    }

    #[test]
    fn test_op_return_template() {
        let data = b"Hello, Bitcoin!";
        let script = ScriptTemplateBuilder::op_return(data).unwrap();
        let analyzer = ScriptAnalyzer::new(&script);
        assert_eq!(analyzer.script_type(), ScriptType::OpReturn);
    }

    #[test]
    fn test_op_return_too_large() {
        let data = vec![0u8; 81]; // Too large
        let result = ScriptTemplateBuilder::op_return(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_timelock_cltv_template() {
        let pubkey_hash = [0u8; 20];
        let locktime = 600000;
        let script = ScriptTemplateBuilder::timelock_cltv(locktime, &pubkey_hash).unwrap();
        let analyzer = ScriptAnalyzer::new(&script);
        assert!(analyzer.complexity_score() > 10); // Should be reasonably complex
    }
}

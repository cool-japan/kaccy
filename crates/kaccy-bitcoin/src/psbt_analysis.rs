//! PSBT analysis tools for validation and security checking
//!
//! Provides comprehensive analysis of PSBTs including fee verification,
//! input/output validation, and security warnings.

use crate::error::BitcoinError;
use bitcoin::psbt::Psbt;
use serde::{Deserialize, Serialize};

/// PSBT analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtAnalysis {
    /// Is the PSBT complete (all signatures present)?
    pub is_complete: bool,
    /// Total input value in satoshis
    pub total_input_value: u64,
    /// Total output value in satoshis
    pub total_output_value: u64,
    /// Transaction fee in satoshis
    pub fee: u64,
    /// Fee rate in sat/vB
    pub fee_rate: f64,
    /// Transaction size in virtual bytes
    pub vsize: usize,
    /// Validation warnings
    pub warnings: Vec<ValidationWarning>,
    /// Security issues detected
    pub security_issues: Vec<SecurityIssue>,
    /// Input analysis
    pub input_analysis: Vec<InputAnalysis>,
    /// Output analysis
    pub output_analysis: Vec<OutputAnalysis>,
}

/// Validation warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarning {
    /// Fee is unusually high
    HighFee { fee: u64, percentage: f64 },
    /// Fee rate is very high
    HighFeeRate { rate: f64 },
    /// Missing witness data
    MissingWitnessData { input_index: usize },
    /// Missing UTXO information
    MissingUtxoInfo { input_index: usize },
    /// Dust output detected
    DustOutput { output_index: usize, amount: u64 },
    /// Change output not identified
    UnidentifiedChange,
}

/// Security issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityIssue {
    /// Potential fee overpayment attack
    FeeOverpayment { expected_fee: u64, actual_fee: u64 },
    /// Unverified input amounts
    UnverifiedInputAmounts,
    /// Reused address detected
    AddressReuse { output_index: usize },
    /// Suspicious script type
    SuspiciousScript { input_index: usize },
}

/// Input analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAnalysis {
    /// Input index
    pub index: usize,
    /// Input value (if known)
    pub value: Option<u64>,
    /// Script type
    pub script_type: String,
    /// Has witness data
    pub has_witness: bool,
    /// Has non-witness UTXO
    pub has_non_witness_utxo: bool,
    /// Has witness UTXO
    pub has_witness_utxo: bool,
    /// Signature count
    pub signature_count: usize,
    /// Required signatures
    pub required_signatures: Option<usize>,
}

/// Output analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputAnalysis {
    /// Output index
    pub index: usize,
    /// Output value
    pub value: u64,
    /// Script type
    pub script_type: String,
    /// Is likely change output
    pub is_likely_change: bool,
    /// Is dust
    pub is_dust: bool,
}

/// PSBT analyzer
#[allow(dead_code)]
pub struct PsbtAnalyzer {
    /// Minimum relay fee rate (sat/vB)
    min_relay_fee_rate: f64,
    /// High fee threshold percentage
    high_fee_threshold: f64,
    /// High fee rate threshold (sat/vB)
    high_fee_rate_threshold: f64,
    /// Dust threshold (satoshis)
    dust_threshold: u64,
}

impl PsbtAnalyzer {
    /// Create a new PSBT analyzer with default settings
    pub fn new() -> Self {
        Self {
            min_relay_fee_rate: 1.0,
            high_fee_threshold: 0.05,       // 5%
            high_fee_rate_threshold: 100.0, // 100 sat/vB
            dust_threshold: 546,
        }
    }

    /// Create analyzer with custom settings
    pub fn with_config(
        min_relay_fee_rate: f64,
        high_fee_threshold: f64,
        high_fee_rate_threshold: f64,
        dust_threshold: u64,
    ) -> Self {
        Self {
            min_relay_fee_rate,
            high_fee_threshold,
            high_fee_rate_threshold,
            dust_threshold,
        }
    }

    /// Analyze a PSBT
    pub fn analyze(&self, psbt: &Psbt) -> Result<PsbtAnalysis, BitcoinError> {
        // Calculate total input value
        let total_input_value = self.calculate_input_value(psbt)?;

        // Calculate total output value
        let total_output_value: u64 = psbt
            .unsigned_tx
            .output
            .iter()
            .map(|o| o.value.to_sat())
            .sum();

        // Calculate fee
        let fee = total_input_value.saturating_sub(total_output_value);

        // Estimate transaction size
        let vsize = self.estimate_vsize(psbt);

        // Calculate fee rate
        let fee_rate = if vsize > 0 {
            fee as f64 / vsize as f64
        } else {
            0.0
        };

        // Check if complete
        let is_complete = self.check_completeness(psbt);

        // Analyze inputs
        let input_analysis = self.analyze_inputs(psbt);

        // Analyze outputs
        let output_analysis = self.analyze_outputs(psbt);

        // Collect warnings
        let mut warnings = Vec::new();
        self.check_fee_warnings(&mut warnings, fee, total_output_value, fee_rate);
        self.check_input_warnings(&mut warnings, &input_analysis);
        self.check_output_warnings(&mut warnings, &output_analysis);

        // Collect security issues
        let mut security_issues = Vec::new();
        self.check_security_issues(&mut security_issues, psbt, fee, &input_analysis);

        Ok(PsbtAnalysis {
            is_complete,
            total_input_value,
            total_output_value,
            fee,
            fee_rate,
            vsize,
            warnings,
            security_issues,
            input_analysis,
            output_analysis,
        })
    }

    /// Calculate total input value
    fn calculate_input_value(&self, psbt: &Psbt) -> Result<u64, BitcoinError> {
        let mut total = 0u64;

        for input in &psbt.inputs {
            if let Some(witness_utxo) = &input.witness_utxo {
                total += witness_utxo.value.to_sat();
            } else if let Some(non_witness_utxo) = &input.non_witness_utxo {
                // Find corresponding output in previous transaction
                if let Some(vout) = psbt
                    .unsigned_tx
                    .input
                    .first()
                    .map(|i| i.previous_output.vout)
                {
                    if let Some(output) = non_witness_utxo.output.get(vout as usize) {
                        total += output.value.to_sat();
                    }
                }
            } else {
                return Err(BitcoinError::InvalidTransaction(
                    "Missing UTXO information for input".to_string(),
                ));
            }
        }

        Ok(total)
    }

    /// Estimate virtual size
    fn estimate_vsize(&self, psbt: &Psbt) -> usize {
        // Basic estimation - in production, use more accurate methods
        let input_count = psbt.unsigned_tx.input.len();
        let output_count = psbt.unsigned_tx.output.len();

        // Base size + inputs + outputs
        10 + (input_count * 68) + (output_count * 31)
    }

    /// Check if PSBT is complete
    fn check_completeness(&self, psbt: &Psbt) -> bool {
        for input in &psbt.inputs {
            if input.final_script_sig.is_none() && input.final_script_witness.is_none() {
                return false;
            }
        }
        true
    }

    /// Analyze inputs
    fn analyze_inputs(&self, psbt: &Psbt) -> Vec<InputAnalysis> {
        psbt.inputs
            .iter()
            .enumerate()
            .map(|(index, input)| {
                let value = input.witness_utxo.as_ref().map(|u| u.value.to_sat());

                let script_type = if input.witness_utxo.is_some() {
                    "SegWit".to_string()
                } else if input.non_witness_utxo.is_some() {
                    "Legacy".to_string()
                } else {
                    "Unknown".to_string()
                };

                let has_witness = input.final_script_witness.is_some();
                let signature_count = input.partial_sigs.len();

                InputAnalysis {
                    index,
                    value,
                    script_type,
                    has_witness,
                    has_non_witness_utxo: input.non_witness_utxo.is_some(),
                    has_witness_utxo: input.witness_utxo.is_some(),
                    signature_count,
                    required_signatures: None, // Could be extracted from redeem script
                }
            })
            .collect()
    }

    /// Analyze outputs
    fn analyze_outputs(&self, psbt: &Psbt) -> Vec<OutputAnalysis> {
        psbt.unsigned_tx
            .output
            .iter()
            .enumerate()
            .map(|(index, output)| {
                let value = output.value.to_sat();
                let is_dust = value < self.dust_threshold;

                let script_type = if output.script_pubkey.is_p2pkh() {
                    "P2PKH".to_string()
                } else if output.script_pubkey.is_p2sh() {
                    "P2SH".to_string()
                } else if output.script_pubkey.is_p2wpkh() {
                    "P2WPKH".to_string()
                } else if output.script_pubkey.is_p2wsh() {
                    "P2WSH".to_string()
                } else if output.script_pubkey.is_p2tr() {
                    "P2TR".to_string()
                } else {
                    "Unknown".to_string()
                };

                OutputAnalysis {
                    index,
                    value,
                    script_type,
                    is_likely_change: false, // Could use heuristics
                    is_dust,
                }
            })
            .collect()
    }

    /// Check fee-related warnings
    fn check_fee_warnings(
        &self,
        warnings: &mut Vec<ValidationWarning>,
        fee: u64,
        total_output: u64,
        fee_rate: f64,
    ) {
        // Check if fee is too high as percentage
        let fee_percentage = fee as f64 / (total_output + fee) as f64;
        if fee_percentage > self.high_fee_threshold {
            warnings.push(ValidationWarning::HighFee {
                fee,
                percentage: fee_percentage * 100.0,
            });
        }

        // Check if fee rate is too high
        if fee_rate > self.high_fee_rate_threshold {
            warnings.push(ValidationWarning::HighFeeRate { rate: fee_rate });
        }
    }

    /// Check input-related warnings
    fn check_input_warnings(
        &self,
        warnings: &mut Vec<ValidationWarning>,
        input_analysis: &[InputAnalysis],
    ) {
        for input in input_analysis {
            if !input.has_witness && !input.has_non_witness_utxo && !input.has_witness_utxo {
                warnings.push(ValidationWarning::MissingUtxoInfo {
                    input_index: input.index,
                });
            }

            if input.script_type == "SegWit" && !input.has_witness {
                warnings.push(ValidationWarning::MissingWitnessData {
                    input_index: input.index,
                });
            }
        }
    }

    /// Check output-related warnings
    fn check_output_warnings(
        &self,
        warnings: &mut Vec<ValidationWarning>,
        output_analysis: &[OutputAnalysis],
    ) {
        for output in output_analysis {
            if output.is_dust {
                warnings.push(ValidationWarning::DustOutput {
                    output_index: output.index,
                    amount: output.value,
                });
            }
        }
    }

    /// Check security issues
    #[allow(dead_code)]
    fn check_security_issues(
        &self,
        security_issues: &mut Vec<SecurityIssue>,
        psbt: &Psbt,
        fee: u64,
        input_analysis: &[InputAnalysis],
    ) {
        // Check for unverified input amounts
        let has_unverified = input_analysis.iter().any(|i| i.value.is_none());
        if has_unverified {
            security_issues.push(SecurityIssue::UnverifiedInputAmounts);
        }

        // Check for potential fee overpayment
        // This is a simple heuristic - in production, use more sophisticated checks
        let vsize = self.estimate_vsize(psbt);
        let expected_fee = (vsize as f64 * 10.0) as u64; // Assume 10 sat/vB is reasonable
        if fee > expected_fee * 10 {
            security_issues.push(SecurityIssue::FeeOverpayment {
                expected_fee,
                actual_fee: fee,
            });
        }
    }
}

impl Default for PsbtAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Fee verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeVerification {
    /// Is the fee reasonable?
    pub is_reasonable: bool,
    /// Calculated fee
    pub fee: u64,
    /// Fee rate
    pub fee_rate: f64,
    /// Expected fee range
    pub expected_fee_range: (u64, u64),
    /// Issues found
    pub issues: Vec<String>,
}

/// Verify PSBT fees
pub fn verify_psbt_fees(
    psbt: &Psbt,
    expected_fee_rate: f64,
    tolerance: f64,
) -> Result<FeeVerification, BitcoinError> {
    let analyzer = PsbtAnalyzer::new();
    let analysis = analyzer.analyze(psbt)?;

    let min_expected_fee = (analysis.vsize as f64 * expected_fee_rate * (1.0 - tolerance)) as u64;
    let max_expected_fee = (analysis.vsize as f64 * expected_fee_rate * (1.0 + tolerance)) as u64;

    let is_reasonable = analysis.fee >= min_expected_fee && analysis.fee <= max_expected_fee;

    let mut issues = Vec::new();
    if analysis.fee < min_expected_fee {
        issues.push(format!(
            "Fee too low: {} sat (expected at least {} sat)",
            analysis.fee, min_expected_fee
        ));
    }
    if analysis.fee > max_expected_fee {
        issues.push(format!(
            "Fee too high: {} sat (expected at most {} sat)",
            analysis.fee, max_expected_fee
        ));
    }

    Ok(FeeVerification {
        is_reasonable,
        fee: analysis.fee,
        fee_rate: analysis.fee_rate,
        expected_fee_range: (min_expected_fee, max_expected_fee),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psbt_analyzer_new() {
        let analyzer = PsbtAnalyzer::new();
        assert_eq!(analyzer.min_relay_fee_rate, 1.0);
        assert_eq!(analyzer.high_fee_threshold, 0.05);
        assert_eq!(analyzer.dust_threshold, 546);
    }

    #[test]
    fn test_psbt_analyzer_custom() {
        let analyzer = PsbtAnalyzer::with_config(2.0, 0.10, 200.0, 1000);
        assert_eq!(analyzer.min_relay_fee_rate, 2.0);
        assert_eq!(analyzer.high_fee_threshold, 0.10);
        assert_eq!(analyzer.high_fee_rate_threshold, 200.0);
        assert_eq!(analyzer.dust_threshold, 1000);
    }

    #[test]
    fn test_validation_warning_types() {
        let warning = ValidationWarning::HighFee {
            fee: 10000,
            percentage: 10.0,
        };
        assert!(matches!(warning, ValidationWarning::HighFee { .. }));
    }

    #[test]
    fn test_security_issue_types() {
        let issue = SecurityIssue::FeeOverpayment {
            expected_fee: 1000,
            actual_fee: 10000,
        };
        assert!(matches!(issue, SecurityIssue::FeeOverpayment { .. }));
    }
}

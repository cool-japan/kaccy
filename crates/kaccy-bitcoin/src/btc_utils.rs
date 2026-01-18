//! Bitcoin utility functions for common calculations and conversions
//!
//! Provides helper functions for:
//! - Unit conversions (BTC, mBTC, satoshis)
//! - Fee calculations
//! - Transaction size estimation
//! - Script size calculations

use bitcoin::{Amount, ScriptBuf, Transaction, TxOut};

/// Convert satoshis to BTC
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::satoshis_to_btc;
///
/// let btc = satoshis_to_btc(100_000_000);
/// assert_eq!(btc, 1.0);
/// ```
#[must_use]
pub fn satoshis_to_btc(satoshis: u64) -> f64 {
    satoshis as f64 / 100_000_000.0
}

/// Convert BTC to satoshis
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::btc_to_satoshis;
///
/// let sats = btc_to_satoshis(1.0);
/// assert_eq!(sats, 100_000_000);
/// ```
#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
pub fn btc_to_satoshis(btc: f64) -> u64 {
    (btc * 100_000_000.0) as u64
}

/// Convert satoshis to mBTC (milliBitcoin)
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::satoshis_to_mbtc;
///
/// let mbtc = satoshis_to_mbtc(100_000);
/// assert_eq!(mbtc, 1.0);
/// ```
#[must_use]
pub fn satoshis_to_mbtc(satoshis: u64) -> f64 {
    satoshis as f64 / 100_000.0
}

/// Convert mBTC to satoshis
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::mbtc_to_satoshis;
///
/// let sats = mbtc_to_satoshis(1.0);
/// assert_eq!(sats, 100_000);
/// ```
#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
pub fn mbtc_to_satoshis(mbtc: f64) -> u64 {
    (mbtc * 100_000.0) as u64
}

/// Estimate virtual size (vsize) for a transaction
///
/// For legacy transactions, vsize equals size.
/// For SegWit transactions, vsize = (weight / 4)
#[must_use]
pub fn estimate_transaction_vsize(tx: &Transaction) -> u64 {
    let weight = tx.weight();
    weight.to_wu().div_ceil(4)
}

/// Calculate fee rate in sat/vB
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::calculate_fee_rate;
///
/// let fee_rate = calculate_fee_rate(1000, 200);
/// assert_eq!(fee_rate, 5); // 1000 sats / 200 vbytes = 5 sat/vB
/// ```
#[must_use]
pub fn calculate_fee_rate(fee_satoshis: u64, vsize: u64) -> u64 {
    if vsize == 0 {
        return 0;
    }
    fee_satoshis / vsize
}

/// Calculate fee from fee rate and vsize
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::calculate_fee_from_rate;
///
/// let fee = calculate_fee_from_rate(5, 200);
/// assert_eq!(fee, 1000); // 5 sat/vB * 200 vbytes = 1000 sats
/// ```
#[must_use]
pub fn calculate_fee_from_rate(fee_rate_sat_per_vb: u64, vsize: u64) -> u64 {
    fee_rate_sat_per_vb * vsize
}

/// Estimate P2PKH input size in vbytes
///
/// P2PKH input: ~148 vbytes
pub const P2PKH_INPUT_VSIZE: u64 = 148;

/// Estimate P2WPKH input size in vbytes
///
/// P2WPKH input: ~68 vbytes (SegWit native)
pub const P2WPKH_INPUT_VSIZE: u64 = 68;

/// Estimate P2SH-P2WPKH input size in vbytes
///
/// P2SH-P2WPKH input: ~91 vbytes (SegWit wrapped)
pub const P2SH_P2WPKH_INPUT_VSIZE: u64 = 91;

/// Estimate P2TR (Taproot) input size in vbytes
///
/// P2TR key-path spend: ~58 vbytes
pub const P2TR_INPUT_VSIZE: u64 = 58;

/// Estimate P2PKH output size in vbytes
///
/// P2PKH output: ~34 vbytes
pub const P2PKH_OUTPUT_VSIZE: u64 = 34;

/// Estimate P2WPKH output size in vbytes
///
/// P2WPKH output: ~31 vbytes
pub const P2WPKH_OUTPUT_VSIZE: u64 = 31;

/// Estimate P2SH output size in vbytes
///
/// P2SH output: ~32 vbytes
pub const P2SH_OUTPUT_VSIZE: u64 = 32;

/// Estimate P2TR (Taproot) output size in vbytes
///
/// P2TR output: ~43 vbytes
pub const P2TR_OUTPUT_VSIZE: u64 = 43;

/// Transaction overhead (version, locktime, input/output counts)
pub const TX_OVERHEAD_VSIZE: u64 = 10;

/// Estimate transaction size for a simple transaction
///
/// # Arguments
///
/// * `num_inputs` - Number of inputs
/// * `num_outputs` - Number of outputs
/// * `input_type_vsize` - Average vsize per input (use constants like P2WPKH_INPUT_VSIZE)
/// * `output_type_vsize` - Average vsize per output (use constants like P2WPKH_OUTPUT_VSIZE)
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::{estimate_simple_transaction_vsize, P2WPKH_INPUT_VSIZE, P2WPKH_OUTPUT_VSIZE};
///
/// let vsize = estimate_simple_transaction_vsize(2, 2, P2WPKH_INPUT_VSIZE, P2WPKH_OUTPUT_VSIZE);
/// // 2 inputs * 68 + 2 outputs * 31 + 10 overhead = 208 vbytes
/// assert_eq!(vsize, 208);
/// ```
#[must_use]
pub fn estimate_simple_transaction_vsize(
    num_inputs: u64,
    num_outputs: u64,
    input_type_vsize: u64,
    output_type_vsize: u64,
) -> u64 {
    TX_OVERHEAD_VSIZE + (num_inputs * input_type_vsize) + (num_outputs * output_type_vsize)
}

/// Check if amount is considered dust
///
/// Dust is an output where the cost to spend it (in fees) would be
/// more than 1/3 of its value. This is based on Bitcoin Core's dust
/// calculation where dust_threshold = (input_size + output_size) * 3 * fee_rate.
///
/// For P2WPKH: (68 + 31) * 3 = 297 vbytes * fee_rate
/// At 3 sat/vB: 297 * 3 = 891 satoshis (rounds to ~900)
/// Standard dust threshold is 546 satoshis at 1 sat/vB for P2WPKH.
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::is_dust;
///
/// assert!(is_dust(500, 3)); // 500 sats is dust at 3 sat/vB
/// assert!(!is_dust(1000, 3)); // 1000 sats is not dust at 3 sat/vB
/// ```
#[must_use]
pub fn is_dust(amount_satoshis: u64, fee_rate_sat_per_vb: u64) -> bool {
    // Bitcoin Core dust calculation: amount < (input_size + output_size) * 3 * fee_rate
    // For P2WPKH: (68 + 31) * 3 = 297
    let dust_threshold = (P2WPKH_INPUT_VSIZE + P2WPKH_OUTPUT_VSIZE) * 3 * fee_rate_sat_per_vb;
    amount_satoshis < dust_threshold
}

/// Standard dust threshold in satoshis
///
/// This is the commonly used dust threshold for P2WPKH outputs at 1 sat/vB
pub const DUST_THRESHOLD: u64 = 546;

/// Calculate the effective value of a UTXO
///
/// Effective value = amount - cost_to_spend
/// This helps in coin selection to account for the fee cost of spending each input.
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::{calculate_effective_value, P2WPKH_INPUT_VSIZE};
///
/// // UTXO of 10,000 sats, at 5 sat/vB fee rate
/// let effective = calculate_effective_value(10_000, 5, P2WPKH_INPUT_VSIZE);
/// // 10,000 - (5 * 68) = 10,000 - 340 = 9,660
/// assert_eq!(effective, 9_660);
/// ```
#[must_use]
pub fn calculate_effective_value(
    amount_satoshis: u64,
    fee_rate_sat_per_vb: u64,
    input_vsize: u64,
) -> i64 {
    let amount = amount_satoshis as i64;
    let cost = (fee_rate_sat_per_vb * input_vsize) as i64;
    amount - cost
}

/// Convert Amount to satoshis
#[must_use]
pub fn amount_to_satoshis(amount: &Amount) -> u64 {
    amount.to_sat()
}

/// Check if a transaction output is an OP_RETURN (data carrier)
#[must_use]
pub fn is_op_return(output: &TxOut) -> bool {
    output.script_pubkey.is_op_return()
}

/// Get the size of a script in bytes
#[must_use]
pub fn script_size(script: &ScriptBuf) -> usize {
    script.len()
}

/// Calculate the percentage fee of a transaction
///
/// Returns the fee as a percentage of the total output value
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::calculate_fee_percentage;
///
/// // Fee is 1000 sats, total output is 100,000 sats
/// let percentage = calculate_fee_percentage(1000, 100_000);
/// assert_eq!(percentage, 1.0); // 1%
/// ```
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn calculate_fee_percentage(fee_satoshis: u64, total_output_satoshis: u64) -> f64 {
    if total_output_satoshis == 0 {
        return 0.0;
    }
    (fee_satoshis as f64 / total_output_satoshis as f64) * 100.0
}

/// Round amount to the nearest satoshi value for privacy
///
/// Rounds to the specified number of significant digits to avoid
/// fingerprintable amounts (e.g., round 12,345 to 12,000)
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::round_for_privacy;
///
/// // Round to nearest 1000
/// assert_eq!(round_for_privacy(12_345, 1000), 12_000);
/// assert_eq!(round_for_privacy(12_678, 1000), 13_000);
/// ```
#[must_use]
pub fn round_for_privacy(amount_satoshis: u64, round_to: u64) -> u64 {
    if round_to == 0 {
        return amount_satoshis;
    }
    ((amount_satoshis + round_to / 2) / round_to) * round_to
}

/// Convert weight units to virtual bytes (vsize)
///
/// vsize = weight / 4 (rounded up)
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::weight_to_vsize;
///
/// assert_eq!(weight_to_vsize(400), 100); // 400 WU = 100 vB
/// assert_eq!(weight_to_vsize(401), 101); // Rounds up
/// ```
#[must_use]
pub fn weight_to_vsize(weight: u64) -> u64 {
    weight.div_ceil(4)
}

/// Convert vsize to weight units
///
/// weight = vsize * 4
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::vsize_to_weight;
///
/// assert_eq!(vsize_to_weight(100), 400); // 100 vB = 400 WU
/// ```
#[must_use]
pub fn vsize_to_weight(vsize: u64) -> u64 {
    vsize * 4
}

/// Calculate change amount for a transaction
///
/// change = total_input - target_output - fee
///
/// Returns `None` if insufficient funds or change would be dust
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::calculate_change;
///
/// // Sufficient change
/// let change = calculate_change(100_000, 50_000, 1_000, 600);
/// assert_eq!(change, Some(49_000)); // 100,000 - 50,000 - 1,000
///
/// // Change is dust (below minimum)
/// let dust_change = calculate_change(51_500, 50_000, 1_000, 600);
/// assert_eq!(dust_change, None); // Change of 500 is below minimum (600)
/// ```
#[must_use]
pub fn calculate_change(
    total_input: u64,
    target_output: u64,
    fee: u64,
    min_change: u64,
) -> Option<u64> {
    let total_deductions = target_output.saturating_add(fee);

    if total_input < total_deductions {
        return None; // Insufficient funds
    }

    let change = total_input - total_deductions;

    if change < min_change {
        None // Change too small (dust)
    } else {
        Some(change)
    }
}

/// Validate that an amount is within valid Bitcoin range
///
/// Bitcoin amounts must be between 0 and 21 million BTC (2,100,000,000,000,000 satoshis)
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::is_valid_amount;
///
/// assert!(is_valid_amount(100_000_000)); // 1 BTC is valid
/// assert!(is_valid_amount(0)); // 0 is valid
/// assert!(!is_valid_amount(2_100_000_000_000_001)); // Over max supply
/// ```
#[must_use]
pub fn is_valid_amount(satoshis: u64) -> bool {
    const MAX_SATOSHIS: u64 = 21_000_000 * 100_000_000; // 21 million BTC
    satoshis <= MAX_SATOSHIS
}

/// Calculate the total fee for multiple transactions
///
/// Useful for batch processing and estimating costs
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::calculate_batch_fee;
///
/// let vsizes = vec![200, 150, 180];
/// let fee = calculate_batch_fee(&vsizes, 5);
/// assert_eq!(fee, 2_650); // (200 + 150 + 180) * 5 = 2,650 sats
/// ```
#[must_use]
pub fn calculate_batch_fee(transaction_vsizes: &[u64], fee_rate_sat_per_vb: u64) -> u64 {
    transaction_vsizes.iter().sum::<u64>() * fee_rate_sat_per_vb
}

/// Calculate fee savings from batching transactions
///
/// Compares individual transaction fees vs batched transaction fee
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::btc_utils::calculate_batch_savings;
///
/// let individual_vsizes = vec![200, 200, 200]; // 3 separate transactions
/// let batched_vsize = 450; // Combined with shared overhead
/// let savings = calculate_batch_savings(&individual_vsizes, batched_vsize, 5);
/// assert_eq!(savings, 750); // (600 - 450) * 5 = 750 sats saved
/// ```
#[must_use]
pub fn calculate_batch_savings(
    individual_vsizes: &[u64],
    batched_vsize: u64,
    fee_rate_sat_per_vb: u64,
) -> i64 {
    let individual_fee = calculate_batch_fee(individual_vsizes, fee_rate_sat_per_vb);
    let batched_fee = batched_vsize * fee_rate_sat_per_vb;
    individual_fee as i64 - batched_fee as i64
}

/// Estimate witness size for P2WPKH input
///
/// Witness for P2WPKH: signature (72-73 bytes) + pubkey (33 bytes)
#[must_use]
pub fn estimate_p2wpkh_witness_size() -> usize {
    // Signature (72) + pubkey (33) + compact size prefixes
    107
}

/// Estimate witness size for P2TR key-path spend
///
/// Witness for P2TR key-path: signature (64-65 bytes)
#[must_use]
pub fn estimate_p2tr_keypath_witness_size() -> usize {
    // Schnorr signature (64 bytes) + compact size prefix
    65
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_satoshis_to_btc() {
        assert_eq!(satoshis_to_btc(100_000_000), 1.0);
        assert_eq!(satoshis_to_btc(50_000_000), 0.5);
        assert_eq!(satoshis_to_btc(1), 0.00000001);
    }

    #[test]
    fn test_btc_to_satoshis() {
        assert_eq!(btc_to_satoshis(1.0), 100_000_000);
        assert_eq!(btc_to_satoshis(0.5), 50_000_000);
        assert_eq!(btc_to_satoshis(0.00000001), 1);
    }

    #[test]
    fn test_satoshis_to_mbtc() {
        assert_eq!(satoshis_to_mbtc(100_000), 1.0);
        assert_eq!(satoshis_to_mbtc(50_000), 0.5);
    }

    #[test]
    fn test_mbtc_to_satoshis() {
        assert_eq!(mbtc_to_satoshis(1.0), 100_000);
        assert_eq!(mbtc_to_satoshis(0.5), 50_000);
    }

    #[test]
    fn test_calculate_fee_rate() {
        assert_eq!(calculate_fee_rate(1000, 200), 5);
        assert_eq!(calculate_fee_rate(500, 100), 5);
        assert_eq!(calculate_fee_rate(0, 100), 0);
        assert_eq!(calculate_fee_rate(100, 0), 0);
    }

    #[test]
    fn test_calculate_fee_from_rate() {
        assert_eq!(calculate_fee_from_rate(5, 200), 1000);
        assert_eq!(calculate_fee_from_rate(10, 150), 1500);
    }

    #[test]
    fn test_estimate_simple_transaction_vsize() {
        // 2 P2WPKH inputs + 2 P2WPKH outputs + overhead
        let vsize =
            estimate_simple_transaction_vsize(2, 2, P2WPKH_INPUT_VSIZE, P2WPKH_OUTPUT_VSIZE);
        assert_eq!(vsize, 208); // 2*68 + 2*31 + 10
    }

    #[test]
    fn test_is_dust() {
        // At 1 sat/vB: dust threshold = (68 + 31) * 3 * 1 = 297 sats
        assert!(is_dust(200, 1));
        assert!(!is_dust(300, 1));
        assert!(!is_dust(1000, 1));

        // At 3 sat/vB: dust threshold = (68 + 31) * 3 * 3 = 891 sats
        assert!(is_dust(800, 3));
        assert!(!is_dust(900, 3));

        // Standard dust threshold (546 sats) is not dust at low fee rates
        assert!(!is_dust(DUST_THRESHOLD, 1));
    }

    #[test]
    fn test_calculate_effective_value() {
        let effective = calculate_effective_value(10_000, 5, P2WPKH_INPUT_VSIZE);
        assert_eq!(effective, 9_660); // 10,000 - (5 * 68)
    }

    #[test]
    fn test_calculate_effective_value_negative() {
        // Small UTXO with high fee rate results in negative effective value
        let effective = calculate_effective_value(100, 10, P2WPKH_INPUT_VSIZE);
        assert_eq!(effective, -580); // 100 - (10 * 68)
    }

    #[test]
    fn test_calculate_fee_percentage() {
        assert_eq!(calculate_fee_percentage(1000, 100_000), 1.0);
        assert_eq!(calculate_fee_percentage(5000, 100_000), 5.0);
        assert_eq!(calculate_fee_percentage(0, 100_000), 0.0);
        assert_eq!(calculate_fee_percentage(1000, 0), 0.0);
    }

    #[test]
    fn test_round_for_privacy() {
        assert_eq!(round_for_privacy(12_345, 1000), 12_000);
        assert_eq!(round_for_privacy(12_678, 1000), 13_000);
        assert_eq!(round_for_privacy(12_500, 1000), 13_000);
        assert_eq!(round_for_privacy(100, 10), 100);
        assert_eq!(round_for_privacy(105, 10), 110);
    }

    #[test]
    fn test_vsize_constants() {
        // Verify constants are reasonable (using static assertions)
        const _: () = assert!(P2WPKH_INPUT_VSIZE < P2PKH_INPUT_VSIZE);
        const _: () = assert!(P2TR_INPUT_VSIZE < P2WPKH_INPUT_VSIZE);
        const _: () = assert!(P2WPKH_OUTPUT_VSIZE < P2PKH_OUTPUT_VSIZE);

        // Also verify at runtime for test coverage
        assert_eq!(P2WPKH_INPUT_VSIZE, 68);
        assert_eq!(P2TR_INPUT_VSIZE, 58);
    }

    #[test]
    fn test_weight_to_vsize() {
        assert_eq!(weight_to_vsize(400), 100); // Exact division
        assert_eq!(weight_to_vsize(401), 101); // Rounds up
        assert_eq!(weight_to_vsize(402), 101); // Rounds up
        assert_eq!(weight_to_vsize(403), 101); // Rounds up
        assert_eq!(weight_to_vsize(404), 101); // Rounds up
        assert_eq!(weight_to_vsize(0), 0); // Zero case
    }

    #[test]
    fn test_vsize_to_weight() {
        assert_eq!(vsize_to_weight(100), 400);
        assert_eq!(vsize_to_weight(0), 0);
        assert_eq!(vsize_to_weight(1), 4);
    }

    #[test]
    fn test_calculate_change() {
        // Normal case with sufficient change
        let change = calculate_change(100_000, 50_000, 1_000, 600);
        assert_eq!(change, Some(49_000)); // 100,000 - 50,000 - 1,000

        // Exact amount (no change)
        let no_change = calculate_change(51_000, 50_000, 1_000, 600);
        assert_eq!(no_change, None);

        // Change too small (dust)
        let dust = calculate_change(51_500, 50_000, 1_000, 600);
        assert_eq!(dust, None);

        // Insufficient funds
        let insufficient = calculate_change(40_000, 50_000, 1_000, 600);
        assert_eq!(insufficient, None);

        // Minimum change threshold
        let min_threshold = calculate_change(51_600, 50_000, 1_000, 600);
        assert_eq!(min_threshold, Some(600)); // Exactly at minimum
    }

    #[test]
    fn test_is_valid_amount() {
        // Valid amounts
        assert!(is_valid_amount(0));
        assert!(is_valid_amount(100_000_000)); // 1 BTC
        assert!(is_valid_amount(2_100_000_000_000_000)); // Max supply (21M BTC)

        // Invalid amounts (over max supply)
        assert!(!is_valid_amount(2_100_000_000_000_001));
        assert!(!is_valid_amount(u64::MAX));
    }

    #[test]
    fn test_calculate_batch_fee() {
        let vsizes = vec![200, 150, 180];
        let fee = calculate_batch_fee(&vsizes, 5);
        assert_eq!(fee, 2_650); // (200 + 150 + 180) * 5

        // Empty batch
        let empty_fee = calculate_batch_fee(&[], 5);
        assert_eq!(empty_fee, 0);

        // Single transaction
        let single_fee = calculate_batch_fee(&[100], 10);
        assert_eq!(single_fee, 1_000);
    }

    #[test]
    fn test_calculate_batch_savings() {
        // 3 individual transactions vs 1 batched
        let individual_vsizes = vec![200, 200, 200]; // Total 600 vB
        let batched_vsize = 450; // Shared overhead saves space
        let savings = calculate_batch_savings(&individual_vsizes, batched_vsize, 5);
        assert_eq!(savings, 750); // (600 - 450) * 5

        // No savings case (batched is larger, should be negative)
        let no_savings = calculate_batch_savings(&[100], 150, 5);
        assert_eq!(no_savings, -250); // (100 - 150) * 5

        // Equal size (no savings)
        let equal = calculate_batch_savings(&[100, 100], 200, 5);
        assert_eq!(equal, 0);
    }

    #[test]
    fn test_estimate_witness_sizes() {
        // Just verify the functions return reasonable values
        let p2wpkh_witness = estimate_p2wpkh_witness_size();
        assert!(p2wpkh_witness > 0);
        assert!(p2wpkh_witness < 150); // Reasonable upper bound

        let p2tr_witness = estimate_p2tr_keypath_witness_size();
        assert!(p2tr_witness > 0);
        assert!(p2tr_witness < 100); // Taproot is more efficient

        // P2TR should be smaller than P2WPKH
        assert!(p2tr_witness < p2wpkh_witness);
    }
}

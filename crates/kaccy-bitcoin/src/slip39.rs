//! SLIP-0039 Shamir's Secret-Sharing for Mnemonic Codes.
//!
//! This module implements the [SatoshiLabs SLIP-0039 specification][slip39]
//! faithfully, without simplification. It splits a master secret into a
//! two-level (group + member) threshold structure of mnemonic shares, any
//! authorized subset of which reconstructs the secret.
//!
//! The cryptographic core is real:
//!
//! - **GF(256)** arithmetic over the Rijndael reducing polynomial `0x11B`
//!   with generator `3`, using precomputed exp/log tables (real table-driven
//!   multiply and inverse).
//! - **Shamir splitting/recovery** with the SLIP-39 digest share (index
//!   `254`) and secret share (index `255`), including the `threshold == 1`
//!   special case and HMAC-SHA256 digest verification on recovery.
//! - **RS1024** Reed–Solomon checksum over GF(1024) on each share's 10-bit
//!   word stream, using the `"shamir"` / `"shamir_extendable"` customization
//!   strings.
//! - **Feistel passphrase encryption** using PBKDF2-HMAC-SHA256 with the
//!   correct round count, salt and iteration exponent (NOT XOR).
//!
//! The PBKDF2/HMAC-SHA256 primitives are built on top of the SHA-256 and HMAC
//! engines re-exported by the `bitcoin` crate (`bitcoin::hashes`), which are
//! RFC 2104 / RFC 8018 compliant. (The crate's dependency set exposes SHA-256
//! through `bitcoin::hashes`; the implementation here does not depend on the
//! `sha2` crate being a direct dependency.)
//!
//! Both the original (`extendable = false`) and the extendable-backup
//! (`extendable = true`) variants of the current spec are supported on decode,
//! so the official SLIP-39 test vectors round-trip.
//!
//! [slip39]: https://github.com/satoshilabs/slips/blob/master/slip-0039.md
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_bitcoin::slip39::{Slip39Generator, ShareThreshold};
//!
//! // Generate 5 shares where any 3 can recover the secret.
//! let generator = Slip39Generator::new();
//! let shares = generator
//!     .generate_shares(&[0u8; 32], ShareThreshold::new(3, 5).unwrap(), None)
//!     .unwrap();
//!
//! // Each share serializes to a SLIP-39 mnemonic.
//! for share in &shares {
//!     println!("{}", share.to_mnemonic());
//! }
//!
//! // Any 3 shares recover the secret.
//! let recovered = Slip39Generator::recover_secret(&shares[0..3]).unwrap();
//! assert_eq!(recovered, [0u8; 32]);
//! ```

use crate::error::{BitcoinError, Result};
use bitcoin::hashes::{Hash, HashEngine, Hmac, HmacEngine, sha256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

// --- SLIP-0039 constants (see spec §"Constants").

/// Number of bits per encoded mnemonic word (the radix is `2^10 = 1024`).
const RADIX_BITS: usize = 10;

/// Size of the SLIP-39 wordlist (`2^RADIX_BITS`).
const WORDLIST_SIZE: usize = 1 << RADIX_BITS;

/// Length of the random share identifier, in bits.
const ID_LENGTH_BITS: usize = 15;

/// Length of the extendable backup flag, in bits.
const EXTENDABLE_FLAG_LENGTH_BITS: usize = 1;

/// Length of the iteration exponent field, in bits.
const ITERATION_EXP_LENGTH_BITS: usize = 4;

/// Number of words used to encode `identifier || ext || iteration_exponent`.
const ID_EXP_LENGTH_WORDS: usize =
    bits_to_words(ID_LENGTH_BITS + EXTENDABLE_FLAG_LENGTH_BITS + ITERATION_EXP_LENGTH_BITS);

/// Number of RS1024 checksum words appended to every share.
const CHECKSUM_LENGTH_WORDS: usize = 3;

/// Length of the truncated HMAC-SHA256 digest, in bytes.
const DIGEST_LENGTH_BYTES: usize = 4;

/// Maximum number of shares in a single group, and maximum number of groups.
const MAX_SHARE_COUNT: usize = 16;

/// Minimum master-secret strength, in bits.
const MIN_STRENGTH_BITS: usize = 128;

/// `x` coordinate at which the shared secret is stored (`f(255)`).
const SECRET_INDEX: u8 = 255;

/// `x` coordinate at which the digest is stored (`f(254)`).
const DIGEST_INDEX: u8 = 254;

/// Number of Feistel rounds in the passphrase cipher.
const ROUND_COUNT: u8 = 4;

/// Base PBKDF2 iteration count (scaled by `2^iteration_exponent`).
const BASE_ITERATION_COUNT: u32 = 10_000;

/// RS1024 customization string for the original (non-extendable) backup.
const CUSTOMIZATION_STRING_ORIG: &[u8] = b"shamir";

/// RS1024 customization string for the extendable backup.
const CUSTOMIZATION_STRING_EXTENDABLE: &[u8] = b"shamir_extendable";

/// Metadata words present in every mnemonic, independent of the share value:
/// `id || ext || e` (2 words) + group/member params (2 words) + checksum (3).
const METADATA_LENGTH_WORDS: usize = ID_EXP_LENGTH_WORDS + 2 + CHECKSUM_LENGTH_WORDS;

/// Minimum number of words in a valid mnemonic (128-bit secret).
const MIN_MNEMONIC_LENGTH_WORDS: usize = METADATA_LENGTH_WORDS + bits_to_words(MIN_STRENGTH_BITS);

/// Default iteration exponent (`0` => `BASE_ITERATION_COUNT` total iterations).
const DEFAULT_ITERATION_EXPONENT: u8 = 0;

/// Number of `radix`-sized digits required to store an `n`-bit value.
const fn round_bits(n: usize, radix_bits: usize) -> usize {
    n.div_ceil(radix_bits)
}

/// Number of bytes required to store an `n`-bit value.
const fn bits_to_bytes(n: usize) -> usize {
    round_bits(n, 8)
}

/// Number of 10-bit words required to store an `n`-bit value.
const fn bits_to_words(n: usize) -> usize {
    round_bits(n, RADIX_BITS)
}

// --- GF(256) finite-field arithmetic (Rijndael field, generator 3, poly 0x11B).

/// Precomputed exp/log tables for GF(256).
struct Gf256Tables {
    /// `exp[i] = generator^i` for `i in 0..255`.
    exp: [u8; 255],
    /// `log[exp[i]] = i`; `log[0]` is unused (multiplication by zero is
    /// special-cased everywhere it occurs).
    log: [u8; 256],
}

/// Returns the lazily-initialized GF(256) exp/log tables.
fn gf256_tables() -> &'static Gf256Tables {
    static TABLES: OnceLock<Gf256Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut exp = [0u8; 255];
        let mut log = [0u8; 256];
        let mut poly: u16 = 1;
        for (i, slot) in exp.iter_mut().enumerate() {
            *slot = poly as u8;
            log[poly as usize] = i as u8;
            // Multiply `poly` by the polynomial `x + 1` (the generator, 3).
            poly = (poly << 1) ^ poly;
            // Reduce modulo `x^8 + x^4 + x^3 + x + 1` (0x11B).
            if poly & 0x100 != 0 {
                poly ^= 0x11B;
            }
        }
        Gf256Tables { exp, log }
    })
}

/// GF(256) multiplication using the log/antilog tables.
fn gf256_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let t = gf256_tables();
    let log_sum = t.log[a as usize] as usize + t.log[b as usize] as usize;
    t.exp[log_sum % 255]
}

/// GF(256) multiplicative inverse (`a^-1`); `inverse(0)` is defined as `0`.
fn gf256_inverse(a: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let t = gf256_tables();
    // a^-1 = generator^(255 - log(a)).
    t.exp[(255 - t.log[a as usize] as usize) % 255]
}

// --- Shamir secret sharing over GF(256) (operates on raw `(x, data)` shares).

/// A raw Shamir share: an `x` coordinate and the per-byte `y` values.
#[derive(Clone)]
struct RawShare {
    /// The evaluation point (`x` coordinate) in GF(256).
    x: u8,
    /// The share value (`f(x)` evaluated independently per byte).
    data: Vec<u8>,
}

/// Lagrange-interpolates the polynomial(s) defined by `shares`, evaluated at
/// `x`, operating independently on each byte position in GF(256).
///
/// Returns an error if the share `x` coordinates are not unique or the share
/// values do not all share the same length.
fn interpolate(shares: &[RawShare], x: u8) -> Result<Vec<u8>> {
    if shares.is_empty() {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: cannot interpolate an empty set of shares".to_string(),
        ));
    }

    // Validate unique x coordinates and uniform length.
    let len = shares[0].data.len();
    for (i, s) in shares.iter().enumerate() {
        if s.data.len() != len {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: all share values must have the same length".to_string(),
            ));
        }
        for other in &shares[i + 1..] {
            if s.x == other.x {
                return Err(BitcoinError::InvalidInput(
                    "SLIP-39: share indices must be unique".to_string(),
                ));
            }
        }
    }

    // If `x` is one of the known points, return its value directly.
    for s in shares {
        if s.x == x {
            return Ok(s.data.clone());
        }
    }

    // Standard Lagrange interpolation over GF(256), evaluated at `x`, applied
    // independently to each byte position:
    //
    //   f(x) = sum_i y_i * prod_{j != i} (x - x_j) / (x_i - x_j)
    //
    // Subtraction in GF(256) is XOR. The per-share scalar `basis` is the
    // Lagrange basis polynomial L_i(x).
    let mut result = vec![0u8; len];
    for s in shares {
        let mut basis: u8 = 1;
        for other in shares {
            if other.x != s.x {
                // numerator (x XOR x_j), denominator (x_i XOR x_j).
                let numerator = x ^ other.x;
                let denominator = s.x ^ other.x;
                basis = gf256_mul(basis, gf256_mul(numerator, gf256_inverse(denominator)));
            }
        }
        for (res_byte, &share_byte) in result.iter_mut().zip(s.data.iter()) {
            *res_byte ^= gf256_mul(share_byte, basis);
        }
    }
    Ok(result)
}

/// Computes the 4-byte SLIP-39 digest: `HMAC-SHA256(key = random_data,
/// msg = shared_secret)` truncated to the first `DIGEST_LENGTH_BYTES` bytes.
fn create_digest(random_data: &[u8], shared_secret: &[u8]) -> [u8; DIGEST_LENGTH_BYTES] {
    let mac = hmac_sha256(random_data, shared_secret);
    let mut out = [0u8; DIGEST_LENGTH_BYTES];
    out.copy_from_slice(&mac[..DIGEST_LENGTH_BYTES]);
    out
}

/// Splits `shared_secret` into `share_count` raw shares with the given
/// `threshold`, following the SLIP-39 construction (digest + secret shares).
fn split_secret(threshold: u8, share_count: u8, shared_secret: &[u8]) -> Result<Vec<RawShare>> {
    if threshold < 1 {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: threshold must be a positive integer".to_string(),
        ));
    }
    if threshold > share_count {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: threshold must not exceed the number of shares".to_string(),
        ));
    }
    if share_count as usize > MAX_SHARE_COUNT {
        return Err(BitcoinError::InvalidInput(format!(
            "SLIP-39: number of shares must not exceed {MAX_SHARE_COUNT}"
        )));
    }
    if shared_secret.len() < DIGEST_LENGTH_BYTES {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: secret is too short to split".to_string(),
        ));
    }

    // Threshold 1: every share is the secret verbatim (no digest involved).
    if threshold == 1 {
        return Ok((0..share_count)
            .map(|i| RawShare {
                x: i,
                data: shared_secret.to_vec(),
            })
            .collect());
    }

    let random_share_count = (threshold - 2) as usize;
    let mut shares: Vec<RawShare> = Vec::with_capacity(share_count as usize);
    for i in 0..random_share_count {
        shares.push(RawShare {
            x: i as u8,
            data: random_bytes(shared_secret.len()),
        });
    }

    // Build the digest share (index 254) and the secret share (index 255).
    let random_part = random_bytes(shared_secret.len() - DIGEST_LENGTH_BYTES);
    let digest = create_digest(&random_part, shared_secret);
    let mut digest_data = Vec::with_capacity(shared_secret.len());
    digest_data.extend_from_slice(&digest);
    digest_data.extend_from_slice(&random_part);

    let mut base_shares = shares.clone();
    base_shares.push(RawShare {
        x: DIGEST_INDEX,
        data: digest_data,
    });
    base_shares.push(RawShare {
        x: SECRET_INDEX,
        data: shared_secret.to_vec(),
    });

    for i in random_share_count..share_count as usize {
        let data = interpolate(&base_shares, i as u8)?;
        shares.push(RawShare { x: i as u8, data });
    }

    Ok(shares)
}

/// Recovers the shared secret from a set of raw shares, verifying the digest.
fn recover_secret(threshold: u8, shares: &[RawShare]) -> Result<Vec<u8>> {
    if shares.is_empty() {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: no shares provided for recovery".to_string(),
        ));
    }

    // Threshold 1: the secret is any single share's data.
    if threshold == 1 {
        return Ok(shares[0].data.clone());
    }

    let shared_secret = interpolate(shares, SECRET_INDEX)?;
    let digest_share = interpolate(shares, DIGEST_INDEX)?;
    if digest_share.len() < DIGEST_LENGTH_BYTES {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: recovered digest share is too short".to_string(),
        ));
    }
    let (digest, random_part) = digest_share.split_at(DIGEST_LENGTH_BYTES);
    if digest != create_digest(random_part, &shared_secret) {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: invalid digest of the shared secret (wrong or corrupted shares)".to_string(),
        ));
    }
    Ok(shared_secret)
}

// --- PBKDF2-HMAC-SHA256, built on bitcoin::hashes (RFC 8018 / RFC 2104).

/// Computes `HMAC-SHA256(key, msg)`.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut engine = HmacEngine::<sha256::Hash>::new(key);
    engine.input(msg);
    Hmac::<sha256::Hash>::from_engine(engine).to_byte_array()
}

/// PBKDF2 with HMAC-SHA256 as the PRF (RFC 8018), producing `dklen` bytes.
fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32, dklen: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(dklen);
    let mut block_index: u32 = 1;
    while output.len() < dklen {
        // U_1 = PRF(password, salt || INT_32_BE(block_index)).
        let mut salt_with_index = Vec::with_capacity(salt.len() + 4);
        salt_with_index.extend_from_slice(salt);
        salt_with_index.extend_from_slice(&block_index.to_be_bytes());
        let mut u = hmac_sha256(password, &salt_with_index);
        let mut t = u;
        // U_c = PRF(password, U_{c-1}); T = U_1 XOR U_2 XOR ... XOR U_iterations.
        for _ in 1..iterations {
            u = hmac_sha256(password, &u);
            for (tb, ub) in t.iter_mut().zip(u.iter()) {
                *tb ^= ub;
            }
        }
        output.extend_from_slice(&t);
        block_index += 1;
    }
    output.truncate(dklen);
    output
}

// --- SLIP-39 passphrase cipher (4-round Feistel network over PBKDF2-HMAC-SHA256).

/// Builds the PBKDF2 salt prefix for the cipher round function.
///
/// For the original (non-extendable) backup the salt is
/// `"shamir" || identifier_be16`; for the extendable backup it is empty.
fn cipher_salt(identifier: u16, extendable: bool) -> Vec<u8> {
    if extendable {
        return Vec::new();
    }
    let id_len = bits_to_bytes(ID_LENGTH_BITS); // 2 bytes
    let mut salt = Vec::with_capacity(CUSTOMIZATION_STRING_ORIG.len() + id_len);
    salt.extend_from_slice(CUSTOMIZATION_STRING_ORIG);
    // identifier is 15 bits, encoded big-endian in `id_len` bytes.
    let id_bytes = (identifier as u32).to_be_bytes();
    salt.extend_from_slice(&id_bytes[4 - id_len..]);
    salt
}

/// The Feistel round function `F(i, R)`.
fn cipher_round_function(
    round: u8,
    passphrase: &[u8],
    iteration_exponent: u8,
    salt: &[u8],
    r: &[u8],
) -> Vec<u8> {
    // password = round_byte || passphrase
    let mut password = Vec::with_capacity(1 + passphrase.len());
    password.push(round);
    password.extend_from_slice(passphrase);

    // salt = salt_prefix || R
    let mut full_salt = Vec::with_capacity(salt.len() + r.len());
    full_salt.extend_from_slice(salt);
    full_salt.extend_from_slice(r);

    // iterations = (BASE_ITERATION_COUNT << e) / ROUND_COUNT.
    let iterations = (BASE_ITERATION_COUNT << iteration_exponent) / ROUND_COUNT as u32;

    pbkdf2_hmac_sha256(&password, &full_salt, iterations, r.len())
}

/// XORs `a` and `b` (which must be equal length) into a new vector.
fn xor_bytes(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
}

/// Encrypts `master_secret` under `passphrase`, producing the encrypted master
/// secret (EMS). The master secret length must be even.
fn cipher_encrypt(
    master_secret: &[u8],
    passphrase: &[u8],
    iteration_exponent: u8,
    identifier: u16,
    extendable: bool,
) -> Result<Vec<u8>> {
    if master_secret.len() % 2 != 0 {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: master secret length in bytes must be even".to_string(),
        ));
    }
    let half = master_secret.len() / 2;
    let mut l = master_secret[..half].to_vec();
    let mut r = master_secret[half..].to_vec();
    let salt = cipher_salt(identifier, extendable);
    for i in 0..ROUND_COUNT {
        let f = cipher_round_function(i, passphrase, iteration_exponent, &salt, &r);
        let new_r = xor_bytes(&l, &f);
        l = r;
        r = new_r;
    }
    // EMS = R || L.
    let mut ems = r;
    ems.extend_from_slice(&l);
    Ok(ems)
}

/// Decrypts an encrypted master secret (EMS) back to the master secret.
fn cipher_decrypt(
    ems: &[u8],
    passphrase: &[u8],
    iteration_exponent: u8,
    identifier: u16,
    extendable: bool,
) -> Result<Vec<u8>> {
    if ems.len() % 2 != 0 {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: encrypted master secret length in bytes must be even".to_string(),
        ));
    }
    let half = ems.len() / 2;
    let mut l = ems[..half].to_vec();
    let mut r = ems[half..].to_vec();
    let salt = cipher_salt(identifier, extendable);
    for i in (0..ROUND_COUNT).rev() {
        let f = cipher_round_function(i, passphrase, iteration_exponent, &salt, &r);
        let new_r = xor_bytes(&l, &f);
        l = r;
        r = new_r;
    }
    let mut secret = r;
    secret.extend_from_slice(&l);
    Ok(secret)
}

// --- RS1024 checksum (Reed–Solomon over GF(1024)).

/// The RS1024 generator constants (one per 10-bit feedback position).
const RS1024_GEN: [u32; 10] = [
    0x00E0_E040,
    0x01C1_C080,
    0x0383_8100,
    0x0707_0200,
    0x0E0E_0009,
    0x1C0C_2412,
    0x3808_6C24,
    0x3090_FC48,
    0x21B1_F890,
    0x03F3_F120,
];

/// Computes the RS1024 polynomial residue over the 10-bit `values`.
fn rs1024_polymod(values: &[u32]) -> u32 {
    let mut chk: u32 = 1;
    for &v in values {
        let b = chk >> 20;
        chk = ((chk & 0xF_FFFF) << 10) ^ v;
        for (i, generator) in RS1024_GEN.iter().enumerate() {
            if (b >> i) & 1 != 0 {
                chk ^= generator;
            }
        }
    }
    chk
}

/// Selects the RS1024 customization string for the given backup variant.
fn customization_string(extendable: bool) -> &'static [u8] {
    if extendable {
        CUSTOMIZATION_STRING_EXTENDABLE
    } else {
        CUSTOMIZATION_STRING_ORIG
    }
}

/// Creates the 3 RS1024 checksum words for `data`.
fn rs1024_create_checksum(data: &[u16], extendable: bool) -> [u16; CHECKSUM_LENGTH_WORDS] {
    let cust = customization_string(extendable);
    let mut values: Vec<u32> = Vec::with_capacity(cust.len() + data.len() + CHECKSUM_LENGTH_WORDS);
    for &c in cust {
        values.push(c as u32);
    }
    for &d in data {
        values.push(d as u32);
    }
    values.extend([0u32; CHECKSUM_LENGTH_WORDS]);
    let polymod = rs1024_polymod(&values) ^ 1;
    let mut checksum = [0u16; CHECKSUM_LENGTH_WORDS];
    for (i, slot) in checksum.iter_mut().enumerate() {
        let shift = 10 * (CHECKSUM_LENGTH_WORDS - 1 - i);
        *slot = ((polymod >> shift) & 1023) as u16;
    }
    checksum
}

/// Verifies the RS1024 checksum embedded at the end of `data`.
fn rs1024_verify_checksum(data: &[u16], extendable: bool) -> bool {
    let cust = customization_string(extendable);
    let mut values: Vec<u32> = Vec::with_capacity(cust.len() + data.len());
    for &c in cust {
        values.push(c as u32);
    }
    for &d in data {
        values.push(d as u32);
    }
    rs1024_polymod(&values) == 1
}

// --- Randomness.

/// Fills a new vector of length `n` with cryptographically-random bytes.
fn random_bytes(n: usize) -> Vec<u8> {
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut buf = vec![0u8; n];
    for byte in buf.iter_mut() {
        *byte = rng.random_range(0..=255u8);
    }
    buf
}

/// Generates a random 15-bit share identifier.
fn random_identifier() -> u16 {
    use rand::RngExt;
    let mut rng = rand::rng();
    rng.random_range(0..(1u16 << ID_LENGTH_BITS))
}

// --- Word <-> 10-bit index layer (full official 1024-word wordlist embedded).

/// Returns the 0-based index of `word` in the SLIP-39 wordlist, or an error.
fn word_to_index(word: &str) -> Result<u16> {
    // The list is sorted, so a binary search is correct and fast.
    match slip39_wordlist().binary_search(&word) {
        Ok(idx) => Ok(idx as u16),
        Err(_) => Err(BitcoinError::InvalidInput(format!(
            "SLIP-39: unknown mnemonic word '{word}'"
        ))),
    }
}

/// Returns the word at 10-bit `index`, or an error if it is out of range.
fn index_to_word(index: u16) -> Result<&'static str> {
    slip39_wordlist()
        .get(index as usize)
        .copied()
        .ok_or_else(|| {
            BitcoinError::InvalidInput(format!("SLIP-39: word index {index} out of range"))
        })
}

// --- Public configuration types.

/// Share threshold configuration (M-of-N).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareThreshold {
    /// Minimum shares required to recover (M).
    pub threshold: u8,
    /// Total number of shares (N).
    pub total_shares: u8,
}

impl ShareThreshold {
    /// Creates a new share threshold, validating the SLIP-39 constraints.
    ///
    /// A threshold of `1` with more than one share is rejected, matching the
    /// spec rule that forbids ambiguous 1-of-N member sharing (use 1-of-1).
    pub fn new(threshold: u8, total_shares: u8) -> Result<Self> {
        if threshold < 1 {
            return Err(BitcoinError::InvalidInput(
                "Threshold must be at least 1".to_string(),
            ));
        }
        if threshold > total_shares {
            return Err(BitcoinError::InvalidInput(
                "Threshold cannot exceed total shares".to_string(),
            ));
        }
        if total_shares as usize > MAX_SHARE_COUNT {
            return Err(BitcoinError::InvalidInput(format!(
                "Total shares cannot exceed {MAX_SHARE_COUNT}"
            )));
        }
        if threshold == 1 && total_shares > 1 {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39 forbids 1-of-N member sharing with N > 1; use 1-of-1".to_string(),
            ));
        }
        Ok(Self {
            threshold,
            total_shares,
        })
    }

    /// Returns whether this threshold satisfies the SLIP-39 constraints.
    pub fn is_valid(&self) -> bool {
        self.threshold >= 1
            && self.threshold <= self.total_shares
            && self.total_shares as usize <= MAX_SHARE_COUNT
            && !(self.threshold == 1 && self.total_shares > 1)
    }
}

/// Group share configuration for multi-level secret sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupConfig {
    /// Group identifier (0-15).
    pub group_id: u8,
    /// Member threshold for this group (shares needed from this group).
    pub group_threshold: ShareThreshold,
    /// Optional human-readable group description.
    pub description: Option<String>,
}

impl GroupConfig {
    /// Creates a new group configuration with the given ID and member threshold.
    pub fn new(group_id: u8, threshold: ShareThreshold) -> Result<Self> {
        if group_id as usize >= MAX_SHARE_COUNT {
            return Err(BitcoinError::InvalidInput(
                "Group ID must be 0-15".to_string(),
            ));
        }
        Ok(Self {
            group_id,
            group_threshold: threshold,
            description: None,
        })
    }

    /// Sets a human-readable description for this group.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// --- Slip39Share: a single SLIP-39 share with real encoding/decoding.

/// A single SLIP-0039 share.
///
/// The `share_value` field holds the *raw* Shamir share bytes (the encrypted
/// master secret material for this share), exactly as defined by the spec; it
/// is not further obfuscated. The metadata fields use the spec's zero-based
/// indices and 1-based thresholds (the on-the-wire encoding stores
/// `threshold - 1` / `count - 1`, handled transparently in [`Slip39Share::to_indices`]
/// and [`Slip39Share::from_indices`]).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slip39Share {
    /// Random 15-bit share identifier (shared by all shares of one secret).
    pub identifier: u16,
    /// Whether the extendable-backup variant is in use.
    pub extendable: bool,
    /// Iteration exponent for the PBKDF2-based cipher (`0..=15`).
    pub iteration_exponent: u8,
    /// Group index (0-based).
    pub group_index: u8,
    /// Group threshold `Gt` (1-based; how many groups are required).
    pub group_threshold: u8,
    /// Total number of groups `g` (1-based).
    pub group_count: u8,
    /// Member index within the group (0-based).
    pub member_index: u8,
    /// Member threshold `t` (1-based; how many members of this group required).
    pub member_threshold: u8,
    /// Raw Shamir share value bytes.
    pub share_value: Vec<u8>,
}

impl Slip39Share {
    /// Encodes this share into its sequence of 10-bit word indices, including
    /// the appended RS1024 checksum.
    ///
    /// Returns an error if any field exceeds its allotted bit width or if the
    /// share value is empty.
    pub fn to_indices(&self) -> Result<Vec<u16>> {
        if self.share_value.is_empty() {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: share value must not be empty".to_string(),
            ));
        }
        self.validate_fields()?;

        // id || ext || e  -> ID_EXP_LENGTH_WORDS words.
        let id_exp: u32 = ((self.identifier as u32)
            << (ITERATION_EXP_LENGTH_BITS + EXTENDABLE_FLAG_LENGTH_BITS))
            | ((self.extendable as u32) << ITERATION_EXP_LENGTH_BITS)
            | (self.iteration_exponent as u32);
        let mut indices = int_to_indices(id_exp as u128, ID_EXP_LENGTH_WORDS);

        // group/member params -> 2 words (each field is 4 bits).
        let mut params: u32 = self.group_index as u32;
        params = (params << 4) | (self.group_threshold as u32 - 1);
        params = (params << 4) | (self.group_count as u32 - 1);
        params = (params << 4) | (self.member_index as u32);
        params = (params << 4) | (self.member_threshold as u32 - 1);
        indices.extend(int_to_indices(params as u128, 2));

        // Padded share value -> ceil(value_bits / 10) words. The value is
        // left-padded with zero bits to a multiple of 10 (handled by the
        // bit-stream packer), then split into 10-bit words.
        indices.extend(bytes_to_indices(&self.share_value));

        // RS1024 checksum over everything so far.
        let checksum = rs1024_create_checksum(&indices, self.extendable);
        indices.extend_from_slice(&checksum);

        Ok(indices)
    }

    /// Decodes a share from its sequence of 10-bit word indices, verifying the
    /// RS1024 checksum and padding.
    pub fn from_indices(indices: &[u16]) -> Result<Self> {
        if indices.len() < MIN_MNEMONIC_LENGTH_WORDS {
            return Err(BitcoinError::InvalidInput(format!(
                "SLIP-39: mnemonic too short ({} words, minimum {MIN_MNEMONIC_LENGTH_WORDS})",
                indices.len()
            )));
        }
        for &idx in indices {
            if idx as usize >= WORDLIST_SIZE {
                return Err(BitcoinError::InvalidInput(
                    "SLIP-39: word index out of range".to_string(),
                ));
            }
        }

        // Determine the extendable flag from the id/exp prefix so the correct
        // RS1024 customization string is used for verification.
        let id_exp = indices_to_uint(&indices[..ID_EXP_LENGTH_WORDS]) as u32;
        let extendable = ((id_exp >> ITERATION_EXP_LENGTH_BITS) & 1) != 0;

        if !rs1024_verify_checksum(indices, extendable) {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: invalid mnemonic checksum".to_string(),
            ));
        }

        let identifier =
            (id_exp >> (ITERATION_EXP_LENGTH_BITS + EXTENDABLE_FLAG_LENGTH_BITS)) as u16;
        let iteration_exponent = (id_exp & ((1 << ITERATION_EXP_LENGTH_BITS) - 1)) as u8;

        // group/member params (5 four-bit fields packed in 2 words).
        let params_start = ID_EXP_LENGTH_WORDS;
        let params = indices_to_uint(&indices[params_start..params_start + 2]) as u32;
        let member_threshold = ((params & 0xF) + 1) as u8;
        let member_index = ((params >> 4) & 0xF) as u8;
        let group_count = (((params >> 8) & 0xF) + 1) as u8;
        let group_threshold = (((params >> 12) & 0xF) + 1) as u8;
        let group_index = ((params >> 16) & 0xF) as u8;

        if group_count < group_threshold {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: group count must not be less than group threshold".to_string(),
            ));
        }

        // Share value words sit between the params and the checksum.
        let value_words = &indices[params_start + 2..indices.len() - CHECKSUM_LENGTH_WORDS];
        if value_words.is_empty() {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: missing share value".to_string(),
            ));
        }

        // Padding length: (10 * value_word_count) mod 16, must be <= 8, and the
        // padded high bits must be zero.
        let padding_len = (RADIX_BITS * value_words.len()) % 16;
        if padding_len > 8 {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: invalid mnemonic length (padding too large)".to_string(),
            ));
        }
        let value_byte_count = bits_to_bytes(RADIX_BITS * value_words.len() - padding_len);
        let share_value = indices_to_bytes(value_words, padding_len, value_byte_count)?;

        let share = Self {
            identifier,
            extendable,
            iteration_exponent,
            group_index,
            group_threshold,
            group_count,
            member_index,
            member_threshold,
            share_value,
        };
        share.validate_fields()?;
        Ok(share)
    }

    /// Serializes this share to a SLIP-39 mnemonic (space-separated words).
    pub fn to_mnemonic(&self) -> Result<String> {
        let indices = self.to_indices()?;
        let mut words = Vec::with_capacity(indices.len());
        for idx in indices {
            words.push(index_to_word(idx)?);
        }
        Ok(words.join(" "))
    }

    /// Parses a SLIP-39 mnemonic string into a share, validating the checksum.
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self> {
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        if words.is_empty() {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: empty mnemonic".to_string(),
            ));
        }
        let mut indices = Vec::with_capacity(words.len());
        for w in words {
            indices.push(word_to_index(w)?);
        }
        Self::from_indices(&indices)
    }

    /// Returns whether this share's mnemonic checksum is valid.
    pub fn validate_checksum(&self) -> bool {
        match self.to_indices() {
            Ok(indices) => rs1024_verify_checksum(&indices, self.extendable),
            Err(_) => false,
        }
    }

    /// The five parameters that must be identical across every share of one
    /// secret (identifier, extendable, iteration exponent, group threshold,
    /// group count).
    fn common_parameters(&self) -> (u16, bool, u8, u8, u8) {
        (
            self.identifier,
            self.extendable,
            self.iteration_exponent,
            self.group_threshold,
            self.group_count,
        )
    }

    /// Validates that every field fits within its on-the-wire bit width.
    fn validate_fields(&self) -> Result<()> {
        if self.identifier >= (1 << ID_LENGTH_BITS) {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: identifier exceeds 15 bits".to_string(),
            ));
        }
        if self.iteration_exponent >= (1 << ITERATION_EXP_LENGTH_BITS) {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: iteration exponent exceeds 4 bits".to_string(),
            ));
        }
        for (name, value) in [
            ("group index", self.group_index),
            ("member index", self.member_index),
        ] {
            if value as usize >= MAX_SHARE_COUNT {
                return Err(BitcoinError::InvalidInput(format!(
                    "SLIP-39: {name} exceeds 4 bits"
                )));
            }
        }
        for (name, value) in [
            ("group threshold", self.group_threshold),
            ("group count", self.group_count),
            ("member threshold", self.member_threshold),
        ] {
            if value < 1 || value as usize > MAX_SHARE_COUNT {
                return Err(BitcoinError::InvalidInput(format!(
                    "SLIP-39: {name} must be in 1..=16"
                )));
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Slip39Share {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `share_value` is secret material and is intentionally redacted.
        let thresholds = format!(
            "{}-of-(group {}/{})",
            self.member_threshold, self.group_threshold, self.group_count
        );
        f.debug_struct("Slip39Share")
            .field("identifier", &self.identifier)
            .field("extendable", &self.extendable)
            .field("group_index", &self.group_index)
            .field("member_index", &self.member_index)
            .field("thresholds", &thresholds)
            .field("share_value", &"<redacted>")
            .finish()
    }
}

// --- Big-integer <-> 10-bit index / byte conversions.

/// Converts an unsigned integer to `length` 10-bit indices, big-endian.
fn int_to_indices(value: u128, length: usize) -> Vec<u16> {
    let mask: u128 = (1 << RADIX_BITS) - 1;
    (0..length)
        .rev()
        .map(|i| ((value >> (i * RADIX_BITS)) & mask) as u16)
        .collect()
}

/// Converts a slice of 10-bit indices (big-endian) to an unsigned integer.
fn indices_to_uint(indices: &[u16]) -> u128 {
    let mut value: u128 = 0;
    for &idx in indices {
        value = (value << RADIX_BITS) | (idx as u128);
    }
    value
}

/// Packs an arbitrary-length big-endian byte slice into 10-bit indices,
/// left-padding the most-significant end with zero bits so the total bit count
/// is a multiple of `RADIX_BITS`. Works for any length (no integer-width
/// limit), which is required for 256-bit secrets.
///
/// The byte stream is treated as one big-endian bit string; `pad_bits` zero
/// bits are conceptually prepended, then the result is chunked into 10-bit
/// words (most-significant word first).
fn bytes_to_indices(bytes: &[u8]) -> Vec<u16> {
    let total_bits = bytes.len() * 8;
    let word_count = bits_to_words(total_bits);
    let pad_bits = word_count * RADIX_BITS - total_bits;

    // `acc` holds pending bits; the high `acc_bits` bits are the leftmost
    // unread bits. We seed it with `pad_bits` zero bits.
    let mut indices = Vec::with_capacity(word_count);
    let mut acc: u64 = 0;
    let mut acc_bits: usize = pad_bits;
    for &b in bytes {
        acc = (acc << 8) | b as u64;
        acc_bits += 8;
        while acc_bits >= RADIX_BITS {
            acc_bits -= RADIX_BITS;
            indices.push(((acc >> acc_bits) & 1023) as u16);
        }
    }
    debug_assert_eq!(acc_bits, 0, "byte stream should pack exactly into words");
    indices
}

/// Unpacks 10-bit indices into `byte_count` big-endian bytes, dropping the
/// `pad_bits` (< 8) leading zero padding bits. Fails if any padding bit is
/// non-zero or the bit counts are inconsistent.
fn indices_to_bytes(indices: &[u16], pad_bits: usize, byte_count: usize) -> Result<Vec<u8>> {
    // Verify the leading `pad_bits` bits of the first word are zero.
    if pad_bits > 0 {
        if let Some(&first) = indices.first() {
            let high = first >> (RADIX_BITS - pad_bits);
            if high != 0 {
                return Err(BitcoinError::InvalidInput(
                    "SLIP-39: invalid mnemonic padding (non-zero padding bits)".to_string(),
                ));
            }
        }
    }

    // Consume all index bits, skipping the first `pad_bits`, emitting bytes.
    let mut out = Vec::with_capacity(byte_count);
    let mut acc: u64 = 0;
    let mut acc_bits: usize = 0;
    let mut skip = pad_bits;
    for &idx in indices {
        acc = (acc << RADIX_BITS) | idx as u64;
        acc_bits += RADIX_BITS;
        // Drop leading padding bits first.
        if skip > 0 {
            let drop = skip.min(acc_bits);
            acc_bits -= drop;
            // Clear the just-dropped high bits from `acc` so they cannot leak
            // into later reads (mask to the remaining low `acc_bits`).
            if acc_bits < 64 {
                acc &= (1u64 << acc_bits) - 1;
            }
            skip -= drop;
        }
        while acc_bits >= 8 && out.len() < byte_count {
            acc_bits -= 8;
            out.push(((acc >> acc_bits) & 0xFF) as u8);
        }
    }
    if out.len() != byte_count {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: failed to decode share value bytes".to_string(),
        ));
    }
    Ok(out)
}

// --- Official SLIP-0039 wordlist (1024 words).

/// Returns the official SLIP-0039 wordlist (1024 words), sorted lexicographically.
///
/// Each word's 0-based position in this list is its 10-bit index used in the
/// mnemonic encoding. The list is embedded from `slip39_wordlist.txt`, sourced
/// from trezor/python-shamir-mnemonic (SHA-256
/// `bcc4555340332d169718aed8bf31dd9d5248cb7da6e5d355140ef4f1e601eec3`). All
/// words are unique in their first four letters.
fn slip39_wordlist() -> &'static [&'static str] {
    static WORDLIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    WORDLIST.get_or_init(|| {
        const RAW: &str = include_str!("slip39_wordlist.txt");
        let words: Vec<&'static str> = RAW
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .collect();
        debug_assert_eq!(
            words.len(),
            WORDLIST_SIZE,
            "SLIP-39 wordlist must contain 1024 words"
        );
        words
    })
}

// --- Encrypted master secret helper.

/// An encrypted master secret together with the parameters needed to decrypt.
#[derive(Clone)]
struct EncryptedMasterSecret {
    identifier: u16,
    extendable: bool,
    iteration_exponent: u8,
    ciphertext: Vec<u8>,
}

impl EncryptedMasterSecret {
    /// Encrypts a master secret under `passphrase`.
    fn from_master_secret(
        master_secret: &[u8],
        passphrase: &[u8],
        identifier: u16,
        extendable: bool,
        iteration_exponent: u8,
    ) -> Result<Self> {
        let ciphertext = cipher_encrypt(
            master_secret,
            passphrase,
            iteration_exponent,
            identifier,
            extendable,
        )?;
        Ok(Self {
            identifier,
            extendable,
            iteration_exponent,
            ciphertext,
        })
    }

    /// Decrypts back to the master secret.
    fn decrypt(&self, passphrase: &[u8]) -> Result<Vec<u8>> {
        cipher_decrypt(
            &self.ciphertext,
            passphrase,
            self.iteration_exponent,
            self.identifier,
            self.extendable,
        )
    }
}

/// Validates that a passphrase contains only printable ASCII (code points
/// 32-126), as required by SLIP-39.
fn validate_passphrase(passphrase: &[u8]) -> Result<()> {
    if passphrase.iter().all(|&c| (32..=126).contains(&c)) {
        Ok(())
    } else {
        Err(BitcoinError::InvalidInput(
            "SLIP-39: passphrase must contain only printable ASCII (32-126)".to_string(),
        ))
    }
}

/// Validates a master-secret length (even and at least 128 bits).
fn validate_master_secret(secret: &[u8]) -> Result<()> {
    if secret.len() * 8 < MIN_STRENGTH_BITS {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: master secret must be at least 128 bits".to_string(),
        ));
    }
    if secret.len() % 2 != 0 {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: master secret length in bytes must be even".to_string(),
        ));
    }
    Ok(())
}

// --- Group descriptor (member threshold + member count).

/// A `(member_threshold, member_count)` group descriptor.
#[derive(Debug, Clone, Copy)]
pub struct GroupSpec {
    /// Number of member shares required to reconstruct this group's secret.
    pub member_threshold: u8,
    /// Number of member shares produced for this group.
    pub member_count: u8,
}

impl GroupSpec {
    /// Creates a new group spec, validating the SLIP-39 member constraints.
    pub fn new(member_threshold: u8, member_count: u8) -> Result<Self> {
        let _ = ShareThreshold::new(member_threshold, member_count)?;
        Ok(Self {
            member_threshold,
            member_count,
        })
    }
}

/// Core two-level split: encrypts the master secret and produces, for each
/// group, the list of member shares.
///
/// `groups[i]` describes group `i`; `group_threshold` is how many groups are
/// required. Returns one inner `Vec<Slip39Share>` per group.
fn generate_mnemonics_shares(
    group_threshold: u8,
    groups: &[GroupSpec],
    master_secret: &[u8],
    passphrase: &[u8],
    extendable: bool,
    iteration_exponent: u8,
) -> Result<Vec<Vec<Slip39Share>>> {
    validate_master_secret(master_secret)?;
    validate_passphrase(passphrase)?;
    if groups.is_empty() {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: at least one group is required".to_string(),
        ));
    }
    if group_threshold < 1 {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: group threshold must be at least 1".to_string(),
        ));
    }
    if group_threshold as usize > groups.len() {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: group threshold must not exceed the number of groups".to_string(),
        ));
    }
    if groups.len() > MAX_SHARE_COUNT {
        return Err(BitcoinError::InvalidInput(format!(
            "SLIP-39: number of groups must not exceed {MAX_SHARE_COUNT}"
        )));
    }
    // Reject ambiguous 1-of-N member sharing.
    for g in groups {
        let _ = ShareThreshold::new(g.member_threshold, g.member_count)?;
    }

    let identifier = random_identifier();
    let ems = EncryptedMasterSecret::from_master_secret(
        master_secret,
        passphrase,
        identifier,
        extendable,
        iteration_exponent,
    )?;

    // Split the EMS ciphertext among the groups.
    let group_count = groups.len() as u8;
    let group_shares = split_secret(group_threshold, group_count, &ems.ciphertext)?;

    let mut all_groups: Vec<Vec<Slip39Share>> = Vec::with_capacity(groups.len());
    for (group_index, (spec, group_share)) in groups.iter().zip(group_shares.iter()).enumerate() {
        let member_shares =
            split_secret(spec.member_threshold, spec.member_count, &group_share.data)?;
        let mut group_vec = Vec::with_capacity(member_shares.len());
        for member in member_shares {
            group_vec.push(Slip39Share {
                identifier,
                extendable,
                iteration_exponent,
                group_index: group_index as u8,
                group_threshold,
                group_count,
                member_index: member.x,
                member_threshold: spec.member_threshold,
                share_value: member.data,
            });
        }
        all_groups.push(group_vec);
    }

    Ok(all_groups)
}

/// Recovers the encrypted master secret from a flat list of shares, enforcing
/// all SLIP-39 validation rules.
fn recover_ems(shares: &[Slip39Share]) -> Result<EncryptedMasterSecret> {
    if shares.is_empty() {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: the set of shares is empty".to_string(),
        ));
    }

    // All shares must agree on the common parameters.
    let common = shares[0].common_parameters();
    for s in shares {
        if s.common_parameters() != common {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: shares belong to different secrets (mismatched common parameters)"
                    .to_string(),
            ));
        }
    }
    let (identifier, extendable, iteration_exponent, group_threshold, _group_count) = common;

    // Bucket shares by group index.
    let mut groups: HashMap<u8, Vec<&Slip39Share>> = HashMap::new();
    for s in shares {
        groups.entry(s.group_index).or_default().push(s);
    }

    if groups.len() < group_threshold as usize {
        return Err(BitcoinError::InvalidInput(format!(
            "SLIP-39: insufficient groups: need {}, got {}",
            group_threshold,
            groups.len()
        )));
    }
    if groups.len() != group_threshold as usize {
        return Err(BitcoinError::InvalidInput(format!(
            "SLIP-39: wrong number of groups: expected exactly {}, got {}",
            group_threshold,
            groups.len()
        )));
    }

    // Recover each group's secret.
    let mut group_shares: Vec<RawShare> = Vec::with_capacity(groups.len());
    for (group_index, members) in groups {
        // Within a group, the member threshold and remaining group parameters
        // must match.
        let member_threshold = members[0].member_threshold;
        for m in &members {
            if m.member_threshold != member_threshold || m.group_index != group_index {
                return Err(BitcoinError::InvalidInput(
                    "SLIP-39: inconsistent share parameters within a group".to_string(),
                ));
            }
        }
        if members.len() != member_threshold as usize {
            return Err(BitcoinError::InvalidInput(format!(
                "SLIP-39: wrong number of member shares in group {group_index}: expected {member_threshold}, got {}",
                members.len()
            )));
        }
        let raw: Vec<RawShare> = members
            .iter()
            .map(|m| RawShare {
                x: m.member_index,
                data: m.share_value.clone(),
            })
            .collect();
        let group_secret = recover_secret(member_threshold, &raw)?;
        group_shares.push(RawShare {
            x: group_index,
            data: group_secret,
        });
    }

    let ciphertext = recover_secret(group_threshold, &group_shares)?;
    Ok(EncryptedMasterSecret {
        identifier,
        extendable,
        iteration_exponent,
        ciphertext,
    })
}

/// Generates SLIP-39 mnemonics for a two-level group structure.
///
/// Returns, for each group, the list of member mnemonic strings.
pub fn generate_mnemonics(
    group_threshold: u8,
    groups: &[GroupSpec],
    master_secret: &[u8],
    passphrase: &[u8],
    extendable: bool,
    iteration_exponent: u8,
) -> Result<Vec<Vec<String>>> {
    let shares = generate_mnemonics_shares(
        group_threshold,
        groups,
        master_secret,
        passphrase,
        extendable,
        iteration_exponent,
    )?;
    shares
        .into_iter()
        .map(|group| group.iter().map(Slip39Share::to_mnemonic).collect())
        .collect()
}

/// Recovers the master secret from a list of SLIP-39 mnemonic strings.
pub fn combine_mnemonics(mnemonics: &[String], passphrase: &[u8]) -> Result<Vec<u8>> {
    if mnemonics.is_empty() {
        return Err(BitcoinError::InvalidInput(
            "SLIP-39: the list of mnemonics is empty".to_string(),
        ));
    }
    let shares: Vec<Slip39Share> = mnemonics
        .iter()
        .map(|m| Slip39Share::from_mnemonic(m))
        .collect::<Result<Vec<_>>>()?;
    let ems = recover_ems(&shares)?;
    ems.decrypt(passphrase)
}

// --- Slip39Generator: single-group convenience API.

/// SLIP-39 share generator for a single group (M-of-N) backup.
pub struct Slip39Generator {
    identifier: u16,
    extendable: bool,
    iteration_exponent: u8,
}

impl Slip39Generator {
    /// Creates a new generator with a fresh random identifier.
    pub fn new() -> Self {
        Self {
            identifier: random_identifier(),
            extendable: false,
            iteration_exponent: DEFAULT_ITERATION_EXPONENT,
        }
    }

    /// Sets the iteration exponent (0-15), controlling PBKDF2 cost.
    pub fn with_iteration_exponent(mut self, exponent: u8) -> Result<Self> {
        if exponent >= (1 << ITERATION_EXP_LENGTH_BITS) {
            return Err(BitcoinError::InvalidInput(
                "SLIP-39: iteration exponent must be 0-15".to_string(),
            ));
        }
        self.iteration_exponent = exponent;
        Ok(self)
    }

    /// Selects the extendable-backup variant (empty PBKDF2 salt).
    pub fn extendable(mut self, extendable: bool) -> Self {
        self.extendable = extendable;
        self
    }

    /// Returns this generator's share identifier.
    pub fn identifier(&self) -> u16 {
        self.identifier
    }

    /// Generates `total_shares` shares of which `threshold` reconstruct the
    /// secret, as a single SLIP-39 group (`group_threshold = 1`).
    ///
    /// The optional passphrase is applied via the SLIP-39 Feistel cipher.
    pub fn generate_shares(
        &self,
        secret: &[u8],
        threshold: ShareThreshold,
        passphrase: Option<&str>,
    ) -> Result<Vec<Slip39Share>> {
        if secret.len() != 16 && secret.len() != 32 {
            return Err(BitcoinError::InvalidInput(
                "Secret must be 128 or 256 bits (16 or 32 bytes)".to_string(),
            ));
        }
        if !threshold.is_valid() {
            return Err(BitcoinError::InvalidInput("Invalid threshold".to_string()));
        }

        let passphrase_bytes = passphrase.map(str::as_bytes).unwrap_or(b"");
        validate_passphrase(passphrase_bytes)?;

        let ems = EncryptedMasterSecret::from_master_secret(
            secret,
            passphrase_bytes,
            self.identifier,
            self.extendable,
            self.iteration_exponent,
        )?;

        // Single group: split the EMS among members directly.
        let member_shares =
            split_secret(threshold.threshold, threshold.total_shares, &ems.ciphertext)?;

        let mut shares = Vec::with_capacity(member_shares.len());
        for member in member_shares {
            shares.push(Slip39Share {
                identifier: self.identifier,
                extendable: self.extendable,
                iteration_exponent: self.iteration_exponent,
                group_index: 0,
                group_threshold: 1,
                group_count: 1,
                member_index: member.x,
                member_threshold: threshold.threshold,
                share_value: member.data,
            });
        }
        Ok(shares)
    }

    /// Recovers a secret from shares produced without a passphrase.
    ///
    /// For passphrase-protected shares, use [`Slip39Generator::recover_secret_with_passphrase`].
    pub fn recover_secret(shares: &[Slip39Share]) -> Result<Vec<u8>> {
        Self::recover_secret_with_passphrase(shares, None)
    }

    /// Recovers a secret from shares, applying the given passphrase.
    pub fn recover_secret_with_passphrase(
        shares: &[Slip39Share],
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>> {
        if shares.is_empty() {
            return Err(BitcoinError::InvalidInput("No shares provided".to_string()));
        }
        let passphrase_bytes = passphrase.map(str::as_bytes).unwrap_or(b"");
        let ems = recover_ems(shares)?;
        ems.decrypt(passphrase_bytes)
    }
}

impl Default for Slip39Generator {
    fn default() -> Self {
        Self::new()
    }
}

// --- MultiGroupGenerator: two-level group/member API.

/// Multi-group SLIP-39 generator for two-level secret sharing.
pub struct MultiGroupGenerator {
    groups: Vec<GroupConfig>,
    group_threshold: u8,
    extendable: bool,
    iteration_exponent: u8,
}

impl MultiGroupGenerator {
    /// Creates a new multi-group generator.
    pub fn new(groups: Vec<GroupConfig>, group_threshold: u8) -> Result<Self> {
        if groups.is_empty() {
            return Err(BitcoinError::InvalidInput("No groups provided".to_string()));
        }
        if group_threshold as usize > groups.len() {
            return Err(BitcoinError::InvalidInput(
                "Group threshold exceeds number of groups".to_string(),
            ));
        }
        if group_threshold == 0 {
            return Err(BitcoinError::InvalidInput(
                "Group threshold must be at least 1".to_string(),
            ));
        }
        Ok(Self {
            groups,
            group_threshold,
            extendable: false,
            iteration_exponent: DEFAULT_ITERATION_EXPONENT,
        })
    }

    /// Generates all member shares for every group, keyed by `group_id`.
    pub fn generate_all_shares(
        &self,
        secret: &[u8],
        passphrase: Option<&str>,
    ) -> Result<HashMap<u8, Vec<Slip39Share>>> {
        let passphrase_bytes = passphrase.map(str::as_bytes).unwrap_or(b"");
        let specs: Vec<GroupSpec> = self
            .groups
            .iter()
            .map(|g| GroupSpec {
                member_threshold: g.group_threshold.threshold,
                member_count: g.group_threshold.total_shares,
            })
            .collect();

        let grouped = generate_mnemonics_shares(
            self.group_threshold,
            &specs,
            secret,
            passphrase_bytes,
            self.extendable,
            self.iteration_exponent,
        )?;

        let mut out = HashMap::new();
        for (group_config, group_shares) in self.groups.iter().zip(grouped) {
            out.insert(group_config.group_id, group_shares);
        }
        Ok(out)
    }

    /// Recovers the secret from a flat list of shares spanning the required
    /// number of groups (no passphrase).
    pub fn recover_from_groups(shares: &[Slip39Share]) -> Result<Vec<u8>> {
        Self::recover_from_groups_with_passphrase(shares, None)
    }

    /// Recovers the secret from a flat list of shares, applying `passphrase`.
    pub fn recover_from_groups_with_passphrase(
        shares: &[Slip39Share],
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>> {
        if shares.is_empty() {
            return Err(BitcoinError::InvalidInput("No shares provided".to_string()));
        }
        let passphrase_bytes = passphrase.map(str::as_bytes).unwrap_or(b"");
        let ems = recover_ems(shares)?;
        ems.decrypt(passphrase_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes a lowercase hex string into bytes (test helper).
    fn from_hex(s: &str) -> Vec<u8> {
        hex::decode(s).expect("valid hex")
    }

    /// Encodes bytes as a lowercase hex string (test helper).
    fn to_hex(bytes: &[u8]) -> String {
        hex::encode(bytes)
    }

    // --- GF(256) field correctness.

    #[test]
    fn test_gf256_field() {
        let t = gf256_tables();
        assert_eq!(t.exp[0], 1, "generator^0 must be 1");
        // exp/log are inverses; a * a^-1 == 1 for all non-zero a.
        for a in 1u16..=255 {
            let a = a as u8;
            assert_eq!(
                t.exp[t.log[a as usize] as usize], a,
                "exp(log(a)) != a for {a}"
            );
            assert_eq!(gf256_mul(a, gf256_inverse(a)), 1, "a * a^-1 != 1 for {a}");
        }
        // Known Rijndael products.
        assert_eq!(gf256_mul(0x53, 0xca), 0x01); // inverses
        assert_eq!(gf256_mul(0x57, 0x83), 0xc1); // classic AES example
        assert_eq!(gf256_mul(0, 0x42), 0);
        assert_eq!(gf256_mul(0x42, 1), 0x42);
        assert_eq!(gf256_inverse(0), 0);
    }

    // --- Raw Shamir splitting/recovery (with digest verification).

    #[test]
    fn test_raw_shamir_split_recover_and_failures() {
        // Round-trip across thresholds (any `threshold` shares recover).
        let secret = b"an example secret of even length".to_vec(); // 32 bytes
        for (threshold, count) in [(1u8, 1u8), (2, 3), (3, 5), (5, 5), (4, 7)] {
            let shares = split_secret(threshold, count, &secret).expect("split");
            assert_eq!(shares.len(), count as usize);
            let subset: Vec<RawShare> = shares[..threshold as usize].to_vec();
            let recovered = recover_secret(threshold, &subset).expect("recover");
            assert_eq!(recovered, secret, "{threshold}-of-{count} failed");
        }
        // Tampered shares must fail the HMAC digest check.
        let s16 = vec![7u8; 16];
        let mut bad = split_secret(3, 5, &s16).expect("split")[..3].to_vec();
        bad[0].data[0] ^= 0xff;
        assert!(
            recover_secret(3, &bad).is_err(),
            "tampered shares must fail digest"
        );
        // Duplicate x coordinates are rejected by interpolation.
        let dup = vec![
            RawShare {
                x: 1,
                data: vec![1, 2],
            },
            RawShare {
                x: 1,
                data: vec![3, 4],
            },
        ];
        assert!(interpolate(&dup, 0).is_err());
    }

    // --- RS1024 checksum.

    #[test]
    fn test_rs1024_roundtrip_and_rejection() {
        let data: Vec<u16> = (0..20).map(|i| (i * 7 % 1024) as u16).collect();
        for extendable in [false, true] {
            let cs = rs1024_create_checksum(&data, extendable);
            let mut full = data.clone();
            full.extend_from_slice(&cs);
            assert!(rs1024_verify_checksum(&full, extendable));
            // Corrupt one word: must fail.
            let mut corrupt = full.clone();
            corrupt[0] ^= 1;
            assert!(!rs1024_verify_checksum(&corrupt, extendable));
            // Wrong customization string: must fail.
            assert!(!rs1024_verify_checksum(&full, !extendable));
        }
    }

    // --- PBKDF2-HMAC-SHA256 against RFC 6070-style known answers.

    #[test]
    fn test_hmac_and_pbkdf2_known_answers() {
        // RFC 4231 HMAC-SHA256 test case 1: key=0x0b*20, data="Hi There".
        let mac = hmac_sha256(&[0x0bu8; 20], b"Hi There");
        assert_eq!(
            mac.to_vec(),
            from_hex("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
        );
        // RFC 7914 PBKDF2-HMAC-SHA256: P="passwd", S="salt", c=1, dkLen=64.
        assert_eq!(
            pbkdf2_hmac_sha256(b"passwd", b"salt", 1, 64),
            from_hex(
                "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783"
            )
        );
        // RFC 7914 PBKDF2-HMAC-SHA256: P="Password", S="NaCl", c=80000, dkLen=64.
        assert_eq!(
            pbkdf2_hmac_sha256(b"Password", b"NaCl", 80_000, 64),
            from_hex(
                "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56a1d425a1225833549adb841b51c9b3176a272bdebba1d078478f62b397f33c8d"
            )
        );
    }

    // --- Feistel cipher round-trip.

    #[test]
    fn test_cipher_roundtrip_and_passphrase_sensitivity() {
        let secret = from_hex("bb54aac4b89dc868ba37d9cc21b2cece"); // 16 bytes
        for extendable in [false, true] {
            for e in [0u8, 1, 2] {
                let ems = cipher_encrypt(&secret, b"TREZOR", e, 0x1234, extendable).unwrap();
                assert_ne!(ems, secret, "ciphertext must differ from plaintext");
                let back = cipher_decrypt(&ems, b"TREZOR", e, 0x1234, extendable).unwrap();
                assert_eq!(back, secret);
            }
        }
        // Different passphrases yield different ciphertexts.
        let a = cipher_encrypt(&secret, b"alpha", 0, 1, false).unwrap();
        let b = cipher_encrypt(&secret, b"beta", 0, 1, false).unwrap();
        assert_ne!(a, b);
    }

    // --- Bit-packing round-trip (including 256-bit values that exceed u128).

    #[test]
    fn test_bit_packing_roundtrip() {
        for len in [16usize, 32, 20, 18] {
            let bytes: Vec<u8> = (0..len)
                .map(|i| (i as u8).wrapping_mul(37).wrapping_add(11))
                .collect();
            let indices = bytes_to_indices(&bytes);
            let total_bits = len * 8;
            let word_count = bits_to_words(total_bits);
            assert_eq!(indices.len(), word_count);
            let pad_bits = word_count * RADIX_BITS - total_bits;
            let back = indices_to_bytes(&indices, pad_bits, len).unwrap();
            assert_eq!(back, bytes, "bit packing failed for len {len}");
        }
    }

    // --- Wordlist integrity.

    #[test]
    fn test_wordlist_integrity() {
        let wl = slip39_wordlist();
        assert_eq!(wl.len(), WORDLIST_SIZE);
        assert_eq!(wl[0], "academic");
        assert_eq!(wl[WORDLIST_SIZE - 1], "zero");
        // Sorted (required for binary search) and unique first-4-letters.
        let mut prefixes = std::collections::HashSet::new();
        for w in wl.windows(2) {
            assert!(w[0] < w[1], "wordlist not strictly sorted at {:?}", w);
        }
        for w in wl.iter() {
            let p: String = w.chars().take(4).collect();
            assert!(prefixes.insert(p), "duplicate 4-letter prefix for {w}");
        }
        // word <-> index round-trip.
        for (i, w) in wl.iter().enumerate() {
            assert_eq!(word_to_index(w).unwrap() as usize, i);
            assert_eq!(index_to_word(i as u16).unwrap(), *w);
        }
        assert!(word_to_index("notaword").is_err());
    }

    // --- Share <-> mnemonic round-trip.

    #[test]
    fn test_share_mnemonic_roundtrip() {
        let share = Slip39Share {
            identifier: 0x1abc & 0x7fff,
            extendable: false,
            iteration_exponent: 0,
            group_index: 2,
            group_threshold: 2,
            group_count: 3,
            member_index: 1,
            member_threshold: 3,
            share_value: vec![0xa5u8; 16],
        };
        let mnemonic = share.to_mnemonic().unwrap();
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert!(words.len() >= MIN_MNEMONIC_LENGTH_WORDS);
        let parsed = Slip39Share::from_mnemonic(&mnemonic).unwrap();
        assert_eq!(parsed, share, "decoded share must match the original");
        assert!(parsed.validate_checksum());
    }

    #[test]
    fn test_mnemonic_corruption_rejected() {
        let share = Slip39Share {
            identifier: 1,
            extendable: false,
            iteration_exponent: 0,
            group_index: 0,
            group_threshold: 1,
            group_count: 1,
            member_index: 0,
            member_threshold: 1,
            share_value: vec![0u8; 16],
        };
        let mnemonic = share.to_mnemonic().unwrap();
        let mut words: Vec<&str> = mnemonic.split_whitespace().collect();
        // Replace the last word with a different valid word -> checksum fails.
        let last = words.len() - 1;
        words[last] = if words[last] == "academic" {
            "zero"
        } else {
            "academic"
        };
        let corrupted = words.join(" ");
        assert!(Slip39Share::from_mnemonic(&corrupted).is_err());
    }

    // --- Public config validation.

    #[test]
    fn test_share_threshold_creation() {
        let threshold = ShareThreshold::new(3, 5).unwrap();
        assert_eq!(threshold.threshold, 3);
        assert_eq!(threshold.total_shares, 5);
        assert!(threshold.is_valid());

        assert!(ShareThreshold::new(0, 5).is_err());
        assert!(ShareThreshold::new(6, 5).is_err());
        assert!(ShareThreshold::new(3, 20).is_err());
        // 1-of-N (N>1) is forbidden by SLIP-39.
        assert!(ShareThreshold::new(1, 3).is_err());
        // 1-of-1 is allowed.
        assert!(ShareThreshold::new(1, 1).is_ok());
    }

    #[test]
    fn test_group_config() {
        let config = GroupConfig::new(0, ShareThreshold::new(2, 3).unwrap())
            .unwrap()
            .with_description("Family");
        assert_eq!(config.group_id, 0);
        assert_eq!(config.description, Some("Family".to_string()));
        assert!(GroupConfig::new(16, ShareThreshold::new(2, 3).unwrap()).is_err());
    }

    // --- Single-group generation/recovery (full SLIP-39 pipeline).

    #[test]
    fn test_single_group_generation_and_recovery() {
        // 3-of-5, 256-bit: all shares share one id and a valid checksum.
        let secret = [42u8; 32];
        let generator = Slip39Generator::new();
        let shares = generator
            .generate_shares(&secret, ShareThreshold::new(3, 5).unwrap(), None)
            .unwrap();
        assert_eq!(shares.len(), 5);
        let id = shares[0].identifier;
        for share in &shares {
            assert_eq!(share.identifier, id);
            assert!(share.validate_checksum());
        }
        // Any 3-subset recovers; an insufficient 2-subset is rejected.
        assert_eq!(
            Slip39Generator::recover_secret(&shares[0..3]).unwrap(),
            secret
        );
        assert_eq!(
            Slip39Generator::recover_secret(&shares[2..5]).unwrap(),
            secret
        );
        assert!(Slip39Generator::recover_secret(&shares[0..2]).is_err());

        // Invalid secret length is rejected.
        assert!(
            generator
                .generate_shares(&[42u8; 20], ShareThreshold::new(2, 3).unwrap(), None)
                .is_err()
        );
    }

    #[test]
    fn test_generation_recovery_128_via_mnemonics() {
        let secret = [0x11u8; 16];
        let shares = Slip39Generator::new()
            .generate_shares(&secret, ShareThreshold::new(2, 3).unwrap(), None)
            .unwrap();
        // Round-trip through actual mnemonic strings.
        let mnemonics: Vec<String> = shares.iter().map(|s| s.to_mnemonic().unwrap()).collect();
        assert_eq!(combine_mnemonics(&mnemonics[0..2], b"").unwrap(), secret);
    }

    #[test]
    fn test_passphrase_roundtrip() {
        let secret = [0x55u8; 32];
        let shares = Slip39Generator::new()
            .generate_shares(&secret, ShareThreshold::new(2, 3).unwrap(), Some("hunter2"))
            .unwrap();
        // Correct passphrase recovers the secret.
        let ok = Slip39Generator::recover_secret_with_passphrase(&shares[0..2], Some("hunter2"))
            .unwrap();
        assert_eq!(ok, secret);
        // Wrong passphrase yields a *different* secret (SLIP-39 has no MAC on
        // the master secret; any passphrase "succeeds" but to other data).
        let wrong =
            Slip39Generator::recover_secret_with_passphrase(&shares[0..2], Some("wrong")).unwrap();
        assert_ne!(wrong, secret);
        // Non-ASCII passphrase ("é" is two non-ASCII bytes) is rejected.
        assert!(
            Slip39Generator::new()
                .generate_shares(&secret, ShareThreshold::new(2, 3).unwrap(), Some("café"))
                .is_err()
        );
    }

    #[test]
    fn test_recovery_rejects_mismatched_identifiers() {
        let secret = [42u8; 16];
        let thr = ShareThreshold::new(2, 3).unwrap();
        let s1 = Slip39Generator::new()
            .generate_shares(&secret, thr, None)
            .unwrap();
        let s2 = Slip39Generator::new()
            .generate_shares(&secret, thr, None)
            .unwrap();
        // Mix shares from two different secrets (different identifiers).
        let mixed = vec![s1[0].clone(), s2[0].clone()];
        assert!(Slip39Generator::recover_secret(&mixed).is_err());
    }

    // --- Two-level group generation/recovery.

    #[test]
    fn test_multi_group_counts_roundtrip_and_insufficient() {
        // 2-of-3 groups; member thresholds 2-of-3, 3-of-5, 1-of-1.
        let groups = vec![
            GroupConfig::new(0, ShareThreshold::new(2, 3).unwrap()).unwrap(),
            GroupConfig::new(1, ShareThreshold::new(3, 5).unwrap()).unwrap(),
            GroupConfig::new(2, ShareThreshold::new(1, 1).unwrap()).unwrap(),
        ];
        let generator = MultiGroupGenerator::new(groups, 2).unwrap();
        let secret = [0x7eu8; 32];
        let all = generator.generate_all_shares(&secret, None).unwrap();
        // Member counts per group.
        assert_eq!(all.len(), 3);
        assert_eq!(all.get(&0).unwrap().len(), 3);
        assert_eq!(all.get(&1).unwrap().len(), 5);
        assert_eq!(all.get(&2).unwrap().len(), 1);

        // Groups 0 (2 shares) + 2 (1 share) meet the group threshold 2.
        let mut combined: Vec<Slip39Share> = Vec::new();
        combined.extend_from_slice(&all.get(&0).unwrap()[0..2]);
        combined.extend_from_slice(&all.get(&2).unwrap()[0..1]);
        assert_eq!(
            MultiGroupGenerator::recover_from_groups(&combined).unwrap(),
            secret
        );
        // Groups 1 (3 shares) + 2 (1 share) also recover.
        let mut combined2: Vec<Slip39Share> = Vec::new();
        combined2.extend_from_slice(&all.get(&1).unwrap()[0..3]);
        combined2.extend_from_slice(&all.get(&2).unwrap()[0..1]);
        assert_eq!(
            MultiGroupGenerator::recover_from_groups(&combined2).unwrap(),
            secret
        );
        // Only one group satisfied -> insufficient groups, rejected.
        let one_group: Vec<Slip39Share> = all.get(&0).unwrap()[0..2].to_vec();
        assert!(MultiGroupGenerator::recover_from_groups(&one_group).is_err());

        // The same structure round-trips with a passphrase too.
        let pass = "MyWallet#42";
        let all_p = generator.generate_all_shares(&secret, Some(pass)).unwrap();
        let mut combined_p: Vec<Slip39Share> = Vec::new();
        combined_p.extend_from_slice(&all_p.get(&0).unwrap()[0..2]);
        combined_p.extend_from_slice(&all_p.get(&2).unwrap()[0..1]);
        let recovered_p =
            MultiGroupGenerator::recover_from_groups_with_passphrase(&combined_p, Some(pass))
                .unwrap();
        assert_eq!(recovered_p, secret);
    }

    // --- Official SLIP-39 test vectors (mnemonic -> master secret, pass "TREZOR").

    #[test]
    fn test_official_vectors_recover_master_secret() {
        // Each case: (description, &[mnemonics], expected master secret hex).
        // Verified against trezor/python-shamir-mnemonic `vectors.json`,
        // passphrase "TREZOR".
        let cases: &[(&str, &[&str], &str)] = &[
            (
                "1. Valid mnemonic without sharing (128 bits)",
                &[
                    "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision keyboard",
                ],
                "bb54aac4b89dc868ba37d9cc21b2cece",
            ),
            (
                "20. Valid mnemonic without sharing (256 bits)",
                &[
                    "theory painting academic academic armed sweater year military elder discuss acne wildlife boring employer fused large satoshi bundle carbon diagnose anatomy hamster leaves tracks paces beyond phantom capital marvel lips brave detect luck",
                ],
                "989baf9dcaad5b10ca33dfd8cc75e42477025dce88ae83e75a230086a0e00e92",
            ),
            (
                "4. Basic sharing 2-of-3 (128 bits)",
                &[
                    "shadow pistol academic always adequate wildlife fancy gross oasis cylinder mustang wrist rescue view short owner flip making coding armed",
                    "shadow pistol academic acid actress prayer class unknown daughter sweater depict flip twice unkind craft early superior advocate guest smoking",
                ],
                "b43ceb7e57a0ea8766221624d01b0864",
            ),
            (
                "17. Threshold number of groups and members in each group (128 bits)",
                &[
                    "eraser senior decision roster beard treat identify grumpy salt index fake aviation theater cubic bike cause research dragon emphasis counter",
                    "eraser senior ceramic snake clay various huge numb argue hesitate auction category timber browser greatest hanger petition script leaf pickup",
                    "eraser senior ceramic shaft dynamic become junior wrist silver peasant force math alto coal amazing segment yelp velvet image paces",
                    "eraser senior ceramic round column hawk trust auction smug shame alive greatest sheriff living perfect corner chest sled fumes adequate",
                    "eraser senior decision smug corner ruin rescue cubic angel tackle skin skunk program roster trash rumor slush angel flea amazing",
                ],
                "7c3397a292a5941682d7a4ae2d898d11",
            ),
        ];
        for (desc, mnemonics, expected) in cases {
            let owned: Vec<String> = mnemonics.iter().map(|m| m.to_string()).collect();
            let recovered = combine_mnemonics(&owned, b"TREZOR")
                .unwrap_or_else(|e| panic!("combine failed for [{desc}]: {e}"));
            assert_eq!(to_hex(&recovered), *expected, "wrong secret for [{desc}]");
        }
    }

    #[test]
    fn test_official_vectors_rejected() {
        // Vector 1 with its final word changed `keyboard` -> `kidney`: the
        // RS1024 checksum must reject it.
        let bad_checksum = "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision kidney";
        assert!(
            combine_mnemonics(&[bad_checksum.to_string()], b"TREZOR").is_err(),
            "invalid-checksum mnemonic must be rejected"
        );
        // Only one of the 2-of-3 shares from vector 4: must be rejected.
        let one_of_two = "shadow pistol academic always adequate wildlife fancy gross oasis cylinder mustang wrist rescue view short owner flip making coding armed";
        assert!(
            combine_mnemonics(&[one_of_two.to_string()], b"TREZOR").is_err(),
            "single share of a 2-of-3 must be rejected"
        );
    }
}

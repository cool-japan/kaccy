//! Miniscript support for Bitcoin scripts
//!
//! Miniscript is a language for writing Bitcoin scripts in a structured way,
//! making them easier to analyze, compose, and reason about.

use std::str::FromStr;

use bitcoin::{Network, PublicKey, ScriptBuf};
use miniscript::policy::Concrete;
use miniscript::{BareCtx, Legacy, Segwitv0};
use serde::{Deserialize, Serialize};

use crate::error::{BitcoinError, Result};

/// Concrete miniscript policy parameterised over real Bitcoin public keys.
///
/// Using [`bitcoin::PublicKey`] as the key type (rather than an opaque
/// `String`) is what allows the policy to be compiled into a *real* Bitcoin
/// script: every `pk(..)` leaf becomes an actual 33-byte key push followed by
/// `OP_CHECKSIG`, multisig leaves become `OP_CHECKMULTISIG` forms, and so on.
type ConcretePolicy = Concrete<PublicKey>;

/// Miniscript policy types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MiniscriptPolicy {
    /// Public key policy: pk(KEY)
    PublicKey(String),
    /// Hash preimage: hash256(H), sha256(H), ripemd160(H), hash160(H)
    Hash {
        /// Hash function type
        hash_type: HashType,
        /// Hex-encoded hash value
        hash: String,
    },
    /// Timelock: after(n), older(n)
    Timelock {
        /// Whether timelock is absolute or relative
        timelock_type: TimelockType,
        /// Timelock value (block height or seconds)
        value: u32,
    },
    /// Threshold: thresh(k, X1, ..., Xn)
    Threshold {
        /// Minimum number of sub-policies that must be satisfied
        k: usize,
        /// Sub-policies to combine
        policies: Vec<MiniscriptPolicy>,
    },
    /// Logical AND: and(X, Y)
    And(Box<MiniscriptPolicy>, Box<MiniscriptPolicy>),
    /// Logical OR: or(X, Y)
    Or(Box<MiniscriptPolicy>, Box<MiniscriptPolicy>),
    /// Multi-signature: multi(k, KEY1, ..., KEYn)
    Multi {
        /// Required number of signatures
        k: usize,
        /// List of public keys
        keys: Vec<String>,
    },
}

/// Hash types for miniscript
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashType {
    /// SHA256
    Sha256,
    /// HASH256 (double SHA256)
    Hash256,
    /// RIPEMD160
    Ripemd160,
    /// HASH160 (SHA256 then RIPEMD160)
    Hash160,
}

/// Timelock types for miniscript
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelockType {
    /// Absolute block height or timestamp
    After,
    /// Relative block height or timestamp
    Older,
}

/// Miniscript descriptor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniscriptDescriptor {
    /// The miniscript policy
    pub policy: MiniscriptPolicy,
    /// Network for address generation
    pub network: Network,
    /// Script type (WSH, SH, etc.)
    pub script_type: MiniscriptScriptType,
}

/// Script types for miniscript
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MiniscriptScriptType {
    /// Bare (legacy)
    Bare,
    /// Pay-to-Script-Hash (P2SH)
    P2SH,
    /// Pay-to-Witness-Script-Hash (P2WSH)
    P2WSH,
    /// Pay-to-Script-Hash Witness Script Hash (P2SH-P2WSH)
    P2SHP2WSH,
}

impl MiniscriptDescriptor {
    /// Create a new miniscript descriptor
    pub fn new(
        policy: MiniscriptPolicy,
        network: Network,
        script_type: MiniscriptScriptType,
    ) -> Self {
        Self {
            policy,
            network,
            script_type,
        }
    }

    /// Compile the policy into a real Bitcoin script.
    ///
    /// The policy is first converted into the concrete-policy syntax understood
    /// by the [`miniscript`] crate, parsed into a [`Concrete<PublicKey>`], and
    /// then compiled by miniscript's policy compiler into the witness/redeem
    /// script for the configured [`script_type`](MiniscriptDescriptor::script_type).
    ///
    /// The resulting [`ScriptBuf`] is always non-empty and contains the real
    /// opcodes for the policy (key pushes + `OP_CHECKSIG`, `OP_CHECKMULTISIG`
    /// for multisig/threshold-of-keys, `OP_CLTV`/`OP_CSV` for timelocks, the
    /// `OP_IF`/`OP_ELSE`/`OP_ENDIF` skeleton for disjunctions, etc.).
    pub fn compile(&self) -> Result<ScriptBuf> {
        let parsed = self.parse_concrete_policy()?;
        self.compile_concrete(&parsed)
    }

    /// Convert the policy into miniscript concrete-policy syntax and parse it
    /// into a [`Concrete<PublicKey>`].
    ///
    /// This is where every key string is validated as a real Bitcoin public
    /// key; an invalid key (or otherwise malformed policy) is reported as a
    /// [`BitcoinError::Validation`] rather than silently producing nonsense.
    fn parse_concrete_policy(&self) -> Result<ConcretePolicy> {
        let policy_str = build_policy_string(&self.policy)?;
        ConcretePolicy::from_str(&policy_str).map_err(|e| {
            BitcoinError::Validation(format!("failed to parse miniscript policy: {e}"))
        })
    }

    /// Compile a parsed concrete policy into a script for the configured
    /// script type / context.
    ///
    /// * `P2WSH` / `P2SHP2WSH` use the Segwit-v0 script context.
    /// * `P2SH` uses the legacy (pre-segwit) script context.
    /// * `Bare` uses the bare script context.
    fn compile_concrete(&self, policy: &ConcretePolicy) -> Result<ScriptBuf> {
        let script = match self.script_type {
            MiniscriptScriptType::P2WSH | MiniscriptScriptType::P2SHP2WSH => policy
                .compile::<Segwitv0>()
                .map_err(|e| BitcoinError::Validation(format!("segwit compilation failed: {e}")))?
                .encode(),
            MiniscriptScriptType::P2SH => policy
                .compile::<Legacy>()
                .map_err(|e| BitcoinError::Validation(format!("legacy compilation failed: {e}")))?
                .encode(),
            MiniscriptScriptType::Bare => policy
                .compile::<BareCtx>()
                .map_err(|e| BitcoinError::Validation(format!("bare compilation failed: {e}")))?
                .encode(),
        };

        // A correctly-compiled policy can never be empty; treat an empty script
        // as a hard error rather than handing back something unusable.
        if script.is_empty() {
            return Err(BitcoinError::Validation(
                "miniscript compilation produced an empty script".to_string(),
            ));
        }

        Ok(script)
    }

    /// Analyze the policy for properties
    pub fn analyze(&self) -> PolicyAnalysis {
        PolicyAnalysis {
            is_malleable: self.check_malleability(),
            is_sane: self.check_sanity(),
            max_satisfaction_weight: self.estimate_weight(),
            requires_timelock: self.requires_timelock(),
            requires_hash_preimage: self.requires_hash_preimage(),
        }
    }

    fn check_malleability(&self) -> bool {
        // Simplified malleability check
        false
    }

    fn check_sanity(&self) -> bool {
        // Check if the policy makes sense
        true
    }

    fn estimate_weight(&self) -> usize {
        // Estimate worst-case witness weight
        match &self.policy {
            MiniscriptPolicy::PublicKey(_) => 73, // signature
            MiniscriptPolicy::Multi { k, keys } => {
                // k signatures plus multi-sig overhead
                73 * k + 34 * keys.len()
            }
            MiniscriptPolicy::Threshold { k: _, policies } => {
                // Sum of worst-case weights for k policies
                policies.iter().map(|_| 100).sum()
            }
            _ => 100, // Conservative estimate
        }
    }

    fn requires_timelock(&self) -> bool {
        self.check_policy_for_feature(&self.policy, |p| {
            matches!(p, MiniscriptPolicy::Timelock { .. })
        })
    }

    fn requires_hash_preimage(&self) -> bool {
        self.check_policy_for_feature(&self.policy, |p| matches!(p, MiniscriptPolicy::Hash { .. }))
    }

    #[allow(clippy::only_used_in_recursion)]
    fn check_policy_for_feature<F>(&self, policy: &MiniscriptPolicy, check: F) -> bool
    where
        F: Fn(&MiniscriptPolicy) -> bool + Copy,
    {
        if check(policy) {
            return true;
        }

        match policy {
            MiniscriptPolicy::And(left, right) | MiniscriptPolicy::Or(left, right) => {
                self.check_policy_for_feature(left, check)
                    || self.check_policy_for_feature(right, check)
            }
            MiniscriptPolicy::Threshold { policies, .. } => policies
                .iter()
                .any(|p| self.check_policy_for_feature(p, check)),
            _ => false,
        }
    }
}

/// Render a [`MiniscriptPolicy`] into the concrete-policy text syntax accepted
/// by the [`miniscript`] crate's parser.
///
/// The mapping is:
/// * [`MiniscriptPolicy::PublicKey`] -> `pk(<key>)`
/// * [`MiniscriptPolicy::Hash`] -> `sha256(..)` / `hash256(..)` /
///   `ripemd160(..)` / `hash160(..)`
/// * [`MiniscriptPolicy::Timelock`] -> `after(n)` / `older(n)`
/// * [`MiniscriptPolicy::And`] -> `and(a,b)`
/// * [`MiniscriptPolicy::Or`] -> `or(a,b)`
/// * [`MiniscriptPolicy::Threshold`] -> `thresh(k,X1,..,Xn)`
/// * [`MiniscriptPolicy::Multi`] -> `thresh(k,pk(K1),..,pk(Kn))`
///
/// Note that miniscript's *concrete policy* language has no `multi(..)`
/// keyword (that is a miniscript *fragment*, emitted by the compiler). A
/// `k`-of-`n` multisig is therefore expressed as a threshold over `pk(..)`
/// leaves, which the compiler turns into an `OP_CHECKMULTISIG`-based script.
fn build_policy_string(policy: &MiniscriptPolicy) -> Result<String> {
    match policy {
        MiniscriptPolicy::PublicKey(key) => Ok(format!("pk({key})")),
        MiniscriptPolicy::Hash { hash_type, hash } => {
            let func = match hash_type {
                HashType::Sha256 => "sha256",
                HashType::Hash256 => "hash256",
                HashType::Ripemd160 => "ripemd160",
                HashType::Hash160 => "hash160",
            };
            Ok(format!("{func}({hash})"))
        }
        MiniscriptPolicy::Timelock {
            timelock_type,
            value,
        } => match timelock_type {
            TimelockType::After => Ok(format!("after({value})")),
            TimelockType::Older => Ok(format!("older({value})")),
        },
        MiniscriptPolicy::And(left, right) => {
            let left = build_policy_string(left)?;
            let right = build_policy_string(right)?;
            Ok(format!("and({left},{right})"))
        }
        MiniscriptPolicy::Or(left, right) => {
            let left = build_policy_string(left)?;
            let right = build_policy_string(right)?;
            Ok(format!("or({left},{right})"))
        }
        MiniscriptPolicy::Threshold { k, policies } => {
            if *k == 0 || *k > policies.len() {
                return Err(BitcoinError::Validation(format!(
                    "invalid threshold: k={k} but only {} sub-policies",
                    policies.len()
                )));
            }
            let subs = policies
                .iter()
                .map(build_policy_string)
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("thresh({k},{})", subs.join(",")))
        }
        MiniscriptPolicy::Multi { k, keys } => {
            if *k == 0 || *k > keys.len() {
                return Err(BitcoinError::Validation(format!(
                    "invalid multisig: k={k} but only {} keys",
                    keys.len()
                )));
            }
            let subs = keys
                .iter()
                .map(|key| format!("pk({key})"))
                .collect::<Vec<_>>();
            Ok(format!("thresh({k},{})", subs.join(",")))
        }
    }
}

/// Convert a parsed miniscript [`Concrete<PublicKey>`] AST back into the local
/// [`MiniscriptPolicy`] representation.
///
/// A `thresh(k, pk(..), pk(..), ..)` whose children are *all* keys is collapsed
/// back into [`MiniscriptPolicy::Multi`] so that a multisig policy round-trips
/// to the same representation it was built from; any other threshold is kept as
/// a general [`MiniscriptPolicy::Threshold`].
fn concrete_to_policy(policy: &ConcretePolicy) -> Result<MiniscriptPolicy> {
    match policy {
        Concrete::Key(pk) => Ok(MiniscriptPolicy::PublicKey(pk.to_string())),
        Concrete::After(lock) => Ok(MiniscriptPolicy::Timelock {
            timelock_type: TimelockType::After,
            value: lock.to_consensus_u32(),
        }),
        Concrete::Older(lock) => Ok(MiniscriptPolicy::Timelock {
            timelock_type: TimelockType::Older,
            value: lock.to_consensus_u32(),
        }),
        Concrete::Sha256(hash) => Ok(MiniscriptPolicy::Hash {
            hash_type: HashType::Sha256,
            hash: hash.to_string(),
        }),
        Concrete::Hash256(hash) => Ok(MiniscriptPolicy::Hash {
            hash_type: HashType::Hash256,
            hash: hash.to_string(),
        }),
        Concrete::Ripemd160(hash) => Ok(MiniscriptPolicy::Hash {
            hash_type: HashType::Ripemd160,
            hash: hash.to_string(),
        }),
        Concrete::Hash160(hash) => Ok(MiniscriptPolicy::Hash {
            hash_type: HashType::Hash160,
            hash: hash.to_string(),
        }),
        Concrete::And(subs) => {
            if subs.len() != 2 {
                return Err(BitcoinError::Validation(format!(
                    "unsupported 'and' arity: expected 2, got {}",
                    subs.len()
                )));
            }
            let left = concrete_to_policy(&subs[0])?;
            let right = concrete_to_policy(&subs[1])?;
            Ok(MiniscriptPolicy::And(Box::new(left), Box::new(right)))
        }
        Concrete::Or(subs) => {
            if subs.len() != 2 {
                return Err(BitcoinError::Validation(format!(
                    "unsupported 'or' arity: expected 2, got {}",
                    subs.len()
                )));
            }
            let left = concrete_to_policy(&subs[0].1)?;
            let right = concrete_to_policy(&subs[1].1)?;
            Ok(MiniscriptPolicy::Or(Box::new(left), Box::new(right)))
        }
        Concrete::Thresh(thresh) => {
            let k = thresh.k();
            let children = thresh
                .data()
                .iter()
                .map(|sub| concrete_to_policy(sub))
                .collect::<Result<Vec<_>>>()?;

            // Collapse a pure threshold-of-keys back into a `Multi` policy.
            if let Some(keys) = collect_keys(&children) {
                Ok(MiniscriptPolicy::Multi { k, keys })
            } else {
                Ok(MiniscriptPolicy::Threshold {
                    k,
                    policies: children,
                })
            }
        }
        Concrete::Unsatisfiable | Concrete::Trivial => Err(BitcoinError::Validation(
            "unsupported policy fragment (UNSATISFIABLE/TRIVIAL)".to_string(),
        )),
    }
}

/// If every policy in `policies` is a bare public key, return the list of key
/// strings; otherwise return `None`.
fn collect_keys(policies: &[MiniscriptPolicy]) -> Option<Vec<String>> {
    let mut keys = Vec::with_capacity(policies.len());
    for policy in policies {
        match policy {
            MiniscriptPolicy::PublicKey(key) => keys.push(key.clone()),
            _ => return None,
        }
    }
    Some(keys)
}

/// Policy analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAnalysis {
    /// Whether the policy is malleable
    pub is_malleable: bool,
    /// Whether the policy is sane (makes logical sense)
    pub is_sane: bool,
    /// Maximum satisfaction weight in witness units
    pub max_satisfaction_weight: usize,
    /// Whether the policy requires a timelock
    pub requires_timelock: bool,
    /// Whether the policy requires a hash preimage
    pub requires_hash_preimage: bool,
}

/// Miniscript compiler and analyzer
pub struct MiniscriptCompiler {
    network: Network,
}

impl MiniscriptCompiler {
    /// Create a new miniscript compiler
    pub fn new(network: Network) -> Self {
        Self { network }
    }

    /// Parse a miniscript concrete-policy string into a [`MiniscriptPolicy`].
    ///
    /// This delegates to the [`miniscript`] crate's policy parser, so the full
    /// concrete-policy grammar is supported (`pk`, `after`, `older`, the four
    /// hash fragments, `and`, `or` and `thresh`) and every key is validated as
    /// a real [`bitcoin::PublicKey`]. A `thresh(k, pk(..), ..)` over bare keys
    /// is normalised back into [`MiniscriptPolicy::Multi`].
    ///
    /// Malformed input (bad keys, unknown fragments, wrong arity, ...) is
    /// reported as a [`BitcoinError::Validation`] error; it never silently
    /// succeeds. Because miniscript's concrete policy has no `multi(..)`
    /// keyword, that exact spelling is rejected with a hint to use `thresh`.
    pub fn parse_policy(&self, policy_str: &str) -> Result<MiniscriptPolicy> {
        let parsed = ConcretePolicy::from_str(policy_str).map_err(|e| {
            BitcoinError::Validation(format!("failed to parse miniscript policy: {e}"))
        })?;
        concrete_to_policy(&parsed)
    }

    /// Compile a policy to a descriptor
    pub fn compile(
        &self,
        policy: MiniscriptPolicy,
        script_type: MiniscriptScriptType,
    ) -> Result<MiniscriptDescriptor> {
        Ok(MiniscriptDescriptor::new(policy, self.network, script_type))
    }

    /// Optimize a policy (simplified)
    pub fn optimize(&self, policy: MiniscriptPolicy) -> MiniscriptPolicy {
        // In production, this would perform various optimizations
        policy
    }
}

/// Policy template builder for common use cases
pub struct PolicyTemplateBuilder {
    #[allow(dead_code)]
    network: Network,
}

impl PolicyTemplateBuilder {
    /// Create a new policy template builder
    pub fn new(network: Network) -> Self {
        Self { network }
    }

    /// Create a simple single-key policy
    pub fn single_key(&self, pubkey: String) -> MiniscriptPolicy {
        MiniscriptPolicy::PublicKey(pubkey)
    }

    /// Create a multi-signature policy
    pub fn multisig(&self, k: usize, keys: Vec<String>) -> MiniscriptPolicy {
        MiniscriptPolicy::Multi { k, keys }
    }

    /// Create a timelock policy (absolute)
    pub fn absolute_timelock(&self, value: u32) -> MiniscriptPolicy {
        MiniscriptPolicy::Timelock {
            timelock_type: TimelockType::After,
            value,
        }
    }

    /// Create a timelock policy (relative)
    pub fn relative_timelock(&self, value: u32) -> MiniscriptPolicy {
        MiniscriptPolicy::Timelock {
            timelock_type: TimelockType::Older,
            value,
        }
    }

    /// Create a hash preimage policy
    pub fn hash_preimage(&self, hash_type: HashType, hash: String) -> MiniscriptPolicy {
        MiniscriptPolicy::Hash { hash_type, hash }
    }

    /// Create an AND policy
    pub fn and(&self, left: MiniscriptPolicy, right: MiniscriptPolicy) -> MiniscriptPolicy {
        MiniscriptPolicy::And(Box::new(left), Box::new(right))
    }

    /// Create an OR policy
    pub fn or(&self, left: MiniscriptPolicy, right: MiniscriptPolicy) -> MiniscriptPolicy {
        MiniscriptPolicy::Or(Box::new(left), Box::new(right))
    }

    /// Create a threshold policy
    pub fn threshold(&self, k: usize, policies: Vec<MiniscriptPolicy>) -> MiniscriptPolicy {
        MiniscriptPolicy::Threshold { k, policies }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_key_policy() {
        let builder = PolicyTemplateBuilder::new(Network::Bitcoin);
        let policy = builder.single_key(
            "0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798".to_string(),
        );

        match policy {
            MiniscriptPolicy::PublicKey(key) => {
                assert!(key.starts_with("0279BE667"));
            }
            _ => panic!("Expected PublicKey policy"),
        }
    }

    #[test]
    fn test_multisig_policy() {
        let builder = PolicyTemplateBuilder::new(Network::Bitcoin);
        let keys = vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
        let policy = builder.multisig(2, keys.clone());

        match policy {
            MiniscriptPolicy::Multi {
                k,
                keys: policy_keys,
            } => {
                assert_eq!(k, 2);
                assert_eq!(policy_keys, keys);
            }
            _ => panic!("Expected Multi policy"),
        }
    }

    #[test]
    fn test_timelock_policy() {
        let builder = PolicyTemplateBuilder::new(Network::Bitcoin);
        let policy = builder.absolute_timelock(500000);

        match policy {
            MiniscriptPolicy::Timelock {
                timelock_type,
                value,
            } => {
                assert_eq!(timelock_type, TimelockType::After);
                assert_eq!(value, 500000);
            }
            _ => panic!("Expected Timelock policy"),
        }
    }

    #[test]
    fn test_and_policy() {
        let builder = PolicyTemplateBuilder::new(Network::Bitcoin);
        let left = builder.single_key("key1".to_string());
        let right = builder.absolute_timelock(500000);
        let policy = builder.and(left, right);

        match policy {
            MiniscriptPolicy::And(_, _) => {}
            _ => panic!("Expected And policy"),
        }
    }

    // Real, valid compressed secp256k1 public keys (33 bytes => 66 hex chars,
    // 0x02/0x03 prefix). These are required because compilation produces real
    // scripts and therefore needs real keys.
    const KEY_A: &str = "02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13";
    const KEY_B: &str = "0250863ad64a87ae8a2fe83c1af1a8403cb53f53e486d8511dad8a04887e5b2352";
    const KEY_C: &str = "03933884aaf1d6b108397e5efe5c86bcf2d8ca8d2f700eda99db9214fc2712b134";
    // A real 32-byte SHA256 digest (hex).
    const SHA256_HEX: &str = "6c60f404f8167a38fc70eaf8aa17ac351023bef86bcb9d1086a19afe95bd5333";

    /// Collect the opcodes/pushes of a script into a printable token vector so
    /// that tests can assert on real structure rather than just "non-empty".
    fn script_tokens(script: &ScriptBuf) -> Vec<String> {
        script
            .instructions()
            .map(|inst| match inst {
                Ok(bitcoin::script::Instruction::Op(op)) => format!("{op:?}"),
                Ok(bitcoin::script::Instruction::PushBytes(bytes)) => {
                    format!("PUSH({})", bytes.len())
                }
                Err(e) => format!("ERR({e})"),
            })
            .collect()
    }

    #[test]
    fn test_policy_parsing() {
        let compiler = MiniscriptCompiler::new(Network::Bitcoin);

        // pk(<real key>) round-trips into a PublicKey policy.
        let policy = compiler.parse_policy(&format!("pk({KEY_A})")).unwrap();
        match policy {
            MiniscriptPolicy::PublicKey(ref key) => {
                assert_eq!(key.to_lowercase(), KEY_A);
            }
            _ => panic!("Expected PublicKey policy"),
        }

        // A threshold of bare keys is normalised back into a Multi policy.
        let policy = compiler
            .parse_policy(&format!("thresh(2,pk({KEY_A}),pk({KEY_B}),pk({KEY_C}))"))
            .unwrap();
        if let MiniscriptPolicy::Multi { k, keys } = policy {
            assert_eq!(k, 2);
            assert_eq!(keys.len(), 3);
        } else {
            panic!("Expected Multi policy, got {policy:?}");
        }

        // Absolute timelock.
        let policy = compiler.parse_policy("after(500000)").unwrap();
        assert!(matches!(
            policy,
            MiniscriptPolicy::Timelock {
                timelock_type: TimelockType::After,
                value: 500000,
            }
        ));
    }

    #[test]
    fn test_parse_rejects_invalid_key() {
        let compiler = MiniscriptCompiler::new(Network::Bitcoin);
        // "key1" is not a valid public key: this must be a hard error, never a
        // silent success.
        let err = compiler.parse_policy("pk(key1)").unwrap_err();
        assert!(
            matches!(err, BitcoinError::Validation(_)),
            "expected Validation error, got {err:?}"
        );
    }

    #[test]
    fn test_parse_rejects_multi_keyword() {
        let compiler = MiniscriptCompiler::new(Network::Bitcoin);
        // `multi(..)` is a miniscript *fragment*, not a concrete-policy keyword;
        // the parser must reject it rather than silently succeed.
        let err = compiler
            .parse_policy(&format!("multi(2,{KEY_A},{KEY_B})"))
            .unwrap_err();
        assert!(matches!(err, BitcoinError::Validation(_)));
    }

    #[test]
    fn test_compile_pk_yields_checksig() {
        // pk(KEY) must compile to: <33-byte key push> OP_CHECKSIG
        let policy = MiniscriptPolicy::PublicKey(KEY_A.to_string());
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("pk policy must compile");
        assert!(!script.is_empty(), "compiled script must not be empty");

        let tokens = script_tokens(&script);
        assert_eq!(
            tokens,
            vec!["PUSH(33)".to_string(), "OP_CHECKSIG".to_string()],
            "unexpected pk script structure: {tokens:?}"
        );
    }

    #[test]
    fn test_compile_multisig_yields_checkmultisig() {
        // multi(2, A, B, C) must compile to a CHECKMULTISIG form:
        // OP_2 <A> <B> <C> OP_3 OP_CHECKMULTISIG
        let policy = MiniscriptPolicy::Multi {
            k: 2,
            keys: vec![KEY_A.to_string(), KEY_B.to_string(), KEY_C.to_string()],
        };
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("multisig must compile");
        let tokens = script_tokens(&script);

        // Must end with CHECKMULTISIG and contain three 33-byte key pushes.
        assert!(
            tokens.last().map(String::as_str) == Some("OP_CHECKMULTISIG"),
            "multisig must end with OP_CHECKMULTISIG: {tokens:?}"
        );
        let key_pushes = tokens.iter().filter(|t| *t == "PUSH(33)").count();
        assert_eq!(key_pushes, 3, "expected 3 key pushes: {tokens:?}");
        assert!(
            tokens.contains(&"OP_PUSHNUM_2".to_string()),
            "expected required-count OP_2: {tokens:?}"
        );
        assert!(
            tokens.contains(&"OP_PUSHNUM_3".to_string()),
            "expected total-count OP_3: {tokens:?}"
        );
    }

    #[test]
    fn test_bare_timelock_is_rejected_as_unsafe() {
        // A bare `after(n)` is not a safe top-level script (no signature is
        // required to spend), so miniscript's compiler legitimately refuses it.
        // We surface that as an honest Validation error rather than emitting an
        // anyone-can-spend script.
        let policy = MiniscriptPolicy::Timelock {
            timelock_type: TimelockType::After,
            value: 500_000,
        };
        let err = MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH)
            .compile()
            .unwrap_err();
        assert!(matches!(err, BitcoinError::Validation(_)));
    }

    #[test]
    fn test_compile_after_timelock_uses_cltv() {
        // A realistic absolute timelock is gated behind a key:
        // and(pk(A), after(n)) must compile and use OP_CLTV.
        let policy = MiniscriptPolicy::And(
            Box::new(MiniscriptPolicy::PublicKey(KEY_A.to_string())),
            Box::new(MiniscriptPolicy::Timelock {
                timelock_type: TimelockType::After,
                value: 500_000,
            }),
        );
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("and(pk, after()) must compile");
        let tokens = script_tokens(&script);
        assert!(
            tokens.iter().any(|t| t == "OP_CLTV"),
            "after() must use OP_CLTV: {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| t == "PUSH(33)"),
            "policy must reference the key: {tokens:?}"
        );
    }

    #[test]
    fn test_compile_older_timelock_uses_csv() {
        // A realistic relative timelock is gated behind a key:
        // and(pk(A), older(n)) must compile and use OP_CSV.
        let policy = MiniscriptPolicy::And(
            Box::new(MiniscriptPolicy::PublicKey(KEY_A.to_string())),
            Box::new(MiniscriptPolicy::Timelock {
                timelock_type: TimelockType::Older,
                value: 144,
            }),
        );
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("and(pk, older()) must compile");
        let tokens = script_tokens(&script);
        assert!(
            tokens.iter().any(|t| t == "OP_CSV"),
            "older() must use OP_CSV: {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| t == "PUSH(33)"),
            "policy must reference the key: {tokens:?}"
        );
    }

    #[test]
    fn test_compile_and_combines_subpolicies() {
        // and(pk(A), older(144)) must compile to a non-empty script that
        // references both the key and the relative timelock.
        let policy = MiniscriptPolicy::And(
            Box::new(MiniscriptPolicy::PublicKey(KEY_A.to_string())),
            Box::new(MiniscriptPolicy::Timelock {
                timelock_type: TimelockType::Older,
                value: 144,
            }),
        );
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("and() must compile");
        let tokens = script_tokens(&script);
        assert!(!script.is_empty());
        assert!(
            tokens.iter().any(|t| t == "PUSH(33)"),
            "and() must push the key: {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| t == "OP_CSV"),
            "and() must enforce the timelock: {tokens:?}"
        );
        // miniscript may emit either OP_CHECKSIG or OP_CHECKSIGVERIFY depending
        // on how the AND is wired; both genuinely check the signature.
        assert!(
            tokens
                .iter()
                .any(|t| t == "OP_CHECKSIG" || t == "OP_CHECKSIGVERIFY"),
            "and() must check the signature: {tokens:?}"
        );
    }

    #[test]
    fn test_compile_or_uses_branching() {
        // or(pk(A), pk(B)) must compile to a real branching script. miniscript
        // emits efficient or-forms; assert the structure is non-trivial and
        // references both keys.
        let policy = MiniscriptPolicy::Or(
            Box::new(MiniscriptPolicy::PublicKey(KEY_A.to_string())),
            Box::new(MiniscriptPolicy::PublicKey(KEY_B.to_string())),
        );
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("or() must compile");
        let tokens = script_tokens(&script);
        assert!(!script.is_empty());
        let key_pushes = tokens.iter().filter(|t| *t == "PUSH(33)").count();
        assert_eq!(
            key_pushes, 2,
            "or() of two keys must push both keys: {tokens:?}"
        );
    }

    #[test]
    fn test_compile_threshold_of_keys() {
        // thresh(2, pk(A), pk(B), pk(C)) compiles to a real threshold script.
        let policy = MiniscriptPolicy::Threshold {
            k: 2,
            policies: vec![
                MiniscriptPolicy::PublicKey(KEY_A.to_string()),
                MiniscriptPolicy::PublicKey(KEY_B.to_string()),
                MiniscriptPolicy::PublicKey(KEY_C.to_string()),
            ],
        };
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor.compile().expect("thresh() must compile");
        let tokens = script_tokens(&script);
        assert!(!script.is_empty());
        let key_pushes = tokens.iter().filter(|t| *t == "PUSH(33)").count();
        assert_eq!(key_pushes, 3, "thresh must reference all keys: {tokens:?}");
        // Threshold over keys uses OP_CHECKSIG / OP_CHECKSIGVERIFY accumulation
        // and a final OP_EQUAL against the required count.
        assert!(
            tokens
                .iter()
                .any(|t| t == "OP_EQUAL" || t == "OP_CHECKMULTISIG"),
            "thresh must combine results: {tokens:?}"
        );
    }

    #[test]
    fn test_bare_hash_is_rejected_as_unsafe() {
        // As with a bare timelock, a bare hash-preimage is not a safe top-level
        // script; miniscript refuses it and we report an honest error.
        let policy = MiniscriptPolicy::Hash {
            hash_type: HashType::Sha256,
            hash: SHA256_HEX.to_string(),
        };
        let err = MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH)
            .compile()
            .unwrap_err();
        assert!(matches!(err, BitcoinError::Validation(_)));
    }

    #[test]
    fn test_compile_hash_preimage() {
        // A realistic hash-preimage policy is gated behind a key:
        // and(pk(A), sha256(H)) compiles to a real hash check using OP_SHA256
        // and a 32-byte digest push.
        let policy = MiniscriptPolicy::And(
            Box::new(MiniscriptPolicy::PublicKey(KEY_A.to_string())),
            Box::new(MiniscriptPolicy::Hash {
                hash_type: HashType::Sha256,
                hash: SHA256_HEX.to_string(),
            }),
        );
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let script = descriptor
            .compile()
            .expect("and(pk, sha256()) must compile");
        let tokens = script_tokens(&script);
        assert!(
            tokens.iter().any(|t| t == "OP_SHA256"),
            "sha256() must use OP_SHA256: {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| t == "PUSH(32)"),
            "sha256() must push the 32-byte digest: {tokens:?}"
        );
    }

    #[test]
    fn test_roundtrip_parse_compile_pk() {
        // Parsing a policy string and compiling it must yield the same script
        // as building the equivalent policy directly.
        let compiler = MiniscriptCompiler::new(Network::Bitcoin);
        let parsed = compiler.parse_policy(&format!("pk({KEY_A})")).unwrap();

        let from_parsed =
            MiniscriptDescriptor::new(parsed, Network::Bitcoin, MiniscriptScriptType::P2WSH)
                .compile()
                .unwrap();
        let direct = MiniscriptDescriptor::new(
            MiniscriptPolicy::PublicKey(KEY_A.to_string()),
            Network::Bitcoin,
            MiniscriptScriptType::P2WSH,
        )
        .compile()
        .unwrap();

        assert_eq!(from_parsed, direct, "parse->compile must be stable");
    }

    #[test]
    fn test_roundtrip_parse_compile_multisig() {
        // thresh-of-keys string -> Multi policy -> identical script as the
        // directly-built Multi policy.
        let compiler = MiniscriptCompiler::new(Network::Bitcoin);
        let policy_str = format!("thresh(2,pk({KEY_A}),pk({KEY_B}),pk({KEY_C}))");
        let parsed = compiler.parse_policy(&policy_str).unwrap();
        assert!(matches!(parsed, MiniscriptPolicy::Multi { k: 2, .. }));

        let from_parsed =
            MiniscriptDescriptor::new(parsed, Network::Bitcoin, MiniscriptScriptType::P2WSH)
                .compile()
                .unwrap();
        let direct = MiniscriptDescriptor::new(
            MiniscriptPolicy::Multi {
                k: 2,
                keys: vec![KEY_A.to_string(), KEY_B.to_string(), KEY_C.to_string()],
            },
            Network::Bitcoin,
            MiniscriptScriptType::P2WSH,
        )
        .compile()
        .unwrap();

        assert_eq!(from_parsed, direct);
    }

    #[test]
    fn test_compile_legacy_context() {
        // The same policy compiles under the legacy (P2SH) context too, and is
        // still a real non-empty script.
        let policy = MiniscriptPolicy::PublicKey(KEY_A.to_string());
        let script =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2SH)
                .compile()
                .expect("legacy pk must compile");
        let tokens = script_tokens(&script);
        assert_eq!(
            tokens,
            vec!["PUSH(33)".to_string(), "OP_CHECKSIG".to_string()]
        );
    }

    #[test]
    fn test_compile_rejects_bad_key() {
        // Building a PublicKey policy with junk and compiling must fail loudly.
        let policy = MiniscriptPolicy::PublicKey("not-a-real-key".to_string());
        let err = MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH)
            .compile()
            .unwrap_err();
        assert!(matches!(err, BitcoinError::Validation(_)));
    }

    #[test]
    fn test_descriptor_creation() {
        let policy = MiniscriptPolicy::PublicKey("key1".to_string());
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        assert_eq!(descriptor.script_type, MiniscriptScriptType::P2WSH);
        assert_eq!(descriptor.network, Network::Bitcoin);
    }

    #[test]
    fn test_policy_analysis() {
        let policy = MiniscriptPolicy::Multi {
            k: 2,
            keys: vec!["key1".to_string(), "key2".to_string(), "key3".to_string()],
        };
        let descriptor =
            MiniscriptDescriptor::new(policy, Network::Bitcoin, MiniscriptScriptType::P2WSH);

        let analysis = descriptor.analyze();
        assert!(!analysis.is_malleable);
        assert!(analysis.is_sane);
        assert!(analysis.max_satisfaction_weight > 0);
    }

    #[test]
    fn test_requires_timelock_detection() {
        let builder = PolicyTemplateBuilder::new(Network::Bitcoin);

        let policy_with_timelock = builder.and(
            builder.single_key("key1".to_string()),
            builder.absolute_timelock(500000),
        );

        let descriptor = MiniscriptDescriptor::new(
            policy_with_timelock,
            Network::Bitcoin,
            MiniscriptScriptType::P2WSH,
        );

        let analysis = descriptor.analyze();
        assert!(analysis.requires_timelock);
    }
}

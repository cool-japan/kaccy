//! BIP 380-386 Output Descriptor parsing, validation, and analysis.
//!
//! Output descriptors describe how to derive Bitcoin scripts and addresses.
//! This module provides a parser, validator, and utilities for working with
//! the full range of output descriptor types defined by BIPs 380-386.
//!
//! # Supported Descriptor Types
//! - `pk(KEY)` — bare pubkey (BIP 380)
//! - `pkh(KEY)` — P2PKH (BIP 382)
//! - `wpkh(KEY)` — P2WPKH (BIP 381/382)
//! - `sh(SCRIPT)` — P2SH wrapper (BIP 383)
//! - `wsh(SCRIPT)` — P2WSH wrapper (BIP 381)
//! - `combo(KEY)` — multiple script types from one key (BIP 384)
//! - `addr(ADDR)` — raw address (BIP 385)
//! - `raw(HEX)` — raw script (BIP 385)
//! - `tr(KEY)` and `tr(KEY, TREE)` — Taproot (BIP 386)
//! - `multi(K, KEY...)` and `sortedmulti(K, KEY...)` — multisig (BIP 383)
//!
//! # Example
//!
//! ```rust
//! use kaccy_bitcoin::output_descriptor::DescriptorParser;
//!
//! let desc = "pkh(02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5)";
//! let parsed = DescriptorParser::parse(desc).unwrap();
//! assert_eq!(parsed.descriptor_type(), "pkh");
//! assert!(!parsed.is_taproot());
//! ```

/// Error types for output descriptor parsing and validation.
#[derive(Debug, thiserror::Error)]
pub enum DescriptorError {
    /// Descriptor has invalid syntax.
    #[error("Invalid descriptor syntax: {0}")]
    InvalidSyntax(String),
    /// Unknown or unsupported descriptor function type.
    #[error("Unknown descriptor type: {0}")]
    UnknownType(String),
    /// Key expression is malformed or invalid.
    #[error("Invalid key expression: {0}")]
    InvalidKey(String),
    /// Multisig threshold exceeds the number of keys.
    #[error("Invalid threshold: got {got}, max {max}")]
    InvalidThreshold {
        /// The threshold value specified in the descriptor.
        got: usize,
        /// The maximum allowed threshold (equal to the number of keys).
        max: usize,
    },
    /// Descriptor is missing its trailing `#checksum`.
    #[error("Missing checksum")]
    MissingChecksum,
    /// Descriptor checksum does not match the computed value.
    #[error("Invalid checksum: expected {expected}, got {got}")]
    InvalidChecksum {
        /// The checksum appended to the descriptor string.
        expected: String,
        /// The checksum computed from the descriptor body.
        got: String,
    },
    /// Descriptor nesting depth exceeds the limit (8).
    #[error("Nested depth exceeded: {0}")]
    DepthExceeded(usize),
    /// Address string inside `addr()` is not valid.
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    /// Empty input string.
    #[error("Empty descriptor")]
    Empty,
}

// ─── BIP 380 checksum ────────────────────────────────────────────────────────

/// Input charset used by the BIP 380 descriptor checksum algorithm (95 chars).
const INPUT_CHARSET: &str =
    "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH#`";

/// Output charset for the 8-character checksum (32 chars, BCH-like).
const CHECKSUM_CHARSET: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// GF(32) polynomial step used in the BIP 380 polymod calculation.
///
/// Matches Bitcoin Core `DescriptorChecksum`'s generator polynomial.
fn descriptor_polymod(mut c: u64, val: u64) -> u64 {
    let c0 = c >> 35;
    c = ((c & 0x7_ffff_ffff) << 5) ^ val;
    if c0 & 1 != 0 {
        c ^= 0xf5dee51989;
    }
    if c0 & 2 != 0 {
        c ^= 0xa9fdca3312;
    }
    if c0 & 4 != 0 {
        c ^= 0x1bab10e32d;
    }
    if c0 & 8 != 0 {
        c ^= 0x3706b1677a;
    }
    if c0 & 16 != 0 {
        c ^= 0x644d626ffd;
    }
    c
}

// ─── Key types ───────────────────────────────────────────────────────────────

/// Classification of the key material inside a descriptor key expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorKeyType {
    /// Raw compressed public key (66 hex characters).
    RawPubkey,
    /// Extended public key with version bytes (xpub/tpub/ypub/zpub …).
    ExtendedPubkey,
    /// Extended private key with version bytes (xprv/tprv …).
    ExtendedPrivkey,
    /// WIF-encoded private key.
    Wif,
}

/// A single key expression in an output descriptor.
///
/// Covers bare keys, xpubs with optional origin and child paths, and
/// wildcard (ranged) derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorKey {
    /// The raw key token as it appears in the descriptor (after origin prefix).
    pub key_str: String,
    /// How to interpret `key_str`.
    pub key_type: DescriptorKeyType,
    /// 4-byte master fingerprint in hex (8 chars) when an origin is present.
    pub fingerprint: Option<String>,
    /// Derivation path from the master key to the xpub/xprv (origin path).
    pub origin_path: Option<String>,
    /// Child derivation path appended after the key (e.g. `/0/*`).
    pub child_path: Option<String>,
    /// Whether the child path contains a `*` wildcard.
    pub is_ranged: bool,
    /// Whether the wildcard is a hardened range (`*h` / `*'`).
    pub is_hardened_range: bool,
}

impl DescriptorKey {
    /// Parse a key expression string, which may include an origin prefix
    /// `[fingerprint/path]` and a child path suffix `/0/*`.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::InvalidKey`] when the string is malformed.
    pub fn parse(s: &str) -> Result<Self, DescriptorError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(DescriptorError::InvalidKey("empty key expression".into()));
        }

        let mut fingerprint: Option<String> = None;
        let mut origin_path: Option<String> = None;
        let remainder: &str;

        // Consume optional origin: `[fingerprint/path]`
        if s.starts_with('[') {
            let close = s
                .find(']')
                .ok_or_else(|| DescriptorError::InvalidKey("unclosed '[' in origin".into()))?;
            let origin_inner = &s[1..close];
            remainder = &s[close + 1..];

            // origin_inner is "fingerprint" or "fingerprint/path"
            let slash_pos = origin_inner.find('/');
            if let Some(pos) = slash_pos {
                fingerprint = Some(origin_inner[..pos].to_string());
                origin_path = Some(origin_inner[pos + 1..].to_string());
            } else {
                fingerprint = Some(origin_inner.to_string());
            }
        } else {
            remainder = s;
        }

        // Split remainder into key token and optional child path
        // The key token is everything up to the first '/' that comes after
        // the base58-like key body.  We locate the child path boundary.
        let (key_token, child_path_str) = split_key_and_child_path(remainder);

        let child_path = if child_path_str.is_empty() {
            None
        } else {
            Some(child_path_str.to_string())
        };

        let is_ranged = child_path
            .as_deref()
            .map(|p| p.contains('*'))
            .unwrap_or(false);
        let is_hardened_range = child_path
            .as_deref()
            .map(|p| p.contains("*h") || p.contains("*'"))
            .unwrap_or(false);

        let key_type = classify_key(key_token)?;

        Ok(DescriptorKey {
            key_str: key_token.to_string(),
            key_type,
            fingerprint,
            origin_path,
            child_path,
            is_ranged,
            is_hardened_range,
        })
    }

    /// Reconstruct the original string representation of this key expression.
    pub fn to_string_repr(&self) -> String {
        let mut out = String::new();
        if let (Some(fp), Some(op)) = (&self.fingerprint, &self.origin_path) {
            out.push_str(&format!("[{}/{}]", fp, op));
        } else if let Some(fp) = &self.fingerprint {
            out.push_str(&format!("[{}]", fp));
        }
        out.push_str(&self.key_str);
        if let Some(cp) = &self.child_path {
            out.push('/');
            out.push_str(cp);
        }
        out
    }

    /// Returns `true` if this is an extended public key (xpub family).
    pub fn is_xpub(&self) -> bool {
        matches!(self.key_type, DescriptorKeyType::ExtendedPubkey)
    }
}

/// Splits a key+child-path string into `(key_token, child_path)`.
///
/// For bare pubkeys the entire string is the key token.
/// For extended keys the token ends at the start of the child path, which
/// begins at the *first* `/` after the base58 body of the key.
fn split_key_and_child_path(s: &str) -> (&str, &str) {
    // Extended keys start with 'x', 't', 'y', 'z', etc. and use base58.
    // They don't contain '/'.  If the string contains '/' it must mark
    // the beginning of the child derivation path.
    if let Some(pos) = s.find('/') {
        (&s[..pos], &s[pos + 1..])
    } else {
        (s, "")
    }
}

/// Classify a raw key token into a [`DescriptorKeyType`].
fn classify_key(token: &str) -> Result<DescriptorKeyType, DescriptorError> {
    if token.is_empty() {
        return Err(DescriptorError::InvalidKey("empty key token".into()));
    }

    // Extended private keys
    if token.starts_with("xprv")
        || token.starts_with("tprv")
        || token.starts_with("yprv")
        || token.starts_with("zprv")
    {
        return Ok(DescriptorKeyType::ExtendedPrivkey);
    }

    // Extended public keys
    if token.starts_with("xpub")
        || token.starts_with("tpub")
        || token.starts_with("ypub")
        || token.starts_with("zpub")
        || token.starts_with("Ypub")
        || token.starts_with("Zpub")
        || token.starts_with("Xpub")
        || token.starts_with("Vpub")
        || token.starts_with("Upub")
    {
        return Ok(DescriptorKeyType::ExtendedPubkey);
    }

    // Raw compressed public key: 66 hex chars starting with 02 or 03
    if (token.starts_with("02") || token.starts_with("03"))
        && token.len() == 66
        && token.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Ok(DescriptorKeyType::RawPubkey);
    }

    // WIF: starts with 5, K, or L (mainnet) or c (testnet compressed)
    if token.starts_with('5')
        || token.starts_with('K')
        || token.starts_with('L')
        || token.starts_with('c')
    {
        // Plausible WIF length range: 51-52 chars
        if token.len() >= 51 && token.len() <= 52 {
            return Ok(DescriptorKeyType::Wif);
        }
    }

    // Fall back: treat anything that looks like base58 as an extended key
    if token
        .chars()
        .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=')
    {
        return Ok(DescriptorKeyType::ExtendedPubkey);
    }

    Err(DescriptorError::InvalidKey(format!(
        "unrecognised key token: '{}'",
        token
    )))
}

// ─── Script tree ─────────────────────────────────────────────────────────────

/// A node in the Taproot script tree.
///
/// See BIP 386 for the tree notation.
#[derive(Debug, Clone)]
pub enum DescriptorTree {
    /// A leaf script with a specific script version and body.
    Leaf {
        /// Tapscript version byte (0xc0 for normal tapscript).
        version: u8,
        /// The script expression at this leaf.
        script: Box<DescriptorScript>,
    },
    /// A branch combining two sub-trees.
    Branch(Box<DescriptorTree>, Box<DescriptorTree>),
}

// ─── Descriptor script ───────────────────────────────────────────────────────

/// The parsed inner expression of an output descriptor.
///
/// This enum models all descriptor types defined in BIPs 380-386.
#[derive(Debug, Clone)]
pub enum DescriptorScript {
    /// `pk(KEY)` — bare pubkey output (BIP 380).
    Key(DescriptorKey),
    /// `pkh(KEY)` — P2PKH (BIP 382).
    Pkh(DescriptorKey),
    /// `wpkh(KEY)` — P2WPKH (BIP 381/382).
    Wpkh(DescriptorKey),
    /// `sh(SCRIPT)` — P2SH wrapper (BIP 383).
    Sh(Box<DescriptorScript>),
    /// `wsh(SCRIPT)` — P2WSH wrapper (BIP 381).
    Wsh(Box<DescriptorScript>),
    /// `combo(KEY)` — pk + pkh + if-segwit wpkh + p2sh-wpkh (BIP 384).
    Combo(DescriptorKey),
    /// `multi(K, KEY…)` — bare multisig (BIP 383).
    Multi {
        /// The required-signature threshold K.
        threshold: usize,
        /// The N keys.
        keys: Vec<DescriptorKey>,
    },
    /// `sortedmulti(K, KEY…)` — multisig with sorted keys (BIP 383).
    SortedMulti {
        /// The required-signature threshold K.
        threshold: usize,
        /// The N keys (sorted at script generation time).
        keys: Vec<DescriptorKey>,
    },
    /// `addr(ADDRESS)` — raw address descriptor (BIP 385).
    Addr(String),
    /// `raw(HEX)` — raw script hex (BIP 385).
    Raw(String),
    /// `tr(KEY)` or `tr(KEY, TREE)` — Taproot (BIP 386).
    Tr {
        /// Taproot internal key.
        internal_key: DescriptorKey,
        /// Optional script-path tree.
        tree: Option<Box<DescriptorTree>>,
    },
}

impl DescriptorScript {
    /// Return a short string naming the top-level function.
    pub fn type_name(&self) -> &str {
        match self {
            DescriptorScript::Key(_) => "pk",
            DescriptorScript::Pkh(_) => "pkh",
            DescriptorScript::Wpkh(_) => "wpkh",
            DescriptorScript::Sh(_) => "sh",
            DescriptorScript::Wsh(_) => "wsh",
            DescriptorScript::Combo(_) => "combo",
            DescriptorScript::Multi { .. } => "multi",
            DescriptorScript::SortedMulti { .. } => "sortedmulti",
            DescriptorScript::Addr(_) => "addr",
            DescriptorScript::Raw(_) => "raw",
            DescriptorScript::Tr { .. } => "tr",
        }
    }

    /// Collect all [`DescriptorKey`] references in this script recursively.
    pub fn keys(&self) -> Vec<&DescriptorKey> {
        match self {
            DescriptorScript::Key(k)
            | DescriptorScript::Pkh(k)
            | DescriptorScript::Wpkh(k)
            | DescriptorScript::Combo(k) => vec![k],
            DescriptorScript::Sh(inner) | DescriptorScript::Wsh(inner) => inner.keys(),
            DescriptorScript::Multi { keys, .. } | DescriptorScript::SortedMulti { keys, .. } => {
                keys.iter().collect()
            }
            DescriptorScript::Addr(_) | DescriptorScript::Raw(_) => vec![],
            DescriptorScript::Tr { internal_key, tree } => {
                let mut v = vec![internal_key];
                if let Some(t) = tree {
                    v.extend(collect_tree_keys(t));
                }
                v
            }
        }
    }

    /// Returns `true` when at least one key path in this script contains a
    /// derivation wildcard (`*`).
    pub fn is_ranged(&self) -> bool {
        self.keys().iter().any(|k| k.is_ranged)
    }
}

/// Recursively collect all keys from a [`DescriptorTree`].
fn collect_tree_keys(tree: &DescriptorTree) -> Vec<&DescriptorKey> {
    match tree {
        DescriptorTree::Leaf { script, .. } => script.keys(),
        DescriptorTree::Branch(left, right) => {
            let mut v = collect_tree_keys(left);
            v.extend(collect_tree_keys(right));
            v
        }
    }
}

// ─── Parsed descriptor ───────────────────────────────────────────────────────

/// A fully parsed output descriptor, including optional checksum.
///
/// Obtain one via [`DescriptorParser::parse`] or the convenience
/// `ParsedDescriptor::parse` method.
#[derive(Debug, Clone)]
pub struct ParsedDescriptor {
    /// The original descriptor string (with checksum if provided).
    pub raw: String,
    /// The parsed script expression.
    pub script: DescriptorScript,
    /// The 8-character checksum following `#`, if present.
    pub checksum: Option<String>,
    /// Whether any key in the descriptor is ranged (contains `*`).
    pub is_ranged: bool,
}

impl ParsedDescriptor {
    /// Parse a descriptor string.
    ///
    /// # Errors
    ///
    /// Returns a [`DescriptorError`] when the input is malformed.
    pub fn parse(s: &str) -> Result<Self, DescriptorError> {
        DescriptorParser::parse(s)
    }

    /// Return the top-level descriptor function name.
    ///
    /// Examples: `"pkh"`, `"wpkh"`, `"tr"`, `"multi"`.
    pub fn descriptor_type(&self) -> &str {
        self.script.type_name()
    }

    /// Count the total number of keys referenced in this descriptor.
    pub fn key_count(&self) -> usize {
        self.extract_keys().len()
    }

    /// Returns `true` if this is a Taproot (`tr(…)`) descriptor.
    pub fn is_taproot(&self) -> bool {
        matches!(self.script, DescriptorScript::Tr { .. })
    }

    /// Returns `true` if this descriptor produces a SegWit output
    /// (wpkh, wsh, or tr).
    pub fn is_segwit(&self) -> bool {
        matches!(
            self.script,
            DescriptorScript::Wpkh(_) | DescriptorScript::Wsh(_) | DescriptorScript::Tr { .. }
        )
    }

    /// Returns `true` if this is a multisig descriptor (multi or sortedmulti).
    pub fn is_multisig(&self) -> bool {
        matches!(
            self.script,
            DescriptorScript::Multi { .. } | DescriptorScript::SortedMulti { .. }
        )
    }

    /// Return the threshold K for multisig descriptors, or `None` otherwise.
    pub fn threshold(&self) -> Option<usize> {
        match &self.script {
            DescriptorScript::Multi { threshold, .. }
            | DescriptorScript::SortedMulti { threshold, .. } => Some(*threshold),
            _ => None,
        }
    }

    /// Whether this descriptor uses derivation wildcards.
    pub fn is_ranged(&self) -> bool {
        self.is_ranged
    }

    /// Collect references to all [`DescriptorKey`]s in this descriptor.
    pub fn extract_keys(&self) -> Vec<&DescriptorKey> {
        self.script.keys()
    }
}

// ─── Parser ──────────────────────────────────────────────────────────────────

/// A stateless parser for Bitcoin output descriptors (BIPs 380-386).
///
/// Entry points:
/// - [`DescriptorParser::parse`] — parse a descriptor string.
/// - [`DescriptorParser::validate_checksum`] — verify the `#checksum` suffix.
/// - [`DescriptorParser::compute_checksum`] — compute the BIP 380 checksum.
/// - [`DescriptorParser::strip_checksum`] — remove `#checksum` from a string.
pub struct DescriptorParser;

impl DescriptorParser {
    /// Parse a descriptor string, with or without a trailing `#checksum`.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::Empty`] for an empty input, or various
    /// parse errors for malformed input.
    pub fn parse(descriptor: &str) -> Result<ParsedDescriptor, DescriptorError> {
        let descriptor = descriptor.trim();
        if descriptor.is_empty() {
            return Err(DescriptorError::Empty);
        }

        let (desc_part, checksum) = Self::split_checksum(descriptor);

        let script = parse_script(desc_part, 0)?;
        let is_ranged = script.is_ranged();

        Ok(ParsedDescriptor {
            raw: descriptor.to_string(),
            script,
            checksum: checksum.map(|s| s.to_string()),
            is_ranged,
        })
    }

    /// Validate the 8-character `#checksum` appended to a descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::MissingChecksum`] when no `#` is present,
    /// or [`DescriptorError::InvalidChecksum`] when the checksum is wrong.
    pub fn validate_checksum(descriptor: &str) -> Result<(), DescriptorError> {
        let (desc_part, checksum) = Self::split_checksum(descriptor);
        let checksum = checksum.ok_or(DescriptorError::MissingChecksum)?;
        let computed = Self::compute_checksum(desc_part);
        if computed == checksum {
            Ok(())
        } else {
            Err(DescriptorError::InvalidChecksum {
                expected: checksum.to_string(),
                got: computed,
            })
        }
    }

    /// Return the descriptor string with any `#checksum` suffix removed.
    pub fn strip_checksum(descriptor: &str) -> &str {
        let (desc_part, _) = Self::split_checksum(descriptor);
        desc_part
    }

    /// Compute the 8-character BIP 380 descriptor checksum.
    ///
    /// The algorithm is a BCH code over GF(32) using the
    /// `INPUT_CHARSET` / `CHECKSUM_CHARSET` defined by BIP 380.
    pub fn compute_checksum(descriptor: &str) -> String {
        let input_charset_chars: Vec<char> = INPUT_CHARSET.chars().collect();
        let checksum_chars: Vec<char> = CHECKSUM_CHARSET.chars().collect();

        let mut c: u64 = 1;
        let mut cls: u64 = 0;
        let mut clscount: u32 = 0;

        for ch in descriptor.chars() {
            let pos = input_charset_chars.iter().position(|&x| x == ch);
            let pos = match pos {
                Some(p) => p as u64,
                // Characters not in INPUT_CHARSET produce a non-matching
                // checksum; feed a sentinel value that pollutes the polymod.
                None => {
                    c = descriptor_polymod(c, 0x7f);
                    continue;
                }
            };
            // Feed low 5 bits of the class-3 group
            c = descriptor_polymod(c, pos & 31);
            cls = cls * 3 + (pos >> 5);
            clscount += 1;
            if clscount == 3 {
                c = descriptor_polymod(c, cls);
                cls = 0;
                clscount = 0;
            }
        }
        // Flush any remaining class bits
        if clscount > 0 {
            c = descriptor_polymod(c, cls);
        }
        // 8 rounds of polymod(0) to "finalise"
        for _ in 0..8 {
            c = descriptor_polymod(c, 0);
        }
        c ^= 1;

        // Extract 8 × 5-bit groups from the 40-bit result
        let mut result = String::with_capacity(8);
        for i in (0..8).rev() {
            let idx = ((c >> (5 * i)) & 31) as usize;
            result.push(checksum_chars[idx]);
        }
        result
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Split a descriptor into `(body, Option<checksum>)`.
    fn split_checksum(s: &str) -> (&str, Option<&str>) {
        if let Some(pos) = s.rfind('#') {
            (&s[..pos], Some(&s[pos + 1..]))
        } else {
            (s, None)
        }
    }
}

// ─── Recursive-descent parser internals ──────────────────────────────────────

/// Maximum nesting depth (prevents stack overflow on adversarial input).
const MAX_DEPTH: usize = 8;

/// Parse a descriptor expression starting at depth `depth`.
///
/// `s` should be the part of the descriptor *without* the `#checksum` suffix.
fn parse_script(s: &str, depth: usize) -> Result<DescriptorScript, DescriptorError> {
    if depth > MAX_DEPTH {
        return Err(DescriptorError::DepthExceeded(depth));
    }
    let s = s.trim();
    if s.is_empty() {
        return Err(DescriptorError::InvalidSyntax("empty expression".into()));
    }

    // Find the opening parenthesis to separate function name from argument.
    let paren_open = s.find('(').ok_or_else(|| {
        // Could be just a key expression at the top level — but the BIP
        // requires a function wrapper at the top level.
        DescriptorError::InvalidSyntax(format!("expected '(' in '{}'", s))
    })?;

    let func_name = &s[..paren_open];

    // Verify the string ends with ')'
    if !s.ends_with(')') {
        return Err(DescriptorError::InvalidSyntax(format!(
            "missing closing ')' in '{}'",
            s
        )));
    }
    let inner = &s[paren_open + 1..s.len() - 1];

    match func_name {
        "pk" => {
            let key = DescriptorKey::parse(inner)?;
            Ok(DescriptorScript::Key(key))
        }
        "pkh" => {
            let key = DescriptorKey::parse(inner)?;
            Ok(DescriptorScript::Pkh(key))
        }
        "wpkh" => {
            let key = DescriptorKey::parse(inner)?;
            Ok(DescriptorScript::Wpkh(key))
        }
        "combo" => {
            let key = DescriptorKey::parse(inner)?;
            Ok(DescriptorScript::Combo(key))
        }
        "sh" => {
            let inner_script = parse_script(inner, depth + 1)?;
            Ok(DescriptorScript::Sh(Box::new(inner_script)))
        }
        "wsh" => {
            let inner_script = parse_script(inner, depth + 1)?;
            Ok(DescriptorScript::Wsh(Box::new(inner_script)))
        }
        "addr" => {
            if inner.is_empty() {
                return Err(DescriptorError::InvalidAddress("empty address".into()));
            }
            Ok(DescriptorScript::Addr(inner.to_string()))
        }
        "raw" => Ok(DescriptorScript::Raw(inner.to_string())),
        "multi" | "sortedmulti" => parse_multisig(inner, func_name == "sortedmulti"),
        "tr" => parse_taproot(inner, depth),
        other => Err(DescriptorError::UnknownType(other.to_string())),
    }
}

/// Parse `K,KEY1,KEY2,...` from the body of a multi/sortedmulti expression.
fn parse_multisig(inner: &str, sorted: bool) -> Result<DescriptorScript, DescriptorError> {
    // Split on top-level commas (no nesting inside multisig args)
    let parts: Vec<&str> = split_top_level_commas(inner);
    if parts.is_empty() {
        return Err(DescriptorError::InvalidSyntax(
            "empty multisig arguments".into(),
        ));
    }

    let threshold: usize = parts[0]
        .trim()
        .parse()
        .map_err(|_| DescriptorError::InvalidSyntax(format!("invalid threshold '{}'", parts[0])))?;

    let keys_raw = &parts[1..];
    if threshold > keys_raw.len() {
        return Err(DescriptorError::InvalidThreshold {
            got: threshold,
            max: keys_raw.len(),
        });
    }

    let mut keys = Vec::with_capacity(keys_raw.len());
    for k in keys_raw {
        keys.push(DescriptorKey::parse(k.trim())?);
    }

    if sorted {
        Ok(DescriptorScript::SortedMulti { threshold, keys })
    } else {
        Ok(DescriptorScript::Multi { threshold, keys })
    }
}

/// Parse `KEY` or `KEY,TREE` from the body of a `tr(…)` expression.
fn parse_taproot(inner: &str, depth: usize) -> Result<DescriptorScript, DescriptorError> {
    // Find the first top-level comma that separates internal key from tree.
    let comma_pos = find_top_level_comma(inner);
    let (key_str, tree_str) = if let Some(pos) = comma_pos {
        (&inner[..pos], Some(&inner[pos + 1..]))
    } else {
        (inner, None)
    };

    let internal_key = DescriptorKey::parse(key_str.trim())?;
    let tree = if let Some(tree_s) = tree_str {
        Some(Box::new(parse_tree(tree_s.trim(), depth + 1)?))
    } else {
        None
    };

    Ok(DescriptorScript::Tr { internal_key, tree })
}

/// Parse a Taproot script tree node: either `{LEFT,RIGHT}` or a script.
fn parse_tree(s: &str, depth: usize) -> Result<DescriptorTree, DescriptorError> {
    if depth > MAX_DEPTH {
        return Err(DescriptorError::DepthExceeded(depth));
    }
    let s = s.trim();
    if s.starts_with('{') && s.ends_with('}') {
        let inner = &s[1..s.len() - 1];
        let mid = find_top_level_comma(inner).ok_or_else(|| {
            DescriptorError::InvalidSyntax("tree branch requires two children".into())
        })?;
        let left = parse_tree(inner[..mid].trim(), depth + 1)?;
        let right = parse_tree(inner[mid + 1..].trim(), depth + 1)?;
        Ok(DescriptorTree::Branch(Box::new(left), Box::new(right)))
    } else {
        // Treat as a leaf script expression (default version 0xc0)
        let script = parse_script(s, depth + 1)?;
        Ok(DescriptorTree::Leaf {
            version: 0xc0,
            script: Box::new(script),
        })
    }
}

// ─── Comma-splitting helpers ──────────────────────────────────────────────────

/// Split `s` on commas that are at nesting depth 0 (not inside parentheses or
/// braces).
fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Find the byte position of the first top-level comma in `s`.
fn find_top_level_comma(s: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Compressed public keys used across tests
    const PK1: &str = "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    const PK2: &str = "02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
    const PK3: &str = "03a34b99f22c790c4e36b2b3c2c35a36db06226e41c692fc82b8b56ac1c540c5bd";

    #[test]
    fn test_parse_pkh() {
        let desc = format!("pkh({})", PK1);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse pkh");
        assert_eq!(parsed.descriptor_type(), "pkh");
        assert_eq!(parsed.key_count(), 1);
        assert!(!parsed.is_taproot());
        assert!(!parsed.is_multisig());
        assert!(!parsed.is_segwit());
    }

    #[test]
    fn test_parse_wpkh() {
        let desc = format!("wpkh({})", PK2);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse wpkh");
        assert_eq!(parsed.descriptor_type(), "wpkh");
        assert!(parsed.is_segwit());
        assert!(!parsed.is_taproot());
        assert_eq!(parsed.key_count(), 1);
    }

    #[test]
    fn test_parse_sh_wpkh() {
        let desc = format!("sh(wpkh({}))", PK2);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse sh(wpkh)");
        assert_eq!(parsed.descriptor_type(), "sh");
        assert!(!parsed.is_segwit());
        assert_eq!(parsed.key_count(), 1);
    }

    #[test]
    fn test_parse_multisig() {
        let desc = format!("multi(2,{},{},{})", PK1, PK2, PK3);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse multi");
        assert_eq!(parsed.descriptor_type(), "multi");
        assert!(parsed.is_multisig());
        assert_eq!(parsed.threshold(), Some(2));
        assert_eq!(parsed.key_count(), 3);
    }

    #[test]
    fn test_parse_sortedmulti() {
        let desc = format!("sortedmulti(1,{},{})", PK1, PK2);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse sortedmulti");
        assert_eq!(parsed.descriptor_type(), "sortedmulti");
        assert!(parsed.is_multisig());
        assert_eq!(parsed.threshold(), Some(1));
        assert_eq!(parsed.key_count(), 2);
    }

    #[test]
    fn test_parse_addr() {
        let desc = "addr(bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4)";
        let parsed = ParsedDescriptor::parse(desc).expect("should parse addr");
        assert_eq!(parsed.descriptor_type(), "addr");
        assert_eq!(parsed.key_count(), 0);
        assert!(!parsed.is_taproot());
        assert!(!parsed.is_multisig());
    }

    #[test]
    fn test_parse_taproot_simple() {
        let desc = format!("tr({})", PK1);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse tr");
        assert_eq!(parsed.descriptor_type(), "tr");
        assert!(parsed.is_taproot());
        assert!(parsed.is_segwit());
        assert_eq!(parsed.key_count(), 1);
    }

    #[test]
    fn test_descriptor_type_name() {
        let cases = vec![
            (format!("pk({})", PK1), "pk"),
            (format!("wpkh({})", PK1), "wpkh"),
            ("raw(deadbeef)".to_string(), "raw"),
            (
                "addr(1BitcoinEaterAddressDontSendf59kuE)".to_string(),
                "addr",
            ),
        ];
        for (desc, expected) in cases {
            let parsed = ParsedDescriptor::parse(&desc)
                .unwrap_or_else(|e| panic!("failed to parse '{}': {:?}", desc, e));
            assert_eq!(parsed.descriptor_type(), expected);
        }
    }

    #[test]
    fn test_is_taproot() {
        let tr_desc = format!("tr({})", PK1);
        let pkh_desc = format!("pkh({})", PK1);
        assert!(ParsedDescriptor::parse(&tr_desc).unwrap().is_taproot());
        assert!(!ParsedDescriptor::parse(&pkh_desc).unwrap().is_taproot());
    }

    #[test]
    fn test_is_multisig() {
        let multi = format!("multi(2,{},{})", PK1, PK2);
        let sorted = format!("sortedmulti(1,{},{})", PK1, PK2);
        let single = format!("pkh({})", PK1);
        assert!(ParsedDescriptor::parse(&multi).unwrap().is_multisig());
        assert!(ParsedDescriptor::parse(&sorted).unwrap().is_multisig());
        assert!(!ParsedDescriptor::parse(&single).unwrap().is_multisig());
    }

    #[test]
    fn test_key_count_multi() {
        let desc = format!("multi(2,{},{},{})", PK1, PK2, PK3);
        let parsed = ParsedDescriptor::parse(&desc).unwrap();
        assert_eq!(parsed.key_count(), 3);
    }

    #[test]
    fn test_strip_checksum() {
        let with_cs = "wpkh(xpub6...)#12345678";
        assert_eq!(DescriptorParser::strip_checksum(with_cs), "wpkh(xpub6...)");

        let without_cs = "wpkh(xpub6...)";
        assert_eq!(
            DescriptorParser::strip_checksum(without_cs),
            "wpkh(xpub6...)"
        );
    }

    #[test]
    fn test_compute_checksum_length() {
        let desc = format!("pkh({})", PK1);
        let cs = DescriptorParser::compute_checksum(&desc);
        assert_eq!(cs.len(), 8, "BIP 380 checksum must be exactly 8 chars");
        // All characters must be from CHECKSUM_CHARSET
        for ch in cs.chars() {
            assert!(
                CHECKSUM_CHARSET.contains(ch),
                "checksum char '{}' not in CHECKSUM_CHARSET",
                ch
            );
        }
    }

    #[test]
    fn test_parse_ranged_descriptor() {
        // Descriptor with a wildcard child path
        let desc = "wpkh(xpub661MyMwAqRbcGHoJePhy7S4JdFEFXwg/0/*)";
        let parsed = ParsedDescriptor::parse(desc).expect("should parse ranged");
        assert!(parsed.is_ranged(), "descriptor should be ranged");
    }

    #[test]
    fn test_parse_empty_fails() {
        let result = ParsedDescriptor::parse("");
        assert!(matches!(result, Err(DescriptorError::Empty)));
    }

    #[test]
    fn test_parse_unknown_type_fails() {
        let result = ParsedDescriptor::parse("foo(bar)");
        assert!(matches!(result, Err(DescriptorError::UnknownType(_))));
    }

    #[test]
    fn test_multisig_threshold_too_high() {
        let desc = format!("multi(3,{},{})", PK1, PK2); // 3-of-2 is invalid
        let result = ParsedDescriptor::parse(&desc);
        assert!(matches!(
            result,
            Err(DescriptorError::InvalidThreshold { .. })
        ));
    }

    #[test]
    fn test_parse_wsh_multi() {
        let desc = format!("wsh(multi(2,{},{}))", PK1, PK2);
        let parsed = ParsedDescriptor::parse(&desc).expect("should parse wsh(multi)");
        assert_eq!(parsed.descriptor_type(), "wsh");
        assert!(parsed.is_segwit());
        // key_count walks into the inner multi
        assert_eq!(parsed.key_count(), 2);
    }

    #[test]
    fn test_parse_raw() {
        let desc = "raw(76a91489abcdefabbaabbaabbaabbaabbaabbaabbaabba88ac)";
        let parsed = ParsedDescriptor::parse(desc).expect("should parse raw");
        assert_eq!(parsed.descriptor_type(), "raw");
        assert_eq!(parsed.key_count(), 0);
    }

    #[test]
    fn test_descriptor_key_parse_with_origin() {
        let key_str = format!("[deadbeef/84'/0'/0']{}/0/*", PK1);
        let key = DescriptorKey::parse(&key_str).expect("should parse key with origin");
        assert_eq!(key.fingerprint.as_deref(), Some("deadbeef"));
        assert!(key.is_ranged);
        assert!(!key.is_hardened_range);
    }

    #[test]
    fn test_descriptor_key_is_xpub() {
        let raw_key = DescriptorKey::parse(PK1).unwrap();
        assert!(!raw_key.is_xpub());

        let xpub = "xpub661MyMwAqRbcGHoJePhy7S4JdFEFXwg";
        let xpub_key = DescriptorKey::parse(xpub).unwrap();
        assert!(xpub_key.is_xpub());
    }

    #[test]
    fn test_validate_checksum_missing() {
        let desc = format!("pkh({})", PK1);
        let result = DescriptorParser::validate_checksum(&desc);
        assert!(matches!(result, Err(DescriptorError::MissingChecksum)));
    }

    #[test]
    fn test_validate_checksum_correct() {
        // Generate a checksum and then validate it
        let desc = format!("pkh({})", PK1);
        let cs = DescriptorParser::compute_checksum(&desc);
        let full = format!("{}#{}", desc, cs);
        DescriptorParser::validate_checksum(&full).expect("checksum should validate");
    }

    #[test]
    fn test_validate_checksum_wrong() {
        let desc = format!("pkh({})", PK1);
        let full = format!("{}#xxxxxxxx", desc);
        let result = DescriptorParser::validate_checksum(&full);
        assert!(matches!(
            result,
            Err(DescriptorError::InvalidChecksum { .. })
        ));
    }
}

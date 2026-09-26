//! Finite first-order model certificates.
//!
//! A [`ModelCertificate`] is pure data: a finite domain, a complete
//! interpretation of every symbol the problem uses, the equality semantics, and
//! a deterministic digest. It lives in `mrs-core` so that a *producer* (the
//! search's certified saturation paths) and a *checker* (the strict kernel) can
//! speak the same format without either depending on the other's crate: the
//! kernel's trust boundary is that it validates this structure itself, and it
//! keeps doing so in [`mrs_proof_kernel::model`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Maximum aggregate number of dense interpretation entries accepted in a
/// finite-model certificate. Shared by producers and the independent kernel
/// so neither side constructs or scans oversized tables.
pub const MAX_MODEL_TABLE_ENTRIES: usize = 4_000_000;

/// Equality semantics required for model evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EqualitySemantics {
    /// Strict identity: `d1 = d2` iff `d1 == d2`.
    StrictIdentity,
}

/// A complete function interpretation table over domain `0..domain_size - 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionTable {
    pub arity: usize,
    /// Flat row-major table of length `domain_size^arity`.
    pub table: Vec<usize>,
}

/// A complete predicate interpretation table over domain `0..domain_size - 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateTable {
    pub arity: usize,
    /// Flat row-major table of length `domain_size^arity`.
    pub table: Vec<bool>,
}

/// A finite first-order model certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCertificate {
    /// Size of the finite universe `D = {0, ..., domain_size - 1}`.
    pub domain_size: usize,
    /// Interpretation of individual constant symbols.
    pub constants: BTreeMap<String, usize>,
    /// Interpretation of function symbols (if non-nullary functions are present).
    #[serde(default)]
    pub functions: BTreeMap<String, FunctionTable>,
    /// Interpretation of predicate symbols.
    pub predicates: BTreeMap<String, PredicateTable>,
    /// Equality semantics.
    pub equality: EqualitySemantics,
    /// Deterministic SHA-256 digest of this model.
    pub digest: String,
}

impl ModelCertificate {
    /// Computes the deterministic SHA-256 digest of this model certificate.
    pub fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"MRS_MODEL_CERTIFICATE_V1\n");
        hasher.update(format!("domain_size={}\n", self.domain_size).as_bytes());
        for (name, val) in &self.constants {
            hasher.update(format!("const {}={}\n", name, val).as_bytes());
        }
        for (name, func) in &self.functions {
            hasher.update(format!("func {}/{}={:?}\n", name, func.arity, func.table).as_bytes());
        }
        for (name, pred) in &self.predicates {
            hasher.update(format!("pred {}/{}={:?}\n", name, pred.arity, pred.table).as_bytes());
        }
        let mut digest = String::with_capacity(64);
        for byte in hasher.finalize() {
            use std::fmt::Write as _;
            write!(&mut digest, "{byte:02x}").expect("writing to a String cannot fail");
        }
        digest
    }

    /// Attempts to extract and parse a model certificate from prover stdout text.
    pub fn extract_from_text(text: &str) -> Result<Self, String> {
        // Look for % SZS output start FiniteInterpretation / Model / ModelCertificate
        if let Some(start_idx) = text.find("% SZS output start") {
            let after_start = &text[start_idx..];
            if let Some(newline_idx) = after_start.find('\n') {
                let body = &after_start[newline_idx + 1..];
                if let Some(end_idx) = body.find("% SZS output end") {
                    let block = body[..end_idx].trim();
                    return Self::from_json_or_szs(block);
                }
            }
        }
        Self::from_json_or_szs(text.trim())
    }

    /// Parses a model certificate from either JSON or TPTP text.
    pub fn from_json_or_szs(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        // If it starts with '{', deserialize as JSON directly
        if trimmed.starts_with('{') {
            return serde_json::from_str(trimmed)
                .map_err(|e| format!("invalid model certificate JSON: {e}"));
        }

        // Otherwise look for first '{' and last '}'
        if let (Some(first_brace), Some(last_brace)) = (trimmed.find('{'), trimmed.rfind('}'))
            && first_brace < last_brace
        {
            let json_slice = &trimmed[first_brace..=last_brace];
            return serde_json::from_str(json_slice)
                .map_err(|e| format!("invalid model certificate JSON: {e}"));
        }

        Err("no model certificate JSON payload found in input".into())
    }

    /// Formats the certificate as a standard TPTP SZS output block.
    pub fn to_szs_block(&self, problem_name: &str) -> String {
        let json = serde_json::to_string_pretty(self).unwrap_or_default();
        format!(
            "% SZS output start FiniteInterpretation for {problem_name}\n\
             {json}\n\
             % SZS output end FiniteInterpretation for {problem_name}\n"
        )
    }
}

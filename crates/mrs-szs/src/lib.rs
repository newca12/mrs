//! SZS ontology status types for TPTP-based automated reasoning.
//!
//! The SZS (Satisfiability/Validity Status) ontology defines a standardized
//! set of result statuses for automated theorem provers. This crate provides
//! a type-safe representation of these statuses and formatting utilities.
//!
//! # References
//!
//! - [SZS Ontology](https://www.tptp.org/cgi-bin/SeeTPTP?Category=Documents&File=SZSOntology)

use std::fmt;

/// SZS status for a problem.
///
/// These statuses follow the SZS ontology used by the TPTP community.
/// A prover reports one of these statuses as the result of attempting
/// to solve a problem.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum SzsStatus {
    // --- Success statuses ---
    /// The conjecture is a theorem of the axioms.
    Theorem,
    /// The clause set (without conjecture) is unsatisfiable.
    Unsatisfiable,
    /// The formula set is satisfiable (no conjecture to refute).
    Satisfiable,
    /// The negation of the conjecture is satisfiable (conjecture is not a theorem).
    CounterSatisfiable,

    // --- Non-success statuses ---
    /// The prover exceeded the time limit.
    Timeout,
    /// The prover gave up without determining the status.
    GaveUp,
    /// The prover ran out of memory or other resources.
    ResourceOut,
    /// The status could not be determined.
    Unknown,
    /// An error occurred during processing.
    Error,
}

impl SzsStatus {
    /// Returns the standard SZS string for this status.
    pub fn as_str(&self) -> &'static str {
        match self {
            SzsStatus::Theorem => "Theorem",
            SzsStatus::Unsatisfiable => "Unsatisfiable",
            SzsStatus::Satisfiable => "Satisfiable",
            SzsStatus::CounterSatisfiable => "CounterSatisfiable",
            SzsStatus::Timeout => "Timeout",
            SzsStatus::GaveUp => "GaveUp",
            SzsStatus::ResourceOut => "ResourceOut",
            SzsStatus::Unknown => "Unknown",
            SzsStatus::Error => "Error",
        }
    }

    /// Returns `true` if this is a success status (problem was conclusively decided).
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            SzsStatus::Theorem
                | SzsStatus::Unsatisfiable
                | SzsStatus::Satisfiable
                | SzsStatus::CounterSatisfiable
        )
    }

    /// Parses an SZS status string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Theorem" => Some(SzsStatus::Theorem),
            "Unsatisfiable" => Some(SzsStatus::Unsatisfiable),
            "Satisfiable" => Some(SzsStatus::Satisfiable),
            "CounterSatisfiable" => Some(SzsStatus::CounterSatisfiable),
            "Timeout" => Some(SzsStatus::Timeout),
            "GaveUp" => Some(SzsStatus::GaveUp),
            "ResourceOut" => Some(SzsStatus::ResourceOut),
            "Unknown" => Some(SzsStatus::Unknown),
            "Error" => Some(SzsStatus::Error),
            _ => None,
        }
    }
}

impl fmt::Display for SzsStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Formats an SZS status line as expected by CASC and TPTP tools.
///
/// # Examples
///
/// ```
/// use mrs_szs::{SzsStatus, szs_status_line};
///
/// let line = szs_status_line(SzsStatus::Theorem, "PUZ001+1");
/// assert_eq!(line, "% SZS status Theorem for PUZ001+1");
/// ```
pub fn szs_status_line(status: SzsStatus, problem: &str) -> String {
    format!("% SZS status {} for {}", status, problem)
}

/// Formats the beginning of an SZS output section (e.g., proof, model).
pub fn szs_output_start(output_type: &str, problem: &str) -> String {
    format!("% SZS output start {} for {}", output_type, problem)
}

/// Formats the end of an SZS output section.
pub fn szs_output_end(output_type: &str, problem: &str) -> String {
    format!("% SZS output end {} for {}", output_type, problem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_display() {
        assert_eq!(SzsStatus::Theorem.to_string(), "Theorem");
        assert_eq!(SzsStatus::Timeout.to_string(), "Timeout");
    }

    #[test]
    fn status_line() {
        let line = szs_status_line(SzsStatus::Theorem, "PUZ001+1");
        assert_eq!(line, "% SZS status Theorem for PUZ001+1");
    }

    #[test]
    fn success_check() {
        assert!(SzsStatus::Theorem.is_success());
        assert!(SzsStatus::Unsatisfiable.is_success());
        assert!(!SzsStatus::Timeout.is_success());
        assert!(!SzsStatus::GaveUp.is_success());
    }

    #[test]
    fn parse_roundtrip() {
        for status in [
            SzsStatus::Theorem,
            SzsStatus::Unsatisfiable,
            SzsStatus::Satisfiable,
            SzsStatus::CounterSatisfiable,
            SzsStatus::Timeout,
            SzsStatus::GaveUp,
            SzsStatus::ResourceOut,
            SzsStatus::Unknown,
            SzsStatus::Error,
        ] {
            assert_eq!(SzsStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn output_markers() {
        assert_eq!(
            szs_output_start("Proof", "PUZ001+1"),
            "% SZS output start Proof for PUZ001+1"
        );
        assert_eq!(
            szs_output_end("Proof", "PUZ001+1"),
            "% SZS output end Proof for PUZ001+1"
        );
    }
}

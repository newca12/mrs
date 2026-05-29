//! External ATP bridge.

use std::time::Duration;

use mrs_core::{Formula, SymbolTable};

pub mod discover;
pub mod external;
pub mod ladder;

pub use discover::{find_eprover, find_mrs, find_vampire};
pub use external::{EProverAtp, MrsAtp, VampireAtp};
pub use ladder::LadderAtp;

/// Verdict returned by an ATP about a single inference step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtpVerdict {
    /// The ATP confirmed `premises ⊨ conclusion`.
    Sound,
    /// The ATP refuted the entailment (premises are consistent with ¬conclusion).
    Unsound,
    /// The ATP timed out or returned `GaveUp`/`Unknown`.
    Unknown,
}

/// Trait implemented by every ATP backend.
pub trait Atp {
    /// Identifier used in error messages.
    fn name(&self) -> &'static str;
    /// Test whether `premises ⊨ conclusion` within the given wall budget.
    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
    ) -> AtpVerdict;
}

/// An ATP that always says `Unknown`. Useful as a placeholder while the real
/// backends are being implemented.
pub struct NoopAtp;

impl Atp for NoopAtp {
    fn name(&self) -> &'static str {
        "noop"
    }
    fn check_step(
        &self,
        _symbols: &SymbolTable,
        _premises: &[Formula],
        _conclusion: &Formula,
        _budget: Duration,
    ) -> AtpVerdict {
        AtpVerdict::Unknown
    }
}

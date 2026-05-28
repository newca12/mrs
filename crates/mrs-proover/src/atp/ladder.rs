//! A ladder ATP that tries a sequence of backends in order, returning the
//! first definite verdict (`Sound` or `Unsound`).

use std::time::Duration;

use mrs_core::{Formula, SymbolTable};

use super::{Atp, AtpVerdict};

/// Try each backend in turn. Stops at the first `Sound` or `Unsound`.
/// Returns `Unknown` if all backends are inconclusive.
pub struct LadderAtp {
    pub backends: Vec<Box<dyn Atp + Sync + Send>>,
}

impl LadderAtp {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    pub fn push(mut self, b: Box<dyn Atp + Sync + Send>) -> Self {
        self.backends.push(b);
        self
    }
}

impl Default for LadderAtp {
    fn default() -> Self {
        Self::new()
    }
}

impl Atp for LadderAtp {
    fn name(&self) -> &'static str {
        "ladder"
    }
    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
    ) -> AtpVerdict {
        if self.backends.is_empty() {
            return AtpVerdict::Unknown;
        }
        // Split budget evenly across backends; each gets at least 1 second.
        let n = self.backends.len() as u32;
        let per = std::cmp::max(Duration::from_secs(1), budget / n);
        for b in &self.backends {
            match b.check_step(symbols, premises, conclusion, per) {
                AtpVerdict::Sound => return AtpVerdict::Sound,
                AtpVerdict::Unsound => return AtpVerdict::Unsound,
                AtpVerdict::Unknown => continue,
            }
        }
        AtpVerdict::Unknown
    }
}

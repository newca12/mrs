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
        // Each backend gets the *full* per-step budget. We do not pre-divide
        // it across backends because vampire/eprover need at least a few
        // hundred ms of real CPU to crack non-trivial steps, and a sub-100ms
        // share is essentially wasted call-overhead. Instead, we rely on
        // the wall-clock kill inside `run_atp` to enforce the real budget
        // and bail to the next backend on Unknown.
        //
        // We also enforce a 1-second floor per backend: empirically vampire
        // resolves most reachable steps within a second, and below that the
        // hit-rate collapses sharply. The wall-clock kill makes this floor
        // safe — the verify-loop's `remaining / steps_remaining` math will
        // self-correct on subsequent steps.
        //
        // Worst case for a hard step: total wall time ≈ n_backends × max(1s, budget).
        let per = std::cmp::max(Duration::from_secs(1), budget);
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

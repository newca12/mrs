//! # mrs-proover
//!
//! Proof verifier for TPTP/TSTP refutation proofs, targeting the
//! [ProoVer 2026](https://proover-competition.github.io/) competition.
//!
//! The library is structured as a pipeline:
//!
//! 1. [`load`] — parse the proof file and the linked problem file.
//! 2. [`dag`] — build a node table, topo sort, check the structure.
//! 3. [`lower`] — convert TPTP FOF AST into [`mrs_core`] [`Formula`]s.
//! 4. [`checks`] — run the rule-specific internal checks
//!    (negated_conjecture, skolemize, axiom-leaf).
//! 5. [`atp`] — delegate remaining steps to an external ATP.
//! 6. [`verify`] — orchestrate the loop and aggregate a verdict.
//! 7. [`verdict`] — emit the final SZS status.

pub mod atp;
pub mod checks;
pub mod dag;
pub mod load;
pub mod lower;
pub mod strict;
pub mod verdict;
pub mod verify;

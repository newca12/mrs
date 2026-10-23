//! Core logic types for the MRS automated theorem prover.
//!
//! This crate provides the foundational data types that all other MRS crates build upon:
//!
//! - [`Term`] - First-order terms (variables, function applications)
//! - [`Formula`] - First-order formulas (quantified, propositional connectives)
//! - [`Atom`] - Atomic formulas (predicates, equality)
//! - [`Literal`] - Signed atoms (positive or negative)
//! - [`Clause`] - Disjunctions of literals
//! - [`Substitution`] - Variable-to-term mappings
//! - [`SymbolTable`] - Bidirectional symbol interning

// Fast hash collections used throughout the crate.
// FxHashMap/FxHashSet use a multiplicative hash ideal for small integer keys
// (TermId, VarId, SymbolId, ClauseId) — 10-25% faster than SipHash on those.
pub(crate) use rustc_hash::FxHashMap as HashMap;
pub(crate) use rustc_hash::FxHashSet as HashSet;

pub mod clause;
pub mod display;
pub mod formula;
pub mod model;
pub mod profile;
pub mod subst;
pub mod symbol;
pub mod term;
pub mod term_bank;
pub mod witness;

#[cfg(feature = "proover")]
pub mod alpha;

#[cfg(feature = "ml")]
pub mod ml;

pub use clause::{Clause, ClauseId, ClauseSource, Literal};
pub use formula::{Atom, Formula};
pub use model::{EqualitySemantics, FunctionTable, ModelCertificate, PredicateTable};
pub use profile::{InputMetadata, ProblemArchetype, ProblemProfile};
pub use smallvec::SmallVec;

/// Stack size for any thread that runs unification, indexing, ordering,
/// paramodulation, proof verification, or certification.
///
/// The search recurses to the depth of the term it is working on: resolving a
/// literal walks a subterm tree, paramodulating rewrites in the same, and the
/// strict proof kernel replays a whole derivation. Nothing bounds that depth
/// from the input, so a thread that runs any of it needs a stack that is not
/// sized for a leaf task.
///
/// 64 MiB is on the order of 300,000 frames, comfortably more than the nesting
/// depth of any real TPTP problem, and the size the search has been measured
/// and shipped with. It is deliberately set here, in code, at every spawn site,
/// rather than left to the ambient default: Rust's default for a spawned thread
/// is 2 MiB, and a stack overflow inside a thread aborts the whole process with
/// no output at all, which is a far worse failure than a slow one.
///
/// A thread's stack is reserved address space and committed on demand, so this
/// costs nothing in resident memory. It is not free in *address space*
/// though, and a run that also caps `RLIMIT_AS` has to budget for it: at 64
/// workers these reservations are 4 GiB. The fixed-work performance probe
/// accounts for that and refuses a worker count that does not fit.
///
/// Threads that do not recurse — pipe readers and writers, watchdogs, audit
/// orchestration — should keep the default, precisely because this is not free.
pub const RECURSION_STACK_BYTES: usize = 64 * 1024 * 1024;
pub use subst::Substitution;
pub use symbol::{SymbolId, SymbolTable};
pub use term::{Term, VarId};
pub use witness::{DemodStepWitness, ProofArena, ProofNode, ProofNodeId, ProofWitness};

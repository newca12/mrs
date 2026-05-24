//! Clausification: converting first-order formulas to clause normal form.
//!
//! This crate implements the standard clausification pipeline:
//!
//! 1. **NNF** ([`nnf`]) - Negation Normal Form: push negations inward
//! 2. **Miniscoping** ([`miniscope`]) - Push quantifiers inward to reduce Skolem arity
//! 3. **Skolemization** ([`skolem`]) - Eliminate existential quantifiers
//! 4. **CNF conversion** ([`cnf`]) - Convert to conjunctive normal form
//! 5. **Clause extraction** ([`flatten`]) - Extract clauses from CNF formula
//! 6. **Simplification** ([`simplify`]) - Remove tautologies and duplicates
//!
//! The main entry point is [`clausify`], which runs the full pipeline.
//!
//! # Examples
//!
//! ```
//! use mrs_core::{Formula, Atom, Term, SymbolTable};
//! use mrs_core::clause::ClauseIdGen;
//! use mrs_cnf::clausify;
//!
//! let mut syms = SymbolTable::new();
//! let p = syms.intern("p");
//! let a = syms.intern("a");
//!
//! // Clausify: ∀X. p(X) => p(a)
//! let formula = Formula::forall(0,
//!     Formula::implies(
//!         Formula::atom(Atom::pred(p, vec![Term::var(0)])),
//!         Formula::atom(Atom::pred(p, vec![Term::constant(a)])),
//!     )
//! );
//!
//! let mut id_gen = ClauseIdGen::new();
//! let clauses = clausify(&formula, &mut syms, &mut id_gen, "test", "axiom");
//! assert!(!clauses.is_empty());
//! ```

pub mod cnf;
pub mod definitional;
pub mod flatten;
pub mod miniscope;
pub mod nnf;
pub mod simplify;
pub mod skolem;

use mrs_core::clause::{Clause, ClauseIdGen};
use mrs_core::{Formula, SymbolTable};

/// Clausifies a formula through the full pipeline.
///
/// Returns a vector of clauses. The `name` and `role` are used to
/// tag the clause source for proof reconstruction.
///
/// Uses definitional CNF (Tseitin) when the formula contains conjunctions
/// under disjunctions (which would cause exponential blowup with distributive
/// CNF). Falls back to simple distributive CNF for formulas without blowup.
pub fn clausify(
    formula: &Formula,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    name: &str,
    role: &str,
) -> Vec<Clause> {
    // Step 1: Convert to NNF
    let nnf_formula = nnf::to_nnf(formula);

    // Step 2: Miniscope (push quantifiers inward to reduce Skolem arity)
    let mini_formula = miniscope::miniscope(&nnf_formula);

    // Step 3: Skolemize (eliminate existential quantifiers)
    let skolem_formula = skolem::skolemize(&mini_formula, symbols, name);

    // Step 4: Drop universal quantifiers (implicit in clausal form)
    let stripped = strip_forall(&skolem_formula);

    // Step 5: Convert to CNF
    // Use definitional CNF if the formula has And-under-Or (blowup risk),
    // otherwise use simple distributive CNF (no extra definition symbols).
    let cnf_formula = if has_and_under_or(&stripped) {
        definitional::to_cnf_definitional(&stripped, symbols, name)
    } else {
        cnf::to_cnf(&stripped)
    };

    // Step 6: Extract clauses
    let clauses = flatten::extract_clauses(&cnf_formula, id_gen, name, role);

    // Step 7: Simplify
    simplify::simplify_clauses(clauses)
}

/// Strips all universal quantifiers from a formula, recursing into
/// And/Or/Neg nodes. After Skolemization, all remaining quantifiers
/// are universal, and in clausal form they are implicit.
fn strip_forall(formula: &Formula) -> Formula {
    match formula {
        Formula::Forall(_, body) => strip_forall(body),
        Formula::And(cs) => Formula::and(cs.iter().map(strip_forall).collect()),
        Formula::Or(ds) => Formula::or(ds.iter().map(strip_forall).collect()),
        Formula::Neg(inner) => Formula::neg(strip_forall(inner)),
        other => other.clone(),
    }
}

/// Returns true if the formula contains an And node that is a descendant
/// of an Or node. This pattern causes exponential blowup with distributive
/// CNF, so we use definitional CNF instead.
fn has_and_under_or(formula: &Formula) -> bool {
    match formula {
        Formula::Or(ds) => ds.iter().any(contains_and),
        Formula::And(cs) => cs.iter().any(has_and_under_or),
        _ => false,
    }
}

/// Returns true if the formula contains an And node (at any depth).
fn contains_and(formula: &Formula) -> bool {
    match formula {
        Formula::And(_) => true,
        Formula::Or(ds) => ds.iter().any(contains_and),
        _ => false,
    }
}

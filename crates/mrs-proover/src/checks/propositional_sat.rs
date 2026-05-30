//! Propositional SAT fast-path for ATP dispatch.
//!
//! Many proof steps in Vampire-style proofs (e.g. `rat`, `avatar_*`,
//! `sat_conversion`) operate over pure propositional clauses built from
//! 0-ary predicate symbols (the `spl0_N` avatar splits). For such steps
//! the entailment check `premises ⊨ conclusion` is a finite SAT problem
//! and can be decided in microseconds by a real SAT solver — but
//! eprover/vampire, asked to check the same query via FOL saturation,
//! routinely time out on the larger instances (8-of-12 NV failures in a
//! recent corpus sample were of this shape).
//!
//! This module exposes [`try_propositional`], which detects when every
//! input formula is purely propositional (only 0-ary `Atom::Pred`, no
//! `Atom::Eq`, no quantifiers) and, if so, asks varisat whether
//! `(premises) ∧ ¬conclusion` is satisfiable. UNSAT means the step is
//! sound; SAT means it is unsound (there's a propositional
//! counter-model). Returns `None` whenever any input contains a
//! non-propositional construct, in which case the caller must fall
//! through to the FOL ATP ladder.
//!
//! The encoding is a textbook Tseitin transformation: each subformula
//! receives a fresh SAT variable, with clauses enforcing the
//! connective's semantics. The root variable of every premise is
//! asserted true and the root of the negated conclusion is asserted
//! true. Tseitin is preferred over naive CNF expansion because the
//! larger `rat` steps contain disjunctions of 30+ literals and would
//! otherwise blow up combinatorially through iff/implies.

use std::collections::HashMap;

use mrs_core::{Atom, Formula, SymbolId};
use varisat::{ExtendFormula, Lit, Solver, Var};

/// Outcome of the propositional fast-path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropOutcome {
    /// `premises ⊨ conclusion` is propositionally valid; the step is sound.
    Sound,
    /// `premises ∪ {¬conclusion}` has a propositional model; the step is unsound.
    Unsound,
}

/// Try to decide `premises ⊨ conclusion` propositionally.
///
/// Returns `Some(Sound)` / `Some(Unsound)` only when *every* premise and
/// the conclusion are purely propositional (i.e. built from 0-ary
/// `Atom::Pred`, `Neg`, `And`, `Or`, `Implies`, `Iff`, `True`, `False`).
/// Any first-order construct — equality, quantifiers, predicates with
/// arguments — causes a `None` return so the caller can fall through to
/// the FOL ATP ladder.
pub fn try_propositional(premises: &[Formula], conclusion: &Formula) -> Option<PropOutcome> {
    let mut enc = Encoder::default();
    // Verify everything is propositional in a quick pre-pass; cheaper to
    // walk twice than to start encoding and roll back on failure.
    for p in premises {
        if !is_propositional(p) {
            return None;
        }
    }
    if !is_propositional(conclusion) {
        return None;
    }

    let mut solver = Solver::new();
    for p in premises {
        let lit = enc.encode(p, &mut solver);
        solver.add_clause(&[lit]);
    }
    let neg_concl = enc.encode(conclusion, &mut solver);
    solver.add_clause(&[!neg_concl]);

    match solver.solve() {
        Ok(false) => Some(PropOutcome::Sound),
        Ok(true) => Some(PropOutcome::Unsound),
        Err(_) => None,
    }
}

/// Walks `f` returning `true` iff it contains only propositional
/// constructs. Free variables are not propositional — a `Forall`/`Exists`
/// node, a `Pred` with arguments, or any `Eq` atom causes `false`.
fn is_propositional(f: &Formula) -> bool {
    match f {
        Formula::True | Formula::False => true,
        Formula::Atom(Atom::Pred(_, args)) => args.is_empty(),
        Formula::Atom(Atom::Eq(..)) => false,
        Formula::Neg(g) => is_propositional(g),
        Formula::And(gs) | Formula::Or(gs) => gs.iter().all(is_propositional),
        Formula::Implies(a, b) | Formula::Iff(a, b) => is_propositional(a) && is_propositional(b),
        Formula::Forall(..) | Formula::Exists(..) => false,
    }
}

/// Tseitin encoder: each propositional atom gets a stable SAT variable
/// (cached by `SymbolId`) and each compound subformula gets a fresh
/// auxiliary variable with the connective clauses added on-encounter.
#[derive(Default)]
struct Encoder {
    atom_vars: HashMap<SymbolId, Var>,
}

impl Encoder {
    /// Encode `f` and return a literal that is true exactly when `f` is
    /// true under the SAT assignment. May add clauses to `solver` as a
    /// side-effect.
    fn encode(&mut self, f: &Formula, solver: &mut Solver) -> Lit {
        match f {
            Formula::True => {
                // A fresh variable forced to true. Cheaper than threading
                // a constant through the encoder.
                let v = solver.new_var();
                solver.add_clause(&[Lit::from_var(v, true)]);
                Lit::from_var(v, true)
            }
            Formula::False => {
                let v = solver.new_var();
                solver.add_clause(&[Lit::from_var(v, false)]);
                Lit::from_var(v, true)
            }
            Formula::Atom(Atom::Pred(sym, args)) => {
                debug_assert!(args.is_empty(), "non-propositional atom slipped through");
                let var = *self
                    .atom_vars
                    .entry(*sym)
                    .or_insert_with(|| solver.new_var());
                Lit::from_var(var, true)
            }
            Formula::Atom(Atom::Eq(..)) => {
                unreachable!("Eq slipped past is_propositional")
            }
            Formula::Neg(g) => !self.encode(g, solver),
            Formula::And(gs) => {
                // y ↔ (a₁ ∧ … ∧ aₙ):
                //   for each i: ¬y ∨ aᵢ   (y → aᵢ)
                //   one clause:  y ∨ ¬a₁ ∨ … ∨ ¬aₙ   (a₁ ∧ … ∧ aₙ → y)
                let y = Lit::from_var(solver.new_var(), true);
                let mut big = Vec::with_capacity(gs.len() + 1);
                big.push(y);
                for g in gs {
                    let a = self.encode(g, solver);
                    solver.add_clause(&[!y, a]);
                    big.push(!a);
                }
                solver.add_clause(&big);
                y
            }
            Formula::Or(gs) => {
                // y ↔ (a₁ ∨ … ∨ aₙ):
                //   one clause:  ¬y ∨ a₁ ∨ … ∨ aₙ   (y → at least one aᵢ)
                //   for each i: y ∨ ¬aᵢ            (aᵢ → y)
                let y = Lit::from_var(solver.new_var(), true);
                let mut big = Vec::with_capacity(gs.len() + 1);
                big.push(!y);
                for g in gs {
                    let a = self.encode(g, solver);
                    solver.add_clause(&[y, !a]);
                    big.push(a);
                }
                solver.add_clause(&big);
                y
            }
            Formula::Implies(a, b) => {
                // a → b  ≡  ¬a ∨ b
                let la = self.encode(a, solver);
                let lb = self.encode(b, solver);
                let y = Lit::from_var(solver.new_var(), true);
                // y ↔ (¬a ∨ b):
                solver.add_clause(&[!y, !la, lb]);
                solver.add_clause(&[y, la]);
                solver.add_clause(&[y, !lb]);
                y
            }
            Formula::Iff(a, b) => {
                // a ↔ b  ≡  (¬a ∨ b) ∧ (a ∨ ¬b)
                let la = self.encode(a, solver);
                let lb = self.encode(b, solver);
                let y = Lit::from_var(solver.new_var(), true);
                // y ↔ (la ↔ lb):
                solver.add_clause(&[!y, !la, lb]);
                solver.add_clause(&[!y, la, !lb]);
                solver.add_clause(&[y, !la, !lb]);
                solver.add_clause(&[y, la, lb]);
                y
            }
            Formula::Forall(..) | Formula::Exists(..) => {
                unreachable!("quantifier slipped past is_propositional")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::{Atom, Formula, SymbolTable, Term};

    fn p(s: &mut SymbolTable, name: &str) -> Formula {
        Formula::Atom(Atom::Pred(s.intern(name), vec![]))
    }

    #[test]
    fn unit_resolution_is_sound() {
        // (p ∨ q) ∧ ¬q ⊨ p
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::Or(vec![pp.clone(), pq.clone()]);
        let prem2 = Formula::neg(pq);
        let goal = pp;
        assert_eq!(
            try_propositional(&[prem1, prem2], &goal),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn rat_style_resolution_is_sound() {
        // The shape of SYN485+1's s709:
        // (spl0_3 ∨ spl0_1) ∧ (¬spl0_3 ∨ spl0_1) ⊨ spl0_1
        let mut s = SymbolTable::new();
        let a = p(&mut s, "spl0_3");
        let b = p(&mut s, "spl0_1");
        let prem1 = Formula::Or(vec![a.clone(), b.clone()]);
        let prem2 = Formula::Or(vec![Formula::neg(a), b.clone()]);
        assert_eq!(
            try_propositional(&[prem1, prem2], &b),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn larger_rat_chain_is_sound() {
        // The shape of SYN485+1's s732 (subset):
        // From {spl18, ¬spl18 ∨ spl33 ∨ spl35 ∨ spl36, ¬spl36} derive
        // spl33 ∨ spl35.
        let mut s = SymbolTable::new();
        let spl18 = p(&mut s, "spl18");
        let spl33 = p(&mut s, "spl33");
        let spl35 = p(&mut s, "spl35");
        let spl36 = p(&mut s, "spl36");
        let prem1 = spl18.clone();
        let prem2 = Formula::Or(vec![
            Formula::neg(spl18),
            spl33.clone(),
            spl35.clone(),
            spl36.clone(),
        ]);
        let prem3 = Formula::neg(spl36);
        let goal = Formula::Or(vec![spl33, spl35]);
        assert_eq!(
            try_propositional(&[prem1, prem2, prem3], &goal),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn counter_model_is_unsound() {
        // p ⊨ q  is invalid; varisat finds the counter-model p=true, q=false.
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        assert_eq!(try_propositional(&[pp], &pq), Some(PropOutcome::Unsound));
    }

    #[test]
    fn iff_premise_is_handled() {
        // (p ↔ q) ∧ p ⊨ q
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::iff(pp.clone(), pq.clone());
        assert_eq!(
            try_propositional(&[prem1, pp], &pq),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn implies_premise_is_handled() {
        // (p → q) ∧ p ⊨ q
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::implies(pp.clone(), pq.clone());
        assert_eq!(
            try_propositional(&[prem1, pp], &pq),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn quantifier_in_premise_returns_none() {
        let mut s = SymbolTable::new();
        let psym = s.intern("p");
        let xvar = 0u32;
        // ∀X. p(X)  is not propositional (p has an arg).
        let prem = Formula::Forall(
            xvar,
            Box::new(Formula::Atom(Atom::Pred(psym, vec![Term::var(xvar)]))),
        );
        let goal = p(&mut s, "q");
        assert_eq!(try_propositional(&[prem], &goal), None);
    }

    #[test]
    fn equality_atom_returns_none() {
        let mut s = SymbolTable::new();
        let asym = s.intern("a");
        let bsym = s.intern("b");
        let eq = Formula::Atom(Atom::Eq(Term::constant(asym), Term::constant(bsym)));
        let goal = p(&mut s, "q");
        assert_eq!(try_propositional(&[eq], &goal), None);
    }

    #[test]
    fn predicate_with_args_returns_none() {
        let mut s = SymbolTable::new();
        let psym = s.intern("p");
        let asym = s.intern("a");
        let prem = Formula::Atom(Atom::Pred(psym, vec![Term::constant(asym)]));
        let goal = p(&mut s, "q");
        assert_eq!(try_propositional(&[prem], &goal), None);
    }

    #[test]
    fn empty_premises_unsound_unless_tautology() {
        // From nothing, p does not follow.
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        assert_eq!(try_propositional(&[], &pp), Some(PropOutcome::Unsound));
        // But (p ∨ ¬p) follows from nothing.
        let pp2 = p(&mut s, "q");
        let taut = Formula::Or(vec![pp2.clone(), Formula::neg(pp2)]);
        assert_eq!(try_propositional(&[], &taut), Some(PropOutcome::Sound));
    }
}

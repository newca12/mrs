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
//! `Atom::Eq`, no quantifiers) and, if so, asks cadical whether
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

use mrs_core::{Atom, Formula};
use cadical::Solver;

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
        solver.add_clause([lit]);
    }
    let neg_concl = enc.encode(conclusion, &mut solver);
    solver.add_clause([-neg_concl]);

    match solver.solve() {
        Some(false) => Some(PropOutcome::Sound),
        Some(true) => Some(PropOutcome::Unsound),
        None => None,
    }
}

/// Try to decide `premises ⊨ conclusion` by **propositional abstraction**:
/// every distinct atom (of *any* arity, including equalities) is mapped to
/// a fresh boolean variable, ignoring all first-order structure beneath it.
///
/// This is a sound *over*-approximation of satisfiability. If the abstracted
/// `premises ∧ ¬conclusion` is UNSAT, then the original is UNSAT in every
/// FOL model (each model induces a propositional valuation that must satisfy
/// the same boolean constraints), so the step is genuinely sound and we
/// return `true`.
///
/// Crucially, a *satisfiable* abstraction proves **nothing** — the boolean
/// counter-model may be spurious once equality/instantiation constraints are
/// restored. We therefore return `false` (defer to the FOL ATP ladder) on
/// SAT, and **never** report unsoundness from this path. This asymmetry is
/// what keeps it safe under ProoVer's heavy `bad→good` penalty.
///
/// Quantifiers cannot be abstracted as a single boolean (their body links
/// instances), so any quantified input makes the function decline.
///
/// This handles steps that are propositionally valid but range over
/// argumented predicates — e.g. Vampire's `avatar_component_clause`
/// (`spl <=> body` ⊢ `¬body ∨ spl`), where `body` is an arbitrary atom and
/// the FOL ATP ladder may stall or choke on exotic operator symbols.
pub fn try_propositional_abstraction(premises: &[Formula], conclusion: &Formula) -> bool {
    for p in premises {
        if !is_quantifier_free(p) {
            return false;
        }
    }
    if !is_quantifier_free(conclusion) {
        return false;
    }

    let mut enc = Encoder::default();
    let mut solver = Solver::new();
    for p in premises {
        let lit = enc.encode(p, &mut solver);
        solver.add_clause([lit]);
    }
    let neg_concl = enc.encode(conclusion, &mut solver);
    solver.add_clause([-neg_concl]);

    // UNSAT ⇒ truly entailed (sound). SAT ⇒ abstraction too coarse, defer.
    matches!(solver.solve(), Some(false))
}

/// Walks `f` returning `true` iff it contains no quantifier. Unlike
/// [`is_propositional`], predicate arguments and equality atoms are allowed —
/// they are treated as opaque booleans by [`try_propositional_abstraction`].
fn is_quantifier_free(f: &Formula) -> bool {
    match f {
        Formula::True | Formula::False | Formula::Atom(_) => true,
        Formula::Neg(g) => is_quantifier_free(g),
        Formula::And(gs) | Formula::Or(gs) => gs.iter().all(is_quantifier_free),
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            is_quantifier_free(a) && is_quantifier_free(b)
        }
        Formula::Forall(..) | Formula::Exists(..) => false,
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
struct Encoder {
    atom_vars: HashMap<Atom, i32>,
    next_var: i32,
}

impl Default for Encoder {
    fn default() -> Self {
        Self {
            atom_vars: HashMap::new(),
            next_var: 1, // cadical variables start from 1
        }
    }
}

impl Encoder {
    fn new_var(&mut self) -> i32 {
        let v = self.next_var;
        self.next_var += 1;
        v
    }

    /// Encode `f` and return a literal that is true exactly when `f` is
    /// true under the SAT assignment. May add clauses to `solver` as a
    /// side-effect.
    fn encode(&mut self, f: &Formula, solver: &mut Solver) -> i32 {
        match f {
            Formula::True => {
                // A fresh variable forced to true. Cheaper than threading
                // a constant through the encoder.
                let v = self.new_var();
                solver.add_clause([v]);
                v
            }
            Formula::False => {
                let v = self.new_var();
                solver.add_clause([-v]);
                v // returning the variable itself is fine, it's forced to false, wait no, we should return -v or false literal
            }
            Formula::Atom(atom) => {
                // Key on the full atom (any arity, including equalities).
                // For the pure-propositional caller this is just the 0-ary
                // predicate; for the abstraction caller it treats every
                // distinct argumented atom as an opaque boolean.
                let var = *self
                    .atom_vars
                    .entry(atom.clone())
                    .or_insert_with(|| {
                        let v = self.next_var;
                        self.next_var += 1;
                        v
                    });
                var
            }
            Formula::Neg(g) => -self.encode(g, solver),
            Formula::And(gs) => {
                // y ↔ (a₁ ∧ … ∧ aₙ):
                //   for each i: ¬y ∨ aᵢ   (y → aᵢ)
                //   one clause:  y ∨ ¬a₁ ∨ … ∨ ¬aₙ   (a₁ ∧ … ∧ aₙ → y)
                let y = self.new_var();
                let mut big = Vec::with_capacity(gs.len() + 1);
                big.push(y);
                for g in gs {
                    let a = self.encode(g, solver);
                    solver.add_clause([-y, a]);
                    big.push(-a);
                }
                solver.add_clause(big);
                y
            }
            Formula::Or(gs) => {
                // y ↔ (a₁ ∨ … ∨ aₙ):
                //   one clause:  ¬y ∨ a₁ ∨ … ∨ aₙ   (y → at least one aᵢ)
                //   for each i: y ∨ ¬aᵢ            (aᵢ → y)
                let y = self.new_var();
                let mut big = Vec::with_capacity(gs.len() + 1);
                big.push(-y);
                for g in gs {
                    let a = self.encode(g, solver);
                    solver.add_clause([y, -a]);
                    big.push(a);
                }
                solver.add_clause(big);
                y
            }
            Formula::Implies(a, b) => {
                // a → b  ≡  ¬a ∨ b
                let la = self.encode(a, solver);
                let lb = self.encode(b, solver);
                let y = self.new_var();
                // y ↔ (¬a ∨ b):
                solver.add_clause([-y, -la, lb]);
                solver.add_clause([y, la]);
                solver.add_clause([y, -lb]);
                y
            }
            Formula::Iff(a, b) => {
                // a ↔ b  ≡  (¬a ∨ b) ∧ (a ∨ ¬b)
                let la = self.encode(a, solver);
                let lb = self.encode(b, solver);
                let y = self.new_var();
                // y ↔ (la ↔ lb):
                solver.add_clause([-y, -la, lb]);
                solver.add_clause([-y, la, -lb]);
                solver.add_clause([y, -la, -lb]);
                solver.add_clause([y, la, lb]);
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
        // p ⊨ q  is invalid; cadical finds the counter-model p=true, q=false.
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

    // ---- propositional-abstraction tests ----

    /// An argumented atom: `pred(args...)`.
    fn pa(s: &mut SymbolTable, name: &str, args: Vec<Term>) -> Formula {
        Formula::Atom(Atom::Pred(s.intern(name), args))
    }

    #[test]
    fn abstraction_decides_avatar_component_clause() {
        // The exact shape of LCL894+1 f44:
        //   premise:  spl0_1 <=> ge(c, b)
        //   concl:    ¬ge(c, b) ∨ spl0_1
        // ge(c,b) is an argumented atom; the pure-0-ary path declines, but
        // abstraction treats ge(c,b) as opaque and proves the entailment.
        let mut s = SymbolTable::new();
        let c = Term::constant(s.intern("c"));
        let b = Term::constant(s.intern("b"));
        let ge = pa(&mut s, "ge", vec![c, b]);
        let spl = p(&mut s, "spl0_1");
        let iff = Formula::iff(spl.clone(), ge.clone());
        let concl = Formula::Or(vec![Formula::neg(ge), spl]);
        assert!(try_propositional_abstraction(&[iff], &concl));
    }

    #[test]
    fn abstraction_handles_pure_0ary() {
        // Pure propositional unit resolution is also provable by abstraction.
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::Or(vec![pp.clone(), pq.clone()]);
        let prem2 = Formula::neg(pq);
        assert!(try_propositional_abstraction(&[prem1, prem2], &pp));
    }

    #[test]
    fn abstraction_defers_on_equality_transitivity() {
        // SOUNDNESS-CRITICAL: a=b, b=c ⊨ a=c is a real FOL entailment, but it
        // relies on equality transitivity, which abstraction cannot see (the
        // three equalities are distinct opaque booleans). It must return
        // `false` (defer to ATP), NOT falsely claim soundness.
        let mut s = SymbolTable::new();
        let a = Term::constant(s.intern("a"));
        let b = Term::constant(s.intern("b"));
        let c = Term::constant(s.intern("c"));
        let ab = Formula::Atom(Atom::Eq(a.clone(), b.clone()));
        let bc = Formula::Atom(Atom::Eq(b, c.clone()));
        let ac = Formula::Atom(Atom::Eq(a, c));
        assert!(!try_propositional_abstraction(&[ab, bc], &ac));
    }

    #[test]
    fn abstraction_defers_on_first_order_instantiation() {
        // SOUNDNESS-CRITICAL: ∀X.p(X) ⊨ p(a) is valid but needs instantiation.
        // The premise is quantified, so abstraction declines outright.
        let mut s = SymbolTable::new();
        let asym = Term::constant(s.intern("a"));
        let psym = s.intern("p");
        let prem = Formula::Forall(
            0,
            Box::new(Formula::Atom(Atom::Pred(psym, vec![Term::var(0)]))),
        );
        let goal = Formula::Atom(Atom::Pred(psym, vec![asym]));
        assert!(!try_propositional_abstraction(&[prem], &goal));
    }

    #[test]
    fn abstraction_defers_when_not_entailed() {
        // p(a) ⊨ p(b) is NOT valid; abstraction sees distinct atoms, SAT,
        // returns false (defer) — never claims soundness.
        let mut s = SymbolTable::new();
        let asym = Term::constant(s.intern("a"));
        let bsym = Term::constant(s.intern("b"));
        let pa_atom = pa(&mut s, "p", vec![asym]);
        let pb_atom = pa(&mut s, "p", vec![bsym]);
        assert!(!try_propositional_abstraction(&[pa_atom], &pb_atom));
    }
}

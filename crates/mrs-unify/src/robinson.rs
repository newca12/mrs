//! Robinson's unification algorithm.
//!
//! This is the classic recursive unification algorithm. It's simple and
//! easy to understand, making it ideal for educational purposes.
//!
//! ## Algorithm
//!
//! Given two terms `s` and `t`, the algorithm works as follows:
//! 1. If `s` is a variable, bind it to `t` (with occurs check).
//! 2. If `t` is a variable, bind it to `s` (with occurs check).
//! 3. If both are function applications with the same symbol and arity,
//!    recursively unify corresponding arguments, composing substitutions.
//! 4. Otherwise, fail.

use std::collections::HashSet;

use mrs_core::Substitution;
use mrs_core::SymbolId;
use mrs_core::term::{Term, VarId};
use mrs_core::term_bank::{IdSubstitution, TermBank, TermId, TermNode};

use crate::{UnifyError, UnifyResult};

/// Unifies two terms using Robinson's algorithm.
///
/// Returns the most general unifier (MGU) or an error if unification fails.
pub fn unify(s: &Term, t: &Term) -> UnifyResult {
    let mut subst = Substitution::new();
    unify_rec(s, t, &mut subst)?;
    Ok(subst)
}

/// Unifies two terms using Robinson's algorithm, operating on `TermId`s.
pub fn unify_id(s: TermId, t: TermId, bank: &TermBank) -> Result<IdSubstitution, UnifyError> {
    let mut subst = IdSubstitution::new();
    unify_rec_id(s, t, &mut subst, bank)?;
    Ok(subst)
}

/// Recursive unification, accumulating bindings in `subst`.
fn unify_rec(s: &Term, t: &Term, subst: &mut Substitution) -> Result<(), UnifyError> {
    // Apply current substitution to both sides
    let s = subst.apply_term(s);
    let t = subst.apply_term(t);

    match (&s, &t) {
        // Identical terms: nothing to do
        _ if s == t => Ok(()),

        // Variable on the left
        (Term::Var(v), _) => bind_var(*v, &t, subst),

        // Variable on the right
        (_, Term::Var(v)) => bind_var(*v, &s, subst),

        // Two function applications
        (Term::App(f1, args1), Term::App(f2, args2)) => {
            if f1 != f2 {
                return Err(UnifyError::SymbolClash {
                    left: format!("{:?}", f1),
                    right: format!("{:?}", f2),
                });
            }
            if args1.len() != args2.len() {
                return Err(UnifyError::ArityMismatch {
                    expected: args1.len(),
                    found: args2.len(),
                });
            }
            for (a1, a2) in args1.iter().zip(args2.iter()) {
                unify_rec(a1, a2, subst)?;
            }
            Ok(())
        }
    }
}

fn unify_rec_id(
    s: TermId,
    t: TermId,
    subst: &mut IdSubstitution,
    bank: &TermBank,
) -> Result<(), UnifyError> {
    let s = deref_id(s, subst, bank);
    let t = deref_id(t, subst, bank);

    if s == t {
        return Ok(());
    }

    match (bank.get(s), bank.get(t)) {
        (TermNode::Var(v), _) => bind_var_id(*v, t, subst, bank),
        (_, TermNode::Var(v)) => bind_var_id(*v, s, subst, bank),
        (TermNode::App(f1, args1), TermNode::App(f2, args2)) => {
            if f1 != f2 {
                return Err(UnifyError::SymbolClash {
                    left: format!("{:?}", f1),
                    right: format!("{:?}", f2),
                });
            }
            if args1.len() != args2.len() {
                return Err(UnifyError::ArityMismatch {
                    expected: args1.len(),
                    found: args2.len(),
                });
            }
            for (a1, a2) in args1.iter().zip(args2.iter()) {
                unify_rec_id(*a1, *a2, subst, bank)?;
            }
            Ok(())
        }
    }
}

/// Unifies two terms with commutativity support.
///
/// Behaves identically to [`unify`] when `comm` is empty.  For binary function
/// symbols in `comm`, both argument orderings are tried: normal order first,
/// then swapped.  The first successful permutation is returned.
///
/// Only one MGU is returned per pair of terms; full C-unification (multiple
/// incomparable unifiers) is not implemented.
pub fn unify_comm(s: &Term, t: &Term, comm: &HashSet<SymbolId>) -> UnifyResult {
    if comm.is_empty() {
        return unify(s, t);
    }
    let mut subst = Substitution::new();
    unify_comm_rec(s, t, &mut subst, comm)?;
    Ok(subst)
}

/// Recursive unification with commutativity, accumulating bindings in `subst`.
fn unify_comm_rec(
    s: &Term,
    t: &Term,
    subst: &mut Substitution,
    comm: &HashSet<SymbolId>,
) -> Result<(), UnifyError> {
    // Apply current substitution to both sides
    let s = subst.apply_term(s);
    let t = subst.apply_term(t);

    match (&s, &t) {
        // Identical terms: nothing to do
        _ if s == t => Ok(()),

        // Variable on the left
        (Term::Var(v), _) => bind_var(*v, &t, subst),

        // Variable on the right
        (_, Term::Var(v)) => bind_var(*v, &s, subst),

        // Two function applications
        (Term::App(f1, args1), Term::App(f2, args2)) => {
            if f1 != f2 {
                return Err(UnifyError::SymbolClash {
                    left: format!("{:?}", f1),
                    right: format!("{:?}", f2),
                });
            }
            if args1.len() != args2.len() {
                return Err(UnifyError::ArityMismatch {
                    expected: args1.len(),
                    found: args2.len(),
                });
            }

            // Snapshot substitution before attempting normal order
            let saved = subst.clone();

            // Try normal argument order
            let normal_ok: Result<(), UnifyError> = (|| {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    unify_comm_rec(a1, a2, subst, comm)?;
                }
                Ok(())
            })();

            if normal_ok.is_ok() {
                return Ok(());
            }

            // For binary commutative symbols try swapped order
            if comm.contains(f1) && args1.len() == 2 {
                let mut subst_swap = saved.clone();
                let swap_ok: Result<(), UnifyError> = (|| {
                    unify_comm_rec(&args1[0], &args2[1], &mut subst_swap, comm)?;
                    unify_comm_rec(&args1[1], &args2[0], &mut subst_swap, comm)?;
                    Ok(())
                })();
                if swap_ok.is_ok() {
                    *subst = subst_swap;
                    return Ok(());
                }
            }

            // Both orderings failed — restore clean state and propagate error
            *subst = saved;
            normal_ok
        }
    }
}

pub fn unify_comm_id(
    s: TermId,
    t: TermId,
    bank: &TermBank,
    comm: &HashSet<SymbolId>,
) -> Result<IdSubstitution, UnifyError> {
    if comm.is_empty() {
        return unify_id(s, t, bank);
    }
    let mut subst = IdSubstitution::new();
    unify_comm_rec_id(s, t, &mut subst, bank, comm)?;
    Ok(subst)
}

fn unify_comm_rec_id(
    s: TermId,
    t: TermId,
    subst: &mut IdSubstitution,
    bank: &TermBank,
    comm: &HashSet<SymbolId>,
) -> Result<(), UnifyError> {
    let s = deref_id(s, subst, bank);
    let t = deref_id(t, subst, bank);

    if s == t {
        return Ok(());
    }

    match (bank.get(s), bank.get(t)) {
        (TermNode::Var(v), _) => bind_var_id(*v, t, subst, bank),
        (_, TermNode::Var(v)) => bind_var_id(*v, s, subst, bank),
        (TermNode::App(f1, args1), TermNode::App(f2, args2)) => {
            if f1 != f2 {
                return Err(UnifyError::SymbolClash {
                    left: format!("{:?}", f1),
                    right: format!("{:?}", f2),
                });
            }
            if args1.len() != args2.len() {
                return Err(UnifyError::ArityMismatch {
                    expected: args1.len(),
                    found: args2.len(),
                });
            }

            let saved = subst.clone();

            let normal_ok: Result<(), UnifyError> = (|| {
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    unify_comm_rec_id(*a1, *a2, subst, bank, comm)?;
                }
                Ok(())
            })();

            if normal_ok.is_ok() {
                return Ok(());
            }

            if comm.contains(f1) && args1.len() == 2 {
                let mut subst_swap = saved.clone();
                let swap_ok: Result<(), UnifyError> = (|| {
                    unify_comm_rec_id(args1[0], args2[1], &mut subst_swap, bank, comm)?;
                    unify_comm_rec_id(args1[1], args2[0], &mut subst_swap, bank, comm)?;
                    Ok(())
                })();
                if swap_ok.is_ok() {
                    *subst = subst_swap;
                    return Ok(());
                }
            }

            *subst = saved;
            normal_ok
        }
    }
}

/// Binds a variable to a term, performing the occurs check.
///
/// Before storing, the current substitution is applied to `term` (path
/// compression).  This step is critical for cycle prevention: without it, the
/// raw occurs check only detects *direct* occurrences of `var` in `term`, but
/// misses transitive ones.  For example, with `subst = {5 → f(Var(4))}` and
/// `term = g(Var(5))`, the raw check does not see `Var(4)` in `term`, so it
/// would store `4 → g(Var(5))` — creating the cycle
/// `4 → g(f(4)) → g(f(g(f(...)))) → ∞`.  Applying the substitution first
/// expands `g(Var(5))` to `g(f(Var(4)))`, and the occurs check then correctly
/// rejects the binding.
fn bind_var(var: VarId, term: &Term, subst: &mut Substitution) -> Result<(), UnifyError> {
    // If the term is the same variable, nothing to do
    if let Term::Var(v) = term
        && *v == var
    {
        return Ok(());
    }

    // Path-compress: apply the current substitution to `term` so that the
    // occurs check below can detect transitive cycles.
    let term = subst.apply_term(term);

    // After applying, the term might reduce to the same variable (identity).
    if let Term::Var(v) = &term
        && *v == var
    {
        return Ok(());
    }

    // Occurs check on the fully-applied term.
    if term.contains_var(var) {
        return Err(UnifyError::OccursCheck { var });
    }

    subst.bind(var, term);
    Ok(())
}

fn deref_id(mut t: TermId, subst: &IdSubstitution, bank: &TermBank) -> TermId {
    let mut steps = 0;
    loop {
        if let TermNode::Var(v) = bank.get(t)
            && let Some(next) = subst.get(*v)
        {
            t = next;
            steps += 1;
            debug_assert!(steps < 100_000, "apply_term_opt cycle");
            continue;
        }
        break;
    }
    t
}

fn bind_var_id(
    var: VarId,
    term: TermId,
    subst: &mut IdSubstitution,
    bank: &TermBank,
) -> Result<(), UnifyError> {
    let term = deref_id(term, subst, bank);

    if let TermNode::Var(v) = bank.get(term)
        && *v == var
    {
        return Ok(());
    }

    if contains_var_id(term, var, subst, bank) {
        return Err(UnifyError::OccursCheck { var });
    }

    subst.bind(var, term);
    Ok(())
}

fn contains_var_id(term: TermId, var: VarId, subst: &IdSubstitution, bank: &TermBank) -> bool {
    let term = deref_id(term, subst, bank);
    match bank.get(term) {
        TermNode::Var(v) => *v == var,
        TermNode::App(_, args) => args.iter().any(|&a| contains_var_id(a, var, subst, bank)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use mrs_core::{SymbolId, SymbolTable};

    fn syms() -> (SymbolTable, SymbolId, SymbolId, SymbolId, SymbolId) {
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");
        (st, f, g, a, b)
    }

    #[test]
    fn unify_identical_vars() {
        let mgu = unify(&Term::var(0), &Term::var(0)).unwrap();
        assert!(mgu.is_empty());
    }

    #[test]
    fn unify_var_with_constant() {
        let (_, _, _, a, _) = syms();
        let mgu = unify(&Term::var(0), &Term::constant(a)).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn unify_two_vars() {
        let mgu = unify(&Term::var(0), &Term::var(1)).unwrap();
        // One var should be bound to the other
        let result = mgu.apply_term(&Term::var(0));
        assert_eq!(result, mgu.apply_term(&Term::var(1)));
    }

    #[test]
    fn unify_function_terms() {
        let (_, f, _, a, b) = syms();
        // f(X, a) ~ f(b, Y) -> {X/b, Y/a}
        let t1 = Term::app(f, vec![Term::var(0), Term::constant(a)]);
        let t2 = Term::app(f, vec![Term::constant(b), Term::var(1)]);
        let mgu = unify(&t1, &t2).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(b));
        assert_eq!(mgu.apply_term(&Term::var(1)), Term::constant(a));
    }

    #[test]
    fn unify_nested() {
        let (_, f, g, a, _) = syms();
        // f(X, g(X)) ~ f(a, g(a))
        let t1 = Term::app(f, vec![Term::var(0), Term::app(g, vec![Term::var(0)])]);
        let t2 = Term::app(
            f,
            vec![Term::constant(a), Term::app(g, vec![Term::constant(a)])],
        );
        let mgu = unify(&t1, &t2).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn unify_transitive() {
        let (_, f, _, a, _) = syms();
        // f(X, Y) ~ f(Y, a) -> {X/a, Y/a}
        let t1 = Term::app(f, vec![Term::var(0), Term::var(1)]);
        let t2 = Term::app(f, vec![Term::var(1), Term::constant(a)]);
        let mgu = unify(&t1, &t2).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
        assert_eq!(mgu.apply_term(&Term::var(1)), Term::constant(a));
    }

    #[test]
    fn unify_symbol_clash() {
        let (_, f, g, a, _) = syms();
        // f(a) ~ g(a) -> fail
        let t1 = Term::app(f, vec![Term::constant(a)]);
        let t2 = Term::app(g, vec![Term::constant(a)]);
        assert!(matches!(
            unify(&t1, &t2),
            Err(UnifyError::SymbolClash { .. })
        ));
    }

    #[test]
    fn unify_occurs_check() {
        let (_, f, _, _, _) = syms();
        // X ~ f(X) -> occurs check
        let t1 = Term::var(0);
        let t2 = Term::app(f, vec![Term::var(0)]);
        assert!(matches!(
            unify(&t1, &t2),
            Err(UnifyError::OccursCheck { var: 0 })
        ));
    }

    #[test]
    fn unify_different_constants() {
        let (_, _, _, a, b) = syms();
        // a ~ b -> fail
        assert!(unify(&Term::constant(a), &Term::constant(b)).is_err());
    }

    #[test]
    fn unify_idempotent() {
        let (_, f, _, a, _) = syms();
        // Applying the MGU twice should give the same result
        let t1 = Term::app(f, vec![Term::var(0), Term::var(1)]);
        let t2 = Term::app(f, vec![Term::constant(a), Term::var(0)]);
        let mgu = unify(&t1, &t2).unwrap();
        let applied1 = mgu.apply_term(&t1);
        let applied2 = mgu.apply_term(&applied1);
        assert_eq!(applied1, applied2);
    }

    // --- unify_comm tests ---

    #[test]
    fn unify_comm_normal_order_preferred() {
        // f(a,b) ~ f(a,b) with f commutative — normal order succeeds immediately
        let (_, f, _, a, b) = syms();
        let mut comm = HashSet::new();
        comm.insert(f);
        let t1 = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
        let t2 = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
        let mgu = unify_comm(&t1, &t2, &comm).unwrap();
        assert!(mgu.is_empty());
    }

    #[test]
    fn unify_comm_swapped_constants() {
        // f(a,b) ~ f(b,a) with f commutative — swap succeeds
        let (_, f, _, a, b) = syms();
        let mut comm = HashSet::new();
        comm.insert(f);
        let t1 = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
        let t2 = Term::app(f, vec![Term::constant(b), Term::constant(a)]);
        assert!(unify_comm(&t1, &t2, &comm).is_ok());
    }

    #[test]
    fn unify_comm_swapped_with_vars() {
        // f(X, a) ~ f(a, X) with f commutative — swap gives X=a
        let (_, f, _, a, _) = syms();
        let mut comm = HashSet::new();
        comm.insert(f);
        let t1 = Term::app(f, vec![Term::var(0), Term::constant(a)]);
        let t2 = Term::app(f, vec![Term::constant(a), Term::var(0)]);
        let mgu = unify_comm(&t1, &t2, &comm).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn unify_comm_fails_when_both_orderings_clash() {
        // f(a, b) ~ f(b, a) without commutativity flag — should fail
        let (_, f, _, a, b) = syms();
        let comm = HashSet::new(); // empty
        let t1 = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
        let t2 = Term::app(f, vec![Term::constant(b), Term::constant(a)]);
        assert!(unify_comm(&t1, &t2, &comm).is_err());
    }

    #[test]
    fn unify_comm_non_binary_not_swapped() {
        // g(a, b, c) ~ g(c, b, a) — ternary g not commutativity-extended
        let mut st = SymbolTable::new();
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");
        let c = st.intern("c");
        let mut comm = HashSet::new();
        comm.insert(g); // g declared commutative, but has 3 args
        let t1 = Term::app(
            g,
            vec![Term::constant(a), Term::constant(b), Term::constant(c)],
        );
        let t2 = Term::app(
            g,
            vec![Term::constant(c), Term::constant(b), Term::constant(a)],
        );
        assert!(unify_comm(&t1, &t2, &comm).is_err());
    }

    #[test]
    fn test_unify_id() {
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");

        let mut bank = TermBank::new();
        // t1 = f(X, g(Y))
        let v0 = bank.intern_var(0);
        let v1 = bank.intern_var(1);
        let g_y = bank.intern_app(g, vec![v1]);
        let t1 = bank.intern_app(f, vec![v0, g_y]);

        // t2 = f(a, g(b))
        let c_a = bank.intern_app(a, vec![]);
        let c_b = bank.intern_app(b, vec![]);
        let g_b = bank.intern_app(g, vec![c_b]);
        let t2 = bank.intern_app(f, vec![c_a, g_b]);

        let subst = unify_id(t1, t2, &bank).unwrap();

        let mut expected = IdSubstitution::new();
        expected.bind(0, c_a);
        expected.bind(1, c_b);

        assert_eq!(subst.get(0), expected.get(0));
        assert_eq!(subst.get(1), expected.get(1));
    }
}

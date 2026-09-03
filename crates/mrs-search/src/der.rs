//! Destructive Equality Resolution (DER) for `IdClause`.

use mrs_core::clause::{ClauseIdGen, ClauseSource};
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermNode};
use rustc_hash::FxHashSet as HashSet;

/// Destructive Equality Resolution (DER / eager variable elimination) for `IdClause`.
///
/// Simplifies a clause by iteratively eliminating negative equality literals of the form:
/// 1. `s ≠ s` (trivial inequality reflexivity resolution), and
/// 2. `X ≠ t` or `t ≠ X` where `X` is a variable not occurring in `t` (by applying `{X ↦ t}`).
///
/// Returns `Some((final_clause, intermediate_steps))` if at least one simplification
/// occurred, or `None` if no DER step applied.
pub fn destructive_equality_resolution_id(
    clause: &IdClause,
    term_bank: &mut TermBank,
    id_gen: &mut ClauseIdGen,
) -> Option<(IdClause, Vec<IdClause>)> {
    let mut current = clause.clone();
    let mut steps = Vec::new();
    loop {
        let mut simplified_step = false;
        for (i, lit) in current.literals.iter().enumerate() {
            if lit.positive {
                continue;
            }
            let (l, r) = match &lit.atom {
                IdAtom::Eq(l, r) => (*l, *r),
                _ => continue,
            };
            if l == r {
                let new_lits: Vec<IdLiteral> = current
                    .literals
                    .iter()
                    .enumerate()
                    .filter(|&(k, _)| k != i)
                    .map(|(_, lit)| lit.clone())
                    .collect();
                let next = IdClause::new_avatar(
                    id_gen.next(),
                    new_lits,
                    ClauseSource::Inference {
                        rule: "equality_resolution",
                        parents: vec![current.id].into(),
                    },
                    current.avatar.clone(),
                );
                steps.push(next.clone());
                current = next;
                simplified_step = true;
                break;
            }
            let subst = match (term_bank.get(l), term_bank.get(r)) {
                (TermNode::Var(v), _) => {
                    let mut vars = HashSet::default();
                    term_bank.collect_vars(r, &mut vars);
                    if !vars.contains(v) {
                        Some((*v, r))
                    } else {
                        None
                    }
                }
                (_, TermNode::Var(v)) => {
                    let mut vars = HashSet::default();
                    term_bank.collect_vars(l, &mut vars);
                    if !vars.contains(v) {
                        Some((*v, l))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some((target_var, replacement)) = subst {
                let new_lits: Vec<IdLiteral> = current
                    .literals
                    .iter()
                    .enumerate()
                    .filter(|&(k, _)| k != i)
                    .map(|(_, lit)| term_bank.substitute_var_literal(lit, target_var, replacement))
                    .collect();
                let next = IdClause::new_avatar(
                    id_gen.next(),
                    new_lits,
                    ClauseSource::Inference {
                        rule: "equality_resolution",
                        parents: vec![current.id].into(),
                    },
                    current.avatar.clone(),
                );
                steps.push(next.clone());
                current = next;
                simplified_step = true;
                break;
            }
        }
        if !simplified_step {
            break;
        }
    }
    if steps.is_empty() {
        None
    } else {
        Some((current, steps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::ClauseId;
    use smallvec::smallvec;

    #[test]
    fn test_der_id_variable_elimination() {
        let mut term_bank = TermBank::new();
        let mut id_gen = ClauseIdGen::new();
        let mut syms = SymbolTable::new();

        let x = term_bank.intern_var(0);
        let a_sym = syms.intern("a");
        let a = term_bank.intern_app(a_sym, smallvec![]);
        let f_sym = syms.intern("f");
        let f_a = term_bank.intern_app(f_sym, smallvec![a]);

        let p_sym = syms.intern("p");
        let p_x = IdAtom::Pred(p_sym, smallvec![x]);
        let eq_lit = IdLiteral {
            positive: false,
            atom: IdAtom::Eq(x, f_a),
        };
        let p_lit = IdLiteral {
            positive: true,
            atom: p_x,
        };

        let clause = IdClause::new(
            ClauseId(1),
            vec![eq_lit, p_lit],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        let res = destructive_equality_resolution_id(&clause, &mut term_bank, &mut id_gen);
        assert!(res.is_some());
        let (simplified, steps) = res.unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(simplified.literals.len(), 1);
        assert!(simplified.literals[0].positive);
        match &simplified.literals[0].atom {
            IdAtom::Pred(sym, args) => {
                assert_eq!(*sym, p_sym);
                assert_eq!(args[0], f_a);
            }
            _ => panic!("Expected Pred atom"),
        }
    }

    #[test]
    fn test_der_id_trivial_inequality() {
        let mut term_bank = TermBank::new();
        let mut id_gen = ClauseIdGen::new();
        let mut syms = SymbolTable::new();

        let x = term_bank.intern_var(0);
        let f_sym = syms.intern("f");
        let f_x = term_bank.intern_app(f_sym, smallvec![x]);

        let p_sym = syms.intern("p");
        let p_x = IdAtom::Pred(p_sym, smallvec![x]);
        let eq_lit = IdLiteral {
            positive: false,
            atom: IdAtom::Eq(f_x, f_x),
        };
        let p_lit = IdLiteral {
            positive: true,
            atom: p_x,
        };

        let clause = IdClause::new(
            ClauseId(1),
            vec![eq_lit, p_lit],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        let res = destructive_equality_resolution_id(&clause, &mut term_bank, &mut id_gen);
        assert!(res.is_some());
        let (simplified, steps) = res.unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(simplified.literals.len(), 1);
        assert_eq!(
            simplified.literals[0].atom,
            IdAtom::Pred(p_sym, smallvec![x])
        );
    }
}

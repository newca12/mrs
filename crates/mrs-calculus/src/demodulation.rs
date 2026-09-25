use crate::{HashMap, HashSet};

use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::subst::Substitution;
use mrs_core::term::Term;
use mrs_core::witness::{DemodStepWitness, ProofNodeId, ProofWitness};
use mrs_unify::matching::match_term;

/// Performs forward demodulation on a clause using the provided index of unit equalities.
///
/// Returns `Some(simplified_clause)` if the clause was rewritten, or `None` if
/// no rewriting occurred. The returned clause has its redundant literals
/// simplified and parents recorded for proof extraction.
pub fn demodulate(
    clause: &Clause,
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    clause_store: &HashMap<ClauseId, Clause>,
    id_gen: &mut ClauseIdGen,
) -> Option<Clause> {
    let mut current_lits = clause.literals.clone();
    let mut changed = false;
    let mut used_unit_ids = Vec::new();

    // Iterate to fixpoint
    loop {
        let mut changed_this_pass = false;
        for lit in &mut current_lits {
            if rewrite_literal(
                lit,
                &clause.avatar,
                demod_index,
                clause_store,
                &mut used_unit_ids,
            ) {
                changed = true;
                changed_this_pass = true;
            }
        }
        if !changed_this_pass {
            break;
        }
    }

    if changed {
        let mut parents = vec![clause.id];
        parents.extend_from_slice(&used_unit_ids);

        // Deduplicate the parents list (retaining insertion order)
        let mut unique_parents = Vec::new();
        let mut seen = HashSet::default();
        for p in parents {
            if seen.insert(p) {
                unique_parents.push(p);
            }
        }

        let rule_parents: Vec<ProofNodeId> = used_unit_ids
            .iter()
            .map(|p| {
                clause_store
                    .get(p)
                    .and_then(|c| c.proof_id)
                    .unwrap_or(ProofNodeId(p.0))
            })
            .collect();

        let mut derived = Clause::new_avatar(
            id_gen.next(),
            current_lits,
            ClauseSource::Inference {
                rule: "demodulation",
                parents: unique_parents.into(),
            },
            clause.avatar.clone(),
        );
        derived.witness = Some(ProofWitness::Demodulation {
            target: clause.proof_id.unwrap_or(ProofNodeId(clause.id.0)),
            rule_parents,
            // This legacy term-keyed entry point is only used by the unit tests
            // below; the given-clause loop calls `demodulate_id`, which records
            // every rewrite it applies. An empty step list simply means the
            // emitted proof carries no `demodulation_steps` annotation and a
            // verifier has to reconstruct the rewrite sequence itself.
            steps: Vec::new(),
        });
        Some(derived)
    } else {
        None
    }
}

/// Tries to rewrite terms in a literal using the demodulation index.
/// Returns true if any rewrite was performed.
fn rewrite_literal(
    lit: &mut Literal,
    target_avatar: &[u32],
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    clause_store: &HashMap<ClauseId, Clause>,
    used_unit_ids: &mut Vec<ClauseId>,
) -> bool {
    let mut changed = false;
    let new_atom = match &lit.atom {
        Atom::Pred(p, args) => {
            let new_args: Vec<Term> = args
                .iter()
                .map(|arg| {
                    let (new_arg, ch) =
                        rewrite_term(arg, target_avatar, demod_index, clause_store, used_unit_ids);
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            Atom::Pred(*p, new_args)
        }
        Atom::Eq(l, r) => {
            let (new_l, ch_l) =
                rewrite_term(l, target_avatar, demod_index, clause_store, used_unit_ids);
            let (new_r, ch_r) =
                rewrite_term(r, target_avatar, demod_index, clause_store, used_unit_ids);
            if ch_l || ch_r {
                changed = true;
            }
            Atom::Eq(new_l, new_r)
        }
    };
    if changed {
        lit.atom = new_atom;
    }
    changed
}

/// Rewrites a term using the demodulation index.
/// Recurses into subterms, applying the first match found at each level.
fn rewrite_term(
    term: &Term,
    target_avatar: &[u32],
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    clause_store: &HashMap<ClauseId, Clause>,
    used_unit_ids: &mut Vec<ClauseId>,
) -> (Term, bool) {
    // Try matching at the current position first
    let rules = demod_index.get_generalizations(term);
    for (from, to, unit_id) in rules {
        if let Some(rule_clause) = clause_store.get(&unit_id) {
            let subset = rule_clause.avatar.iter().all(|a| target_avatar.contains(a));
            if !subset {
                continue;
            }
            if let Ok(sigma) = match_term(&from, term) {
                if !used_unit_ids.contains(&unit_id) {
                    used_unit_ids.push(unit_id);
                }
                return (apply_matching_subst(&sigma, &to), true);
            }
        }
    }

    // Recurse into subterms
    match term {
        Term::Var(_) => (term.clone(), false),
        Term::App(f, args) => {
            let mut changed = false;
            let new_args: Vec<Term> = args
                .iter()
                .map(|arg| {
                    let (new_arg, ch) =
                        rewrite_term(arg, target_avatar, demod_index, clause_store, used_unit_ids);
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            if changed {
                (Term::App(*f, new_args), true)
            } else {
                (term.clone(), false)
            }
        }
    }
}

fn apply_matching_subst(sigma: &Substitution, term: &Term) -> Term {
    sigma.apply_term(term)
}

use mrs_core::SymbolId;
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId};

pub fn demodulate_id(
    clause: &IdClause,
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    id_gen: &mut ClauseIdGen,
    ac_syms: &HashSet<SymbolId>,
) -> Option<IdClause> {
    let mut current_lits = clause.literals.clone();
    let mut changed = false;
    let mut used_unit_ids = Vec::new();
    let mut passes = 0usize;
    // Every applied rewrite is recorded in application order so the emitted
    // proof can replay the fixpoint instead of forcing a verifier to search
    // for a rewrite sequence (see `mrs-proof`'s `demodulation_steps`
    // annotation and the strict kernel's recorded-replay path).
    let mut steps: Vec<DemodStepWitness> = Vec::new();

    loop {
        // Equational problems can generate cyclic rewrite rules (a→b and b→a).
        // Without a pass limit the rewriter loops indefinitely.  100 passes is
        // a safe upper bound for any real proof step; exceeding it indicates a
        // rewrite cycle and we bail out with whatever simplification we have.
        if passes >= 100 {
            break;
        }
        passes += 1;
        let mut changed_this_pass = false;
        for (lit_idx, lit) in current_lits.iter_mut().enumerate() {
            if rewrite_literal_id(
                lit,
                lit_idx,
                &clause.avatar,
                bank,
                demod_index,
                clause_store,
                &mut used_unit_ids,
                &mut steps,
                ac_syms,
            ) {
                changed = true;
                changed_this_pass = true;
            }
        }
        if !changed_this_pass {
            break;
        }
    }

    if changed {
        let mut parents = vec![clause.id];
        parents.extend_from_slice(&used_unit_ids);

        let mut unique_parents = Vec::new();
        let mut seen = HashSet::default();
        for p in parents {
            if seen.insert(p) {
                unique_parents.push(p);
            }
        }

        let rule_parents: Vec<ProofNodeId> = used_unit_ids
            .iter()
            .map(|p| {
                clause_store
                    .get(p)
                    .and_then(|c| c.proof_id)
                    .unwrap_or(ProofNodeId(p.0))
            })
            .collect();

        let mut derived = IdClause::new_avatar(
            id_gen.next(),
            current_lits,
            ClauseSource::Inference {
                rule: "demodulation",
                parents: unique_parents.into(),
            },
            clause.avatar.clone(),
        );
        derived.witness = Some(ProofWitness::Demodulation {
            target: clause.proof_id.unwrap_or(ProofNodeId(clause.id.0)),
            rule_parents,
            steps,
        });
        Some(derived)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn rewrite_literal_id(
    lit: &mut IdLiteral,
    lit_idx: usize,
    target_avatar: &[u32],
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    used_unit_ids: &mut Vec<ClauseId>,
    steps: &mut Vec<DemodStepWitness>,
    ac_syms: &HashSet<SymbolId>,
) -> bool {
    let mut changed = false;
    let new_atom = match &lit.atom {
        IdAtom::Pred(p, args) => {
            let new_args: smallvec::SmallVec<[TermId; 4]> = args
                .iter()
                .enumerate()
                .map(|(arg_idx, arg)| {
                    // The recorded path is the full chain of argument indices
                    // from the literal's atom down to the rewritten subterm, so
                    // it is threaded down the recursion rather than rebuilt at
                    // each level.
                    let mut path = TermPath::new();
                    path.push(arg_idx);
                    let (new_arg, ch) = rewrite_term_id(
                        *arg,
                        lit_idx,
                        &mut path,
                        target_avatar,
                        bank,
                        demod_index,
                        clause_store,
                        used_unit_ids,
                        steps,
                        ac_syms,
                    );
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            IdAtom::Pred(*p, new_args)
        }
        IdAtom::Eq(l, r) => {
            let mut left_path = TermPath::new();
            left_path.push(0);
            let (new_l, ch_l) = rewrite_term_id(
                *l,
                lit_idx,
                &mut left_path,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
                steps,
                ac_syms,
            );
            let mut right_path = TermPath::new();
            right_path.push(1);
            let (new_r, ch_r) = rewrite_term_id(
                *r,
                lit_idx,
                &mut right_path,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
                steps,
                ac_syms,
            );
            if ch_l || ch_r {
                changed = true;
            }
            IdAtom::Eq(new_l, new_r)
        }
    };
    if changed {
        lit.atom = new_atom;
    }
    changed
}

/// Position of `term` inside its literal: an argument index for a predicate
/// atom, `0`/`1` for the left/right side of an equality atom.
type TermPath = Vec<usize>;

/// `path` is the position of `term` inside its literal: the chain of argument
/// indices from the literal's atom (or `0`/`1` for the left/right side of an
/// equality atom) down to this subterm.
#[allow(clippy::too_many_arguments)]
fn rewrite_term_id(
    term: TermId,
    lit_idx: usize,
    path: &mut TermPath,
    target_avatar: &[u32],
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    used_unit_ids: &mut Vec<ClauseId>,
    steps: &mut Vec<DemodStepWitness>,
    _ac_syms: &HashSet<SymbolId>,
) -> (TermId, bool) {
    let rules = demod_index.get_generalizations(term, bank);
    for (from, to, unit_id) in rules {
        if let Some(rule_clause) = clause_store.get(&unit_id) {
            let subset = rule_clause.avatar.iter().all(|a| target_avatar.contains(a));
            if !subset {
                continue;
            }

            if let Ok(sigma) = mrs_unify::matching::match_term_id(from, term, bank) {
                if !used_unit_ids.contains(&unit_id) {
                    used_unit_ids.push(unit_id);
                }
                let rewritten = apply_matching_subst_id(&sigma, to, bank);
                steps.push(DemodStepWitness {
                    rule_parent: proof_node_id_of(clause_store, unit_id),
                    lit_idx,
                    term_path: path.clone(),
                    substitution: None,
                });
                return (rewritten, true);
            }
        }
    }

    if let mrs_core::term_bank::TermNode::App(sym, args) = bank.get(term).clone() {
        let mut changed = false;
        let mut new_args = Vec::with_capacity(args.len());
        for (arg_idx, arg) in args.into_iter().enumerate() {
            path.push(arg_idx);
            let (new_arg, ch) = rewrite_term_id(
                arg,
                lit_idx,
                path,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
                steps,
                _ac_syms,
            );
            path.pop();
            if ch {
                changed = true;
            }
            new_args.push(new_arg);
        }
        if changed {
            let app_term = bank.intern_app(sym, new_args);
            return (app_term, true);
        }
    }

    (term, false)
}

/// Proof-node identity of a cited unit equality, using the same convention as
/// the step's `rule_parents` list so a recorded rewrite can be mapped back to
/// its position in that list.
fn proof_node_id_of(clause_store: &HashMap<ClauseId, IdClause>, unit_id: ClauseId) -> ProofNodeId {
    clause_store
        .get(&unit_id)
        .and_then(|clause| clause.proof_id)
        .unwrap_or(ProofNodeId(unit_id.0))
}

fn apply_matching_subst_id(
    sigma: &mrs_core::term_bank::IdSubstitution,
    term: TermId,
    bank: &mut TermBank,
) -> TermId {
    match bank.get(term).clone() {
        mrs_core::term_bank::TermNode::Var(v) => match sigma.get(v) {
            Some(t) => t,
            None => term,
        },
        mrs_core::term_bank::TermNode::App(f, args) => {
            let new_args: Vec<TermId> = args
                .iter()
                .map(|&a| apply_matching_subst_id(sigma, a, bank))
                .collect();
            bank.intern_app(f, new_args)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn input_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>, name: &str) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn demodulate_simple() {
        // Unit: f(a) = b (f(a) > b by weight)
        // Target: p(f(a))
        // Expected: p(b)
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let unit = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
            "unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
            "target",
        );

        let mut clause_store = HashMap::default();
        clause_store.insert(unit.id, unit.clone());

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(
            &Term::app(f, vec![Term::constant(a)]),
            (
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
                unit.id,
            ),
        );

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
        assert!(result.is_some());
        let simplified = result.unwrap();
        // Verify demodulation source is recorded
        match &simplified.source {
            ClauseSource::Inference { rule, parents } => {
                assert_eq!(*rule, "demodulation");
                assert_eq!(parents[0], target.id);
                assert_eq!(parents[1], unit.id);
            }
            _ => panic!("expected inference source"),
        }
        match &simplified.literals[0].atom {
            Atom::Pred(_, args) => {
                assert_eq!(args[0], Term::constant(b));
            }
            _ => panic!("expected predicate"),
        }
    }

    #[test]
    fn demodulate_no_match() {
        // Unit: f(a) = b
        // Target: p(g(a))  (no f(a) subterm)
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let unit = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
            "unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(g, vec![Term::constant(a)])],
            ))],
            "target",
        );

        let mut clause_store = HashMap::default();
        clause_store.insert(unit.id, unit.clone());

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(
            &Term::app(f, vec![Term::constant(a)]),
            (
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
                unit.id,
            ),
        );

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
        assert!(result.is_none());
    }

    #[test]
    fn demodulate_with_variable_matching() {
        // Unit: f(X) = X (collapse rule, f(X) > X by weight)
        // Target: p(f(a))
        // Expected: p(a) via matching X=a
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let unit = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::var(0)]),
                Term::var(0),
            ))],
            "unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
            "target",
        );

        let mut clause_store = HashMap::default();
        clause_store.insert(unit.id, unit.clone());

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(
            &Term::app(f, vec![Term::var(0)]),
            (Term::app(f, vec![Term::var(0)]), Term::var(0), unit.id),
        );

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
        assert!(result.is_some());
        let simplified = result.unwrap();
        match &simplified.literals[0].atom {
            Atom::Pred(_, args) => {
                assert_eq!(args[0], Term::constant(a));
            }
            _ => panic!("expected predicate"),
        }
    }

    #[test]
    fn demodulate_non_unit_skipped() {
        // Non-unit clause: a = b ∨ p(c) — not used for demodulation
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");
        let mut id_gen = ClauseIdGen::new();

        let _non_unit = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::eq(Term::constant(a), Term::constant(b))),
                Literal::pos(Atom::pred(p, vec![Term::constant(c)])),
            ],
            "non_unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "target",
        );

        let clause_store = HashMap::default();
        let demod_index = mrs_index::dtree::DTree::new();
        // non_unit is not inserted because it is not a unit equation

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
        assert!(result.is_none());
    }

    /// Run `demodulate_id` with a single unit-equality rule `lhs -> rhs` and
    /// return the derived clause together with its recorded rewrite trace.
    fn demodulate_id_with_rule(
        bank: &mut TermBank,
        lhs: TermId,
        rhs: TermId,
        target_lits: Vec<IdLiteral>,
    ) -> (IdClause, Vec<DemodStepWitness>) {
        let mut id_gen = ClauseIdGen::new();
        let unit = IdClause::new(
            id_gen.next(),
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Eq(lhs, rhs),
            }],
            ClauseSource::Input {
                name: "unit".into(),
                role: "axiom".into(),
            },
        );
        let unit_id = unit.id;
        let target = IdClause::new(
            id_gen.next(),
            target_lits,
            ClauseSource::Input {
                name: "target".into(),
                role: "axiom".into(),
            },
        );

        let mut clause_store = HashMap::default();
        clause_store.insert(unit_id, unit);
        let mut index = mrs_index::stree::STreeId::new();
        index.insert(lhs, bank, (lhs, rhs, unit_id));

        let derived = demodulate_id(
            &target,
            bank,
            &index,
            &clause_store,
            &mut id_gen,
            &Default::default(),
        )
        .expect("demodulation applies");
        let mrs_core::witness::ProofWitness::Demodulation {
            steps,
            rule_parents,
            ..
        } = derived
            .witness
            .as_ref()
            .expect("demodulation records a witness")
        else {
            panic!("expected a demodulation witness")
        };
        assert_eq!(rule_parents.len(), 1);
        for step in steps {
            assert_eq!(step.rule_parent, rule_parents[0]);
        }
        let steps = steps.clone();
        (derived, steps)
    }

    #[test]
    fn demodulate_id_records_the_literal_and_subterm_it_rewrote() {
        // p(f(a), g(f(a))) with rule f(X) = b rewrites the *first* argument,
        // i.e. subterm [0] of literal 0 — not the whole atom, and not the
        // nested f(a) inside the second argument.
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let ca = bank.intern_app(a, smallvec::SmallVec::<[TermId; 4]>::new());
        let cb = bank.intern_app(b, smallvec::SmallVec::<[TermId; 4]>::new());
        let fa = bank.intern_app(f, smallvec::smallvec![ca]);
        let gfa = bank.intern_app(g, smallvec::smallvec![fa]);

        let (derived, steps) = demodulate_id_with_rule(
            &mut bank,
            fa,
            cb,
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Pred(p, smallvec::smallvec![fa, gfa]),
            }],
        );
        // Both occurrences of f(a) are rewritten: the first argument, and the
        // nested one inside g(...). Each is recorded at its own position.
        assert_eq!(steps.len(), 2, "both f(a) occurrences are rewritten");
        let mut paths: Vec<Vec<usize>> = steps.iter().map(|s| s.term_path.clone()).collect();
        paths.sort();
        assert_eq!(paths, vec![vec![0], vec![1, 0]]);
        for step in &steps {
            assert_eq!(step.lit_idx, 0);
        }
        let IdAtom::Pred(_, args) = &derived.literals[0].atom else {
            panic!("expected a predicate atom")
        };
        assert_eq!(args[0], cb);
        let mrs_core::term_bank::TermNode::App(_, nested) = bank.get(args[1]).clone() else {
            panic!("expected an application argument")
        };
        assert_eq!(
            nested[0], cb,
            "the nested f(a) inside g(...) is rewritten too"
        );
    }

    #[test]
    fn demodulate_id_records_the_full_nested_path() {
        // q(f(f(a))) with rule f(X) = b: the rewrite is the *inner* f, whose
        // path from the literal is [0, 0]. A path that lost its ancestors
        // would replay the rewrite at the outer f and produce q(f(b)) — which
        // is the clause we assert against here.
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let ca = bank.intern_app(a, smallvec::SmallVec::<[TermId; 4]>::new());
        let cb = bank.intern_app(b, smallvec::SmallVec::<[TermId; 4]>::new());
        let fa = bank.intern_app(f, smallvec::smallvec![ca]);
        let ffa = bank.intern_app(f, smallvec::smallvec![fa]);
        let ffb = bank.intern_app(f, smallvec::smallvec![cb]);

        let (derived, steps) = demodulate_id_with_rule(
            &mut bank,
            fa,
            cb,
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Pred(q, smallvec::smallvec![ffa]),
            }],
        );
        assert!(!steps.is_empty());
        for step in &steps {
            assert!(
                step.term_path.len() >= 2,
                "a nested rewrite must keep its ancestor path, got {:?}",
                step.term_path
            );
            assert_eq!(step.term_path[0], 0, "argument 0 of the predicate");
        }
        let IdAtom::Pred(_, args) = &derived.literals[0].atom else {
            panic!("expected a predicate atom")
        };
        assert_eq!(args[0], ffb, "q(f(b)): only the inner f was rewritten");
    }

    #[test]
    fn demodulate_id_records_the_equality_side_it_rewrote() {
        // f(a) = f(b) with rule f(X) = g(X): path 0 is the left side and 1 the
        // right side, so a recorded path is always a single side selector here.
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let ca = bank.intern_app(a, smallvec::SmallVec::<[TermId; 4]>::new());
        let cb = bank.intern_app(b, smallvec::SmallVec::<[TermId; 4]>::new());
        let fa = bank.intern_app(f, smallvec::smallvec![ca]);
        let fb = bank.intern_app(f, smallvec::smallvec![cb]);
        let ga = bank.intern_app(g, smallvec::smallvec![ca]);

        let (_derived, steps) = demodulate_id_with_rule(
            &mut bank,
            fa,
            ga,
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Eq(fa, fb),
            }],
        );
        assert!(!steps.is_empty());
        for step in &steps {
            assert_eq!(step.lit_idx, 0);
            assert_eq!(
                step.term_path.len(),
                1,
                "an equality side is selected by a single index"
            );
            assert!(step.term_path[0] <= 1);
        }
    }
}

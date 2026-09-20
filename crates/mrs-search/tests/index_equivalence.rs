//! Indexed lookup equivalence to linear inference generation.
//!
//! The given-clause loop never scans the whole processed set: it asks
//! [`mrs_index::literal_index::LiteralIndex`] (and the demodulation
//! [`mrs_index::stree::STreeId`]) for inference partners, and the
//! discrimination/feature-vector trees answer with an *imperfect* filter.
//! The documented contract is one-directional: the filter may over-approximate
//! (false positives are re-checked exactly by callers) but must never miss a
//! true partner (a miss would silently drop a complete inference).
//!
//! These tests pin that contract differentially: for clause stores whose
//! shapes are mined from solved CASC runs (see `results/` — GRP123-4.004
//! solved Satisfiable, plus UEQ-style equational stores), every query the
//! engine can issue is answered both by the index and by a naive linear
//! scan with an independent exact oracle (Robinson unification / matching on
//! legacy terms, `mrs_calculus::subsumption`), and the suite asserts:
//!
//! - **recall**: every exact partner is in the indexed result;
//! - **validity**: every indexed hit satisfies the coarse filter
//!   (same predicate + complementary polarity for resolution, the FVI
//!   necessary condition for subsumption candidates);
//! - **removal consistency**: recall still holds after removals, and removed
//!   clauses never appear in any result.

use std::collections::HashSet;

use mrs_calculus::subsumption::{subsumes_id, subsumption_resolution_id};
use mrs_core::SymbolTable;
use mrs_core::clause::{ClauseId, ClauseSource};
use mrs_core::symbol::SymbolId;
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId};
use mrs_index::fvi::FeatureVector;
use mrs_index::literal_index::LiteralIndex;
use mrs_index::stree::STreeId;
use smallvec::smallvec;

// ---------------------------------------------------------------------------
// Fixture builder
// ---------------------------------------------------------------------------

struct Fixture {
    syms: SymbolTable,
    bank: TermBank,
    clauses: Vec<IdClause>,
    next_id: u64,
}

impl Fixture {
    fn new() -> Self {
        Self {
            syms: SymbolTable::new(),
            bank: TermBank::new(),
            clauses: Vec::new(),
            next_id: 1,
        }
    }

    fn sym(&mut self, name: &str) -> SymbolId {
        self.syms.intern(name)
    }

    fn var(&mut self, v: u32) -> TermId {
        self.bank.intern_var(v)
    }

    fn con(&mut self, sym: SymbolId) -> TermId {
        self.bank.intern_app(sym, smallvec![])
    }

    fn app(&mut self, sym: SymbolId, args: Vec<TermId>) -> TermId {
        self.bank.intern_app(sym, args)
    }

    fn plit(&self, positive: bool, sym: SymbolId, args: Vec<TermId>) -> IdLiteral {
        IdLiteral {
            positive,
            atom: IdAtom::Pred(sym, args.into()),
        }
    }

    fn eqlit(&self, positive: bool, l: TermId, r: TermId) -> IdLiteral {
        IdLiteral {
            positive,
            atom: IdAtom::Eq(l, r),
        }
    }

    fn push(&mut self, lits: Vec<IdLiteral>) -> ClauseId {
        let id = ClauseId(self.next_id);
        self.next_id += 1;
        let clause = IdClause::new(
            id,
            lits,
            ClauseSource::Inference {
                rule: "fixture",
                parents: vec![].into(),
            },
        );
        self.clauses.push(clause);
        id
    }

    fn index(&self) -> LiteralIndex {
        let mut index = LiteralIndex::new();
        for clause in &self.clauses {
            index.insert(clause.clone(), &self.bank);
        }
        index
    }

    /// Every non-variable subterm occurring anywhere in the store.
    fn store_terms(&self) -> Vec<TermId> {
        let mut terms = Vec::new();
        let mut seen = HashSet::new();
        for clause in &self.clauses {
            for lit in &clause.literals {
                let args: Vec<TermId> = match &lit.atom {
                    IdAtom::Pred(_, args) => args.to_vec(),
                    IdAtom::Eq(l, r) => vec![*l, *r],
                };
                for arg in args {
                    for sub in self.bank.non_variable_subterms(arg) {
                        if seen.insert(sub) {
                            terms.push(sub);
                        }
                    }
                }
            }
        }
        terms
    }
}

// ---------------------------------------------------------------------------
// Exact oracles (independent of the index implementation)
// ---------------------------------------------------------------------------

fn legacy_unify_terms(a: TermId, b: TermId, bank: &TermBank) -> bool {
    let la = bank.to_legacy(a);
    let lb = bank.to_legacy(b);
    mrs_unify::robinson::unify(&la, &lb).is_ok()
}

/// Whole-atom unification with a single substitution. (An earlier revision
/// unified argument pairs with fresh substitutions each, which wrongly
/// reported `product(c2,c4,Z)` ~/​~ `product(X,X,X)` as unifiable by binding
/// `X` inconsistently per position. The discrimination tree correctly
/// rejects such pairs, so the oracle must thread one substitution.)
fn legacy_unify_atoms(a: &IdAtom, b: &IdAtom, bank: &TermBank) -> bool {
    use mrs_core::term::Term;
    match (a, b) {
        (IdAtom::Pred(p1, args1), IdAtom::Pred(p2, args2)) => {
            if p1 != p2 || args1.len() != args2.len() {
                return false;
            }
            let la: Vec<Term> = args1.iter().map(|t| bank.to_legacy(*t)).collect();
            let lb: Vec<Term> = args2.iter().map(|t| bank.to_legacy(*t)).collect();
            mrs_unify::robinson::unify(&Term::app(*p1, la), &Term::app(*p2, lb)).is_ok()
        }
        _ => false,
    }
}

/// Linear exact resolution partners: stored clauses with a complementary
/// same-predicate literal whose atom unifies (predicate atoms only, mirroring
/// engine usage — the index answers `[]` for equality queries).
fn exact_resolution_partners(
    query: &IdLiteral,
    store: &[IdClause],
    bank: &TermBank,
) -> HashSet<ClauseId> {
    let mut out = HashSet::new();
    if !matches!(query.atom, IdAtom::Pred(..)) {
        return out;
    }
    for clause in store {
        for lit in &clause.literals {
            if lit.positive != query.positive && legacy_unify_atoms(&lit.atom, &query.atom, bank) {
                out.insert(clause.id);
                break;
            }
        }
    }
    out
}

/// Linear exact superposition targets: stored clauses containing a
/// non-variable subterm unifiable with the query term.
fn exact_superposition_targets(
    query: TermId,
    store: &[IdClause],
    bank: &TermBank,
) -> HashSet<ClauseId> {
    let mut out = HashSet::new();
    for clause in store {
        'lits: for lit in &clause.literals {
            let args: Vec<TermId> = match &lit.atom {
                IdAtom::Pred(_, args) => args.to_vec(),
                IdAtom::Eq(l, r) => vec![*l, *r],
            };
            for arg in args {
                for sub in bank.non_variable_subterms(arg) {
                    if legacy_unify_terms(sub, query, bank) {
                        out.insert(clause.id);
                        break 'lits;
                    }
                }
            }
        }
    }
    out
}

/// Linear exact superposition sources: stored clauses with a positive
/// equality whose non-variable side unifies with the query.
fn exact_superposition_sources(
    query: TermId,
    store: &[IdClause],
    bank: &TermBank,
) -> HashSet<ClauseId> {
    let mut out = HashSet::new();
    for clause in store {
        'lits: for lit in &clause.literals {
            if !lit.positive {
                continue;
            }
            if let IdAtom::Eq(l, r) = &lit.atom {
                for side in [*l, *r] {
                    if matches!(bank.get(side), mrs_core::term_bank::TermNode::Var(_)) {
                        continue;
                    }
                    if legacy_unify_terms(side, query, bank) {
                        out.insert(clause.id);
                        break 'lits;
                    }
                }
            }
        }
    }
    out
}

/// Linear exact demodulation rules: stored `(lhs, _, _)` triples whose LHS
/// matches (generalizes) the query term.
fn exact_demod_rules(
    query: TermId,
    rules: &[(TermId, TermId, ClauseId)],
    bank: &TermBank,
) -> HashSet<ClauseId> {
    rules
        .iter()
        .filter(|(lhs, _, _)| mrs_unify::matching::match_term_id(*lhs, query, bank).is_ok())
        .map(|(_, _, id)| *id)
        .collect()
}

// ---------------------------------------------------------------------------
// Realistic fixtures mined from solved runs
// ---------------------------------------------------------------------------

/// GRP123-4.004 vocabulary (solved Satisfiable in run 20260616_152730):
/// `equalish/2` ground disequalities, `product/3` mixed clauses,
/// `group_element/1` units including a variable unit, plus a 5-literal
/// negated-conjecture shape. Also carries a defensive mixed-arity trap on a
/// fresh predicate (`trap/1` vs `trap/2`).
fn grp_store() -> Fixture {
    let mut f = Fixture::new();
    let equalish = f.sym("equalish");
    let product = f.sym("product");
    let group_element = f.sym("group_element");
    let trap = f.sym("trap");
    let e1 = f.sym("e_1");
    let e2 = f.sym("e_2");
    let e3 = f.sym("e_3");
    let e4 = f.sym("e_4");
    let c1 = f.con(e1);
    let c2 = f.con(e2);
    let c3 = f.con(e3);
    let c4 = f.con(e4);
    let x = f.var(0);
    let y = f.var(1);
    let z = f.var(2);
    let w = f.var(3);

    // Ground disequalities.
    f.push(vec![f.plit(false, equalish, vec![c1, c4])]);
    f.push(vec![f.plit(false, equalish, vec![c2, c3])]);
    f.push(vec![f.plit(false, equalish, vec![c1, c3])]);
    // Idempotence + column surjectivity shapes.
    f.push(vec![f.plit(true, product, vec![x, x, x])]);
    f.push(vec![
        f.plit(false, group_element, vec![y]),
        f.plit(true, product, vec![x, c4, y]),
        f.plit(true, product, vec![x, c3, y]),
        f.plit(false, group_element, vec![x]),
    ]);
    // Total-function shape with a positive equalish literal.
    f.push(vec![
        f.plit(false, product, vec![x, y, w]),
        f.plit(true, equalish, vec![w, z]),
        f.plit(false, product, vec![x, y, z]),
    ]);
    // Cancellation shape.
    f.push(vec![
        f.plit(true, equalish, vec![w, z]),
        f.plit(false, product, vec![x, z, y]),
        f.plit(false, product, vec![x, w, y]),
    ]);
    // Units: two ground + one variable (the variable unit subsumes both).
    f.push(vec![f.plit(true, group_element, vec![c3])]);
    f.push(vec![f.plit(true, group_element, vec![c1])]);
    f.push(vec![f.plit(true, group_element, vec![x])]);
    // Negated-conjecture shape: 5 literals, mixed polarity.
    f.push(vec![
        f.plit(false, product, vec![x, y, z]),
        f.plit(false, product, vec![w, y, x]),
        f.plit(true, equalish, vec![x, c2]),
        f.plit(false, product, vec![w, c4, c2]),
        f.plit(false, product, vec![c2, c4, z]),
    ]);
    // Defensive trap: same predicate at two arities must not panic or
    // cross-match (the exact oracle says they never unify).
    f.push(vec![f.plit(true, trap, vec![c1])]);
    f.push(vec![f.plit(false, trap, vec![c1, c2])]);
    f
}

/// UEQ-style equational store: ground unit equalities, a nested rewrite
/// rule, non-unit clauses mixing predicates with equalities, and a negative
/// equality unit. Exercises superposition sources/targets and demodulation.
fn equational_store() -> Fixture {
    let mut f = Fixture::new();
    let p = f.sym("p");
    let q = f.sym("q");
    let r = f.sym("r");
    let func = f.sym("f");
    let g = f.sym("g");
    let a = f.sym("a");
    let b = f.sym("b");
    let ca = f.con(a);
    let cb = f.con(b);
    let x = f.var(0);
    let y = f.var(1);
    let fa = f.app(func, vec![ca]);
    let fb = f.app(func, vec![cb]);
    let ffa = f.app(func, vec![fa]);
    let ga = f.app(g, vec![ca]);
    let gb = f.app(g, vec![cb]);
    let fx = f.app(func, vec![x]);

    f.push(vec![f.eqlit(true, fa, cb)]); // f(a) = b
    f.push(vec![f.eqlit(true, ca, gb)]); // a = g(b)
    f.push(vec![f.eqlit(true, ffa, ga)]); // f(f(a)) = g(a)
    f.push(vec![f.plit(true, p, vec![fa])]); // p(f(a))
    f.push(vec![f.plit(true, q, vec![cb, x]), f.eqlit(false, fx, x)]); // q(b,X) | f(X) != X
    f.push(vec![f.eqlit(false, fa, cb)]); // f(a) != b
    f.push(vec![f.plit(true, r, vec![ga, y])]); // r(g(a),Y)
    f.push(vec![f.plit(false, p, vec![fb])]); // ~p(f(b))
    f.push(vec![f.plit(true, p, vec![x])]); // p(X): subsumes p(f(a))
    f.push(vec![f.plit(false, p, vec![fa])]); // ~p(f(a)): resolves with p(f(a))
    f
}

// ---------------------------------------------------------------------------
// Shared checkers
// ---------------------------------------------------------------------------

fn check_resolution_recall(clauses: &[IdClause], bank: &TermBank, index: &LiteralIndex, tag: &str) {
    let live: HashSet<ClauseId> = index.iter().map(|c| c.id).collect();
    let mut exact_total = 0;
    for clause in clauses {
        if !live.contains(&clause.id) {
            continue;
        }
        for (li, lit) in clause.literals.iter().enumerate() {
            let hits = index.get_unifiable_resolution_partners(&lit.atom, lit.positive, bank);
            let got: HashSet<ClauseId> = hits.iter().map(|c| c.id).collect();
            if matches!(lit.atom, IdAtom::Eq(..)) {
                // Engine usage: equality queries get no resolution partners
                // (equality literals are handled by the equality rules).
                assert!(
                    got.is_empty(),
                    "{tag}: equality query must return no resolution partners"
                );
                continue;
            }
            let want = exact_resolution_partners(lit, clauses, bank);
            let mut exact_here = 0;
            for id in want.intersection(&live) {
                exact_here += 1;
                assert!(
                    got.contains(id),
                    "{tag}: index missed exact resolution partner {id:?} for query {:?}[{li}]",
                    clause.id
                );
            }
            exact_total += exact_here;
            // Validity: complementary polarity on the same predicate.
            let (qpred, qpos) = match &lit.atom {
                IdAtom::Pred(sym, _) => (*sym, lit.positive),
                IdAtom::Eq(..) => unreachable!(),
            };
            for hit in hits {
                assert!(
                    live.contains(&hit.id),
                    "{tag}: indexed partner is not a live clause"
                );
                assert!(
                    hit.literals.iter().any(|l| matches!(&l.atom,
                        IdAtom::Pred(sym, _) if *sym == qpred && l.positive != qpos)),
                    "{tag}: indexed partner lacks a complementary same-predicate literal"
                );
            }
        }
    }
    assert!(
        exact_total > 0,
        "{tag}: resolution recall vacuous, no exact partners found"
    );
}

/// Returns `(exact_targets, exact_sources)` so callers can assert
/// non-vacuity where the store shape guarantees exact hits (pure-relational
/// stores have no positive equalities, hence no exact sources).
fn check_superposition_recall(
    clauses: &[IdClause],
    bank: &TermBank,
    index: &LiteralIndex,
    queries: &[TermId],
    tag: &str,
) -> (u64, u64) {
    let live: HashSet<ClauseId> = index.iter().map(|c| c.id).collect();
    let mut exact_targets = 0;
    let mut exact_sources = 0;
    for &q in queries {
        let got_targets: HashSet<ClauseId> = index
            .get_superposition_targets(q, bank)
            .into_iter()
            .map(|c| c.id)
            .collect();
        for id in exact_superposition_targets(q, clauses, bank) {
            if live.contains(&id) {
                exact_targets += 1;
                assert!(
                    got_targets.contains(&id),
                    "{tag}: index missed exact superposition target {id:?}"
                );
            }
        }
        let mut got_sources = Vec::new();
        for hit in index.get_superposition_sources(q, bank) {
            // Validity: every source must carry a positive equality.
            assert!(
                hit.literals
                    .iter()
                    .any(|l| l.positive && matches!(l.atom, IdAtom::Eq(..))),
                "{tag}: superposition source lacks a positive equality"
            );
            assert!(
                live.contains(&hit.id),
                "{tag}: superposition source is not a live clause"
            );
            got_sources.push(hit.id);
        }
        let got_sources: HashSet<ClauseId> = got_sources.into_iter().collect();
        for id in exact_superposition_sources(q, clauses, bank) {
            if live.contains(&id) {
                exact_sources += 1;
                assert!(
                    got_sources.contains(&id),
                    "{tag}: index missed exact superposition source {id:?}"
                );
            }
        }
    }
    assert!(
        exact_targets > 0,
        "{tag}: superposition-target recall vacuous"
    );
    (exact_targets, exact_sources)
}

/// Returns `(exact_subsumers, exact_sr, exact_subsumed, exact_bsr)` so
/// callers assert non-vacuity per direction.
fn check_subsumption_recall(
    index: &LiteralIndex,
    bank: &mut TermBank,
    tag: &str,
) -> (u64, u64, u64, u64) {
    let live: HashSet<ClauseId> = index.iter().map(|c| c.id).collect();
    let mut exact_subsumers = 0;
    let mut exact_sr = 0;
    let mut exact_subsumed = 0;
    let mut exact_bsr = 0;
    for target in index.iter() {
        let target_fv = FeatureVector::from_id_clause(target, &*bank);
        let got_subsumers: HashSet<ClauseId> = index
            .get_subsumption_candidates(&target_fv)
            .into_iter()
            .map(|c| c.id)
            .collect();
        // Validity: the FVI necessary condition holds for every candidate.
        for hit in index.get_subsumption_candidates(&target_fv) {
            let hit_fv = FeatureVector::from_id_clause(&hit, &*bank);
            assert!(
                hit_fv.can_subsume(&target_fv),
                "{tag}: subsumption candidate violates the FVI necessary condition"
            );
        }
        let got_sr: HashSet<ClauseId> = index
            .get_subsumption_resolution_candidates(&target_fv)
            .into_iter()
            .map(|c| c.id)
            .collect();
        for cand in index.iter() {
            if !live.contains(&cand.id) || cand.id == target.id {
                continue;
            }
            if subsumes_id(cand, target, bank) {
                exact_subsumers += 1;
                assert!(
                    got_subsumers.contains(&cand.id),
                    "{tag}: index missed exact subsumer {:?} of {:?}",
                    cand.id,
                    target.id
                );
            }
            if subsumption_resolution_id(cand, target, bank).is_some() {
                exact_sr += 1;
                assert!(
                    got_sr.contains(&cand.id),
                    "{tag}: index missed exact SR partner {:?} of {:?}",
                    cand.id,
                    target.id
                );
            }
        }
    }
    for subsumer in index.iter() {
        let subsumer_fv = FeatureVector::from_id_clause(subsumer, &*bank);
        let got_subsumed: HashSet<ClauseId> = index
            .get_subsumed_candidates(&subsumer_fv)
            .into_iter()
            .map(|c| c.id)
            .collect();
        let got_bsr: HashSet<ClauseId> = index
            .get_backward_subsumption_resolution_candidates(&subsumer_fv)
            .into_iter()
            .map(|c| c.id)
            .collect();
        for cand in index.iter() {
            if !live.contains(&cand.id) || cand.id == subsumer.id {
                continue;
            }
            if subsumes_id(subsumer, cand, bank) {
                exact_subsumed += 1;
                assert!(
                    got_subsumed.contains(&cand.id),
                    "{tag}: index missed exact subsumed clause {:?} of {:?}",
                    cand.id,
                    subsumer.id
                );
            }
            if subsumption_resolution_id(subsumer, cand, bank).is_some() {
                exact_bsr += 1;
                assert!(
                    got_bsr.contains(&cand.id),
                    "{tag}: index missed exact backward-SR target {:?} of {:?}",
                    cand.id,
                    subsumer.id
                );
            }
        }
    }
    (exact_subsumers, exact_sr, exact_subsumed, exact_bsr)
}

/// Returns the exact-rule hit count so callers assert non-vacuity where
/// the store shape guarantees rules (pure-relational stores have none).
fn check_demod_recall(
    queries: &[TermId],
    rules: &[(TermId, TermId, ClauseId)],
    demod: &STreeId<(TermId, TermId, ClauseId)>,
    live: &HashSet<ClauseId>,
    bank: &TermBank,
    tag: &str,
) -> u64 {
    let mut exact_rules = 0;
    for &q in queries {
        let got: HashSet<ClauseId> = demod
            .get_generalizations(q, bank)
            .into_iter()
            .map(|(_, _, id)| id)
            .collect();
        for id in exact_demod_rules(q, rules, bank) {
            if live.contains(&id) {
                exact_rules += 1;
                assert!(
                    got.contains(&id),
                    "{tag}: demod index missed exact rule {id:?}"
                );
            }
        }
        for (_, _, id) in demod.get_generalizations(q, bank) {
            assert!(
                live.contains(&id),
                "{tag}: demod index returned a stale rule"
            );
        }
    }
    exact_rules
}

/// Build the demod rule set mirroring engine insertion: every non-variable
/// side of every positive unit equality becomes a stored LHS rule.
fn demod_rules(fx: &Fixture) -> Vec<(TermId, TermId, ClauseId)> {
    let mut rules = Vec::new();
    for clause in &fx.clauses {
        if clause.literals.len() != 1 || !clause.literals[0].positive {
            continue;
        }
        if let IdAtom::Eq(l, r) = &clause.literals[0].atom {
            for (from, to) in [(*l, *r), (*r, *l)] {
                if matches!(fx.bank.get(from), mrs_core::term_bank::TermNode::Var(_)) {
                    continue;
                }
                rules.push((from, to, clause.id));
            }
        }
    }
    rules
}

fn build_demod_index(
    rules: &[(TermId, TermId, ClauseId)],
    bank: &TermBank,
) -> STreeId<(TermId, TermId, ClauseId)> {
    let mut demod = STreeId::new();
    for (from, to, id) in rules {
        demod.insert(*from, bank, (*from, *to, *id));
    }
    demod
}

/// Store terms plus foreign robustness queries (a fresh constant and a deep
/// nest over it, matching nothing in either fixture).
fn queries_with_foreign(fx: &mut Fixture) -> Vec<TermId> {
    let mut queries = fx.store_terms();
    let z = fx.sym("zz_foreign");
    let cz = fx.con(z);
    let h = fx.sym("hh_foreign");
    let inner = fx.app(h, vec![cz]);
    let mid = fx.app(h, vec![inner]);
    let deep = fx.app(h, vec![mid]);
    queries.push(cz);
    queries.push(deep);
    queries
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn legacy_oracle_distinguishes_shapes() {
    let mut fx = Fixture::new();
    let p = fx.sym("p");
    let q = fx.sym("q");
    let a = fx.sym("a");
    let ca = fx.con(a);
    let x = fx.var(0);
    let t_ground = fx.app(p, vec![ca]);
    let t_var = fx.app(p, vec![x]);
    assert!(legacy_unify_terms(t_ground, t_ground, &fx.bank));
    assert!(legacy_unify_terms(t_ground, t_var, &fx.bank));
    assert!(legacy_unify_terms(t_var, t_ground, &fx.bank));
    let t_other = fx.app(q, vec![ca]);
    assert!(!legacy_unify_terms(t_ground, t_other, &fx.bank));
    assert!(!legacy_unify_terms(
        t_ground,
        fx.app(p, vec![ca, ca]),
        &fx.bank
    ));
}

#[test]
fn grp_resolution_partners_match_linear_scan() {
    let fx = grp_store();
    let index = fx.index();
    check_resolution_recall(&fx.clauses, &fx.bank, &index, "grp");
}

#[test]
fn equational_resolution_partners_match_linear_scan() {
    let fx = equational_store();
    let index = fx.index();
    check_resolution_recall(&fx.clauses, &fx.bank, &index, "equational");
}

#[test]
fn grp_superposition_matches_linear_scan() {
    let mut fx = grp_store();
    let queries = queries_with_foreign(&mut fx);
    let index = fx.index();
    let (targets, sources) =
        check_superposition_recall(&fx.clauses, &fx.bank, &index, &queries, "grp");
    assert!(targets > 0);
    // The GRP store is pure-relational: no positive equalities exist, so no
    // exact superposition source can exist either.
    assert_eq!(sources, 0);
}

#[test]
fn equational_superposition_matches_linear_scan() {
    let mut fx = equational_store();
    let queries = queries_with_foreign(&mut fx);
    let index = fx.index();
    let (targets, sources) =
        check_superposition_recall(&fx.clauses, &fx.bank, &index, &queries, "equational");
    assert!(targets > 0);
    assert!(sources > 0);
}

#[test]
fn grp_subsumption_matches_linear_scan() {
    let mut fx = grp_store();
    let index = fx.index();
    // Sanity: the variable unit really does subsume ground units here, so
    // the recall assertions below are non-vacuous.
    let group_element = fx.syms.resolve_name("group_element").expect("grp symbols");
    let var_unit = fx
        .clauses
        .iter()
        .find(|c| {
            c.literals.len() == 1
                && matches!(&c.literals[0].atom,
                IdAtom::Pred(sym, args)
                    if *sym == group_element
                        && args.iter().any(|t| matches!(
                            fx.bank.get(*t),
                            mrs_core::term_bank::TermNode::Var(_)
                        )))
        })
        .expect("grp store must contain a variable group_element unit")
        .clone();
    let mut subsumed_count = 0;
    for target in &fx.clauses {
        if target.id != var_unit.id && subsumes_id(&var_unit, target, &mut fx.bank) {
            subsumed_count += 1;
        }
    }
    assert!(
        subsumed_count >= 2,
        "grp variable unit should subsume at least two ground units"
    );
    let (subsumers, sr, subsumed, bsr) = check_subsumption_recall(&index, &mut fx.bank, "grp");
    assert!(subsumers > 0 && sr > 0 && subsumed > 0 && bsr > 0);
}

#[test]
fn equational_subsumption_matches_linear_scan() {
    let mut fx = equational_store();
    let index = fx.index();
    let (subsumers, sr, subsumed, bsr) =
        check_subsumption_recall(&index, &mut fx.bank, "equational");
    assert!(subsumers > 0 && sr > 0 && subsumed > 0 && bsr > 0);
}

#[test]
fn demod_generalizations_match_linear_scan() {
    let mut fx = equational_store();
    let queries = queries_with_foreign(&mut fx);
    let rules = demod_rules(&fx);
    assert!(!rules.is_empty(), "equational store must yield demod rules");
    let demod = build_demod_index(&rules, &fx.bank);
    let live: HashSet<ClauseId> = fx.clauses.iter().map(|c| c.id).collect();
    let exact_rules = check_demod_recall(&queries, &rules, &demod, &live, &fx.bank, "demod");
    assert!(exact_rules > 0);
}

#[test]
fn removal_keeps_equivalence_and_drops_stale_hits() {
    for (tag, mut fx, expect_sources) in [
        ("removal-grp", grp_store(), false),
        ("removal-eq", equational_store(), true),
    ] {
        // Remove the first two clauses: enough to exercise every
        // sub-index removal path while preserving the clauses that keep
        // the recall assertions non-vacuous (variable units, SR pairs).
        let victim_ids: HashSet<ClauseId> = fx.clauses.iter().take(2).map(|c| c.id).collect();
        assert_eq!(victim_ids.len(), 2);
        let mut index = fx.index();
        for id in &victim_ids {
            index.remove(*id, &fx.bank);
        }
        // Stale hits: no query may return a removed clause.
        for clause in &fx.clauses {
            for lit in &clause.literals {
                if matches!(lit.atom, IdAtom::Eq(..)) {
                    continue;
                }
                for hit in
                    index.get_unifiable_resolution_partners(&lit.atom, lit.positive, &fx.bank)
                {
                    assert!(
                        !victim_ids.contains(&hit.id),
                        "{tag}: stale removed clause in resolution results"
                    );
                }
            }
        }
        // Recall still holds over the live set.
        check_resolution_recall(&fx.clauses, &fx.bank, &index, tag);
        let queries = queries_with_foreign(&mut fx);
        let (_, sources) = check_superposition_recall(&fx.clauses, &fx.bank, &index, &queries, tag);
        assert_eq!(
            sources > 0,
            expect_sources,
            "{tag}: unexpected superposition-source non-vacuity"
        );
        let (subsumers, sr, subsumed, bsr) = check_subsumption_recall(&index, &mut fx.bank, tag);
        assert!(subsumers > 0 && sr > 0 && subsumed > 0 && bsr > 0);
        // The demod index is exercised through its own removal path:
        // build from all rules, then remove the victim rules in place.
        let all_rules = demod_rules(&fx);
        let mut demod = build_demod_index(&all_rules, &fx.bank);
        let mut live_rules = Vec::new();
        for (from, to, id) in &all_rules {
            if victim_ids.contains(id) {
                assert!(
                    demod.remove(*from, &fx.bank, &(*from, *to, *id)),
                    "{tag}: demod removal must report success"
                );
            } else {
                live_rules.push((*from, *to, *id));
            }
        }
        let live: HashSet<ClauseId> = index.iter().map(|c| c.id).collect();
        let exact_rules = check_demod_recall(&queries, &live_rules, &demod, &live, &fx.bank, tag);
        // Only the equational store carries unit equalities; the GRP store
        // exercises the empty-rule path instead.
        assert_eq!(exact_rules > 0, expect_sources, "{tag}: demod non-vacuity");
    }
}

#[test]
fn empty_store_queries_are_empty() {
    let mut fx = Fixture::new();
    let index = LiteralIndex::new();
    let p = fx.sym("p");
    let a = fx.sym("a");
    let ca = fx.con(a);
    let atom = IdAtom::Pred(p, smallvec![ca]);
    assert!(
        index
            .get_unifiable_resolution_partners(&atom, true, &fx.bank)
            .is_empty()
    );
    assert!(index.get_superposition_targets(ca, &fx.bank).is_empty());
    assert!(index.get_superposition_sources(ca, &fx.bank).is_empty());
    let target = IdClause::new(
        ClauseId(999),
        vec![IdLiteral {
            positive: true,
            atom,
        }],
        ClauseSource::Inference {
            rule: "probe",
            parents: vec![].into(),
        },
    );
    let fv = FeatureVector::from_id_clause(&target, &fx.bank);
    assert!(index.get_subsumption_candidates(&fv).is_empty());
    assert!(index.get_subsumed_candidates(&fv).is_empty());
    assert!(index.get_subsumption_resolution_candidates(&fv).is_empty());
    assert!(
        index
            .get_backward_subsumption_resolution_candidates(&fv)
            .is_empty()
    );
    let demod: STreeId<(TermId, TermId, ClauseId)> = STreeId::new();
    assert!(demod.get_generalizations(ca, &fx.bank).is_empty());
}

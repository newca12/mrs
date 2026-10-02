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
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId, TermNode};

/// Root cell of a term: the symbol and arity a rewrite rule's left-hand side
/// must share with a term for that rule to be able to rewrite it.
///
/// `TermNode` has no separate constant case, so a constant is `App(sym, 0)` and
/// is covered by the same key.
type RootKey = (SymbolId, u8);

/// Cap on memoised terms.
///
/// A memo entry is one interned `TermId` plus a root key and a generation, so
/// the cap is a bound on how much of the term bank the memo can pin. It is
/// sized from the clauses-per-second these searches reach: a run that retires
/// hundreds of thousands of given clauses visits far more distinct terms than
/// this, and past the cap the memo is cleared rather than grown. Clearing costs
/// one rebuild of the entries and is counted, so a run that is thrashing the
/// cap says so instead of quietly paying for it.
const MAX_MEMO_ENTRIES: usize = 400_000;

/// Memo of terms already found irreducible by one demodulation index.
///
/// `rewrite_term_id` is a pure function of the term, the rules in the index,
/// and the literal's AVATAR context, and the search asks it the same question
/// about the same interned term over and over: an argument that occurs in one
/// clause occurs in most of the clauses that mention its symbol. A callgrind
/// profile of a single-strategy run on `casc-j13/UEQ/LAT141-1.p` attributes
/// about 40 % of search instructions to this walk, and almost every answer is
/// "no rule applies".
///
/// # Only the negative answer is memoised
///
/// A positive answer carries a witness — which unit clause was applied, at
/// which term path — and `mrs-proof` replays that witness to justify the step
/// to a checker. A synthesised substitute would be a step that did not happen,
/// the same failure `mrs-search`'s `fvo` module documents. `rewrite_term_id`
/// also returns at the first rewrite, so positives are rare regardless.
///
/// # Invalidation
///
/// A negative result covers the entire term tree. A rule for a nested subterm
/// can make an enclosing term reducible even when its root is unrelated. Every
/// index mutation therefore advances one global generation in O(1), invalidating
/// all prior entries without scanning the memo.
///
/// # Scope
///
/// The memo is only valid for the index it was populated against, so
/// [`demodulate_id`] takes it as an `Option`: a caller rewriting against a
/// temporary index passes `None`. It is also only valid when the clause carries
/// no AVATAR context, because the rule-availability test depends on it; the
/// caller enforces that by passing `None` for a split clause.
pub struct DemodMemo {
    entries: HashMap<TermId, u64>,
    generation: u64,
    lookups: u64,
    hits: u64,
    records: u64,
    evictions: u64,
}

impl DemodMemo {
    pub fn new() -> Self {
        DemodMemo {
            entries: HashMap::default(),
            generation: 0,
            lookups: 0,
            hits: 0,
            records: 0,
            evictions: 0,
        }
    }

    /// Root cell of `term`, or `None` for a variable.
    ///
    /// A variable can never be rewritten: the index only ever holds a
    /// non-variable side of a unit equality, and a variable has no root cell for
    /// that cell to match.
    fn root_key(term: TermId, bank: &TermBank) -> Option<RootKey> {
        match bank.get(term) {
            TermNode::Var(_) => None,
            TermNode::App(sym, args) => Some((*sym, args.len() as u8)),
        }
    }

    /// Returns `true` if `term` is already known to be irreducible.
    pub fn is_irreducible(&mut self, term: TermId, bank: &TermBank) -> bool {
        let Some(_key) = Self::root_key(term, bank) else {
            return true;
        };
        self.lookups += 1;
        let hit = self.entries.get(&term) == Some(&self.generation);
        if hit {
            self.hits += 1;
        }
        hit
    }

    /// Records that `term` was found irreducible by the current index.
    pub fn record_irreducible(&mut self, term: TermId, bank: &TermBank) {
        let Some(_key) = Self::root_key(term, bank) else {
            return;
        };
        if self.entries.len() >= MAX_MEMO_ENTRIES {
            self.entries.clear();
            self.evictions += 1;
        }
        self.entries.insert(term, self.generation);
        self.records += 1;
    }

    /// Declares that the demodulation rule set has changed.
    ///
    /// Irreducibility covers the entire term tree: a rule rooted at a descendant
    /// can change the answer for a memoized parent even when the parent's root
    /// is unrelated. Therefore every mutation invalidates entries across all
    /// roots. The generation makes this O(1); old entries are overwritten on
    /// demand and the existing cap bounds retained stale entries. On the
    /// practically unreachable counter wrap, clearing prevents an ancient entry
    /// from becoming valid again.
    pub fn invalidate(&mut self, term: TermId, bank: &TermBank) {
        if Self::root_key(term, bank).is_some() {
            if let Some(next) = self.generation.checked_add(1) {
                self.generation = next;
            } else {
                self.entries.clear();
                self.generation = 0;
            }
        }
    }

    /// `(lookups, hits, records, evictions)`.
    pub fn stats(&self) -> (u64, u64, u64, u64) {
        (self.lookups, self.hits, self.records, self.evictions)
    }
}

impl Default for DemodMemo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn demodulate_id(
    clause: &IdClause,
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    id_gen: &mut ClauseIdGen,
    ac_syms: &HashSet<SymbolId>,
    memo: Option<&mut DemodMemo>,
) -> Option<IdClause> {
    demodulate_id_until(
        clause,
        bank,
        demod_index,
        clause_store,
        id_gen,
        ac_syms,
        memo,
        None,
    )
}

/// Demodulation with an optional deadline.
///
/// The fixpoint returns as soon as the instant passes, with whatever
/// simplification was reached: `Some` if at least one rewrite applied before
/// the deadline, `None` otherwise. Callers must treat a return after the
/// deadline as "search over budget" rather than a complete simplification,
/// because further rewrites may still have applied. The unbounded variant
/// [`demodulate_id`] passes `None`.
///
/// The bound is checked per fixpoint pass and per literal, so a single call
/// can no longer run past the search deadline no matter how many literals the
/// clause has or how many passes the rewrite cycle needs.
#[allow(clippy::too_many_arguments)]
pub fn demodulate_id_until(
    clause: &IdClause,
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    id_gen: &mut ClauseIdGen,
    ac_syms: &HashSet<SymbolId>,
    memo: Option<&mut DemodMemo>,
    deadline: Option<std::time::Instant>,
) -> Option<IdClause> {
    // A split clause's rewrite availability depends on its AVATAR context, which
    // the memo key does not carry, so a split clause runs unmemoised.
    let mut memo = memo.filter(|_| clause.avatar.is_empty());
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
        if deadline.is_some_and(|limit| std::time::Instant::now() >= limit) {
            break;
        }
        passes += 1;
        let mut changed_this_pass = false;
        for (lit_idx, lit) in current_lits.iter_mut().enumerate() {
            if deadline.is_some_and(|limit| std::time::Instant::now() >= limit) {
                break;
            }
            if rewrite_literal_id_until(
                lit,
                lit_idx,
                &clause.avatar,
                bank,
                demod_index,
                clause_store,
                &mut used_unit_ids,
                &mut steps,
                ac_syms,
                memo.as_deref_mut(),
                deadline,
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
fn rewrite_literal_id_until(
    lit: &mut IdLiteral,
    lit_idx: usize,
    target_avatar: &[u32],
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    used_unit_ids: &mut Vec<ClauseId>,
    steps: &mut Vec<DemodStepWitness>,
    ac_syms: &HashSet<SymbolId>,
    mut memo: Option<&mut DemodMemo>,
    deadline: Option<std::time::Instant>,
) -> bool {
    let mut changed = false;
    // One path buffer for the whole literal, truncated and re-pushed per
    // argument, instead of one `Vec` allocation per argument.
    let mut path = TermPath::new();
    let new_atom = match &lit.atom {
        IdAtom::Pred(p, args) => {
            let arg_ids: smallvec::SmallVec<[TermId; 4]> = args.iter().copied().collect();
            let new_args: smallvec::SmallVec<[TermId; 4]> = arg_ids
                .iter()
                .enumerate()
                .map(|(arg_idx, arg)| {
                    // The recorded path is the full chain of argument indices
                    // from the literal's atom down to the rewritten subterm, so
                    // it is threaded down the recursion rather than rebuilt at
                    // each level.
                    path.clear();
                    path.push(arg_idx);
                    let (new_arg, ch) = rewrite_term_id_until(
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
                        memo.as_deref_mut(),
                        deadline,
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
            let (l, r) = (*l, *r);
            path.clear();
            path.push(0);
            let (new_l, ch_l) = rewrite_term_id_until(
                l,
                lit_idx,
                &mut path,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
                steps,
                ac_syms,
                memo.as_deref_mut(),
                deadline,
            );
            path.clear();
            path.push(1);
            let (new_r, ch_r) = rewrite_term_id_until(
                r,
                lit_idx,
                &mut path,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
                steps,
                ac_syms,
                // Argument position reborrows implicitly, so the binding goes
                // across as-is.
                memo,
                deadline,
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
// `as_deref_mut` is load-bearing here: it reborrows the memo so that the
// recursive call in the middle can also take it. Clippy reads it as a no-op
// because `Option<&mut T>::as_deref_mut()` has the same type, and taking the
// binding by value instead would move it out.
//
// The optional deadline stops the descent: a past deadline returns the term
// unchanged, so the caller observes "no rewrite within budget" for the
// remaining positions.
#[allow(clippy::too_many_arguments, clippy::needless_option_as_deref)]
fn rewrite_term_id_until(
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
    mut memo: Option<&mut DemodMemo>,
    deadline: Option<std::time::Instant>,
) -> (TermId, bool) {
    if deadline.is_some_and(|limit| std::time::Instant::now() >= limit) {
        return (term, false);
    }
    // Already known irreducible against the current index: the whole subtree
    // walk, and every trie descent under it, can be skipped. See `DemodMemo`
    // for why only this answer is cached.
    if let Some(known) = memo.as_deref_mut()
        && known.is_irreducible(term, bank)
    {
        return (term, false);
    }
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
        // Collect the rewritten children instead of rebuilding the argument list
        // eagerly: on the overwhelmingly common no-op path this used to
        // allocate and fill a vector per term node, per literal, per clause.
        let mut rewritten_args: smallvec::SmallVec<[(usize, TermId); 4]> =
            smallvec::SmallVec::new();
        for (arg_idx, arg) in args.iter().copied().enumerate() {
            if deadline.is_some_and(|limit| std::time::Instant::now() >= limit) {
                break;
            }
            path.push(arg_idx);
            let (new_arg, ch) = rewrite_term_id_until(
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
                memo.as_deref_mut(),
                deadline,
            );
            path.pop();
            if ch {
                changed = true;
                rewritten_args.push((arg_idx, new_arg));
            }
        }
        if changed {
            let mut new_args: smallvec::SmallVec<[TermId; 4]> = args.iter().copied().collect();
            for (arg_idx, new_arg) in rewritten_args {
                new_args[arg_idx] = new_arg;
            }
            let app_term = bank.intern_app(sym, new_args);
            return (app_term, true);
        }
    }

    if let Some(fresh) = memo.as_deref_mut() {
        fresh.record_irreducible(term, bank);
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
            None,
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

    #[test]
    fn demodulate_until_expired_deadline_returns_none() {
        // p(f(a)) rewrites to p(b) under f(a) = b, unless the deadline passed.
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let ca = bank.intern_app(a, smallvec::SmallVec::<[TermId; 4]>::new());
        let cb = bank.intern_app(b, smallvec::SmallVec::<[TermId; 4]>::new());
        let fa = bank.intern_app(f, smallvec::smallvec![ca]);

        let mut id_gen = ClauseIdGen::new();
        let unit = IdClause::new(
            id_gen.next(),
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Eq(fa, cb),
            }],
            ClauseSource::Input {
                name: "unit".into(),
                role: "axiom".into(),
            },
        );
        let unit_id = unit.id;
        let target = IdClause::new(
            id_gen.next(),
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Pred(p, smallvec::smallvec![fa]),
            }],
            ClauseSource::Input {
                name: "target".into(),
                role: "axiom".into(),
            },
        );

        let mut clause_store = HashMap::default();
        clause_store.insert(unit_id, unit);
        let mut index = mrs_index::stree::STreeId::new();
        index.insert(fa, &mut bank, (fa, cb, unit_id));

        let full = demodulate_id(
            &target,
            &mut bank,
            &index,
            &clause_store,
            &mut id_gen,
            &Default::default(),
            None,
        );
        assert!(full.is_some());

        let past = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let partial = demodulate_id_until(
            &target,
            &mut bank,
            &index,
            &clause_store,
            &mut id_gen,
            &Default::default(),
            None,
            Some(past),
        );
        assert!(partial.is_none());

        let same = demodulate_id_until(
            &target,
            &mut bank,
            &index,
            &clause_store,
            &mut id_gen,
            &Default::default(),
            None,
            None,
        );
        assert!(same.is_some());
    }

    // ── DemodMemo ─────────────────────────────────────────────────────────────

    fn term(bank: &mut TermBank, t: &Term) -> TermId {
        bank.from_legacy(t)
    }

    fn app1(bank: &mut TermBank, f: SymbolId, c: SymbolId) -> TermId {
        let arg = term(bank, &Term::constant(c));
        bank.intern_app(f, smallvec::SmallVec::from_vec(vec![arg]))
    }

    fn id_pred_clause(id_gen: &mut ClauseIdGen, p: SymbolId, arg: TermId, name: &str) -> IdClause {
        let lits = vec![IdLiteral {
            positive: true,
            atom: IdAtom::Pred(p, smallvec::SmallVec::from_vec(vec![arg])),
        }];
        IdClause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.into(),
                role: "axiom".into(),
            },
        )
    }

    fn add_rule(
        index: &mut mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
        store: &mut HashMap<ClauseId, IdClause>,
        id_gen: &mut ClauseIdGen,
        bank: &mut TermBank,
        lhs: TermId,
        rhs: TermId,
    ) -> ClauseId {
        let lits = vec![IdLiteral {
            positive: true,
            atom: IdAtom::Eq(lhs, rhs),
        }];
        let unit = IdClause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: "unit".into(),
                role: "axiom".into(),
            },
        );
        let unit_id = unit.id;
        index.insert(lhs, bank, (lhs, rhs, unit_id));
        store.insert(unit_id, unit);
        unit_id
    }

    fn no_rewrite(
        target: &IdClause,
        bank: &mut TermBank,
        index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
        store: &HashMap<ClauseId, IdClause>,
        id_gen: &mut ClauseIdGen,
        memo: &mut DemodMemo,
    ) -> bool {
        demodulate_id(
            target,
            bank,
            index,
            store,
            id_gen,
            &Default::default(),
            Some(&mut *memo),
        )
        .is_none()
    }

    #[test]
    fn memo_answers_the_same_thing_as_a_fresh_walk() {
        // `p(f(a))` is irreducible with no rules, stays irreducible when a rule
        // with an unrelated root is added, and must stop claiming so once a rule
        // rooted at f/1 exists. Forgetting the `invalidate` call fails the third
        // step, and it fails it as a *silent* loss of demodulation.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let mut id_gen = ClauseIdGen::new();
        let fa = app1(&mut bank, f, a);
        let fb = app1(&mut bank, f, b);
        let ga = app1(&mut bank, g, a);

        let target = id_pred_clause(&mut id_gen, p, fa, "target");
        let mut index = mrs_index::stree::STreeId::new();
        let mut store = HashMap::default();
        let mut memo = DemodMemo::new();

        assert!(no_rewrite(
            &target,
            &mut bank,
            &index,
            &store,
            &mut id_gen,
            &mut memo
        ));
        assert_eq!(
            memo.stats().1,
            0,
            "the first walk populates the memo, it cannot hit it"
        );
        assert!(no_rewrite(
            &target,
            &mut bank,
            &index,
            &store,
            &mut id_gen,
            &mut memo
        ));
        assert!(
            memo.stats().1 > 0,
            "an identical second walk should be answered from the memo"
        );

        // A rule with a different root cannot rewrite p(f(a)), so the memo entry
        // stays valid and the answer stays "no rewrite".
        add_rule(&mut index, &mut store, &mut id_gen, &mut bank, ga, fa);
        assert!(no_rewrite(
            &target,
            &mut bank,
            &index,
            &store,
            &mut id_gen,
            &mut memo
        ));

        // A rule rooted at f/1 can rewrite it, so the memo must not still claim
        // the term is irreducible. `invalidate` is what the search must do after
        // every index insert; leaving it out here is exactly the bug the
        // assertion is here to catch.
        add_rule(&mut index, &mut store, &mut id_gen, &mut bank, fa, fb);
        memo.invalidate(fa, &bank);
        let hits_before = memo.stats().1;
        assert!(
            !no_rewrite(&target, &mut bank, &index, &store, &mut id_gen, &mut memo),
            "a stale memo entry would suppress this rewrite"
        );
        assert_eq!(
            memo.stats().1,
            hits_before,
            "a stale memo entry would also be reported as a hit"
        );
    }

    #[test]
    fn memo_is_not_used_for_a_split_clause() {
        // The rewrite-availability test reads the literal's AVATAR context, which
        // the memo key does not carry, so a split clause must run unmemoised.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let mut id_gen = ClauseIdGen::new();
        let fa = app1(&mut bank, f, a);
        let fb = app1(&mut bank, f, b);
        let mut index = mrs_index::stree::STreeId::new();
        let mut store = HashMap::default();
        let unit_id = add_rule(&mut index, &mut store, &mut id_gen, &mut bank, fa, fb);

        // The rule is cited in split 2 while the target carries split 1, so the
        // rule is not available and nothing may be rewritten.
        store
            .get_mut(&unit_id)
            .expect("rule is in the store")
            .avatar = vec![2];
        let mut target = id_pred_clause(&mut id_gen, p, fa, "target");
        target.avatar = vec![1];

        let mut memo = DemodMemo::new();
        assert!(no_rewrite(
            &target,
            &mut bank,
            &index,
            &store,
            &mut id_gen,
            &mut memo
        ));
        assert_eq!(
            memo.stats(),
            (0, 0, 0, 0),
            "a split clause must not memoise"
        );
    }

    #[test]
    fn memo_treats_a_variable_as_irreducible() {
        // No rewrite rule can have a variable as its left-hand side, so the
        // answer is structural and must not depend on the index.
        let mut syms = SymbolTable::new();
        let _ = syms.intern("unused");
        let mut bank = TermBank::new();
        let v = bank.intern_var(0);
        let mut memo = DemodMemo::new();
        assert!(memo.is_irreducible(v, &bank));
        memo.record_irreducible(v, &bank);
        assert_eq!(
            memo.stats(),
            (0, 0, 0, 0),
            "a variable is answered structurally, so it costs no lookup either"
        );
    }

    #[test]
    fn memo_entry_is_dropped_when_its_root_changes() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let mut bank = TermBank::new();
        let fa = app1(&mut bank, f, a);
        let mut memo = DemodMemo::new();
        memo.record_irreducible(fa, &bank);
        assert!(memo.is_irreducible(fa, &bank));
        memo.invalidate(fa, &bank);
        assert!(
            !memo.is_irreducible(fa, &bank),
            "invalidating the root must retire exactly the entries that root can reach"
        );
    }

    #[test]
    fn inserting_a_nested_rule_invalidates_parent_irreducibility() {
        // A negative answer for f(a) includes its child a. A newly indexed rule
        // for a must invalidate that parent entry even though the rule's root
        // differs from the memoized term's root.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a_sym = syms.intern("a");
        let b_sym = syms.intern("b");
        let mut bank = TermBank::new();
        let mut id_gen = ClauseIdGen::new();
        let a = term(&mut bank, &Term::constant(a_sym));
        let b = term(&mut bank, &Term::constant(b_sym));
        let fa = bank.intern_app(f, smallvec::smallvec![a]);
        let target = id_pred_clause(&mut id_gen, p, fa, "target");
        let mut index = mrs_index::stree::STreeId::new();
        let mut store = HashMap::default();
        let mut memo = DemodMemo::new();

        assert!(no_rewrite(
            &target,
            &mut bank,
            &index,
            &store,
            &mut id_gen,
            &mut memo
        ));
        assert!(memo.is_irreducible(fa, &bank));

        add_rule(&mut index, &mut store, &mut id_gen, &mut bank, a, b);
        memo.invalidate(a, &bank);
        assert!(
            !no_rewrite(&target, &mut bank, &index, &store, &mut id_gen, &mut memo),
            "a rule for a nested subterm must not be hidden by the parent's old negative cache entry"
        );
    }
}

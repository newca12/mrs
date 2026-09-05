//! Instantiation-based reasoning for EPR (Essentially Propositional Reasoning)
//! problems.
//!
//! A clause set is **EPR** iff every term in every literal is either a variable
//! or a constant (nullary function application).  No function symbol of arity ≥ 1
//! occurs.  For EPR the Herbrand universe is finite — it is exactly the set of
//! distinct constants that appear in the clauses.
//!
//! The [`preprocess_epr`] function detects EPR problems and, if detected, expands
//! every non-ground clause into all of its ground instances over the Herbrand
//! universe.  The resulting ground clause set is then passed to the ordinary
//! given-clause loop, which will quickly saturate (SAT) or derive the empty
//! clause (UNSAT).
//!
//! ## Termination guarantee
//! Instantiation over a finite Herbrand universe terminates.  We add a hard
//! instance-count limit (`MAX_INSTANCES`) to protect against combinatorial
//! explosions caused by clauses with many variables.  When the limit would be
//! exceeded we return `None` and the caller falls back to the standard loop.
use crate::{HashMap, HashSet, SearchConfig, SearchResult};
use smallvec::SmallVec;
use std::time::{Duration, Instant};

use mrs_cadical::{SolveResult, Solver};
use mrs_calculus::ordering::SymbolConfig;
use mrs_calculus::rename::{max_var, rename_clause};
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::{SymbolId, SymbolTable};
use mrs_core::term::{Term, VarId};
use mrs_proof::tstp::format_tstp;

/// Maximum total number of ground instances we are willing to generate.
/// If expanding the clause set would exceed this, we fall back to the
/// standard loop.
const MAX_INSTANCES: usize = 200_000;

/// Tries to preprocess `clauses` as an EPR problem.
///
/// Returns `Some(ground_clauses)` if the clause set is EPR and the expansion
/// fits within [`MAX_INSTANCES`].  Returns `None` otherwise.
///
/// `id_gen` is advanced as new ground clause IDs are minted.
pub fn preprocess_epr(clauses: &[Clause], id_gen: &mut ClauseIdGen) -> Option<Vec<Clause>> {
    if !is_epr(clauses) {
        return None;
    }

    let constants = collect_constants(clauses);
    if constants.is_empty() {
        // No constants at all; the Herbrand universe is empty.
        // This only happens when every clause is all-variable — e.g. { p(X) | ~p(X) }.
        // We fall back to the standard loop which handles this naturally.
        return None;
    }

    // Pre-check: estimate total instance count to avoid blowing up.
    let mut total: usize = 0;
    for clause in clauses {
        let n_vars = collect_clause_vars(clause).len();
        let instances = constants.len().saturating_pow(n_vars as u32);
        total = total.saturating_add(instances);
        if total > MAX_INSTANCES {
            return None;
        }
    }

    // Generate ground instances.
    let mut ground_clauses = Vec::new();
    for clause in clauses {
        let vars: Vec<VarId> = collect_clause_vars(clause).into_iter().collect();
        if vars.is_empty() {
            // Already ground — keep as-is.
            ground_clauses.push(clause.clone());
        } else {
            for subst in enumerate_instances(&vars, &constants) {
                let new_lits: Vec<Literal> = clause
                    .literals
                    .iter()
                    .map(|lit| subst.apply_literal(lit))
                    .collect();
                let new_clause = Clause::new(
                    id_gen.next(),
                    new_lits,
                    ClauseSource::Inference {
                        rule: "instantiation",
                        parents: vec![clause.id].into(),
                    },
                );
                ground_clauses.push(new_clause);
            }
        }
    }

    // Include the original non-ground clauses in the output so they end up
    // in `clause_store` and proof extraction can follow the `Inference` parent
    // pointers back to them.
    //
    // These originals will also be placed in `unprocessed` by `SearchState::new`,
    // which is harmless: they are non-ground and will be subsumed immediately by
    // their ground instances, or will just generate no useful new inferences in
    // a ground-complete search.
    for clause in clauses {
        let vars = collect_clause_vars(clause);
        if !vars.is_empty() {
            ground_clauses.push(clause.clone());
        }
    }

    Some(ground_clauses)
}

/// Returns `true` iff every clause in `clauses` is EPR: all terms are either
/// variables or constants (nullary function applications).
///
/// This is `pub` so that callers can disable AVATAR for EPR problems even when
/// the full Herbrand expansion exceeds [`MAX_INSTANCES`] and `preprocess_epr`
/// returns `None`.
pub fn is_epr(clauses: &[Clause]) -> bool {
    !clauses.is_empty()
        && clauses.iter().all(|c| {
            c.literals.iter().all(|lit| match &lit.atom {
                Atom::Pred(_, args) => args.iter().all(term_is_epr),
                Atom::Eq(l, r) => term_is_epr(l) && term_is_epr(r),
            })
        })
}

/// Returns `true` iff every clause in `clauses` is pure relational EPR:
/// all terms are variables or constants, and all atoms are predicates (no equality).
pub fn is_pure_relational_epr(clauses: &[Clause]) -> bool {
    !clauses.is_empty()
        && clauses.iter().all(|c| {
            c.literals.iter().all(|lit| match &lit.atom {
                Atom::Pred(_, args) => args.iter().all(term_is_epr),
                Atom::Eq(_, _) => false,
            })
        })
}

/// Returns `true` if `term` is a variable or a constant.
pub fn term_is_epr(term: &Term) -> bool {
    match term {
        Term::Var(_) => true,
        Term::App(_, args) => args.is_empty(),
    }
}

/// Collects all distinct constants (nullary function symbols) from `clauses`.
fn collect_constants(clauses: &[Clause]) -> Vec<SymbolId> {
    let mut seen: HashSet<SymbolId> = HashSet::default();
    let mut constants: Vec<SymbolId> = Vec::new();
    for clause in clauses {
        for lit in &clause.literals {
            match &lit.atom {
                Atom::Pred(_, args) => {
                    for t in args {
                        collect_constants_term(t, &mut seen, &mut constants);
                    }
                }
                Atom::Eq(l, r) => {
                    for t in [l, r] {
                        collect_constants_term(t, &mut seen, &mut constants);
                    }
                }
            }
        }
    }
    constants
}

fn collect_constants_term(term: &Term, seen: &mut HashSet<SymbolId>, out: &mut Vec<SymbolId>) {
    if let Term::App(sym, args) = term
        && args.is_empty()
        && seen.insert(*sym)
    {
        out.push(*sym);
    }
}

/// Collects all distinct variable IDs from a single clause.
fn collect_clause_vars(clause: &Clause) -> HashSet<VarId> {
    let mut vars: HashSet<VarId> = HashSet::default();
    for lit in &clause.literals {
        match &lit.atom {
            Atom::Pred(_, args) => {
                for t in args {
                    collect_vars_term(t, &mut vars);
                }
            }
            Atom::Eq(l, r) => {
                collect_vars_term(l, &mut vars);
                collect_vars_term(r, &mut vars);
            }
        }
    }
    vars
}

fn collect_vars_term(term: &Term, vars: &mut HashSet<VarId>) {
    match term {
        Term::Var(v) => {
            vars.insert(*v);
        }
        Term::App(_, args) => {
            for a in args {
                collect_vars_term(a, vars);
            }
        }
    }
}

/// Enumerates all substitutions that map each variable in `vars` to some
/// constant in `constants`.  Returns `|constants|^|vars|` substitutions.
fn enumerate_instances(
    vars: &[VarId],
    constants: &[SymbolId],
) -> Vec<mrs_core::subst::Substitution> {
    let mut result = vec![mrs_core::subst::Substitution::new()];
    for &var in vars {
        let mut next_result = Vec::with_capacity(result.len() * constants.len());
        for subst in &result {
            for &c in constants {
                let mut new_subst = subst.clone();
                new_subst.bind(var, Term::constant(c));
                next_result.push(new_subst);
            }
        }
        result = next_result;
    }
    result
}

// ---------------------------------------------------------------------------
// InstGen Calculus Loop for EPR
// ---------------------------------------------------------------------------

/// Signed propositional literal (DIMACS convention, 1-indexed).
type PL = i32;

/// A propositional clause: sorted, deduplicated literals.
type PC = Vec<PL>;

/// Maximum instances generated by InstGen before falling back to full portfolio.
const MAX_TOTAL_INSTANCES: usize = 50_000;

/// Maximum InstGen iterations (rounds of CaDiCaL + MGU instantiation).
const MAX_ROUNDS: usize = 200;

/// Default time budget for InstGen pre-pass before falling back to portfolio.
const DEFAULT_INSTGEN_TIMEOUT: Duration = Duration::from_millis(1500);

/// Ground atom key in EPR: predicate symbol + list of constant SymbolIds.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct GroundAtom {
    pub pred: SymbolId,
    pub args: SmallVec<[SymbolId; 4]>,
}

/// Bijective mapping between first-order ground atoms and DIMACS variables.
#[derive(Default)]
pub struct PropAbstraction {
    pub atom_to_var: HashMap<GroundAtom, i32>,
    pub var_to_atom: Vec<GroundAtom>,
}

impl PropAbstraction {
    pub fn new() -> Self {
        Self::default()
    }

    /// Abstract a term to a constant SymbolId. Variables are represented by a
    /// distinguished placeholder, while compound terms are rejected because
    /// this abstraction is only used for the pure relational EPR fragment.
    pub fn abstract_term(t: &Term, bot_sym: SymbolId) -> SymbolId {
        match t {
            Term::Var(_) => bot_sym,
            Term::App(sym, _) => *sym,
        }
    }

    /// Abstract a predicate literal to a signed DIMACS integer.
    pub fn abstract_literal(&mut self, lit: &Literal, bot_sym: SymbolId) -> Option<PL> {
        let Atom::Pred(sym, args) = &lit.atom else {
            return None;
        };
        let key = GroundAtom {
            pred: *sym,
            args: args
                .iter()
                .map(|a| Self::abstract_term(a, bot_sym))
                .collect(),
        };
        let var = *self.atom_to_var.entry(key.clone()).or_insert_with(|| {
            let v = (self.var_to_atom.len() + 1) as i32;
            self.var_to_atom.push(key);
            v
        });
        Some(if lit.positive { var } else { -var })
    }

    /// Abstract a clause to a propositional clause `PC`.
    /// Returns `None` when the clause contains an unsupported atom type.
    /// Complementary literals after abstraction are retained: that can be a
    /// first-order non-tautology when the original variables differ.
    pub fn abstract_clause(&mut self, clause: &Clause, bot_sym: SymbolId) -> Option<PC> {
        let mut pc = Vec::with_capacity(clause.literals.len());
        for lit in &clause.literals {
            if let Some(pl) = self.abstract_literal(lit, bot_sym) {
                pc.push(pl);
            }
        }
        pc.sort_unstable();
        pc.dedup();
        Some(pc)
    }
}

fn cmp_terms(t1: &Term, t2: &Term) -> std::cmp::Ordering {
    match (t1, t2) {
        (Term::Var(v1), Term::Var(v2)) => v1.cmp(v2),
        (Term::Var(_), Term::App(..)) => std::cmp::Ordering::Less,
        (Term::App(..), Term::Var(_)) => std::cmp::Ordering::Greater,
        (Term::App(s1, args1), Term::App(s2, args2)) => s1
            .index()
            .cmp(&s2.index())
            .then_with(|| args1.len().cmp(&args2.len()))
            .then_with(|| {
                for (a, b) in args1.iter().zip(args2.iter()) {
                    let ord = cmp_terms(a, b);
                    if !ord.is_eq() {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            }),
    }
}

/// Canonicalize variable IDs to consecutive 0, 1, 2, ... and sort literals.
fn canonicalize_clause(lits: &[Literal]) -> Vec<Literal> {
    let mut var_map: HashMap<VarId, VarId> = HashMap::default();
    let mut next_var: VarId = 0;
    let mut res: Vec<Literal> = lits
        .iter()
        .map(|lit| {
            let atom = match &lit.atom {
                Atom::Pred(sym, args) => {
                    let new_args = args
                        .iter()
                        .map(|t| match t {
                            Term::Var(v) => {
                                let nv = *var_map.entry(*v).or_insert_with(|| {
                                    let id = next_var;
                                    next_var += 1;
                                    id
                                });
                                Term::var(nv)
                            }
                            Term::App(s, a) => Term::app(*s, a.clone()),
                        })
                        .collect();
                    Atom::pred(*sym, new_args)
                }
                Atom::Eq(l, r) => {
                    let mut map_t = |t: &Term| match t {
                        Term::Var(v) => {
                            let nv = *var_map.entry(*v).or_insert_with(|| {
                                let id = next_var;
                                next_var += 1;
                                id
                            });
                            Term::var(nv)
                        }
                        Term::App(s, a) => Term::app(*s, a.clone()),
                    };
                    Atom::eq(map_t(l), map_t(r))
                }
            };
            Literal {
                positive: lit.positive,
                atom,
            }
        })
        .collect();

    res.sort_by(|a, b| {
        a.positive
            .cmp(&b.positive)
            .then_with(|| match (&a.atom, &b.atom) {
                (Atom::Pred(s1, args1), Atom::Pred(s2, args2)) => s1
                    .index()
                    .cmp(&s2.index())
                    .then_with(|| args1.len().cmp(&args2.len()))
                    .then_with(|| {
                        for (t1, t2) in args1.iter().zip(args2.iter()) {
                            let ord = cmp_terms(t1, t2);
                            if !ord.is_eq() {
                                return ord;
                            }
                        }
                        std::cmp::Ordering::Equal
                    }),
                (Atom::Eq(l1, r1), Atom::Eq(l2, r2)) => {
                    cmp_terms(l1, l2).then_with(|| cmp_terms(r1, r2))
                }
                (Atom::Pred(..), Atom::Eq(..)) => std::cmp::Ordering::Less,
                (Atom::Eq(..), Atom::Pred(..)) => std::cmp::Ordering::Greater,
            })
    });
    res.dedup();
    res
}

/// Checks if a clause contains complementary literals.
fn is_tautology(lits: &[Literal]) -> bool {
    for (i, l1) in lits.iter().enumerate() {
        for l2 in &lits[i + 1..] {
            if l1.positive != l2.positive && l1.atom == l2.atom {
                return true;
            }
        }
    }
    false
}

#[derive(Clone)]
enum PSrc {
    Input(usize),
    Resolvent { left: usize, right: usize },
}

fn resolve_prop(c1: &[PL], c2: &[PL], lit: PL) -> Option<PC> {
    let mut result: Vec<PL> = c1
        .iter()
        .chain(c2.iter())
        .copied()
        .filter(|&l| l != lit && l != -lit)
        .collect::<HashSet<PL>>()
        .into_iter()
        .collect();
    result.sort_unstable();
    for &l in &result {
        if l > 0 && result.binary_search(&-l).is_ok() {
            return None;
        }
    }
    Some(result)
}

fn prop_bfs_refute(input: &[PC]) -> Option<(Vec<PC>, Vec<PSrc>, usize)> {
    let mut clauses: Vec<PC> = Vec::new();
    let mut sources: Vec<PSrc> = Vec::new();
    let mut seen: HashSet<PC> = HashSet::default();

    for (i, c) in input.iter().enumerate() {
        if seen.insert(c.clone()) {
            let is_empty = c.is_empty();
            clauses.push(c.clone());
            sources.push(PSrc::Input(i));
            if is_empty {
                let idx = clauses.len() - 1;
                return Some((clauses, sources, idx));
            }
        }
    }

    let mut head = 0;
    while head < clauses.len() {
        let c_head = clauses[head].clone();
        for j in 0..head {
            let c_j = clauses[j].clone();
            for &lit in &c_head {
                if c_j.binary_search(&-lit).is_err() {
                    continue;
                }
                let Some(resolvent) = resolve_prop(&c_head, &c_j, lit) else {
                    continue;
                };
                if seen.insert(resolvent.clone()) {
                    let is_empty = resolvent.is_empty();
                    clauses.push(resolvent);
                    sources.push(PSrc::Resolvent {
                        left: head,
                        right: j,
                    });
                    if is_empty {
                        let idx = clauses.len() - 1;
                        return Some((clauses, sources, idx));
                    }
                    if clauses.len() > 100_000 {
                        return None;
                    }
                }
            }
        }
        head += 1;
    }
    None
}

fn dfs_topo(
    idx: usize,
    prop_sources: &[PSrc],
    visited: &mut HashSet<usize>,
    order: &mut Vec<usize>,
) {
    if !visited.insert(idx) {
        return;
    }
    if let PSrc::Resolvent { left, right } = &prop_sources[idx] {
        dfs_topo(*left, prop_sources, visited, order);
        dfs_topo(*right, prop_sources, visited, order);
    }
    order.push(idx);
}

/// Tries to decide an EPR problem using lazy SAT-guided InstGen.
///
/// Returns `Some(SearchResult::Refutation(..))` if unsatisfiable,
/// `Some(SearchResult::Saturated)` if satisfiable,
/// or `None` if the budget/heuristics expire without a conclusive result.
pub fn try_instgen_epr(
    clauses: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
) -> Option<SearchResult> {
    if !is_pure_relational_epr(clauses) {
        return None;
    }

    let trace = std::env::var("TRACE_INSTGEN").is_ok();
    let start_time = Instant::now();

    if trace {
        eprintln!(
            "[InstGen] Starting InstGen on {} pure relational EPR clauses",
            clauses.len()
        );
    }

    // Canonical dummy constant symbol for variable abstraction (⊥)
    let mut symbols_local = symbols.clone();
    let bot_sym = symbols_local.intern("$bot");

    let mut abs = PropAbstraction::new();
    let mut solver = Solver::new();

    let mut all_clauses: Vec<Clause> = clauses.to_vec();
    let mut seen_clauses: HashSet<Vec<Literal>> = HashSet::default();
    for c in clauses {
        seen_clauses.insert(canonicalize_clause(&c.literals));
    }

    // Propositional representation tracking:
    // `prop_clauses` stores each active clause's propositional abstraction.
    // `prop_to_clause_idx` maps propositional clause index to `all_clauses` index.
    let mut prop_clauses: Vec<PC> = Vec::new();
    let mut prop_to_clause_idx: Vec<usize> = Vec::new();

    // Initial abstraction of all input clauses
    for (idx, c) in all_clauses.iter().enumerate() {
        if let Some(pc) = abs.abstract_clause(c, bot_sym) {
            solver.add_clause(&pc);
            prop_to_clause_idx.push(idx);
            prop_clauses.push(pc);
        }
    }

    let mut round: usize = 0;

    while round < MAX_ROUNDS && start_time.elapsed() < DEFAULT_INSTGEN_TIMEOUT {
        round += 1;

        match solver.solve() {
            SolveResult::Unsat => {
                if trace {
                    eprintln!(
                        "[InstGen] CaDiCaL returned UNSAT after {} rounds! Extracting proof...",
                        round
                    );
                }

                // 1. Try fast propositional BFS proof extraction
                if let Some((bfs_pcs, bfs_sources, empty_idx)) = prop_bfs_refute(&prop_clauses) {
                    let mut visited: HashSet<usize> = HashSet::default();
                    let mut order: Vec<usize> = Vec::new();
                    dfs_topo(empty_idx, &bfs_sources, &mut visited, &mut order);

                    let mut prop_idx_to_fof_id: HashMap<usize, ClauseId> = HashMap::default();
                    let mut fof_proof: Vec<Clause> =
                        Vec::with_capacity(provenance.len() + all_clauses.len() + order.len());

                    fof_proof.extend(provenance.iter().cloned());

                    let mut empty_id = None;

                    for &idx in &order {
                        match &bfs_sources[idx] {
                            PSrc::Input(p_idx) => {
                                let orig_clause = &all_clauses[prop_to_clause_idx[*p_idx]];
                                let has_vars = !orig_clause.free_vars().is_empty();

                                if has_vars {
                                    // Ground the clause with bot_sym
                                    let ground_lits: Vec<Literal> = orig_clause
                                        .literals
                                        .iter()
                                        .map(|lit| {
                                            let Atom::Pred(sym, args) = &lit.atom else {
                                                unreachable!()
                                            };
                                            let new_args = args
                                                .iter()
                                                .map(|t| match t {
                                                    Term::Var(_) => Term::constant(bot_sym),
                                                    Term::App(s, a) => Term::app(*s, a.clone()),
                                                })
                                                .collect();
                                            Literal {
                                                positive: lit.positive,
                                                atom: Atom::pred(*sym, new_args),
                                            }
                                        })
                                        .collect();
                                    let ground_c = Clause::new(
                                        id_gen.next(),
                                        ground_lits,
                                        ClauseSource::Inference {
                                            rule: "instantiation",
                                            parents: vec![orig_clause.id].into(),
                                        },
                                    );
                                    prop_idx_to_fof_id.insert(idx, ground_c.id);
                                    fof_proof.push(orig_clause.clone());
                                    fof_proof.push(ground_c);
                                } else {
                                    prop_idx_to_fof_id.insert(idx, orig_clause.id);
                                    fof_proof.push(orig_clause.clone());
                                }
                            }
                            PSrc::Resolvent { left, right } => {
                                let left_id = prop_idx_to_fof_id[left];
                                let right_id = prop_idx_to_fof_id[right];
                                let res_lits: Vec<Literal> = bfs_pcs[idx]
                                    .iter()
                                    .map(|&pl| {
                                        let atom_key =
                                            &abs.var_to_atom[pl.unsigned_abs() as usize - 1];
                                        let fo_atom = Atom::pred(
                                            atom_key.pred,
                                            atom_key
                                                .args
                                                .iter()
                                                .map(|&c| Term::constant(c))
                                                .collect(),
                                        );
                                        Literal {
                                            positive: pl > 0,
                                            atom: fo_atom,
                                        }
                                    })
                                    .collect();

                                let is_empty = res_lits.is_empty();
                                let resolvent_c = Clause::new(
                                    id_gen.next(),
                                    res_lits,
                                    ClauseSource::Inference {
                                        rule: "resolution",
                                        parents: vec![left_id, right_id].into(),
                                    },
                                );
                                let cid = resolvent_c.id;
                                prop_idx_to_fof_id.insert(idx, cid);
                                fof_proof.push(resolvent_c);
                                if is_empty {
                                    empty_id = Some(cid);
                                }
                            }
                        }
                    }

                    if let Some(eid) = empty_id {
                        let mut clause_store: HashMap<ClauseId, Clause> = HashMap::default();
                        for c in provenance {
                            clause_store.insert(c.id, c.clone());
                        }
                        for c in &all_clauses {
                            clause_store.insert(c.id, c.clone());
                        }

                        let mut full_proof_ids: HashSet<ClauseId> = HashSet::default();
                        let mut queue: Vec<Clause> = fof_proof.clone();
                        let mut complete_proof: Vec<Clause> = Vec::new();

                        while let Some(c) = queue.pop() {
                            if full_proof_ids.insert(c.id) {
                                if let ClauseSource::Inference { parents, .. } = &c.source {
                                    for &p in parents.iter() {
                                        if !full_proof_ids.contains(&p) {
                                            queue.extend(clause_store.get(&p).cloned());
                                        }
                                    }
                                }
                                complete_proof.push(c);
                            }
                        }

                        let tstp = format_tstp(&complete_proof, &symbols_local);
                        return Some(SearchResult::Refutation(eid, tstp));
                    }
                }

                // 2. Fallback: run fast given-clause search on the augmented clause set
                if trace {
                    eprintln!("[InstGen] Running given-clause proof extraction fallback...");
                }
                let mut state = crate::state::SearchState::new_with_ml(
                    all_clauses.clone(),
                    provenance.to_vec(),
                    id_gen.clone(),
                    std::sync::Arc::new(SymbolConfig::default()),
                    std::sync::Arc::new(symbols_local.clone()),
                    false,
                    None,
                    false,
                    crate::ClauseWeightFn::Standard,
                );
                let config = SearchConfig {
                    time_limit: Duration::from_secs(2),
                    ordering: crate::TermOrdering::KBO,
                    literal_selection: crate::LiteralSelection::AllNegative,
                    selection: crate::SelectionStrategy::SmallestFirst,
                    use_avatar: false,
                    ..SearchConfig::default()
                };
                let res = crate::given_clause::search(&mut state, &config);
                if matches!(res, SearchResult::Refutation(..)) {
                    return Some(res);
                }

                return None;
            }

            SolveResult::Sat => {
                // Find model-satisfied literals across all clauses
                let mut pos_by_sym: HashMap<SymbolId, Vec<(usize, usize)>> = HashMap::default();
                let mut neg_by_sym: HashMap<SymbolId, Vec<(usize, usize)>> = HashMap::default();

                for (c_idx, c) in all_clauses.iter().enumerate() {
                    for (l_idx, lit) in c.literals.iter().enumerate() {
                        let Atom::Pred(sym, args) = &lit.atom else {
                            continue;
                        };
                        let key = GroundAtom {
                            pred: *sym,
                            args: args
                                .iter()
                                .map(|a| PropAbstraction::abstract_term(a, bot_sym))
                                .collect(),
                        };
                        if let Some(&var) = abs.atom_to_var.get(&key) {
                            let is_satisfied = if lit.positive {
                                solver.value(var) == Some(true)
                            } else {
                                solver.value(var) == Some(false)
                            };
                            if is_satisfied {
                                if lit.positive {
                                    pos_by_sym.entry(*sym).or_default().push((c_idx, l_idx));
                                } else {
                                    neg_by_sym.entry(*sym).or_default().push((c_idx, l_idx));
                                }
                            }
                        }
                    }
                }

                // Find conflicting complementary literal pairs and compute MGUs
                let mut new_instances: Vec<Clause> = Vec::new();

                for (sym, pos_list) in &pos_by_sym {
                    let Some(neg_list) = neg_by_sym.get(sym) else {
                        continue;
                    };
                    for &(c1_idx, l1_idx) in pos_list {
                        let c1 = &all_clauses[c1_idx];
                        for &(c2_idx, l2_idx) in neg_list {
                            let c2 = &all_clauses[c2_idx];
                            if c1_idx == c2_idx && l1_idx == l2_idx {
                                continue;
                            }

                            let offset = max_var(c1);
                            let c2_renamed = if offset > 0 {
                                rename_clause(c2, offset)
                            } else {
                                c2.clone()
                            };

                            let lit1 = &c1.literals[l1_idx];
                            let lit2 = &c2_renamed.literals[l2_idx];

                            let Atom::Pred(p1, args1) = &lit1.atom else {
                                continue;
                            };
                            let Atom::Pred(p2, args2) = &lit2.atom else {
                                continue;
                            };
                            debug_assert_eq!(p1, p2);

                            let t1 = Term::app(*p1, args1.clone());
                            let t2 = Term::app(*p2, args2.clone());

                            if let Ok(mgu) = mrs_unify::unify(&t1, &t2) {
                                // Instance of C1
                                let inst1_lits: Vec<Literal> =
                                    c1.literals.iter().map(|l| mgu.apply_literal(l)).collect();
                                if !is_tautology(&inst1_lits) {
                                    let canon1 = canonicalize_clause(&inst1_lits);
                                    if seen_clauses.insert(canon1.clone()) {
                                        new_instances.push(Clause::new(
                                            id_gen.next(),
                                            canon1,
                                            ClauseSource::Inference {
                                                rule: "instantiation",
                                                parents: vec![c1.id].into(),
                                            },
                                        ));
                                    }
                                }

                                // Instance of C2
                                let inst2_lits: Vec<Literal> = c2_renamed
                                    .literals
                                    .iter()
                                    .map(|l| mgu.apply_literal(l))
                                    .collect();
                                if !is_tautology(&inst2_lits) {
                                    let canon2 = canonicalize_clause(&inst2_lits);
                                    if seen_clauses.insert(canon2.clone()) {
                                        new_instances.push(Clause::new(
                                            id_gen.next(),
                                            canon2,
                                            ClauseSource::Inference {
                                                rule: "instantiation",
                                                parents: vec![c2.id].into(),
                                            },
                                        ));
                                    }
                                }

                                if all_clauses.len() + new_instances.len() > MAX_TOTAL_INSTANCES {
                                    break;
                                }
                            }
                        }
                        if all_clauses.len() + new_instances.len() > MAX_TOTAL_INSTANCES {
                            break;
                        }
                    }
                }

                if new_instances.is_empty() {
                    // No candidate pair can be unified -> the propositional model lifts
                    // to a first-order Herbrand model!
                    if trace {
                        eprintln!(
                            "[InstGen] Round {}: SAT model verified with 0 unifiable pairs -> Satisfiable!",
                            round
                        );
                    }
                    return Some(SearchResult::Saturated);
                }

                if trace {
                    eprintln!(
                        "[InstGen] Round {}: generated {} new instances (total: {})",
                        round,
                        new_instances.len(),
                        all_clauses.len() + new_instances.len()
                    );
                }

                // Add newly generated instances into solver and active set
                for inst in new_instances {
                    let idx = all_clauses.len();
                    if let Some(pc) = abs.abstract_clause(&inst, bot_sym) {
                        solver.add_clause(&pc);
                        prop_to_clause_idx.push(idx);
                        prop_clauses.push(pc);
                    }
                    all_clauses.push(inst);
                }
            }

            SolveResult::Unknown => return None,
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseIdGen, ClauseSource};
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
    fn epr_detection_pure_propositional() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let clause = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax",
        );
        assert!(is_epr(&[clause]));
    }

    #[test]
    fn epr_detection_with_variables() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let clause = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "ax",
        );
        assert!(is_epr(&[clause]));
    }

    #[test]
    fn epr_detection_non_epr_function() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        // p(f(a)) — f has arity 1, so NOT EPR
        let clause = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
            "ax",
        );
        assert!(!is_epr(&[clause]));
    }

    #[test]
    fn preprocess_epr_ground_instances() {
        // p(X) | ~p(X) with constant a:  should expand to p(a) | ~p(a)
        // which is a tautology — but the important thing is that we get 1 ground clause.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let ax = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax_ground",
        );
        // ~p(X): should be instantiated to ~p(a)
        let neg = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "ax_neg",
        );

        let result = preprocess_epr(&[ax, neg], &mut id_gen);
        assert!(result.is_some(), "EPR problem should be preprocessed");
        let ground = result.unwrap();
        // Every literal in the ground output must be ground.
        for clause in &ground {
            for lit in &clause.literals {
                let is_ground = match &lit.atom {
                    Atom::Pred(_, args) => args.iter().all(|t| !t.is_var()),
                    Atom::Eq(l, r) => !l.is_var() && !r.is_var(),
                };
                // Original non-ground clauses are appended for the clause store;
                // they may contain variables — skip those.
                if matches!(&clause.source, ClauseSource::Inference { rule, .. } if *rule == "instantiation")
                {
                    assert!(
                        is_ground,
                        "instantiated clause should be ground: {:?}",
                        clause
                    );
                }
            }
        }
    }

    #[test]
    fn preprocess_epr_refutation() {
        // Simple EPR refutation: p(a), ~p(X) — should become p(a), ~p(a) -> refutation.
        use crate::given_clause::search;
        use crate::state::SearchState;
        use crate::{SearchConfig, SearchResult};
        use mrs_calculus::ordering::SymbolConfig;
        use std::sync::Arc;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let pos = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "pos",
        );
        let neg = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "neg",
        );

        let clauses = vec![pos, neg];
        let ground = preprocess_epr(&clauses, &mut id_gen).expect("should detect EPR");

        let mut state = SearchState::new(
            ground,
            id_gen,
            Arc::new(SymbolConfig::default()),
            std::sync::Arc::new(mrs_core::SymbolTable::new()),
            false,
        );
        let result = search(&mut state, &SearchConfig::default());
        assert!(
            matches!(result, SearchResult::Refutation(..)),
            "EPR refutation should be found, got {:?}",
            result
        );
    }

    #[test]
    fn instgen_propositional_unsat() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        let res = try_instgen_epr(&[c1, c2], &[], &mut id_gen, &syms);
        assert!(
            matches!(res, Some(SearchResult::Refutation(..))),
            "Expected Refutation, got {:?}",
            res
        );
    }

    #[test]
    fn instgen_variable_unsat() {
        // C1: p(X), C2: ~p(a) -> Refutation via X -> a
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        let res = try_instgen_epr(&[c1, c2], &[], &mut id_gen, &syms);
        assert!(
            matches!(res, Some(SearchResult::Refutation(..))),
            "Expected Refutation, got {:?}",
            res
        );
    }

    #[test]
    fn instgen_transitivity_unsat() {
        // C1: p(a)
        // C2: ~p(X) | q(X)
        // C3: ~q(a)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "c2",
        );
        let c3 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(q, vec![Term::constant(a)]))],
            "c3",
        );

        let res = try_instgen_epr(&[c1, c2, c3], &[], &mut id_gen, &syms);
        assert!(
            matches!(res, Some(SearchResult::Refutation(..))),
            "Expected Refutation, got {:?}",
            res
        );
    }

    #[test]
    fn instgen_satisfiable_finite_model() {
        // C1: p(X, X)
        // C2: ~p(a, b) where a != b
        // Unification fails -> SAT model lifts to FO model!
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::var(0), Term::var(0)],
            ))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(
                p,
                vec![Term::constant(a), Term::constant(b)],
            ))],
            "c2",
        );

        let res = try_instgen_epr(&[c1, c2], &[], &mut id_gen, &syms);
        assert!(
            matches!(res, Some(SearchResult::Saturated)),
            "Expected Saturated (Satisfiable), got {:?}",
            res
        );
    }

    #[test]
    fn instgen_satisfiable_disjunction() {
        // C1: p(X) | q(X)
        // C2: ~p(a)
        // C3: ~q(b)
        // where a != b. Model: p(b) = true, q(a) = true. Satisfiable!
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );
        let c3 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(q, vec![Term::constant(b)]))],
            "c3",
        );

        let res = try_instgen_epr(&[c1, c2, c3], &[], &mut id_gen, &syms);
        assert!(
            matches!(res, Some(SearchResult::Saturated)),
            "Expected Saturated (Satisfiable), got {:?}",
            res
        );
    }

    #[test]
    fn instgen_does_not_drop_variable_tautology_instances() {
        // C1: p(X) | ~p(Y) is not a first-order tautology: with p(a) true
        // and p(b) false, its instance p(b) | ~p(a) is false.
        // Its one-constant abstraction is nevertheless propositional-tautologous.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();
        let c1 = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(p, vec![Term::var(1)])),
            ],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );
        let c3 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(b)]))],
            "c3",
        );

        let res = try_instgen_epr(&[c1, c2, c3], &[], &mut id_gen, &syms);
        assert!(
            matches!(res, Some(SearchResult::Refutation(..))),
            "variable-tautology clauses must not make an unsatisfiable EPR set look satisfiable: {res:?}"
        );
    }

    #[test]
    fn instgen_tstp_proof_inspection() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        let res = try_instgen_epr(&[c1, c2], &[], &mut id_gen, &syms);
        let Some(SearchResult::Refutation(_, tstp)) = res else {
            panic!("Expected refutation");
        };
        assert!(
            tstp.contains("$false") || tstp.contains("status(thm)"),
            "Proof should contain empty clause derivation: {}",
            tstp
        );
    }
}

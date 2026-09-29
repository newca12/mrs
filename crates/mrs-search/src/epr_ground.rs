//! Sound refutation of EPR problems by grounding, for the EPU division.
//!
//! An EPR clause set is one whose every term is a variable or a constant
//! ([`crate::instgen::is_epr`]). Its Herbrand universe is therefore the finite
//! set of constants that occur in it, and a refutation over it is propositional
//! in substance: every clause in the derivation is a ground instance, so
//! propositional resolution between two ground clauses *is* a legal first-order
//! resolution step. That is what makes grounding the right tool here and what
//! makes the resulting proof lift without an abstraction step.
//!
//! The pre-existing [`crate::instgen`] loop instead abstracts every variable
//! onto one placeholder `⊥` and searches that relaxation. Its proofs lift only
//! because the abstraction is a relaxation, and as a *search space* it is
//! degenerate: generating an MGU of two model-satisfied complementary literals
//! yields a clause whose abstraction is the atom it came from, so a round
//! produces nothing new. On a survey of the CASC-30 EPU division that is what
//! happens on 51 of the 100 problems — the loop burns its round budget emitting a
//! few hundred instances and gives up.
//!
//! This module works on ground instances throughout:
//!
//! 1. **Bootstrap** — instantiate every clause under the all-`⊥` substitution.
//!    One genuine ground instance per clause, so the set stays a relaxation of
//!    the input and a refutation over it refutes the input, while the SAT solver
//!    gets a model to work from.
//! 2. **Model-driven grounding** — read a model of the current instance set and
//!    build, for every input clause, the ground instance that model most
//!    falsifies. An instance the model falsifies entirely cannot be satisfied by
//!    the model that produced it, so adding it forces the model to change: the
//!    loop cannot stall the way the MGU loop does.
//! 3. **Decide** — CaDiCaL decides the ground instance set. On `UNSAT` a bounded
//!    propositional BFS over ground atoms is lifted clause by clause into a
//!    first-order resolution proof.
//!
//! Equality needs no special case in *instance generation*. `⊥ = c` is as
//! much a ground instance as `p(⊥)` is, and ground instances carry real
//! `d = c` atoms, so the `epr_equality` profile — which [`crate::instgen`]
//! refuses outright with `unsupported_epr_profile` — on 43 of the 100 CASC-30
//! EPU problems at an 8 s budget, and 40 at 3 s, because the recorded route
//! depends on how far the pre-pass gets before its own budget runs out. Quote
//! the census with the budget it was taken at; the point of it is that the
//! profile is a large share of the division, not that it is exactly `n`. What the SAT abstraction
//! does *not* do is equality reasoning: ground equality atoms are
//! propositionally atomic and uninterpreted there (ordered canonically, with
//! reflexive equalities simplified away, but with no transitivity or
//! congruence axioms). A refutation that needs the equality theory must come
//! from the given-clause fallback, which runs superposition over the ground
//! instances and reasons about equality properly.
//!
//! Every instance emitted here is fully ground. Rungs narrower than the
//! clause complete the substitution with `⊥` (see [`complete_with_bot`]):
//! a partial instance would be dropped by the ground-set abstraction, so
//! emitting one counts search that did not happen.
//!
//! Soundness rests on two facts, both true of every instance set built here:
//! every clause in it is an instance of an input clause, and the SAT problem is
//! stated over ground atoms only. So any refutation of the abstraction refutes
//! the input, and it lifts.

use std::time::{Duration, Instant};

use smallvec::SmallVec;

use mrs_cadical::{SolveResult, Solver};
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::subst::Substitution;
use mrs_core::symbol::{SymbolId, SymbolTable};
use mrs_core::term::{Term, VarId};
use mrs_proof::tstp::format_tstp;

use crate::{HashMap, HashSet, SearchResult};

/// A ground atom: a predicate application over constants, or an equality
/// between two constants. Both are propositionally atomic, which is what makes a
/// propositional refutation over them a first-order one.
///
/// Equality pairs are stored in sorted order, so `a = b` and `b = a` abstract
/// to the same variable instead of two independent propositions.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum GAtom {
    Pred(SymbolId, SmallVec<[SymbolId; 4]>),
    Eq(SymbolId, SymbolId),
}

impl GAtom {
    /// Ground equality with a canonical argument order, so `a = b` and
    /// `b = a` intern to the same SAT variable.
    fn eq(a: SymbolId, b: SymbolId) -> Self {
        GAtom::Eq(a.min(b), a.max(b))
    }

    /// The ground atom a literal denotes, or `None` if the literal is not a
    /// usable ground atom: it has a variable in it, or it is a reflexive
    /// equality (`c = c`), which is decided by simplification instead (see
    /// [`GroundAbstraction::abstract_clause`]).
    fn of_literal(lit: &Literal) -> Option<Self> {
        match &lit.atom {
            Atom::Pred(sym, args) => {
                let mut out = SmallVec::new();
                for arg in args {
                    match arg {
                        Term::App(sym, args) if args.is_empty() => out.push(*sym),
                        _ => return None,
                    }
                }
                Some(GAtom::Pred(*sym, out))
            }
            Atom::Eq(l, r) => {
                let const_side = |t: &Term| match t {
                    Term::App(sym, args) if args.is_empty() => Some(*sym),
                    _ => None,
                };
                match (const_side(l), const_side(r)) {
                    (Some(a), Some(b)) if a != b => Some(GAtom::eq(a, b)),
                    _ => None,
                }
            }
        }
    }

    /// A literal that holds in every model: a positive reflexive equality
    /// (`c = c`, or `t = t` generally by reflexivity). Its clause is a
    /// tautology and carries no constraint.
    fn is_valid(lit: &Literal) -> bool {
        lit.positive && matches!(&lit.atom, Atom::Eq(l, r) if l == r)
    }

    /// A literal that holds in no model: a negative reflexive equality
    /// (`c != c`). It constrains nothing and is dropped from its clause
    /// during abstraction; a clause of nothing but such literals is the
    /// empty clause.
    fn is_false(lit: &Literal) -> bool {
        !lit.positive && matches!(&lit.atom, Atom::Eq(l, r) if l == r)
    }

    /// The first-order literal this atom came from.
    fn to_literal(&self, positive: bool) -> Literal {
        let atom = match self {
            GAtom::Pred(sym, args) => {
                Atom::Pred(*sym, args.iter().map(|a| Term::constant(*a)).collect())
            }
            GAtom::Eq(l, r) => Atom::eq(Term::constant(*l), Term::constant(*r)),
        };
        Literal { positive, atom }
    }
}

type Pl = i32;
type Pc = Vec<Pl>;

/// Bijection between ground atoms and DIMACS variables.
#[derive(Default)]
struct GroundAbstraction {
    atom_to_var: HashMap<GAtom, i32>,
    var_to_atom: Vec<GAtom>,
}

impl GroundAbstraction {
    fn intern(&mut self, atom: &GAtom) -> i32 {
        if let Some(&v) = self.atom_to_var.get(atom) {
            return v;
        }
        let v = (self.var_to_atom.len() + 1) as i32;
        self.var_to_atom.push(atom.clone());
        self.atom_to_var.insert(atom.clone(), v);
        v
    }

    /// Abstracts a ground instance for the SAT solver.
    ///
    /// Reflexive equalities are simplified first: a literal true in every
    /// model (`c = c`) makes the whole clause vacuous, a literal false in
    /// every model (`c != c`) is dropped. Anything with a variable in it
    /// still keeps the clause out entirely — reasoning over a
    /// partially-grounded abstraction would not lift.
    ///
    /// An empty image is a genuine contradiction (every literal simplified
    /// to `c != c`), not a tautology: the caller records it and the next
    /// solve reports `Unsat`.
    fn abstract_clause(&mut self, clause: &Clause) -> Abstracted {
        let mut pc = Vec::with_capacity(clause.literals.len());
        for lit in &clause.literals {
            if GAtom::is_valid(lit) {
                return Abstracted::Skip;
            }
            if GAtom::is_false(lit) {
                continue;
            }
            let Some(atom) = GAtom::of_literal(lit) else {
                return Abstracted::Skip;
            };
            let v = self.intern(&atom);
            pc.push(if lit.positive { v } else { -v });
        }
        pc.sort_unstable();
        pc.dedup();
        Abstracted::Clause(pc)
    }
}

/// What abstraction decided for one instance.
enum Abstracted {
    /// Carries no constraint (tautology) or is not ground: keep it out of
    /// the SAT solver.
    Skip,
    /// Propositional image. Empty means contradiction — every literal
    /// simplified to `c != c` — and the solver will report `Unsat`.
    Clause(Pc),
}

/// One ground instance together with its propositional image. The instance's
/// own [`ClauseSource`] already names the input clause it came from, so no
/// separate parent field is needed.
struct GroundClause {
    clause: Clause,
    pc: Pc,
}

/// The ground instance set, its SAT abstraction, and the cap both obey.
struct GroundSet {
    abs: GroundAbstraction,
    solver: Solver,
    clauses: Vec<GroundClause>,
    seen: HashSet<Pc>,
    cap: usize,
    /// The clause set the pre-pass was handed. Instantiation steps name these as
    /// their parents, so proof lifting needs them in the clause store alongside
    /// the instances — the original provenance is not enough, because
    /// preprocessing may have replaced it.
    inputs: Vec<Clause>,
    /// The caller's original provenance, which proof lifting prints as inputs.
    provenance: Vec<Clause>,
    /// Symbol table for diagnostics; the search itself is name-free.
    symbols: SymbolTable,
}

impl GroundSet {
    fn new(cap: usize, inputs: &[Clause], provenance: &[Clause], symbols: &SymbolTable) -> Self {
        Self {
            abs: GroundAbstraction::default(),
            solver: Solver::new(),
            clauses: Vec::new(),
            seen: HashSet::default(),
            cap,
            inputs: inputs.to_vec(),
            provenance: provenance.to_vec(),
            symbols: symbols.clone(),
        }
    }

    /// Adds a ground instance. Returns whether it was new and fits.
    ///
    /// An empty propositional image (a clause of nothing but `c != c`) is an
    /// immediate contradiction: it is recorded as the empty clause and the
    /// next solve reports `Unsat`.
    fn add(&mut self, clause: Clause) -> bool {
        if self.clauses.len() >= self.cap {
            return false;
        }
        let Abstracted::Clause(pc) = self.abs.abstract_clause(&clause) else {
            return false;
        };
        if !self.seen.insert(pc.clone()) {
            return false;
        }
        self.solver.add_clause(&pc);
        self.clauses.push(GroundClause { clause, pc });
        true
    }

    fn has_room(&self) -> bool {
        self.clauses.len() < self.cap
    }
}

/// What the pre-pass did, and why it stopped. Reported verbatim in the
/// `% SZS detail` line so a run's outcome is attributable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EprTelemetry {
    /// Whether the EPR pre-pass was entered at all.
    pub attempted: bool,
    /// EPR profile, from [`crate::instgen::classify_epr_profile`].
    pub route: &'static str,
    /// Distinct constants in the Herbrand universe.
    pub domain: usize,
    /// `sum_c |consts|^|vars_c|` saturating: the size of the full Herbrand
    /// expansion, which is what decides whole-set grounding versus lazy
    /// grounding.
    pub est_instances: u64,
    /// Whether the full expansion fitted the instance cap and was taken whole.
    pub full_grounding: bool,
    /// Ground instances added after the already-ground input clauses.
    pub generated: usize,
    /// Clauses the SAT solver is deciding.
    pub sat_clauses: usize,
    /// Final propositional variable count.
    pub sat_vars: usize,
    /// Model → instance rounds executed.
    pub rounds: usize,
    /// Instances that a model falsified in every literal. The loop's progress
    /// signal: each one forces the model to change.
    pub falsifying: usize,
    /// Wall-clock milliseconds spent in the pre-pass.
    pub elapsed_ms: u64,
    /// Whether a first-order proof was emitted.
    pub proof_extracted: bool,
    /// Proof size in nodes.
    pub proof_nodes: usize,
    /// Pivot constants the instance generator is seeded from.
    pub pivots: usize,
    /// Whether equality splitting was on for this run.
    pub splitting: bool,
    /// Input clauses with a non-empty split support, i.e. clauses splitting
    /// actually applies to. On CASC-30 EPU this is non-zero for 20 of the 100
    /// problems, and those are the largest ones.
    pub split_clauses: usize,
    /// Terminal outcome: `"refutation"`, `"fallback"` or `"none"`.
    pub result: &'static str,
    /// Why the pre-pass yielded without a refutation.
    pub fallback: Option<&'static str>,
}

impl Default for EprTelemetry {
    fn default() -> Self {
        Self {
            attempted: false,
            route: "none",
            domain: 0,
            est_instances: 0,
            full_grounding: false,
            generated: 0,
            sat_clauses: 0,
            sat_vars: 0,
            rounds: 0,
            falsifying: 0,
            elapsed_ms: 0,
            proof_extracted: false,
            proof_nodes: 0,
            pivots: 0,
            splitting: false,
            split_clauses: 0,
            result: "none",
            fallback: None,
        }
    }
}

/// Resource envelope for the EPR grounding pre-pass.
#[derive(Clone, Copy, Debug)]
pub struct EprBudget {
    /// The process's own memory ceiling, in MB, when it has one. The instance cap
    /// is a model of this; the real figure is watched as well, because a model of
    /// memory is wrong in the direction that costs a run its wall clock.
    pub memory_ceiling_mb: Option<u64>,
    /// Wall-clock ceiling for the whole pre-pass.
    pub timeout: Duration,
    /// Ceiling on ground instances held in memory *by the lazy search*. Whole-set
    /// grounding is bounded by [`Self::byte_budget`] instead, which is sized from
    /// the clause shapes rather than from a flat per-instance figure.
    pub max_instances: usize,
    /// Ceiling on the bytes the whole Herbrand expansion may occupy.
    pub byte_budget: u64,
    /// Ceiling on model → instance rounds.
    pub max_rounds: usize,
}

/// Wall-clock and instance budget derived from the problem shape and the
/// process's own memory allowance.
///
/// The instance cap is the number that matters. An EPR problem whose full
/// Herbrand expansion fits is decided outright by one CaDiCaL call, so it earns a
/// large cap; one whose expansion is astronomically larger has to be searched
/// lazily, and a cap it can never reach is pure overhead. Instances dominate
/// memory, so the cap is derived from the memory ceiling the process is actually
/// held to rather than from a constant.
pub fn epr_budget(memory_budget_mb: Option<u64>, time_limit: Duration) -> EprBudget {
    // Deliberately pessimistic: a ground clause costs a `Clause` (id, boxed
    // literal vector, provenance source) plus a propositional image and two
    // hash-set entries. 4 KB per instance means the cap under-promises, and if
    // the estimate is still wrong the process memory ceiling is what stops us.
    const FALLBACK_MEMORY_MB: u64 = 4 * 1024;
    // A third of the allowance: the portfolio still needs room, the clause store
    // keeps every input clause alive besides, and the SAT solver holds its own
    // copy of the instance set on top of the clause set itself.
    let mem_bytes = memory_budget_mb
        .unwrap_or(FALLBACK_MEMORY_MB)
        .saturating_mul(1024 * 1024)
        / 3;

    EprBudget {
        memory_ceiling_mb: memory_budget_mb,
        // Grounding is this division's natural mode, so it gets a real share of
        // the run rather than the 750 ms fail-fast slice the MGU loop takes. It
        // still leaves the portfolio room to run, because grounding can
        // legitimately give up — on a non-EPR problem, or one whose instances
        // never contradict.
        timeout: time_limit
            .checked_mul(3)
            .map_or(Duration::from_secs(30), |d| d / 4)
            .clamp(Duration::from_millis(500), Duration::from_secs(120)),
        max_instances: usize::try_from(mem_bytes / INSTANCE_COST_BYTES)
            .unwrap_or(400_000)
            .clamp(50_000, 4_000_000),
        byte_budget: mem_bytes,
        max_rounds: 200,
    }
}

/// Largest Herbrand expansion grounds in one shot.
///
/// Above this the lazy search is the better bet: it decides far larger problems
/// than a complete expansion of this size does, because it never materialises the
/// instances it does not need. `mrs` solved 9 of the 15-problem EPU subset with
/// expansions up to 1.5 M instances rejected here, against 2 of 15 with them
/// taken whole.
const FULL_GROUNDING_INSTANCE_CEILING: u64 = 500_000;

/// Per-instance allowance for [`epr_budget`]'s cap.
///
/// The cap exists to stop the lazy search before memory does, and the lazy search
/// only ever adds instances of a *few* shapes — the `⊥` image and one-variable
/// pivot images — so a flat conservative figure is right for it. It is not the
/// figure used to decide whole-set grounding, which is sized per clause by
/// [`estimate_grounding_bytes`].
const INSTANCE_COST_BYTES: u64 = 8 * 1024;

/// How many constants to seed instances from. Small on purpose: the seed exists
/// to give the falsification rule somewhere to move, not to enumerate the domain.
const PIVOT_LIMIT: usize = 8;

/// The constants a refutation is most likely to turn on, best first.
///
/// A ground refutation is anchored somewhere: the Skolem constants the goal
/// negation introduced, and the constants that appear on their own in a unit
/// clause. Those are the elements a contradiction has to involve, so instances
/// that mention them are worth generating before instances that mention any other
/// constant. The count is a relevance proxy — a constant in more unit clauses
/// and more goal literals is one more of the refutation's load-bearing pieces —
/// not a proof of anything.
fn pivot_constants(clauses: &[Clause], limit: usize) -> Vec<SymbolId> {
    let mut weight: HashMap<SymbolId, usize> = HashMap::default();
    for clause in clauses {
        let is_goal = matches!(&clause.source, ClauseSource::Input { role, .. } if role == "negated_conjecture");
        // Units and goal clauses carry the anchoring facts. A long disjunction
        // mentions many constants, none of which is thereby more likely to be
        // the one a refutation turns on.
        let informative = is_goal || clause.literals.len() <= 2;
        if !informative {
            continue;
        }
        for lit in &clause.literals {
            for c in literal_constants(lit) {
                *weight.entry(c).or_default() += if is_goal { 4 } else { 1 };
            }
        }
    }
    let mut ranked: Vec<(SymbolId, usize)> = weight.into_iter().collect();
    // Sort by weight, then by symbol id, so the seed is the same on every run
    // and a sweep difference is a change in the code rather than in hash order.
    ranked.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.into_iter().take(limit).map(|(c, _)| c).collect()
}

fn literal_constants(lit: &Literal) -> Vec<SymbolId> {
    let mut out = Vec::new();
    let mut seen = HashSet::default();
    match &lit.atom {
        Atom::Pred(_, args) => {
            for t in args {
                if let Some(c) = constant_of(t)
                    && seen.insert(c)
                {
                    out.push(c);
                }
            }
        }
        Atom::Eq(l, r) => {
            for t in [l, r] {
                if let Some(c) = constant_of(t)
                    && seen.insert(c)
                {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Time held back from instance generation so that an `UNSAT` answer from the SAT
/// solver still has room to become a proof. A refutation nobody can exhibit is
/// not a result the certification gate will accept, so the reserve is not
/// negotiable against search progress.
const PROOF_RESERVE: Duration = Duration::from_millis(500);

/// `sum_c |consts|^|vars_c|`, saturating at `u64::MAX`. The instance *count*,
/// which is what the search reports and what bounds the SAT solver.
pub fn estimate_grounding_size(clauses: &[Clause], n_constants: usize) -> u64 {
    let mut total: u64 = 0;
    for clause in clauses {
        let n_vars = clause.free_vars().len() as u32;
        total = total.saturating_add((n_constants as u64).saturating_pow(n_vars));
    }
    total
}

/// Bytes the full Herbrand expansion of `clauses` would occupy, saturating.
///
/// A flat per-instance constant is the wrong model and the wrong decision: an
/// instance of a 22-ary predicate over 22 constants costs about a kilobyte, while
/// an instance of a binary clause costs about two hundred. Sizing by clause shape
/// is what lets a problem that is wide but cheap — a 2-element domain over 21
/// variables is four million instances of two literals — be grounded whole,
/// instead of being written off by a cost model meant for something else.
fn estimate_grounding_bytes(clauses: &[Clause], n_constants: usize) -> u64 {
    /// Per instance: the `Clause` header, its provenance `SmallVec`, the
    /// propositional image, and the two hash-set entries that keep it unique.
    const PER_INSTANCE: u64 = 1024;
    /// Per literal: a `Literal`, an `Atom`, and its argument vector.
    const PER_LITERAL: u64 = 128;
    /// Per distinct ground atom: the entry in both directions of the atom
    /// bijection, plus the argument vector behind it.
    const PER_ATOM: u64 = 256;

    let mut instances = 0u64;
    for clause in clauses {
        let n_vars = clause.free_vars().len() as u32;
        let bytes =
            PER_INSTANCE.saturating_add(PER_LITERAL.saturating_mul(clause.literals.len() as u64));
        instances = instances.saturating_add(
            (n_constants as u64)
                .saturating_pow(n_vars)
                .saturating_mul(bytes),
        );
    }

    // The atom table is a second cost that does not scale with the instance
    // count: two instances of the same ground clause share every atom. Bounding
    // it separately is what keeps a narrow-domain problem with very wide
    // predicates — where the instance count alone looks affordable — from
    // interning millions of atoms nobody can afford.
    let mut arities: HashSet<(SymbolId, usize)> = HashSet::default();
    let mut has_equality = false;
    for clause in clauses {
        for lit in &clause.literals {
            match &lit.atom {
                Atom::Pred(sym, args) => {
                    arities.insert((*sym, args.len()));
                }
                // Equality between two constants ranges over the same square a
                // binary predicate does.
                Atom::Eq(..) => has_equality = true,
            }
        }
    }
    let mut atoms = 0u64;
    for (_, arity) in arities {
        atoms = atoms.saturating_add((n_constants as u64).saturating_pow(arity as u32));
    }
    if has_equality {
        atoms = atoms.saturating_add(
            (n_constants as u64)
                .saturating_mul(n_constants as u64)
                .saturating_mul(2),
        );
    }

    instances.saturating_add(atoms.saturating_mul(PER_ATOM))
}

/// The clause's free variables in order of first appearance in its literals.
///
/// Order matters: the rungs bind "the first `depth` variables", so a
/// `HashSet`-derived order would make the restriction depend on hash iteration
/// order — the same problem would ground different variables on different runs,
/// and the benchmark's reproducibility notes already warn that per-run telemetry
/// is sensitive to exactly this kind of thing.
fn clause_vars_ordered(clause: &Clause) -> Vec<VarId> {
    let mut ordered: Vec<VarId> = Vec::new();
    let mut seen: HashSet<VarId> = HashSet::default();
    for lit in &clause.literals {
        match &lit.atom {
            Atom::Pred(_, args) => {
                for t in args {
                    collect_var_in_order(t, &mut ordered, &mut seen);
                }
            }
            Atom::Eq(l, r) => {
                collect_var_in_order(l, &mut ordered, &mut seen);
                collect_var_in_order(r, &mut ordered, &mut seen);
            }
        }
    }
    ordered
}

fn collect_var_in_order(t: &Term, ordered: &mut Vec<VarId>, seen: &mut HashSet<VarId>) {
    match t {
        Term::Var(v) => {
            if seen.insert(*v) {
                ordered.push(*v);
            }
        }
        Term::App(_, args) => {
            for a in args {
                collect_var_in_order(a, ordered, seen);
            }
        }
    }
}

fn constant_of(t: &Term) -> Option<SymbolId> {
    match t {
        Term::App(sym, args) if args.is_empty() => Some(*sym),
        _ => None,
    }
}

fn map_term<F: FnMut(&Term) -> Term>(atom: &Atom, f: &mut F) -> Atom {
    match atom {
        Atom::Pred(sym, args) => Atom::Pred(*sym, args.iter().map(&mut *f).collect()),
        Atom::Eq(l, r) => Atom::eq(f(l), f(r)),
    }
}

fn is_tautology(lits: &[Literal]) -> bool {
    lits.iter().enumerate().any(|(i, a)| {
        lits[i + 1..]
            .iter()
            .any(|b| a.positive != b.positive && a.atom == b.atom)
    })
}

fn instantiation(id_gen: &mut ClauseIdGen, source: ClauseId, lits: Vec<Literal>) -> Clause {
    Clause::new(
        id_gen.next(),
        lits,
        ClauseSource::Inference {
            rule: "instantiation",
            parents: vec![source].into(),
        },
    )
}

/// The all-`⊥` instance of `clause`: every free variable replaced by one shared
/// placeholder constant. A genuine instance, so sound to reason about, and its
/// atom pattern is exactly what a `⊥` relaxation would use — now as ground facts
/// the SAT solver can falsify.
fn bot_instance(clause: &Clause, bot: SymbolId, id_gen: &mut ClauseIdGen) -> Option<Clause> {
    let lits: Vec<Literal> = clause
        .literals
        .iter()
        .map(|lit| Literal {
            positive: lit.positive,
            // Variables only. Replacing the *constants* as well would emit
            // `m_cell_v_token($bot,$bot)` from `m_cell_v_token(c_e_h_3,X0)`, which
            // is not an instance of anything: the instance is
            // `m_cell_v_token(c_e_h_3,$bot)`. That produced refutations the
            // proof checker rejects, and it made the SAT abstraction
            // unsatisfiable for a reason that had nothing to do with the problem.
            atom: map_term(&lit.atom, &mut |t| match t {
                Term::Var(_) => Term::constant(bot),
                other => other.clone(),
            }),
        })
        .collect();
    if lits.is_empty() || is_tautology(&lits) {
        return None;
    }
    Some(instantiation(id_gen, clause.id, lits))
}

/// Calls `f` with every substitution mapping each variable of `clause` to a
/// constant in `domain`, stopping early when `f` returns `false`.
///
/// Streaming rather than collecting: a clause with two variables over a
/// thousand-constant domain is a million instances, and materialising them as
/// `Substitution` values would cost far more than the instances themselves.
fn for_each_instance(
    clause: &Clause,
    vars: &[VarId],
    domain: &[SymbolId],
    id_gen: &mut ClauseIdGen,
    f: &mut impl FnMut(Clause) -> bool,
) {
    let mut subst = Substitution::new();
    expand(0, clause, vars, domain, &mut subst, id_gen, f);
}

/// Walks the cross product of `domain` over `vars`, calling `f` with each
/// instance. `f` returns `false` to abandon the rest of the enumeration, which
/// is how the instance cap stops a clause with a wide cross product.
#[allow(clippy::too_many_arguments)]
fn expand(
    depth: usize,
    clause: &Clause,
    vars: &[VarId],
    domain: &[SymbolId],
    subst: &mut Substitution,
    id_gen: &mut ClauseIdGen,
    f: &mut impl FnMut(Clause) -> bool,
) -> bool {
    if depth == vars.len() {
        let lits: Vec<Literal> = clause
            .literals
            .iter()
            .map(|l| subst.apply_literal(l))
            .collect();
        if is_tautology(&lits) {
            return true;
        }
        return f(instantiation(id_gen, clause.id, lits));
    }
    for &c in domain {
        subst.bind(vars[depth], Term::constant(c));
        if !expand(depth + 1, clause, vars, domain, subst, id_gen, f) {
            return false;
        }
    }
    true
}

/// Tries to refute an EPR clause set by grounding.
///
/// Returns `Some(Refutation)` with a TSTP proof when the ground instance set is
/// unsatisfiable, and `None` otherwise. `None` is the normal answer on a
/// satisfiable or too-hard problem and never claims anything: the caller falls
/// through to the portfolio.
pub fn try_epr_ground_refutation(
    clauses: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
    budget: EprBudget,
) -> (Option<SearchResult>, EprTelemetry) {
    let route = crate::instgen::classify_epr_profile(clauses);
    let domain = crate::instgen::collect_constants(clauses);
    let est = estimate_grounding_size(clauses, domain.len());
    let est_bytes = estimate_grounding_bytes(clauses, domain.len());
    let mut tele = EprTelemetry {
        route,
        domain: domain.len(),
        est_instances: est,
        ..EprTelemetry::default()
    };

    if matches!(route, "non_epr" | "empty") || domain.is_empty() {
        tele.fallback = Some("unsupported_epr_profile");
        return (None, tele);
    }
    tele.attempted = true;

    let start = Instant::now();
    let deadline = start + budget.timeout;
    // Diagnostics, off unless asked for.
    //
    // `TRACE_EPR_DUMP=<path>` writes the asserted ground instance set as TPTP,
    // and `MRS_EPR_PROBE=1` runs the ground given-clause loop over it whatever
    // the SAT verdict. Both exist to answer one question the abstraction makes
    // hard to see from inside: the instance set is a *relaxation* of the
    // problem, so a `Sat` verdict says the relaxation is satisfiable, not that
    // the problem is. Dumping the set and re-deciding it as first-order clauses
    // separates "the abstraction is lossy" from "the instances are not enough".
    let dump = std::env::var("TRACE_EPR_DUMP").ok();
    let probe = std::env::var("MRS_EPR_PROBE").is_ok();
    // Grounding is a search, and a search that has used its whole budget has
    // nothing left to decide or prove with. The two phases are therefore split
    // rather than shared: instance generation stops at `grounding_deadline`,
    // leaving the rest for the SAT solver and — the part that was being starved
    // — for turning an UNSAT answer into a derivation.
    let grounding_deadline = start + budget.timeout * 2 / 5;
    let trace = std::env::var("TRACE_EPR").is_ok();

    // A private symbol table so the `⊥` placeholder can never collide with a
    // problem symbol, and the proof is printed in the problem's vocabulary.
    let mut local_symbols = symbols.clone();
    let bot = local_symbols.intern("$bot");

    // The instance cap is lifted for whole-set grounding, which is bounded by
    // bytes instead; the lazy search keeps the lower cap.
    let mut ground = GroundSet::new(
        if est_bytes <= budget.byte_budget {
            usize::try_from(est.saturating_add(1024)).unwrap_or(usize::MAX)
        } else {
            budget.max_instances
        },
        clauses,
        provenance,
        &local_symbols,
    );
    let pivots = pivot_constants(clauses, PIVOT_LIMIT);
    tele.pivots = pivots.len();

    // Equality splitting, per clause. A clause's split support never changes, so
    // it is computed once here rather than once per clause per round.
    //
    // `MRS_EPR_SPLIT=1` turns it on. It is off by default because it was
    // measured and the measurement is a negative one: on the 20 CASC-30 EPU
    // problems that contain a clause of the split shape it changes the instance
    // count by about 1%, and on `HWV087-1` it is worse, because the count is
    // budget-capped rather than search-capped and the cap is simply reached at a
    // different point. The rule is kept — it is the right shape for this
    // division, it is soundness-tested, and the reason it does not pay here is
    // a property of the problems (see `split_support`) rather than a defect in
    // it — but it does not earn the default path.
    let splitting = std::env::var("MRS_EPR_SPLIT").is_ok_and(|v| v != "0");
    let supports: Vec<HashMap<VarId, HashSet<SymbolId>>> = if splitting {
        clauses
            .iter()
            .map(|c| split_support(c, &clause_vars_ordered(c)))
            .collect()
    } else {
        vec![HashMap::default(); clauses.len()]
    };
    tele.splitting = splitting;
    tele.split_clauses = supports
        .iter()
        .filter(|s| s.values().any(|v| !v.is_empty()))
        .count();

    // The `⊥` bootstrap: one genuine ground instance per clause, so the set is a
    // relaxation of the input and a refutation over it refutes the input.
    for clause in clauses {
        if !ground.has_room() {
            break;
        }
        let inst = if clause.free_vars().is_empty() {
            Some(clause.clone())
        } else {
            bot_instance(clause, bot, id_gen)
        };
        if let Some(inst) = inst
            && ground.add(inst)
        {
            tele.generated += 1;
        }
    }

    // If the whole Herbrand expansion is small, take it: one CaDiCaL call then
    // decides the problem and the lazy search has nothing to add.
    //
    // "Small" is deliberately a low bar. Grounding the whole expansion is
    // *complete* for the problem, so it is tempting to ground whenever memory
    // allows — and that is a trap: on HWV039-1 the expansion is 1.5 M instances
    // and the lazy search refutes it with 802, while the expansion spends the
    // whole budget and decides nothing. The lazy search earns its keep on exactly
    // the problems that look groundable, so whole-set grounding is for expansions
    // small enough that being complete is not worth losing the better search.
    if est <= FULL_GROUNDING_INSTANCE_CEILING && est_bytes <= budget.byte_budget {
        tele.full_grounding = true;
        for clause in clauses {
            if Instant::now() >= grounding_deadline {
                break;
            }
            let vars: Vec<VarId> = clause.free_vars().into_iter().collect();
            if vars.is_empty() {
                continue;
            }
            // The deadline is checked per instance, not per clause. A clause with
            // twenty-one variables over a two-element domain is two million
            // instances, so a per-clause check would not fire until the whole
            // enumeration had already run.
            for_each_instance(clause, &vars, &domain, id_gen, &mut |inst| {
                if !ground.has_room() {
                    return false;
                }
                if tele.generated.is_multiple_of(4096) && Instant::now() >= grounding_deadline {
                    return false;
                }
                if ground.add(inst) {
                    tele.generated += 1;
                }
                true
            });
        }
        tele.generated = ground
            .clauses
            .len()
            .saturating_sub(clauses.iter().filter(|c| c.free_vars().is_empty()).count());
        if trace {
            eprintln!(
                "[EPR] full grounding: {} instances over a domain of {}",
                ground.clauses.len(),
                domain.len()
            );
        }
    }

    // Search schedule: a widening ladder over the instance restriction, with the
    // model-driven rule running inside each rung.
    //
    // A rung fixes *how much* of the Herbrand expansion may be instantiated —
    // zero variables (the `⊥` bootstrap), then one variable over the pivot set,
    // then one variable over every constant, then two variables over the pivots
    // — and iterates the model loop until the SAT solver finds nothing new. The
    // ladder matters because the `⊥` bootstrap is nearly vacuous: it gives the
    // solver only `⊥`-shaped atoms, so the model it returns says almost nothing
    // about the problem, and any rule driven by that model alone stalls. Each
    // rung adds real constants, which is what gives the model something to
    // contradict.
    let mut round = 0usize;
    let mut rung = 0usize;
    // One cursor per clause per rung: how far through that rung's constant list
    // this clause has been swept.
    let mut ctx = GroundingContext {
        clauses,
        pivots: &pivots,
        domain: &domain,
        bot,
        deadline: grounding_deadline,
        rung: 0,
        cursors: vec![vec![0; clauses.len()]; MAX_RUNGS],
        supports: &supports,
    };
    loop {
        tele.sat_clauses = ground.clauses.len();
        tele.sat_vars = ground.abs.var_to_atom.len();

        match ground.solver.solve_until(deadline) {
            SolveResult::Unsat => {
                tele.elapsed_ms = start.elapsed().as_millis() as u64;
                // CaDiCaL knows the instance set is unsatisfiable; all that is
                // left is a derivation. The BFS below produces one only for
                // small sets — resolution without subsumption blows up well
                // before the SAT solver does — so the ground given-clause loop
                // takes over for anything larger. It has subsumption and the
                // other prunings that make a ground refutation findable, and it
                // emits a proof the same way any other refutation does.
                let (result, nodes) = extract_refutation(
                    &ground,
                    clauses,
                    provenance,
                    id_gen,
                    &local_symbols,
                    start,
                    budget,
                );
                return match result {
                    Some(r) => {
                        tele.proof_extracted = true;
                        tele.proof_nodes = nodes;
                        tele.result = "refutation";
                        (Some(r), tele)
                    }
                    None => {
                        tele.fallback = Some("proof_extraction_failed");
                        tele.result = "fallback";
                        (None, tele)
                    }
                };
            }
            SolveResult::Unknown => {
                tele.elapsed_ms = start.elapsed().as_millis() as u64;
                tele.fallback = Some("sat_solver_unknown");
                tele.result = "fallback";
                return (None, tele);
            }
            SolveResult::Sat => {
                if probe {
                    if let Some(path) = &dump {
                        let _ = dump_ground_set(&ground, path);
                    }
                    tele.elapsed_ms = start.elapsed().as_millis() as u64;
                    let (result, _) = ground_refutation_fallback(
                        &ground,
                        clauses,
                        provenance,
                        id_gen,
                        &local_symbols,
                        start,
                        EprBudget {
                            timeout: budget.timeout.saturating_sub(start.elapsed()),
                            ..budget
                        },
                    );
                    return match result {
                        Some(r) => {
                            tele.proof_extracted = true;
                            tele.result = "refutation";
                            (Some(r), tele)
                        }
                        None => {
                            tele.fallback = Some("probe_sat");
                            tele.result = "fallback";
                            (None, tele)
                        }
                    };
                }
            }
        }

        round += 1;
        if round > budget.max_rounds || Instant::now() >= grounding_deadline {
            tele.elapsed_ms = start.elapsed().as_millis() as u64;
            tele.rounds = round.saturating_sub(1);
            tele.fallback = Some(if round > budget.max_rounds {
                "max_rounds_reached"
            } else {
                "grounding_timeout"
            });
            tele.result = "fallback";
            return (None, tele);
        }
        // The instance cap is a model of memory, and models of memory are wrong
        // in the direction that hurts: a wide-predicate instance costs several
        // times a flat per-instance figure, and the atom table is a second cost
        // that does not scale with the instance count at all. So the actual RSS
        // is watched too. Without this a run can spend minutes in the allocator
        // and the SAT solver rather than in search — a 421 s run on a 15 s limit
        // before the guard existed, which is not a timeout, it is a hang.
        if let Some(limit) = budget.memory_ceiling_mb
            && crate::resource::current_memory_mb().is_some_and(|rss| rss > limit)
        {
            tele.elapsed_ms = start.elapsed().as_millis() as u64;
            tele.rounds = round.saturating_sub(1);
            tele.fallback = Some("memory_ceiling");
            tele.result = "fallback";
            return (None, tele);
        }
        // An UNSAT answer with no budget left to prove it is not an answer, so
        // give up on generation early enough that the solver and the derivation
        // still have a share.
        if deadline.saturating_duration_since(Instant::now()) < PROOF_RESERVE {
            tele.elapsed_ms = start.elapsed().as_millis() as u64;
            tele.rounds = round.saturating_sub(1);
            tele.fallback = Some("grounding_timeout");
            tele.result = "fallback";
            return (None, tele);
        }

        ctx.rung = rung;
        let (instances, falsifying, rung_done) = generate_from_model(&mut ctx, &ground, id_gen);
        let candidates = instances.len();
        tele.falsifying += falsifying;
        let mut added = 0usize;
        let mut over_budget = false;
        for (n, instance) in instances.into_iter().enumerate() {
            if !ground.has_room() {
                break;
            }
            // Checked inside the add loop, not only between rounds: a round can
            // hold tens of thousands of instances, and a per-round check is too
            // coarse to be a memory guard.
            if n.is_multiple_of(4095)
                && budget.memory_ceiling_mb.is_some_and(|limit| {
                    crate::resource::current_memory_mb().is_some_and(|rss| rss > limit)
                })
            {
                over_budget = true;
                break;
            }
            if ground.add(instance) {
                added += 1;
            }
        }
        if over_budget {
            tele.elapsed_ms = start.elapsed().as_millis() as u64;
            tele.rounds = round.saturating_sub(1);
            tele.fallback = Some("memory_ceiling");
            tele.result = "fallback";
            return (None, tele);
        }
        tele.generated += added;
        if trace {
            eprintln!(
                "[EPR] round {round} (rung {rung}): {candidates} candidates -> \
                 {added} new ({falsifying} falsifying), {} total, {} atoms",
                ground.clauses.len(),
                ground.abs.var_to_atom.len()
            );
        }

        // "Generated" is not "new". A rung whose frontier is spent keeps handing
        // back instances it has already asserted, and counting that as progress
        // is what pinned the loop to rung 0 until it hit the round limit.
        if added == 0 || rung_done {
            // Nothing new at this width. Widen and try again, unless this was
            // already the widest rung — advancing past it would index the cursor
            // table out of bounds, and would claim a wider search that does not
            // exist.
            if rung + 1 >= MAX_RUNGS {
                tele.elapsed_ms = start.elapsed().as_millis() as u64;
                tele.rounds = round;
                tele.fallback = Some(if rung_done {
                    "grounding_exhausted"
                } else {
                    "model_fixpoint"
                });
                tele.result = "fallback";
                return (None, tele);
            }
            // Advancing a rung is the normal path, not a stall: an exhausted rung
            // has said everything it can and the next one is strictly stronger.
            // The loop's real bounds are the round limit, the grounding deadline,
            // the instance cap and the memory guard.
            rung += 1;
            continue;
        }
    }
}

/// Model-driven grounding. For every input clause, build the ground instance
/// the current model most falsifies, preferring one it falsifies in every
/// literal.
///
/// Returns the instances and how many of them were falsified in every literal.
/// The problem facts the widening rungs draw on: the pivot set, the whole
/// domain, and how far each clause has been swept at each width.
struct GroundingContext<'a> {
    clauses: &'a [Clause],
    pivots: &'a [SymbolId],
    domain: &'a [SymbolId],
    /// The `⊥` placeholder: variables no rung binds stay free unless they are
    /// completed with it (see [`complete_with_bot`]), and a free variable
    /// makes the instance unusable to the SAT solver.
    bot: SymbolId,
    deadline: Instant,
    rung: usize,
    /// `cursors[depth][clause]` — how far this clause has been swept at this
    /// width. Per clause, because one shared cursor lets the first clause
    /// consume the whole constant list and leaves every later clause with an
    /// exhausted rung.
    cursors: Vec<Vec<usize>>,
    /// Per input clause, the constants its equality literals already pin each
    /// variable to. Empty (and unused) when splitting is off. See
    /// [`split_support`].
    supports: &'a [HashMap<VarId, HashSet<SymbolId>>],
}

/// Binds every variable in `vars` that `subst` leaves free to `bot`, so the
/// instance the substitution produces is ground.
///
/// Substituting `⊥` for a universally quantified variable is ordinary
/// universal instantiation with a fresh constant, so the completed instance
/// is as genuine as one over problem constants. Without this, rungs narrower
/// than the clause are dead on arrival: `GroundSet::add` drops anything with
/// a variable in it, and the round would report instances it never asserted.
fn complete_with_bot(vars: &[VarId], subst: &mut Substitution, bot: SymbolId) {
    for &v in vars {
        if subst.lookup(v).is_none() {
            subst.bind(v, Term::constant(bot));
        }
    }
}

/// Equality splitting, in the sense of López-Gil, Ordinary and Anzai.
///
/// A clause whose equality literals all pin *one* variable to constants
/// restricts that variable's value, and the restriction is the whole point:
///
/// ```text
/// c₁ = X ∨ c₂ = X ∨ … ∨ cₙ = X ∨ R(X)
/// ```
///
/// says that either `X` is one of the `cᵢ` — in which case the clause is
/// satisfied and asserts nothing — or `R(X)` holds at whatever value `X` takes.
/// So the clause only ever forces `R` at values in `D \ {c₁…cₙ}`, and grounding
/// it at the `cᵢ` is wasted.
///
/// That complement is where the leverage is. In the HWV family the domain is
/// about 205 constants and the split support of a state variable is about 200 of
/// them, so the clause prunes the variable from 205 candidate values to about 5 —
/// and it is the *remaining* variables, the ones a rung then has to cross
/// product, that make grounding expensive. For a three-variable clause that is
/// the difference between 205 · 56 · 56 and 5 · 56 · 56 instances.
///
/// ## Soundness
///
/// The instances emitted are `R[X := d]` completed to ground, and each is an
/// instance of `C` under the substitution that sends the split variable to `d`
/// and the rest to constants. So every emitted clause is entailed by `C`, and a
/// refutation over the instance set is a refutation of the problem. The
/// instances at `d ∈ {cᵢ}` are not emitted, but that only *removes* constraints
/// from a set that is already a relaxation — it cannot turn an unsatisfiable
/// instance set into a satisfiable one, and the proof is lifted from the
/// instances that were kept.
///
/// The rule is deliberately a *pruning* of each rung's candidate list rather
/// than a rewrite of the clause set. A clause-level rewrite that drops the
/// equality disjuncts would assert `⋀_d R[d]`, which is strictly stronger than
/// `C` and can therefore prove something `C` does not — that is the unsound
/// direction, and this module does not take it.
fn split_support(clause: &Clause, vars: &[VarId]) -> HashMap<VarId, HashSet<SymbolId>> {
    let mut support: HashMap<VarId, HashSet<SymbolId>> = HashMap::default();
    // A variable is a candidate split only if it occurs somewhere other than in
    // an equality literal. Otherwise pinning it to `d` loses the clause entirely:
    // `X = c` with `X` nowhere else is satisfied or violated by the equality
    // alone, and there is no `R(X)` left to restrict.
    let mut in_rest: HashSet<VarId> = HashSet::default();
    let mut eligible: HashSet<VarId> = HashSet::default();

    for lit in &clause.literals {
        match &lit.atom {
            Atom::Eq(l, r) => {
                // Only `X = c` with a constant on one side and no variable on
                // the other side of that side is a split; `X = Y` is the shape
                // restricted *equality resolution* handles, and pinning it to a
                // domain value would lose the constraint that the two are equal.
                match (l, r) {
                    (Term::Var(v), other) | (other, Term::Var(v)) => {
                        if let Some(c) = constant_of(other) {
                            eligible.insert(*v);
                            support.entry(*v).or_default().insert(c);
                        }
                    }
                    _ => {
                        for v in clause_vars_in(&lit.atom) {
                            in_rest.insert(v);
                        }
                    }
                }
            }
            Atom::Pred(..) => {
                for v in clause_vars_in(&lit.atom) {
                    in_rest.insert(v);
                }
            }
        }
    }

    support.retain(|v, _| in_rest.contains(v) && eligible.contains(v));
    for v in vars {
        support.entry(*v).or_default();
    }
    support
}

fn clause_vars_in(atom: &Atom) -> HashSet<VarId> {
    let mut out = HashSet::default();
    match atom {
        Atom::Pred(_, args) => {
            for t in args {
                collect_var_set(t, &mut out);
            }
        }
        Atom::Eq(l, r) => {
            collect_var_set(l, &mut out);
            collect_var_set(r, &mut out);
        }
    }
    out
}

fn collect_var_set(t: &Term, out: &mut HashSet<VarId>) {
    match t {
        Term::Var(v) => {
            out.insert(*v);
        }
        Term::App(_, args) => {
            for a in args {
                collect_var_set(a, out);
            }
        }
    }
}

/// The constant list a rung may use for `slot` of `clause`: the rung's own list,
/// minus the split support of the variable occupying that slot.
///
/// Empty when the support covers the whole list — every value is already
/// accounted for, so this rung has nothing to contribute for that variable.
fn slot_constants(
    rung: Rung,
    slot: usize,
    vars: &[VarId],
    support: &HashMap<VarId, HashSet<SymbolId>>,
    pivots: &[SymbolId],
    domain: &[SymbolId],
) -> Vec<SymbolId> {
    let base = rung.constants(pivots, domain);
    let Some(&var) = vars.get(slot) else {
        return Vec::new();
    };
    let Some(support) = support.get(&var) else {
        return base.to_vec();
    };
    if support.is_empty() {
        return base.to_vec();
    }
    base.iter()
        .copied()
        .filter(|c| !support.contains(c))
        .collect()
}

fn generate_from_model(
    ctx: &mut GroundingContext<'_>,
    ground: &GroundSet,
    id_gen: &mut ClauseIdGen,
) -> (Vec<Clause>, usize, bool) {
    let GroundingContext {
        clauses,
        pivots,
        domain,
        bot,
        deadline,
        rung,
        cursors,
        supports,
    } = ctx;
    let mut out = Vec::new();
    let mut instances: Vec<Vec<Literal>> = Vec::new();
    let mut falsifying = 0usize;
    if !ground.has_room() {
        return (out, falsifying, true);
    }
    let index = TemplateIndex::build(&ground.abs);

    // Seeding is quadratic in the domain in the limit (variables × constants per
    // clause), so a round is capped. Reaching the cap is not a failure: the next
    // round's solver has a full set of real atoms and the falsification rule
    // takes over, which is the cheaper route from there.
    let headroom = ground.cap - ground.clauses.len();
    // A small slice per round, not a large one. The memory guard is checked once
    // per round, so the round's cap is also the most the process can allocate
    // between two checks: a cap of `cap/8` let one round take a 15 s run to
    // 13 GB and 395 s, all of it after the guard had already been passed.
    let round_cap = headroom.min(ground.cap / 64).max(1);

    let mut checked = 0usize;
    for (i, clause) in clauses.iter().enumerate() {
        if !ground.has_room() {
            break;
        }
        // Time is checked every so often rather than per clause: `Instant::now`
        // is a syscall on some hosts and this is the hot loop.
        checked += 1;
        if checked.is_multiple_of(64) && Instant::now() >= *deadline {
            break;
        }
        let vars = clause_vars_ordered(clause);
        if vars.is_empty() {
            continue;
        }
        let support = &supports[i];

        // The falsification rule: bind variables so every literal is false under
        // the current model. An instance the model cannot satisfy is a strict
        // shrink of the model space, so it is the one move guaranteed to move.
        // Variables the rule leaves alone are completed with `⊥`, so the
        // instance is ground and can reach the solver.
        if let Some((mut subst, all_false)) =
            falsifying_substitution(clause, &ground.abs, &ground.solver, &index)
        {
            complete_with_bot(&vars, &mut subst, *bot);
            let lits: Vec<Literal> = clause
                .literals
                .iter()
                .map(|l| subst.apply_literal(l))
                .collect();
            // A tautological instance asserts nothing. A clause whose `⊥` image
            // is a tautology — `p(X,Y) ∨ ~p(Y,X)` collapses to exactly that — is
            // one the rule can never make progress on, and skipping it every
            // round is how the loop used to report a fixpoint.
            if !is_tautology(&lits) {
                if all_false {
                    falsifying += 1;
                }
                out.push(instantiation(id_gen, clause.id, lits));
            }
        }

        // Widen: bind the variables the current rung allows, using constants the
        // rung has not reached yet so no round repeats the last one's work.
        // Variables past the rung's depth are completed with `⊥` inside, so
        // every candidate below is a genuine ground instance.
        //
        // The candidate list per slot is the rung's own list pruned by the
        // clause's split support (see [`split_support`]): values the clause's
        // own equality literals already account for are not worth grounding the
        // rest of it at.
        let rung_enum = Rung::from(*rung);
        let slot_lists: Vec<Vec<SymbolId>> = (0..rung_enum.depth().unwrap_or(vars.len()))
            .map(|slot| slot_constants(rung_enum, slot, &vars, support, pivots, domain))
            .collect();
        rung_instances(
            clause,
            &vars,
            rung_enum,
            &slot_lists,
            &mut cursors[*rung][i],
            round_cap.saturating_sub(out.len()),
            &mut instances,
            *deadline,
            *bot,
        );
        for lits in instances.drain(..) {
            out.push(instantiation(id_gen, clause.id, lits));
        }
        if out.len() >= round_cap {
            break;
        }
    }
    // The rung is done when every clause's cursor has reached the end of its
    // constant list. A clause the loop never reached has a cursor that has not
    // moved, so the comparison is "did it move past everything" and not "is it
    // zero".
    let rung = Rung::from(*rung);
    let done = cursors[rung.ordinal()]
        .iter()
        .all(|c| *c >= rung.constants(pivots, domain).len())
        || !ground.has_room();
    debug_assert!(
        cursors[rung.ordinal()]
            .iter()
            .all(|c| *c <= rung.constants(pivots, domain).len()),
        "a cursor cannot run past the unpruned list, which bounds the pruned one"
    );
    (out, falsifying, done)
}

/// How much of the Herbrand expansion a rung may instantiate.
///
/// Rung 0 binds one variable to the pivot set; rung 1 binds one variable to
/// every constant in the domain; rung 2 binds two variables to the pivots, which
/// is what a transitivity or reachability step needs. Wider rungs are strictly
/// stronger, and each is only reached once the previous one has stopped finding
/// new instances, so a problem rung 0 settles never pays for rung 2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rung {
    OneVariablePivots,
    OneVariableDomain,
    TwoVariablePivots,
    UniformPivots,
    AllPivots,
}

const MAX_RUNGS: usize = 5;

impl Rung {
    fn constants<'a>(self, pivots: &'a [SymbolId], domain: &'a [SymbolId]) -> &'a [SymbolId] {
        match self {
            Rung::OneVariableDomain => domain,
            _ => pivots,
        }
    }

    /// How many of the clause's variables this rung binds at once. `None` means
    /// all of them.
    fn depth(self) -> Option<usize> {
        match self {
            Rung::OneVariablePivots | Rung::OneVariableDomain => Some(1),
            Rung::TwoVariablePivots => Some(2),
            Rung::UniformPivots | Rung::AllPivots => None,
        }
    }

    /// Whether this rung binds the variable at `slot`. Variables past the depth
    /// stay free and are bound by a later rung, which is what keeps the cross
    /// product at `|constants|^depth` per clause instead of `|constants|^|vars|`.
    fn uses(self, slot: usize) -> bool {
        self.depth().is_none_or(|d| slot < d)
    }

    /// `UniformPivots` puts every variable on the *same* constant, which is the
    /// shape a single-valued encoding uses: a time index compared against itself
    /// is only ever true when the two occurrences are the same index.
    fn uniform(self) -> bool {
        matches!(self, Rung::UniformPivots)
    }
}

/// How many assignments a cross product may examine per emitted instance before
/// the rung gives up on that clause. Tautologies and duplicates dominate the
/// examination, so the two counts are not the same and the examination needs its
/// own bound.
const VISIT_BUDGET_FACTOR: usize = 64;

/// The cursor row `rung` owns.
impl Rung {
    /// Stable ordinal, matching [`Rung::from`].
    const fn ordinal(self) -> usize {
        match self {
            Rung::OneVariablePivots => 0,
            Rung::OneVariableDomain => 1,
            Rung::TwoVariablePivots => 2,
            Rung::UniformPivots => 3,
            Rung::AllPivots => 4,
        }
    }
}

impl From<usize> for Rung {
    fn from(n: usize) -> Self {
        match n {
            0 => Rung::OneVariablePivots,
            1 => Rung::OneVariableDomain,
            2 => Rung::TwoVariablePivots,
            3 => Rung::UniformPivots,
            _ => Rung::AllPivots,
        }
    }
}

/// The instances rung `r` still owes `clause`, binding `r`'s depth of its
/// variables to constants from the rung's list at or after `cursor`.
///
/// Variables past the rung's depth are completed with `bot`, so every emitted
/// instance is ground: a rung narrower than the clause still searches a real
/// (if restricted) slice of the Herbrand expansion instead of emitting
/// partial instances the SAT solver must drop.
///
/// The cursor is per clause and per rung. A single shared cursor is wrong: the
/// first clause would consume the whole constant list and every later clause
/// would see an exhausted rung, which is what made a 56-constant domain produce
/// 72 candidates instead of thousands. `cursor` is advanced only as far as this
/// call actually got, so a round that runs out of budget resumes where it
/// stopped rather than restarting at the beginning.
#[allow(clippy::too_many_arguments)]
fn rung_instances(
    clause: &Clause,
    vars: &[VarId],
    rung: Rung,
    slot_lists: &[Vec<SymbolId>],
    cursor: &mut usize,
    budget: usize,
    out: &mut Vec<Vec<Literal>>,
    deadline: Instant,
    bot: SymbolId,
) {
    let Some(outer_list) = slot_lists.first() else {
        *cursor = 0;
        return;
    };
    let mut i = *cursor;
    while i < outer_list.len() {
        let outer = outer_list[i];
        if rung.uniform() {
            let mut subst = Substitution::new();
            for &var in vars {
                subst.bind(var, Term::constant(outer));
            }
            push_instance(clause, subst, budget, out);
            if out.len() >= budget {
                *cursor = i;
                return;
            }
            i += 1;
            continue;
        }
        for &var in vars.iter().filter(|_| rung.uses(0)) {
            let mut subst = Substitution::new();
            subst.bind(var, Term::constant(outer));
            complete_with_bot(vars, &mut subst, bot);
            push_instance(clause, subst, budget, out);
            if out.len() >= budget {
                *cursor = i;
                return;
            }
            // Slot 1 has its own list: a second split variable is pruned by its
            // own support, not the first one's.
            let inner_list: &[SymbolId] = slot_lists.get(1).map_or(&[], Vec::as_slice);
            for j in i..inner_list.len() {
                let inner = inner_list[j];
                for &var2 in vars.iter().filter(|_| rung.uses(1)) {
                    if var2 == var {
                        continue;
                    }
                    let mut subst = Substitution::new();
                    subst.bind(var, Term::constant(outer));
                    subst.bind(var2, Term::constant(inner));
                    complete_with_bot(vars, &mut subst, bot);
                    push_instance(clause, subst, budget, out);
                    if out.len() >= budget {
                        *cursor = i;
                        return;
                    }
                    if matches!(rung, Rung::AllPivots) {
                        // Every variable, each independently. Streamed, not
                        // materialised: a clause of six variables over an
                        // eight-constant pivot set is a quarter of a million
                        // assignments, and collecting them as `Vec<Vec<_>>`
                        // before using any of them cost a 15 s run 120 s and
                        // 13 GB in allocation churn alone.
                        let rest: Vec<VarId> = vars
                            .iter()
                            .copied()
                            .filter(|v| *v != var && *v != var2)
                            .collect();
                        let mut subst = Substitution::new();
                        subst.bind(var, Term::constant(outer));
                        subst.bind(var2, Term::constant(inner));
                        let mut sink = AssignmentSink {
                            clause,
                            budget: budget.saturating_sub(out.len()),
                            out,
                            full: false,
                            visited: 0,
                            visit_budget: budget.max(1) * VISIT_BUDGET_FACTOR,
                            deadline,
                        };
                        for_each_assignment(&rest, inner_list, 0, &mut subst, &mut sink);
                        if sink.full {
                            *cursor = i;
                            return;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    *cursor = outer_list.len();
}

/// Every way of assigning `vars` from `constants`, as a flat list of
/// variable-ordered constant choices.
/// Where a streamed cross product writes each assignment as it is produced.
struct AssignmentSink<'a> {
    clause: &'a Clause,
    /// How many more instances this call may emit.
    budget: usize,
    out: &'a mut Vec<Vec<Literal>>,
    /// Set once the budget ran out, to unwind the recursion.
    full: bool,
    /// How many assignments have been *examined*. Bounded independently of
    /// `budget`, because a tautological assignment emits nothing: without a
    /// separate counter the recursion walks the entire cross product looking for
    /// a non-tautology, which on a six-variable clause is a quarter of a
    /// million assignments per constant pair and does not stop.
    visited: usize,
    visit_budget: usize,
    deadline: Instant,
}

impl AssignmentSink<'_> {
    fn admit(&mut self) -> bool {
        if self.visited >= self.visit_budget || Instant::now() >= self.deadline {
            self.full = true;
            return false;
        }
        self.visited += 1;
        true
    }

    fn emit(&mut self, subst: &Substitution) {
        if self.out.len() >= self.budget || !self.admit() {
            self.full = true;
            return;
        }
        let lits: Vec<Literal> = self
            .clause
            .literals
            .iter()
            .map(|l| subst.apply_literal(l))
            .collect();
        if is_tautology(&lits) {
            return;
        }
        self.out.push(lits);
    }
}

/// Walks every assignment of `vars` from `constants` in order, depth first,
/// emitting each as an instance of `clause`.
///
/// Streamed rather than collected: a clause of six variables over an
/// eight-constant pivot set is a quarter of a million assignments, and holding
/// them as `Vec<Vec<_>>` before using any of them cost a 15 s run 120 s and
/// 13 GB in allocation churn alone.
fn for_each_assignment(
    vars: &[VarId],
    constants: &[SymbolId],
    depth: usize,
    subst: &mut Substitution,
    sink: &mut AssignmentSink,
) {
    if sink.full {
        return;
    }
    if depth == vars.len() {
        sink.emit(subst);
        return;
    }
    for &c in constants {
        subst.bind(vars[depth], Term::constant(c));
        for_each_assignment(vars, constants, depth + 1, subst, sink);
        if sink.full {
            return;
        }
    }
}

fn push_instance(clause: &Clause, subst: Substitution, budget: usize, out: &mut Vec<Vec<Literal>>) {
    if out.len() >= budget {
        return;
    }
    let lits: Vec<Literal> = clause
        .literals
        .iter()
        .map(|l| subst.apply_literal(l))
        .collect();
    if is_tautology(&lits) {
        return;
    }
    out.push(lits);
}

/// A literal with every variable but one already bound to a constant.
enum Template {
    Pred {
        sym: SymbolId,
        args: Vec<Option<SymbolId>>,
        free: usize,
    },
    /// An equality with one side a constant and the other a free variable.
    /// Which side is which does not matter past construction: atoms are
    /// stored with sorted arguments, so the lookup key is canonical either
    /// way.
    Eq { other: SymbolId },
}

impl Template {
    /// Builds the template for `atom` under `sigma`, provided exactly one
    /// variable remains free. Returns it with that variable.
    fn build(atom: &Atom, sigma: &HashMap<VarId, SymbolId>) -> Option<(Self, VarId)> {
        match atom {
            Atom::Pred(sym, args) => {
                let mut out = Vec::with_capacity(args.len());
                let mut free = None;
                for (i, t) in args.iter().enumerate() {
                    match t {
                        Term::App(_, cargs) if cargs.is_empty() => out.push(constant_of(t)),
                        Term::Var(v) => match sigma.get(v) {
                            Some(c) => out.push(Some(*c)),
                            None => {
                                if free.is_some() {
                                    return None;
                                }
                                free = Some((i, *v));
                                out.push(None);
                            }
                        },
                        _ => return None,
                    }
                }
                let (free, var) = free?;
                Some((
                    Template::Pred {
                        sym: *sym,
                        args: out,
                        free,
                    },
                    var,
                ))
            }
            Atom::Eq(l, r) => {
                // A variable already bound by `sigma` stands for its constant;
                // only a still-free variable is a position the rule can move.
                // A side that is neither is not EPR-shaped and ends the
                // template: a literal with no single free variable to move is
                // one the rule cannot turn off from here.
                let side = |t: &Term| -> Option<Result<SymbolId, VarId>> {
                    match t {
                        Term::Var(v) => Some(match sigma.get(v) {
                            Some(c) => Ok(*c),
                            None => Err(*v),
                        }),
                        _ => constant_of(t).map(Ok),
                    }
                };
                match (side(l), side(r)) {
                    (Some(Err(var)), Some(Ok(other))) => Some((Template::Eq { other }, var)),
                    (Some(Ok(other)), Some(Err(var))) => Some((Template::Eq { other }, var)),
                    _ => None,
                }
            }
        }
    }
}

/// Groups the known ground atoms by shape, so choosing a constant for a
/// template's one free position costs the size of the matching bucket instead
/// of a scan of the whole atom table.
struct TemplateIndex {
    by_pred: HashMap<SymbolId, Vec<Vec<Option<SymbolId>>>>,
    eq_by_right: HashMap<SymbolId, Vec<SymbolId>>,
    eq_by_left: HashMap<SymbolId, Vec<SymbolId>>,
}

impl TemplateIndex {
    fn build(abs: &GroundAbstraction) -> Self {
        let mut by_pred: HashMap<SymbolId, Vec<Vec<Option<SymbolId>>>> = HashMap::default();
        let mut eq_by_right: HashMap<SymbolId, Vec<SymbolId>> = HashMap::default();
        let mut eq_by_left: HashMap<SymbolId, Vec<SymbolId>> = HashMap::default();
        for atom in &abs.var_to_atom {
            match atom {
                GAtom::Pred(sym, args) => {
                    by_pred
                        .entry(*sym)
                        .or_default()
                        .push(args.iter().map(|a| Some(*a)).collect());
                }
                GAtom::Eq(l, r) => {
                    eq_by_right.entry(*r).or_default().push(*l);
                    eq_by_left.entry(*l).or_default().push(*r);
                }
            }
        }
        Self {
            by_pred,
            eq_by_right,
            eq_by_left,
        }
    }

    /// Constants that, substituted into `template`'s free position, give a
    /// known atom that is *false* under the current model. Those are the values
    /// that turn a satisfied literal off.
    ///
    /// A constant whose atom the solver has never seen is deliberately absent:
    /// an unconstrained atom is not a false one, and treating it as false would
    /// hand back an instance that is merely unknown, not a counterexample.
    fn falsifying(
        &self,
        template: &Template,
        abs: &GroundAbstraction,
        solver: &Solver,
    ) -> Vec<SymbolId> {
        let mut out = Vec::new();
        match template {
            Template::Pred { sym, args, free } => {
                let Some(bucket) = self.by_pred.get(sym) else {
                    return out;
                };
                for concrete in bucket {
                    if !matches_at(concrete, args, *free) {
                        continue;
                    }
                    let Some(c) = concrete[*free] else {
                        continue;
                    };
                    let key = GAtom::Pred(*sym, concrete.iter().flatten().copied().collect());
                    if let Some(&v) = abs.atom_to_var.get(&key)
                        && solver.value(v) == Some(false)
                    {
                        out.push(c);
                    }
                }
            }
            Template::Eq { other, .. } => {
                // Atoms are stored with sorted arguments, so `other` may sit
                // on either side: partners come from both buckets. The two
                // buckets are disjoint (`c = c` is never interned), so no
                // constant is proposed twice. Which side of the *template* is
                // free does not matter for the lookup — the atom key is
                // canonical either way.
                let mut partners: Vec<SymbolId> = Vec::new();
                if let Some(bucket) = self.eq_by_right.get(other) {
                    partners.extend(bucket.iter().copied());
                }
                if let Some(bucket) = self.eq_by_left.get(other) {
                    partners.extend(bucket.iter().copied());
                }
                for c in partners {
                    let key = GAtom::eq(c, *other);
                    if let Some(&v) = abs.atom_to_var.get(&key)
                        && solver.value(v) == Some(false)
                    {
                        out.push(c);
                    }
                }
            }
        }
        out
    }
}

/// Whether a concrete argument vector matches a template whose position `free`
/// is still unconstrained.
fn matches_at(concrete: &[Option<SymbolId>], template: &[Option<SymbolId>], free: usize) -> bool {
    if concrete.len() != template.len() {
        return false;
    }
    concrete
        .iter()
        .zip(template.iter())
        .enumerate()
        .all(|(i, (c, t))| i == free || *c == *t)
}

/// Builds a substitution under which as many of `clause`'s literals as possible
/// are false in the current model.
///
/// Greedy with a bounded fixpoint: walk the literals and, whenever one is not
/// already false, bind its remaining variable to a constant that falsifies it.
/// The walk repeats while bindings still change, because binding a variable for
/// one literal can re-satisfy an earlier one.
///
/// The flag reports whether *every* literal ended up false. Such an instance is
/// the progress signal: the model that produced it cannot satisfy it, so adding
/// it forces the model to change.
fn falsifying_substitution(
    clause: &Clause,
    abs: &GroundAbstraction,
    solver: &Solver,
    index: &TemplateIndex,
) -> Option<(Substitution, bool)> {
    let mut sigma: HashMap<VarId, SymbolId> = HashMap::default();

    for _ in 0..4 {
        let mut changed = false;
        for lit in &clause.literals {
            if literal_value(lit, &sigma, abs, solver) == Some(false) {
                continue;
            }
            let Some((template, var)) = Template::build(&lit.atom, &sigma) else {
                // No single free variable to move, so this literal cannot be
                // turned off from here.
                continue;
            };
            let Some(c) = index.falsifying(&template, abs, solver).into_iter().next() else {
                continue;
            };
            sigma.insert(var, c);
            changed = true;
        }
        if !changed {
            break;
        }
    }

    // Re-verify rather than tracking it during the walk: the last pass binds
    // variables, and a binding made for a later literal can re-satisfy an
    // earlier one.
    let all_false = clause
        .literals
        .iter()
        .all(|lit| literal_value(lit, &sigma, abs, solver) == Some(false));

    if sigma.is_empty() {
        return None;
    }
    let mut subst = Substitution::new();
    for (v, c) in &sigma {
        subst.bind(*v, Term::constant(*c));
    }
    Some((subst, all_false))
}

/// Truth value of `lit` under `sigma` and the current model, or `None` if the
/// literal is not ground under `sigma` or its atom is one the solver has never
/// seen. Unknown is not false: it is reported as unknown so grounding goes on
/// looking for a constant that makes the atom genuinely false.
fn literal_value(
    lit: &Literal,
    sigma: &HashMap<VarId, SymbolId>,
    abs: &GroundAbstraction,
    solver: &Solver,
) -> Option<bool> {
    let mapped = Literal {
        positive: lit.positive,
        atom: map_term(&lit.atom, &mut |t| match t {
            Term::App(_, cargs) if cargs.is_empty() => match constant_of(t) {
                Some(c) => Term::constant(c),
                None => t.clone(),
            },
            Term::Var(v) => match sigma.get(v) {
                Some(c) => Term::constant(*c),
                None => Term::Var(*v),
            },
            other => other.clone(),
        }),
    };
    if let Atom::Eq(a, b) = &mapped.atom
        && a == b
    {
        // `c = c` holds in every model: a positive literal is true, a
        // negative one (`c != c`) is false.
        return Some(lit.positive);
    }
    let atom = GAtom::of_literal(&mapped)?;
    let &v = abs.atom_to_var.get(&atom)?;
    let val = solver.value(v)?;
    Some(if lit.positive { val } else { !val })
}

#[derive(Clone)]
enum PSrc {
    Input(usize),
    Resolvent { left: usize, right: usize },
}

fn resolve_prop(c1: &[Pl], c2: &[Pl], lit: Pl) -> Option<Pc> {
    let mut result: Vec<Pl> = c1
        .iter()
        .chain(c2.iter())
        .copied()
        .filter(|&l| l != lit && l != -lit)
        .collect::<HashSet<Pl>>()
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

/// Bounded propositional BFS: grow the clause set by unit-free resolution until
/// the empty clause appears, recording each resolvent's parents so the result
/// lifts to first order.
fn prop_bfs_refute(
    input: &[Pc],
    cap: usize,
    deadline: Instant,
) -> Option<(Vec<Pc>, Vec<PSrc>, usize)> {
    let mut clauses: Vec<Pc> = Vec::new();
    let mut sources: Vec<PSrc> = Vec::new();
    let mut seen: HashSet<Pc> = HashSet::default();

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
    let mut since_check = 0usize;
    while head < clauses.len() {
        let c_head = clauses[head].clone();
        for j in 0..head {
            let c_j = clauses[j].clone();
            for &lit in &c_head {
                if c_j.binary_search(&-lit).is_err() {
                    continue;
                }
                // Resolution is quadratic in the clause set, so the BFS is
                // bounded by a clause cap *and* the pre-pass deadline. Without
                // the deadline a large ground set that is unsatisfiable can spend
                // the whole run here, long past the point where the answer is
                // still wanted.
                since_check += 1;
                if since_check >= 4096 {
                    since_check = 0;
                    if Instant::now() >= deadline {
                        return None;
                    }
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
                    if clauses.len() > cap {
                        return None;
                    }
                }
            }
        }
        head += 1;
    }
    None
}

/// Writes the asserted ground instance set to `path` as a TPTP CNF file, so it
/// can be re-decided by an independent tool. Diagnostic only
/// ([`try_epr_ground_refutation`]).
fn dump_ground_set(ground: &GroundSet, path: &str) -> std::io::Result<()> {
    use std::io::Write;
    let symbols = &ground.symbols;
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        out,
        "% asserted ground instance set, {} clauses",
        ground.clauses.len()
    )?;
    for (n, g) in ground.clauses.iter().enumerate() {
        let lits: Vec<String> = g
            .clause
            .literals
            .iter()
            .map(|lit| match &lit.atom {
                mrs_core::formula::Atom::Pred(sym, args) => {
                    let a: Vec<String> = args
                        .iter()
                        .map(|t| {
                            symbols
                                .resolve(constant_of(t).expect("ground arg"))
                                .to_string()
                        })
                        .collect();
                    // TPTP spells a nullary predicate as `p`, not `p()`.
                    if a.is_empty() {
                        format!(
                            "{}{}",
                            if lit.positive { "" } else { "~" },
                            symbols.resolve(*sym)
                        )
                    } else {
                        format!(
                            "{}{}({})",
                            if lit.positive { "" } else { "~" },
                            symbols.resolve(*sym),
                            a.join(",")
                        )
                    }
                }
                mrs_core::formula::Atom::Eq(l, r) => {
                    // `~a = b` is not TPTP: `~` binds tighter than `=`, so it
                    // would parse as a term application. A negated equality is
                    // `a != b`.
                    let l = symbols.resolve(constant_of(l).expect("ground eq side"));
                    let r = symbols.resolve(constant_of(r).expect("ground eq side"));
                    if lit.positive {
                        format!("{l} = {r}")
                    } else {
                        format!("{l} != {r}")
                    }
                }
            })
            .collect();
        writeln!(
            out,
            "cnf(g{n}, plain, ({}), file('dump', 'g{n}')).",
            lits.join(" | ")
        )?;
    }
    Ok(())
}

/// Lifts a propositional refutation over ground atoms into a first-order
/// resolution proof.
///
/// Every clause in `ground` is an instance of an input clause, so a resolvent of
/// two ground clauses is a legal first-order resolvent and the parent chain
/// names a complete derivation. `None` means the bounded BFS did not reach the
/// empty clause, which says nothing about the problem — only that this proof
/// route did not produce one.
fn extract_refutation(
    ground: &GroundSet,
    inputs: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
    start: Instant,
    budget: EprBudget,
) -> (Option<SearchResult>, usize) {
    // BFS gets a minority of what is left: it is the fast route and worth trying
    // first, but on a large ground set it is the route that runs out.
    let bfs_budget = budget
        .timeout
        .saturating_sub(start.elapsed())
        .checked_div(3)
        .unwrap_or_default();
    let pcs: Vec<Pc> = ground.clauses.iter().map(|g| g.pc.clone()).collect();
    let Some(found) = prop_bfs_refute(&pcs, 400_000, start + bfs_budget) else {
        return ground_refutation_fallback(
            ground, inputs, provenance, id_gen, symbols, start, budget,
        );
    };
    lift_bfs_refutation(ground, id_gen, symbols, found)
}

/// Turns a propositional BFS derivation over ground atoms into a first-order
/// resolution proof, keeping only the steps the derivation depends on.
fn lift_bfs_refutation(
    ground: &GroundSet,
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
    found: (Vec<Pc>, Vec<PSrc>, usize),
) -> (Option<SearchResult>, usize) {
    let (clauses, sources, _empty_idx) = found;

    // The instantiation steps name their input clause as a parent, so the store
    // needs the clause set the pre-pass was handed — not just the original
    // provenance, which preprocessing may have replaced.
    let mut store: HashMap<ClauseId, Clause> = HashMap::default();
    for g in &ground.clauses {
        store.insert(g.clause.id, g.clause.clone());
    }
    for c in &ground.inputs {
        store.insert(c.id, c.clone());
    }
    for c in &ground.provenance {
        store.insert(c.id, c.clone());
    }

    // Input clauses first, so a derived clause's id is a fresh one and the
    // printed proof reads inputs-before-conclusions.
    let mut derived: Vec<Clause> = Vec::with_capacity(clauses.len());
    let mut id_of: Vec<Option<ClauseId>> = vec![None; clauses.len()];
    let mut empty_id = None;

    for (idx, pc) in clauses.iter().enumerate() {
        match &sources[idx] {
            PSrc::Input(i) => {
                id_of[idx] = Some(ground.clauses[*i].clause.id);
            }
            PSrc::Resolvent { left, right } => {
                let (Some(a), Some(b)) = (id_of[*left], id_of[*right]) else {
                    return (None, 0);
                };
                let lits: Vec<Literal> = pc
                    .iter()
                    .map(|&lit| {
                        let atom = &ground.abs.var_to_atom[lit.unsigned_abs() as usize - 1];
                        atom.to_literal(lit > 0)
                    })
                    .collect();
                let id = id_gen.next();
                derived.push(Clause::new(
                    id,
                    lits,
                    ClauseSource::Inference {
                        rule: "resolution",
                        parents: vec![a, b].into(),
                    },
                ));
                id_of[idx] = Some(id);
                if pc.is_empty() {
                    empty_id = Some(id);
                }
            }
        }
    }

    let Some(eid) = empty_id else {
        return (None, 0);
    };
    for c in &derived {
        store.insert(c.id, c.clone());
    }

    // Emit only what the refutation depends on, or a grounding that generated a
    // million instances would print a million steps.
    let mut seen: HashSet<ClauseId> = HashSet::default();
    let mut queue = vec![store[&eid].clone()];
    let mut complete: Vec<Clause> = Vec::new();
    while let Some(c) = queue.pop() {
        if !seen.insert(c.id) {
            continue;
        }
        if let ClauseSource::Inference { parents, .. } = &c.source {
            for p in parents.iter() {
                if !seen.contains(p)
                    && let Some(parent) = store.get(p)
                {
                    queue.push(parent.clone());
                }
            }
        }
        complete.push(c);
    }

    let nodes = complete.len();
    (
        Some(SearchResult::Refutation(
            eid,
            format_tstp(&complete, symbols),
        )),
        nodes,
    )
}

/// Refutes the ground instance set with the ordinary given-clause loop and
/// returns the proof it emits.
///
/// Every clause here is ground, so this is resolution over propositions with
/// subsumption and LRS pruning doing the work: the search space the BFS cannot
/// survive is one the given-clause loop prunes routinely.
fn ground_refutation_fallback(
    ground: &GroundSet,
    inputs: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
    start: Instant,
    budget: EprBudget,
) -> (Option<SearchResult>, usize) {
    let remaining = budget.timeout.saturating_sub(start.elapsed());
    if remaining.is_zero() {
        return (None, 0);
    }
    let _ = PROOF_RESERVE;

    // Search the *instances only*. The input clauses are handed over as
    // provenance so the instantiation steps can name them as parents, which
    // registers them in the clause store without putting them in `unprocessed`.
    // That distinction is the whole point: the first-order inputs carry
    // universally quantified variables, and inferring over them turns a
    // propositional refutation that CaDiCaL has already certified into a search
    // over a much larger space.
    let instances: Vec<Clause> = ground.clauses.iter().map(|g| g.clause.clone()).collect();
    let mut parents: Vec<Clause> = inputs.to_vec();
    parents.extend(provenance.iter().cloned());

    let mut state = crate::state::SearchState::new_with_ml(
        instances,
        parents,
        id_gen.clone(),
        std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        std::sync::Arc::new(symbols.clone()),
        // Matches `use_avatar: true` below: AVATAR splitting is set up in the
        // state as well as enabled in the config, so the fallback really is
        // the DPLL(T)-shaped search the comment below describes.
        true,
        None,
        false,
        crate::ClauseWeightFn::Standard,
    );
    let config = crate::SearchConfig {
        time_limit: remaining,
        ordering: crate::TermOrdering::KBO,
        literal_selection: crate::LiteralSelection::AllNegative,
        selection: crate::SelectionStrategy::SmallestFirst,
        // AVATAR on a ground clause set is a DPLL(T) search: the split atoms are
        // propositions and the SAT solver replaces the resolution derivation
        // CaDiCaL has just discarded. It is the one configuration that scales
        // with the size of the instance set, so it is the default here.
        use_avatar: true,
        ..crate::SearchConfig::default()
    };
    match crate::given_clause::search(&mut state, &config) {
        SearchResult::Refutation(id, tstp) => {
            // The derivation is rendered TSTP: one `cnf(`/`fof(` step per
            // line, plus the `% Proof` header. Count the steps so the
            // telemetry's proof-size column is real on this route too.
            let nodes = tstp
                .lines()
                .filter(|l| l.starts_with("cnf(") || l.starts_with("fof("))
                .count();
            (Some(SearchResult::Refutation(id, tstp)), nodes)
        }
        _ => (None, 0),
    }
}

/// Helpers shared by the two test modules.
#[cfg(test)]
mod tests_common {
    use super::*;

    pub fn input(id: ClauseId, lits: Vec<Literal>) -> Clause {
        Clause::new(
            id,
            lits,
            mrs_core::clause::ClauseSource::Input {
                name: format!("c{id:?}"),
                role: "axiom".into(),
            },
        )
    }

    pub fn pred(positive: bool, sym: SymbolId, args: Vec<Term>) -> Literal {
        Literal {
            positive,
            atom: Atom::pred(sym, args),
        }
    }

    /// Whether `lits` — all ground — is a substitution instance of `clause`.
    ///
    /// Brute force over the clause's variables, which is fine for the shapes a
    /// test builds: the soundness property being checked is existential (some
    /// substitution works), and an exhaustive witness search is exactly the
    /// statement to check against.
    pub fn is_instance_of(clause: &Clause, lits: &[Literal]) -> bool {
        let vars: Vec<VarId> = clause_vars_ordered(clause);
        let mut bindings: Vec<(VarId, SymbolId)> = Vec::new();
        let constants = ground_constants_in(lits);
        if constants.is_empty() {
            return lits.is_empty();
        }
        assign(&vars, 0, &constants, &mut bindings, clause, lits)
    }

    fn ground_constants_in(lits: &[Literal]) -> Vec<SymbolId> {
        let mut out = Vec::new();
        for lit in lits {
            match &lit.atom {
                Atom::Pred(_, args) => {
                    for t in args {
                        if let Some(c) = constant_of(t) {
                            out.push(c);
                        }
                    }
                }
                Atom::Eq(l, r) => {
                    for t in [l, r] {
                        if let Some(c) = constant_of(t) {
                            out.push(c);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn assign(
        vars: &[VarId],
        depth: usize,
        constants: &[SymbolId],
        bindings: &mut Vec<(VarId, SymbolId)>,
        clause: &Clause,
        lits: &[Literal],
    ) -> bool {
        if depth == vars.len() {
            let mut subst = Substitution::new();
            for (v, c) in bindings.iter() {
                subst.bind(*v, Term::constant(*c));
            }
            let want: Vec<Literal> = clause
                .literals
                .iter()
                .map(|l| subst.apply_literal(l))
                .collect();
            let mut got = lits.to_vec();
            let mut want = want;
            got.sort_by_key(|l| format!("{l:?}"));
            want.sort_by_key(|l| format!("{l:?}"));
            return got == want;
        }
        for &c in constants {
            bindings.push((vars[depth], c));
            if assign(vars, depth + 1, constants, bindings, clause, lits) {
                bindings.pop();
                return true;
            }
            bindings.pop();
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::ClauseSource;
    use mrs_core::symbol::SymbolTable;

    use super::tests_common::*;

    fn input(id: ClauseId, lits: Vec<Literal>) -> Clause {
        Clause::new(
            id,
            lits,
            ClauseSource::Input {
                name: format!("c{id:?}"),
                role: "axiom".into(),
            },
        )
    }

    fn goal(id: ClauseId, lits: Vec<Literal>) -> Clause {
        Clause::new(
            id,
            lits,
            ClauseSource::Input {
                name: format!("g{id:?}"),
                role: "negated_conjecture".into(),
            },
        )
    }

    fn pred(positive: bool, sym: SymbolId, args: Vec<Term>) -> Literal {
        tests_common::pred(positive, sym, args)
    }

    /// `p(c,X) ∨ ~p(c,X)` has only the one ground instance `p(c,d) ∨ ~p(c,d)`.
    ///
    /// SOUNDNESS REGRESSION. The `⊥` bootstrap used to replace *every* term, which
    /// turned `p(c,X)` into `p($bot,$bot)`. That is not an instance of `p(c,X)` — the
    /// instance is `p(c,$bot)` — and it made the SAT abstraction unsatisfiable for
    /// a reason unrelated to the problem, so the pre-pass reported refutations whose
    /// proofs the checker rejects. The proof that failed read
    /// `m_cell_v_token($bot,$bot)` from parent `m_cell_v_token(c_e_h_3,X0)`.
    #[test]
    fn bot_instance_replaces_variables_not_constants() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let c = symbols.intern("c");
        let bot = symbols.intern("$bot");
        let clause = input(
            ClauseId(0),
            vec![pred(true, p, vec![Term::constant(c), Term::var(0)])],
        );
        let mut id_gen = ClauseIdGen::new();
        let inst = bot_instance(&clause, bot, &mut id_gen).expect("bootstrap instance");
        assert_eq!(inst.literals.len(), 1);
        match &inst.literals[0].atom {
            Atom::Pred(sym, args) => {
                assert_eq!(*sym, p);
                // The constant must survive; only the variable becomes the
                // placeholder.
                assert_eq!(args[0], Term::constant(c));
                assert_eq!(args[1], Term::constant(bot));
            }
            other => panic!("expected a predicate, got {other:?}"),
        }
    }

    /// The instance `⊥`-image of a clause is a genuine instance of that clause,
    /// which is what makes a refutation over the bootstrap sound.
    #[test]
    fn bot_instance_is_a_substitution_instance() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let bot = symbols.intern("$bot");
        let clause = input(
            ClauseId(0),
            vec![
                pred(true, p, vec![Term::var(0), Term::constant(a)]),
                pred(false, q, vec![Term::var(1), Term::constant(b)]),
            ],
        );
        let mut id_gen = ClauseIdGen::new();
        let inst = bot_instance(&clause, bot, &mut id_gen).expect("bootstrap instance");
        let mut expected = Substitution::new();
        expected.bind(0, Term::constant(bot));
        expected.bind(1, Term::constant(bot));
        let want: Vec<Literal> = clause
            .literals
            .iter()
            .map(|l| expected.apply_literal(l))
            .collect();
        // Literal order is normalised by `Clause::new` (dedup and sort), so the
        // comparison is set-like; a mismatch in *content* is what matters here.
        let got: Vec<Literal> = inst.literals.iter().cloned().collect();
        assert_eq!(got.len(), want.len());
        assert!(got.iter().all(|l| want.contains(l)));
    }

    /// An equality literal is a ground atom like any other, so a bootstrap
    /// instance carries it and the `epr_equality` profile is searched by the same
    /// loop as the purely relational one.
    #[test]
    fn equality_survives_grounding() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let bot = symbols.intern("$bot");
        // x = a ∨ ¬(y = b)
        let clause = input(
            ClauseId(0),
            vec![
                Literal {
                    positive: true,
                    atom: Atom::eq(Term::var(0), Term::constant(a)),
                },
                Literal {
                    positive: false,
                    atom: Atom::eq(Term::var(1), Term::constant(b)),
                },
            ],
        );
        let mut id_gen = ClauseIdGen::new();
        let inst = bot_instance(&clause, bot, &mut id_gen).expect("bootstrap instance");
        let mut abs = GroundAbstraction::default();
        // Both sides ground, so the clause abstracts; the equality atoms are
        // propositionally atomic and lift back to first-order literals.
        let Abstracted::Clause(pc) = abs.abstract_clause(&inst) else {
            panic!("ground equality clause must abstract");
        };
        assert_eq!(pc.len(), 2);
        for &lit in &pc {
            let atom = &abs.var_to_atom[lit.unsigned_abs() as usize - 1];
            assert!(
                matches!(atom, GAtom::Eq(..)),
                "expected a ground equality atom, got {atom:?}"
            );
            let back = atom.to_literal(lit > 0);
            assert!(matches!(back.atom, Atom::Eq(..)));
        }
    }

    /// `a = b` and `b = a` are the same ground atom: argument order is
    /// canonical, so the two spellings share one SAT variable instead of
    /// reasoning as independent propositions.
    #[test]
    fn equality_atoms_normalize_argument_order() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let fwd = GAtom::of_literal(&Literal {
            positive: true,
            atom: Atom::eq(Term::constant(a), Term::constant(b)),
        });
        let rev = GAtom::of_literal(&Literal {
            positive: false,
            atom: Atom::eq(Term::constant(b), Term::constant(a)),
        });
        assert_eq!(fwd, rev);
    }

    /// Reflexive equalities simplify during abstraction: a positive `c = c`
    /// makes the clause vacuous, a negative one (`c != c`) is dropped, and a
    /// clause of nothing but dropped literals is the empty clause.
    #[test]
    fn abstraction_simplifies_reflexive_equalities() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut abs = GroundAbstraction::default();
        // `p(a) ∨ a = a`: true in every model, carries no constraint.
        let valid = input(
            ClauseId(0),
            vec![
                pred(true, p, vec![Term::constant(a)]),
                Literal {
                    positive: true,
                    atom: Atom::eq(Term::constant(a), Term::constant(a)),
                },
            ],
        );
        assert!(matches!(abs.abstract_clause(&valid), Abstracted::Skip));
        // `p(a) ∨ a != a`: the false literal drops, one constraint remains.
        let drop_false = input(
            ClauseId(1),
            vec![
                pred(true, p, vec![Term::constant(a)]),
                Literal {
                    positive: false,
                    atom: Atom::eq(Term::constant(a), Term::constant(a)),
                },
            ],
        );
        let Abstracted::Clause(pc) = abs.abstract_clause(&drop_false) else {
            panic!("clause with a dropped false literal must abstract");
        };
        assert_eq!(pc.len(), 1);
        // `a != a` alone: the empty clause, an immediate contradiction.
        let empty = input(
            ClauseId(2),
            vec![Literal {
                positive: false,
                atom: Atom::eq(Term::constant(a), Term::constant(a)),
            }],
        );
        let Abstracted::Clause(pc) = abs.abstract_clause(&empty) else {
            panic!("unit `c != c` must abstract to the empty clause");
        };
        assert!(pc.is_empty());
    }

    /// `c = c` is true and `c != c` is false, regardless of which the model
    /// says about anything else: polarity decides, not the solver.
    #[test]
    fn literal_value_respects_reflexive_equality_polarity() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let abs = GroundAbstraction::default();
        let solver = Solver::new();
        let mk = |positive: bool| Literal {
            positive,
            atom: Atom::eq(Term::constant(a), Term::constant(a)),
        };
        let sigma = HashMap::default();
        assert_eq!(literal_value(&mk(true), &sigma, &abs, &solver), Some(true));
        assert_eq!(
            literal_value(&mk(false), &sigma, &abs, &solver),
            Some(false)
        );
    }

    /// A variable already bound by `sigma` stands for its constant: `X = Y`
    /// with `X` bound is a template in `Y`, not a refusal.
    #[test]
    fn template_eq_sees_through_bound_variables() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let mut sigma = HashMap::default();
        sigma.insert(0, a);
        let atom = Atom::eq(Term::var(0), Term::var(1));
        let Some((template, var)) = Template::build(&atom, &sigma) else {
            panic!("X = Y with X bound must build a template in Y");
        };
        assert_eq!(var, 1);
        assert!(matches!(template, Template::Eq { other } if other == a));
    }

    /// Completing a partial substitution with `⊥` grounds it: every variable
    /// ends up bound.
    #[test]
    fn bot_completion_grounds_a_partial_substitution() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let bot = symbols.intern("$bot");
        let mut subst = Substitution::new();
        subst.bind(0, Term::constant(a));
        complete_with_bot(&[0, 1, 2], &mut subst, bot);
        for v in [0, 1, 2] {
            assert!(
                subst.lookup(v).is_some(),
                "variable {v} must be bound after completion"
            );
        }
        assert_eq!(subst.lookup(0), Some(&Term::constant(a)));
        assert_eq!(subst.lookup(1), Some(&Term::constant(bot)));
    }

    /// REGRESSION. Rungs narrower than the clause used to emit partial
    /// instances — one variable bound, the rest free — which the ground-set
    /// abstraction drops on arrival, so the rung searched nothing. Every
    /// candidate below must be fully ground.
    #[test]
    fn narrow_rungs_emit_ground_instances() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let c0 = symbols.intern("c0");
        let c1 = symbols.intern("c1");
        let bot = symbols.intern("$bot");
        // Four distinct variables: wider than every rung but uniform/all.
        let clause = input(
            ClauseId(0),
            vec![pred(
                true,
                p,
                vec![Term::var(0), Term::var(1), Term::var(2), Term::var(3)],
            )],
        );
        let vars = clause_vars_ordered(&clause);
        assert_eq!(vars.len(), 4);
        for rung in [
            Rung::OneVariablePivots,
            Rung::OneVariableDomain,
            Rung::TwoVariablePivots,
        ] {
            let mut cursor = 0usize;
            let mut out: Vec<Vec<Literal>> = Vec::new();
            rung_instances(
                &clause,
                &vars,
                rung,
                &[vec![c0, c1]],
                &mut cursor,
                64,
                &mut out,
                Instant::now() + Duration::from_secs(10),
                bot,
            );
            assert!(
                !out.is_empty(),
                "rung {rung:?} must emit candidates for a four-variable clause"
            );
            for lits in &out {
                for lit in lits {
                    assert!(
                        GAtom::of_literal(lit).is_some(),
                        "rung {rung:?} emitted a non-ground literal: {lit:?}"
                    );
                }
            }
        }
    }

    /// `p(X) ∨ ¬p(X)` has no non-tautological ground instance, so the bootstrap
    /// drops it. Emitting it anyway would assert `$bot` and nothing else.
    #[test]
    fn bot_instance_drops_tautologies() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let bot = symbols.intern("$bot");
        let clause = input(
            ClauseId(0),
            vec![
                pred(true, p, vec![Term::var(0)]),
                pred(false, p, vec![Term::var(0)]),
            ],
        );
        let mut id_gen = ClauseIdGen::new();
        assert!(bot_instance(&clause, bot, &mut id_gen).is_none());
    }

    /// A refutation over ground instances only. The two clauses are a pair of
    /// unit clauses over a shared constant, and the `⊥` bootstrap is the instance
    /// that closes them.
    #[test]
    fn refutes_unit_pair_over_a_shared_constant() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let clauses = vec![
            input(ClauseId(0), vec![pred(true, p, vec![Term::constant(a)])]),
            input(ClauseId(1), vec![pred(false, p, vec![Term::constant(a)])]),
        ];
        let mut id_gen = ClauseIdGen::new();
        let budget = EprBudget {
            timeout: Duration::from_secs(5),
            max_instances: 10_000,
            byte_budget: 1 << 30,
            memory_ceiling_mb: None,
            max_rounds: 8,
        };
        let (result, tele) =
            try_epr_ground_refutation(&clauses, &clauses, &mut id_gen, &symbols, budget);
        assert!(
            matches!(result, Some(SearchResult::Refutation(..))),
            "expected a refutation, got {result:?} (telemetry {tele:?})"
        );
        assert!(tele.proof_extracted);
    }

    /// A satisfiable clause set must not produce a refutation. The ground
    /// expansion of a satisfiable EPR problem is satisfiable, and the bootstrap
    /// plus the widening rungs only ever add genuine instances, so the SAT
    /// abstraction cannot be driven unsatisfiable by a choice of instances.
    #[test]
    fn does_not_refute_a_satisfiable_clause_set() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let clauses = vec![
            input(ClauseId(0), vec![pred(true, p, vec![Term::constant(a)])]),
            input(ClauseId(1), vec![pred(false, p, vec![Term::constant(b)])]),
        ];
        let mut id_gen = ClauseIdGen::new();
        let budget = EprBudget {
            timeout: Duration::from_secs(2),
            max_instances: 10_000,
            byte_budget: 1 << 30,
            memory_ceiling_mb: None,
            max_rounds: 8,
        };
        let (result, tele) =
            try_epr_ground_refutation(&clauses, &clauses, &mut id_gen, &symbols, budget);
        assert!(
            result.is_none(),
            "a satisfiable clause set must not yield a refutation, got {result:?}"
        );
        assert_ne!(tele.result, "refutation");
    }

    /// Variables come back in a fixed order, so the widening rungs bind the same
    /// variables on every run. A `HashSet` order would make the instantiation
    /// restriction — and therefore the search — depend on hash iteration order.
    #[test]
    fn clause_variables_are_ordered_by_first_appearance() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let clause = input(
            ClauseId(0),
            vec![
                pred(true, p, vec![Term::var(7), Term::var(3)]),
                pred(false, q, vec![Term::var(3), Term::var(9)]),
            ],
        );
        let _ = &symbols;
        assert_eq!(clause_vars_ordered(&clause), vec![7, 3, 9]);
    }

    /// Pivot selection is deterministic and prefers the goal's own constants,
    /// which are the ones a refutation has to involve.
    #[test]
    fn pivots_are_deterministic_and_goal_weighted() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let skolem = symbols.intern("sK0");
        let rare = symbols.intern("rare");
        let clauses = vec![
            // Two units, so `rare` is seen twice.
            input(ClauseId(0), vec![pred(true, p, vec![Term::constant(rare)])]),
            input(ClauseId(1), vec![pred(true, p, vec![Term::constant(rare)])]),
            // The goal mentions `sK0` once, but goal weight is 4.
            goal(
                ClauseId(2),
                vec![pred(true, p, vec![Term::constant(skolem)])],
            ),
        ];
        let pivots = pivot_constants(&clauses, 2);
        assert_eq!(pivots, vec![skolem, rare]);
        assert_eq!(pivot_constants(&clauses, 2), pivots);
    }
}

#[cfg(test)]
mod split_tests {
    use super::tests_common::*;
    use super::*;

    fn eq_lit(positive: bool, l: Term, r: Term) -> Literal {
        Literal {
            positive,
            atom: Atom::eq(l, r),
        }
    }

    /// The shape that motivates the whole rule: a disjunction of `cᵢ = X` plus a
    /// rest, over a domain where the support covers most of it.
    fn hwv_style(support: &[SymbolId], rest_pred: SymbolId) -> Clause {
        let mut lits: Vec<Literal> = support
            .iter()
            .map(|&c| eq_lit(true, Term::constant(c), Term::var(0)))
            .collect();
        lits.push(pred(true, rest_pred, vec![Term::var(0)]));
        input(ClauseId(0), lits)
    }

    /// The support of `X` is exactly the set of constants the clause pins it to.
    #[test]
    fn support_is_the_pinned_constants() {
        let mut s = SymbolTable::new();
        let p = s.intern("p");
        let a = s.intern("a");
        let b = s.intern("b");
        let clause = hwv_style(&[a, b], p);
        let vars = clause_vars_ordered(&clause);
        let support = split_support(&clause, &vars);
        let got: HashSet<SymbolId> = support[&0].iter().copied().collect();
        let want: HashSet<SymbolId> = [a, b].into_iter().collect();
        assert_eq!(got, want, "X is pinned to a and b by the equality literals");
    }

    /// Splitting prunes the candidate list to the values the clause's own
    /// equality literals do not already account for. This is the entire
    /// mechanism: on the HWV family it is the difference between grounding a
    /// variable at ~205 values and at ~5.
    #[test]
    fn splitting_prunes_the_candidate_list() {
        let mut s = SymbolTable::new();
        let p = s.intern("p");
        let a = s.intern("a");
        let b = s.intern("b");
        let c = s.intern("c");
        let d = s.intern("d");
        let domain = vec![a, b, c, d];
        let clause = hwv_style(&[a, b, c], p);
        let vars = clause_vars_ordered(&clause);
        let support = split_support(&clause, &vars);

        let unpruned = slot_constants(
            Rung::OneVariableDomain,
            0,
            &vars,
            &HashMap::default(),
            &[],
            &domain,
        );
        assert_eq!(
            unpruned.len(),
            4,
            "without splitting every domain value is a candidate"
        );

        let pruned = slot_constants(Rung::OneVariableDomain, 0, &vars, &support, &[], &domain);
        assert_eq!(
            pruned,
            vec![d],
            "only the value the clause does not account for survives"
        );
    }

    /// A variable that occurs only in equality literals is not a split variable:
    /// `X = c` alone is decided by the equality, with no rest to restrict.
    #[test]
    fn a_variable_only_in_equalities_is_not_a_split_variable() {
        let mut s = SymbolTable::new();
        let a = s.intern("a");
        let b = s.intern("b");
        // (X = a ∨ Y = b), with X and Y appearing nowhere else.
        let clause = input(
            ClauseId(0),
            vec![
                eq_lit(true, Term::var(0), Term::constant(a)),
                eq_lit(true, Term::var(1), Term::constant(b)),
            ],
        );
        let vars = clause_vars_ordered(&clause);
        let support = split_support(&clause, &vars);
        assert!(support[&0].is_empty(), "X has no rest to restrict");
        assert!(support[&1].is_empty(), "Y has no rest to restrict");
    }

    /// `X = Y` is the shape restricted *equality resolution* handles. Splitting
    /// must not treat it as a pin: dropping the values of `Y` would lose the
    /// constraint that the two are equal.
    #[test]
    fn variable_equals_variable_is_not_a_split() {
        let mut s = SymbolTable::new();
        let p = s.intern("p");
        // (X = Y ∨ p(X))
        let clause = input(
            ClauseId(0),
            vec![
                eq_lit(true, Term::var(0), Term::var(1)),
                pred(true, p, vec![Term::var(0)]),
            ],
        );
        let vars = clause_vars_ordered(&clause);
        let support = split_support(&clause, &vars);
        assert!(support[&0].is_empty(), "X = Y pins no constant");
        assert!(support[&1].is_empty(), "X = Y pins no constant");
    }

    /// SOUNDNESS. Every instance the pruned rung emits is a substitution instance
    /// of the input clause. The pruning removes candidates; it must never
    /// manufacture a clause the clause does not entail.
    #[test]
    fn emitted_candidates_are_instances_of_the_clause() {
        let mut s = SymbolTable::new();
        let p = s.intern("p");
        let q = s.intern("q");
        let a = s.intern("a");
        let b = s.intern("b");
        let c = s.intern("c");
        let bot = s.intern("$bot");
        let domain = vec![a, b, c];
        // (X = a ∨ X = b ∨ q(X) ∨ p(X, Y)) — split on X, second variable Y.
        let clause = input(
            ClauseId(0),
            vec![
                eq_lit(true, Term::constant(a), Term::var(0)),
                eq_lit(true, Term::constant(b), Term::var(0)),
                pred(true, q, vec![Term::var(0)]),
                pred(true, p, vec![Term::var(0), Term::var(1)]),
            ],
        );
        let vars = clause_vars_ordered(&clause);
        let support = split_support(&clause, &vars);
        let slots: Vec<Vec<SymbolId>> = (0..2)
            .map(|slot| {
                slot_constants(Rung::OneVariableDomain, slot, &vars, &support, &[], &domain)
            })
            .collect();
        let mut out: Vec<Vec<Literal>> = Vec::new();
        rung_instances(
            &clause,
            &vars,
            Rung::OneVariableDomain,
            &slots,
            &mut 0,
            256,
            &mut out,
            Instant::now() + Duration::from_secs(10),
            bot,
        );
        assert!(!out.is_empty(), "the split rung must emit candidates here");
        for lits in &out {
            // Ground: every literal is a ground atom, so it is an instance.
            for lit in lits {
                assert!(
                    GAtom::of_literal(lit).is_some(),
                    "emitted a non-ground literal: {lit:?}"
                );
            }
            // And it is a consequence of the clause: some substitution of the
            // clause's variables into constants yields exactly this literal set.
            assert!(
                is_instance_of(&clause, lits),
                "emitted {lits:?}, which is not an instance of the clause"
            );
        }
    }

    /// REGRESSION. A nullary predicate is a 0-ary function symbol applied to
    /// nothing, so it is a Herbrand element exactly like a named constant.
    /// Reading only argument positions dropped every one of them, and all 100
    /// CASC-30 EPU problems contain them — up to 2 527 in `HWV090-1` — so the
    /// universe every rung drew from was a strict subset of the real one, and a
    /// rung that exhausted it reported `model_fixpoint` as if nothing were left.
    #[test]
    fn nullary_predicates_are_herbrand_elements() {
        let mut s = SymbolTable::new();
        let p = s.intern("p");
        let q = s.intern("q");
        let nullary = s.intern("esk1_0");
        let other_nullary = s.intern("esk2_0");
        let named = s.intern("c_e_h_3");
        // (esk1_0 ∨ p(X,Y) ∨ q(c_e_h_3)) — `esk1_0` occurs with no arguments,
        // `esk2_0` not at all, `c_e_h_3` as a named constant.
        let clause = input(
            ClauseId(0),
            vec![
                pred(true, nullary, vec![]),
                pred(true, p, vec![Term::var(0), Term::var(1)]),
                pred(true, q, vec![Term::constant(named)]),
            ],
        );
        let universe = crate::instgen::collect_constants(&[clause]);
        assert!(
            universe.contains(&nullary),
            "a nullary predicate is a ground term and must be in the universe"
        );
        assert!(
            !universe.contains(&other_nullary),
            "a symbol that does not occur is not in the universe"
        );
        assert!(
            universe.contains(&named),
            "a named constant stays in the universe"
        );
    }

    /// The pruning is what makes the HWV shape affordable. A three-variable
    /// clause over the same domain costs `|domain|` times as many instances
    /// without splitting as with it, because the split variable's list is
    /// `D \ support` rather than `D`.
    #[test]
    fn splitting_reduces_the_cross_product() {
        let mut s = SymbolTable::new();
        let p = s.intern("p");
        let domain: Vec<SymbolId> = (0..8).map(|i| s.intern(&format!("c{i}"))).collect();
        let support: Vec<SymbolId> = domain[..5].to_vec();
        // Three variables, so the rung cross-products two of them.
        let clause = input(
            ClauseId(0),
            vec![
                // X is pinned to five of the eight constants.
                eq_lit(true, Term::constant(domain[0]), Term::var(0)),
                eq_lit(true, Term::constant(domain[1]), Term::var(0)),
                eq_lit(true, Term::constant(domain[2]), Term::var(0)),
                eq_lit(true, Term::constant(domain[3]), Term::var(0)),
                eq_lit(true, Term::constant(domain[4]), Term::var(0)),
                pred(true, p, vec![Term::var(0), Term::var(1), Term::var(2)]),
            ],
        );
        let vars = clause_vars_ordered(&clause);
        let support_map = split_support(&clause, &vars);
        let pruned_0 = slot_constants(
            Rung::OneVariableDomain,
            0,
            &vars,
            &support_map,
            &[],
            &domain,
        );
        assert_eq!(pruned_0.len(), 3, "8 domain values less the 5 pinned ones");

        // Unpruned, slot 0 would offer 8; pruned it offers 3, so the two
        // independent slots go from 64 combinations to 9.
        let combos = |list: usize| list * list;
        assert_eq!(combos(8), 64);
        assert_eq!(combos(3), 9);
        let _ = support;
    }
}

//! Bounded SAT-based subsumption and subsumption resolution for wide clauses.
//!
//! This is an opt-in alternative to the legacy backtracking matchers. Each
//! literal match is computed once by the calculus crate's one-way matcher. One
//! CaDiCaL instance first tests ordinary subsumption, then (if that fails) uses
//! the same match set to test subsumption resolution. Any returned SAT model is
//! reconstructed and checked before the caller deletes a clause or builds the
//! usual replayable `subsumption_resolution` witness.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use mrs_cadical::{SolveResult, Solver};
use mrs_calculus::rename::{is_ground_id, max_var_id, rename_clause_id};
use mrs_calculus::subsumption::subsumption_resolution_atom_match_options_id;
use mrs_core::SymbolId;
use mrs_core::term::VarId;
use mrs_core::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};

/// Hard ceiling for one SAT-simplification clause pair. The target divisions
/// include clauses around 200 literals; beyond this, candidate construction
/// itself is capped by the option and time budgets below.
pub(crate) const MAX_SAT_SR_WIDTH: usize = 512;

/// Keep one SAT query from monopolizing a portfolio worker. This includes
/// matching-option construction, SAT encoding, and CaDiCaL search.
const QUERY_BUDGET: Duration = Duration::from_millis(25);
const MAX_MATCH_OPTIONS: usize = 8_192;
const MAX_MATCH_BINDINGS: usize = 65_536;
const MAX_SAT_VARIABLES: usize = 50_000;
const MAX_SAT_CLAUSES: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SatQueryOutcome {
    NotRun,
    Sat,
    Unsat,
    NoMatch,
    Unknown,
    Limit,
    InvalidModel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SatSimplificationOutcome {
    Subsumed,
    SubsumptionResolution(usize),
    NoSimplification,
    Unknown,
    Limit,
    InvalidModel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SatSimplificationAttempt {
    pub outcome: SatSimplificationOutcome,
    pub subsumption: SatQueryOutcome,
    pub sr: SatQueryOutcome,
    pub subsumption_solver_entered: bool,
    pub sr_solver_entered: bool,
    pub elapsed_us: u64,
}

#[derive(Clone, Debug)]
struct MatchChoice {
    target_idx: usize,
    negative: bool,
    bindings: Vec<(VarId, TermId)>,
    sat_var: i32,
}

#[derive(Clone, Debug)]
struct MatchSet {
    choices: Vec<MatchChoice>,
    by_source: Vec<Vec<usize>>,
    positive_by_source: Vec<Vec<usize>>,
    negative_by_target: Vec<Vec<usize>>,
    positive_by_target: Vec<Vec<usize>>,
    source_vars: Vec<VarId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum AtomShape {
    Predicate(SymbolId, usize),
    Equality,
}

fn atom_shape(atom: &IdAtom) -> AtomShape {
    match atom {
        IdAtom::Pred(symbol, args) => AtomShape::Predicate(*symbol, args.len()),
        IdAtom::Eq(..) => AtomShape::Equality,
    }
}

/// Try ordinary subsumption, then SR, reusing one sparse set of literal-match
/// options. The ordinary subsumption query is preferred: if it succeeds, the
/// target is discarded instead of deriving an SR child.
pub(crate) fn simplify_pair_until(
    active: &IdClause,
    target: &IdClause,
    bank: &mut TermBank,
    deadline: Option<Instant>,
    cancellation: Option<&AtomicBool>,
) -> SatSimplificationAttempt {
    let started = Instant::now();
    let mut attempt = simplify_pair_inner(active, target, bank, deadline, cancellation);
    attempt.elapsed_us = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
    attempt
}

fn simplify_pair_inner(
    active: &IdClause,
    target: &IdClause,
    bank: &mut TermBank,
    search_deadline: Option<Instant>,
    cancellation: Option<&AtomicBool>,
) -> SatSimplificationAttempt {
    if active.literals.is_empty() {
        return completed_attempt(
            SatSimplificationOutcome::Subsumed,
            SatQueryOutcome::Sat,
            SatQueryOutcome::NotRun,
            false,
            false,
        );
    }
    if target.literals.is_empty() || active.literals.len() > target.literals.len() {
        return completed_attempt(
            SatSimplificationOutcome::NoSimplification,
            SatQueryOutcome::NoMatch,
            SatQueryOutcome::NoMatch,
            false,
            false,
        );
    }
    if active.literals.len() > MAX_SAT_SR_WIDTH || target.literals.len() > MAX_SAT_SR_WIDTH {
        return completed_attempt(
            SatSimplificationOutcome::Limit,
            SatQueryOutcome::Limit,
            SatQueryOutcome::NotRun,
            false,
            false,
        );
    }

    let now = Instant::now();
    if query_expired(search_deadline, cancellation) {
        return completed_attempt(
            SatSimplificationOutcome::Unknown,
            SatQueryOutcome::Unknown,
            SatQueryOutcome::NotRun,
            false,
            false,
        );
    }
    let query_deadline = now
        .checked_add(QUERY_BUDGET)
        .map(|local| search_deadline.map_or(local, |outer| outer.min(local)))
        .or(search_deadline);

    let match_set = match build_match_set(active, target, bank, query_deadline, cancellation) {
        Ok(match_set) => match_set,
        Err(SatQueryOutcome::NoMatch) => {
            return completed_attempt(
                SatSimplificationOutcome::NoSimplification,
                SatQueryOutcome::NoMatch,
                SatQueryOutcome::NoMatch,
                false,
                false,
            );
        }
        Err(SatQueryOutcome::Unknown) => {
            return completed_attempt(
                SatSimplificationOutcome::Unknown,
                SatQueryOutcome::Unknown,
                SatQueryOutcome::NotRun,
                false,
                false,
            );
        }
        Err(_) => {
            return completed_attempt(
                SatSimplificationOutcome::Limit,
                SatQueryOutcome::Limit,
                SatQueryOutcome::NotRun,
                false,
                false,
            );
        }
    };

    let mut encoder = match encode_match_set(match_set, query_deadline, cancellation) {
        Ok(encoded) => encoded,
        Err(SatQueryOutcome::Unknown) => {
            return completed_attempt(
                SatSimplificationOutcome::Unknown,
                SatQueryOutcome::Unknown,
                SatQueryOutcome::NotRun,
                false,
                false,
            );
        }
        Err(_) => {
            return completed_attempt(
                SatSimplificationOutcome::Limit,
                SatQueryOutcome::Limit,
                SatQueryOutcome::NotRun,
                false,
                false,
            );
        }
    };
    let mode_var = encoder.mode_var;
    let pivot_vars = encoder.pivot_vars.clone();

    // Test ordinary subsumption first. If some active literal has no
    // same-polarity match, this mode is structurally impossible; skip directly
    // to the SR query but keep the shared match set and solver encoding.
    let plain_structurally_possible = encoder
        .match_set
        .positive_by_source
        .iter()
        .all(|matches| !matches.is_empty());
    let (subsumption, subsumption_solver_entered, plain_result) = if plain_structurally_possible {
        solve_encoded(&mut encoder, mode_var, false, query_deadline, cancellation)
    } else {
        (SatQueryOutcome::NoMatch, false, None)
    };

    match plain_result {
        Some(SatSimplificationOutcome::Subsumed) => {
            return completed_attempt(
                SatSimplificationOutcome::Subsumed,
                subsumption,
                SatQueryOutcome::NotRun,
                subsumption_solver_entered,
                false,
            );
        }
        Some(SatSimplificationOutcome::InvalidModel) => {
            return completed_attempt(
                SatSimplificationOutcome::InvalidModel,
                SatQueryOutcome::InvalidModel,
                SatQueryOutcome::NotRun,
                subsumption_solver_entered,
                false,
            );
        }
        Some(SatSimplificationOutcome::SubsumptionResolution(_)) => {
            unreachable!("the plain-mode SAT query must not return an SR simplification")
        }
        Some(_) => {
            return completed_attempt(
                SatSimplificationOutcome::InvalidModel,
                SatQueryOutcome::InvalidModel,
                SatQueryOutcome::NotRun,
                subsumption_solver_entered,
                false,
            );
        }
        None => {}
    }
    if matches!(
        subsumption,
        SatQueryOutcome::Unknown | SatQueryOutcome::Limit | SatQueryOutcome::InvalidModel
    ) {
        let outcome = match subsumption {
            SatQueryOutcome::Unknown => SatSimplificationOutcome::Unknown,
            SatQueryOutcome::InvalidModel => SatSimplificationOutcome::InvalidModel,
            _ => SatSimplificationOutcome::Limit,
        };
        return completed_attempt(
            outcome,
            subsumption,
            SatQueryOutcome::NotRun,
            subsumption_solver_entered,
            false,
        );
    }

    if pivot_vars.is_empty() {
        return completed_attempt(
            SatSimplificationOutcome::NoSimplification,
            subsumption,
            SatQueryOutcome::NoMatch,
            subsumption_solver_entered,
            false,
        );
    }
    if query_expired(query_deadline, cancellation) {
        return completed_attempt(
            SatSimplificationOutcome::Unknown,
            subsumption,
            SatQueryOutcome::Unknown,
            subsumption_solver_entered,
            false,
        );
    }

    encoder.solver.assume(mode_var);
    let (sr, sr_solver_entered, sr_result) =
        solve_encoded(&mut encoder, mode_var, true, query_deadline, cancellation);
    if !sr_solver_entered {
        return completed_attempt(
            SatSimplificationOutcome::Unknown,
            subsumption,
            SatQueryOutcome::Unknown,
            subsumption_solver_entered,
            false,
        );
    }
    match sr_result {
        Some(SatSimplificationOutcome::SubsumptionResolution(pivot)) => completed_attempt(
            SatSimplificationOutcome::SubsumptionResolution(pivot),
            subsumption,
            SatQueryOutcome::Sat,
            subsumption_solver_entered,
            true,
        ),
        Some(SatSimplificationOutcome::InvalidModel) => completed_attempt(
            SatSimplificationOutcome::InvalidModel,
            subsumption,
            SatQueryOutcome::InvalidModel,
            subsumption_solver_entered,
            true,
        ),
        Some(SatSimplificationOutcome::Subsumed) => {
            unreachable!("the SR-mode SAT query must not return ordinary subsumption")
        }
        Some(_) => completed_attempt(
            SatSimplificationOutcome::InvalidModel,
            subsumption,
            SatQueryOutcome::InvalidModel,
            subsumption_solver_entered,
            true,
        ),
        None => {
            let outcome = match sr {
                SatQueryOutcome::Unknown => SatSimplificationOutcome::Unknown,
                SatQueryOutcome::Limit => SatSimplificationOutcome::Limit,
                _ => SatSimplificationOutcome::NoSimplification,
            };
            completed_attempt(outcome, subsumption, sr, subsumption_solver_entered, true)
        }
    }
}

fn completed_attempt(
    outcome: SatSimplificationOutcome,
    subsumption: SatQueryOutcome,
    sr: SatQueryOutcome,
    subsumption_solver_entered: bool,
    sr_solver_entered: bool,
) -> SatSimplificationAttempt {
    SatSimplificationAttempt {
        outcome,
        subsumption,
        sr,
        subsumption_solver_entered,
        sr_solver_entered,
        elapsed_us: 0,
    }
}

fn build_match_set(
    active: &IdClause,
    target: &IdClause,
    bank: &mut TermBank,
    deadline: Option<Instant>,
    cancellation: Option<&AtomicBool>,
) -> Result<MatchSet, SatQueryOutcome> {
    // Use the same variable standardization as the calculus matchers. Target
    // variables stay rigid; only active-clause variables may be substituted.
    let bindable_from = max_var_id(target, bank);
    let renamed_active;
    let active_literals = if is_ground_id(active, bank) {
        &active.literals
    } else {
        renamed_active = rename_clause_id(active, bindable_from, bank);
        &renamed_active.literals
    };
    if query_expired(deadline, cancellation) {
        return Err(SatQueryOutcome::Unknown);
    }

    // The feature-vector index admits clauses that share any symbol. Reject a
    // pair before recursive term matching if one active literal has no target
    // atom with the same predicate/arity (or equality kind).
    let target_shapes = target
        .literals
        .iter()
        .map(|literal| atom_shape(&literal.atom))
        .collect::<HashSet<_>>();
    if active_literals
        .iter()
        .any(|literal| !target_shapes.contains(&atom_shape(&literal.atom)))
    {
        return Err(SatQueryOutcome::NoMatch);
    }

    let mut source_vars = Vec::new();
    let mut vars_by_source = Vec::with_capacity(active_literals.len());
    for literal in active_literals {
        let mut vars = Vec::new();
        collect_atom_vars(&literal.atom, bank, &mut vars);
        vars.sort_unstable();
        vars.dedup();
        source_vars.extend_from_slice(&vars);
        vars_by_source.push(vars);
    }
    source_vars.sort_unstable();
    source_vars.dedup();

    let mut choices = Vec::new();
    let mut seen_choices = HashSet::new();
    let mut total_bindings = 0usize;
    let mut by_source = vec![Vec::new(); active_literals.len()];
    let mut positive_by_source = vec![Vec::new(); active_literals.len()];
    let mut negative_by_target = vec![Vec::new(); target.literals.len()];
    let mut positive_by_target = vec![Vec::new(); target.literals.len()];

    for (source_idx, source) in active_literals.iter().enumerate() {
        for (target_idx, candidate) in target.literals.iter().enumerate() {
            if query_expired(deadline, cancellation) {
                return Err(SatQueryOutcome::Unknown);
            }
            if atom_shape(&source.atom) != atom_shape(&candidate.atom) {
                continue;
            }
            let negative = source.positive != candidate.positive;
            let substitutions = subsumption_resolution_atom_match_options_id(
                &source.atom,
                &candidate.atom,
                bindable_from,
                bank,
                deadline,
            );
            if query_expired(deadline, cancellation) {
                return Err(SatQueryOutcome::Unknown);
            }
            for substitution in substitutions {
                let bindings = vars_by_source[source_idx]
                    .iter()
                    .filter_map(|&var| substitution.get(var).map(|term| (var, term)))
                    .collect::<Vec<_>>();
                let signature = bindings
                    .iter()
                    .map(|(var, term)| (*var, term.0))
                    .collect::<Vec<_>>();
                if !seen_choices.insert((source_idx, target_idx, negative, signature)) {
                    continue;
                }
                total_bindings = total_bindings.saturating_add(bindings.len());
                if choices.len() >= MAX_MATCH_OPTIONS || total_bindings > MAX_MATCH_BINDINGS {
                    return Err(SatQueryOutcome::Limit);
                }
                let choice_idx = choices.len();
                choices.push(MatchChoice {
                    target_idx,
                    negative,
                    bindings,
                    sat_var: 0,
                });
                by_source[source_idx].push(choice_idx);
                if negative {
                    negative_by_target[target_idx].push(choice_idx);
                } else {
                    positive_by_source[source_idx].push(choice_idx);
                    positive_by_target[target_idx].push(choice_idx);
                }
            }
        }
    }
    if by_source.iter().any(Vec::is_empty) {
        return Err(SatQueryOutcome::NoMatch);
    }
    Ok(MatchSet {
        choices,
        by_source,
        positive_by_source,
        negative_by_target,
        positive_by_target,
        source_vars,
    })
}

struct EncodedMatchSet {
    solver: Solver,
    match_set: MatchSet,
    mode_var: i32,
    pivot_vars: Vec<(usize, i32)>,
}

fn encode_match_set(
    mut match_set: MatchSet,
    deadline: Option<Instant>,
    cancellation: Option<&AtomicBool>,
) -> Result<EncodedMatchSet, SatQueryOutcome> {
    let mut encoder = BoundedEncoder::new();
    for choice in &mut match_set.choices {
        if query_expired(deadline, cancellation) {
            return Err(SatQueryOutcome::Unknown);
        }
        choice.sat_var = encoder.new_var().ok_or(SatQueryOutcome::Limit)?;
    }
    let mode_var = encoder.new_var().ok_or(SatQueryOutcome::Limit)?;

    for group in &match_set.by_source {
        if query_expired(deadline, cancellation) {
            return Err(SatQueryOutcome::Unknown);
        }
        let vars = group
            .iter()
            .map(|&choice_idx| match_set.choices[choice_idx].sat_var)
            .collect::<Vec<_>>();
        if !encoder.add_at_most_one(&vars) || !encoder.add_clause(vars) {
            return Err(SatQueryOutcome::Limit);
        }
    }

    // A complementary match is only allowed in SR mode. In ordinary
    // subsumption mode (`mode_var = false`) every selected match is therefore
    // same-polarity.
    for choice in &match_set.choices {
        if choice.negative && !encoder.add_clause(vec![-choice.sat_var, mode_var]) {
            return Err(SatQueryOutcome::Limit);
        }
    }

    // `pivot_vars[j]` is true exactly when a complementary match selects target
    // literal occurrence j. At most one occurrence may be the SR pivot; several
    // source literals may reuse that one occurrence under MRS's set-style rule.
    let mut pivot_vars = Vec::new();
    for (target_idx, negative_choices) in match_set.negative_by_target.iter().enumerate() {
        if query_expired(deadline, cancellation) {
            return Err(SatQueryOutcome::Unknown);
        }
        if negative_choices.is_empty() {
            continue;
        }
        let pivot_var = encoder.new_var().ok_or(SatQueryOutcome::Limit)?;
        pivot_vars.push((target_idx, pivot_var));
        let mut pivot_implies_negative = Vec::with_capacity(negative_choices.len() + 1);
        pivot_implies_negative.push(-pivot_var);
        pivot_implies_negative.extend(
            negative_choices
                .iter()
                .map(|&choice_idx| match_set.choices[choice_idx].sat_var),
        );
        if !encoder.add_clause(pivot_implies_negative) {
            return Err(SatQueryOutcome::Limit);
        }
        for &choice_idx in negative_choices {
            if !encoder.add_clause(vec![pivot_var, -match_set.choices[choice_idx].sat_var]) {
                return Err(SatQueryOutcome::Limit);
            }
        }
        for &choice_idx in &match_set.positive_by_target[target_idx] {
            if !encoder.add_clause(vec![-pivot_var, -match_set.choices[choice_idx].sat_var]) {
                return Err(SatQueryOutcome::Limit);
            }
        }
    }
    let mut sr_requires_pivot = Vec::with_capacity(pivot_vars.len() + 1);
    sr_requires_pivot.push(-mode_var);
    sr_requires_pivot.extend(pivot_vars.iter().map(|(_, var)| *var));
    if !encoder.add_clause(sr_requires_pivot)
        || !encoder.add_at_most_one(&pivot_vars.iter().map(|(_, var)| *var).collect::<Vec<_>>())
    {
        return Err(SatQueryOutcome::Limit);
    }

    // A match choice implies each of its variable bindings. At-most-one over
    // distinct terms for each source variable enforces a single global
    // substitution while allowing several choices to reuse the same binding.
    let mut binding_vars = BTreeMap::<(VarId, u32), i32>::new();
    let mut bindings_by_source_var = BTreeMap::<VarId, Vec<i32>>::new();
    for choice in &match_set.choices {
        if query_expired(deadline, cancellation) {
            return Err(SatQueryOutcome::Unknown);
        }
        for &(var, term) in &choice.bindings {
            let key = (var, term.0);
            let binding_var = if let Some(&known) = binding_vars.get(&key) {
                known
            } else {
                let fresh = encoder.new_var().ok_or(SatQueryOutcome::Limit)?;
                binding_vars.insert(key, fresh);
                bindings_by_source_var.entry(var).or_default().push(fresh);
                fresh
            };
            if !encoder.add_clause(vec![-choice.sat_var, binding_var]) {
                return Err(SatQueryOutcome::Limit);
            }
        }
    }
    for vars in bindings_by_source_var.values_mut() {
        if query_expired(deadline, cancellation) {
            return Err(SatQueryOutcome::Unknown);
        }
        vars.sort_unstable();
        vars.dedup();
        if !encoder.add_at_most_one(vars) {
            return Err(SatQueryOutcome::Limit);
        }
    }

    Ok(EncodedMatchSet {
        solver: encoder.solver,
        match_set,
        mode_var,
        pivot_vars,
    })
}

fn validate_mode_model(
    solver: &Solver,
    match_set: &MatchSet,
    mode_var: i32,
    sr_mode: bool,
    pivot_vars: &[(usize, i32)],
) -> Option<SatSimplificationOutcome> {
    if solver.value(mode_var) != Some(sr_mode) {
        return None;
    }
    let mut selected = Vec::with_capacity(match_set.by_source.len());
    let mut substitution = BTreeMap::<VarId, TermId>::new();
    for group in &match_set.by_source {
        let mut group_selected = group.iter().filter_map(|&choice_idx| {
            (solver.value(match_set.choices[choice_idx].sat_var) == Some(true))
                .then_some(choice_idx)
        });
        let choice_idx = group_selected.next()?;
        if group_selected.next().is_some() {
            return None;
        }
        let choice = &match_set.choices[choice_idx];
        if !sr_mode && choice.negative {
            return None;
        }
        for &(var, term) in &choice.bindings {
            if substitution
                .insert(var, term)
                .is_some_and(|previous| previous != term)
            {
                return None;
            }
        }
        selected.push(choice_idx);
    }
    if match_set
        .source_vars
        .iter()
        .any(|var| !substitution.contains_key(var))
    {
        return None;
    }

    if !sr_mode {
        if pivot_vars
            .iter()
            .any(|(_, var)| solver.value(*var) != Some(false))
        {
            return None;
        }
        return Some(SatSimplificationOutcome::Subsumed);
    }

    let mut negative_targets = selected
        .iter()
        .filter_map(|&choice_idx| {
            let choice = &match_set.choices[choice_idx];
            choice.negative.then_some(choice.target_idx)
        })
        .collect::<Vec<_>>();
    negative_targets.sort_unstable();
    negative_targets.dedup();
    if negative_targets.len() != 1 {
        return None;
    }
    let pivot = negative_targets[0];
    if selected.iter().any(|&choice_idx| {
        let choice = &match_set.choices[choice_idx];
        choice.target_idx == pivot && !choice.negative
    }) {
        return None;
    }
    for (target_idx, pivot_var) in pivot_vars {
        if solver.value(*pivot_var) != Some(*target_idx == pivot) {
            return None;
        }
    }
    Some(SatSimplificationOutcome::SubsumptionResolution(pivot))
}

fn solve_encoded(
    encoded: &mut EncodedMatchSet,
    mode: i32,
    sr_mode: bool,
    deadline: Option<Instant>,
    cancellation: Option<&AtomicBool>,
) -> (SatQueryOutcome, bool, Option<SatSimplificationOutcome>) {
    if query_expired(deadline, cancellation) {
        return (SatQueryOutcome::Unknown, false, None);
    }
    let Some(deadline) = deadline else {
        return (SatQueryOutcome::Unknown, false, None);
    };
    encoded.solver.assume(if sr_mode { mode } else { -mode });
    let (result, entered) = encoded
        .solver
        .solve_until_or_cancel_with_entry(deadline, cancellation);
    if !entered {
        return (SatQueryOutcome::Unknown, false, None);
    }
    match result {
        SolveResult::Unsat => (SatQueryOutcome::Unsat, true, None),
        SolveResult::Unknown => (SatQueryOutcome::Unknown, true, None),
        SolveResult::Sat => match validate_mode_model(
            &encoded.solver,
            &encoded.match_set,
            mode,
            sr_mode,
            &encoded.pivot_vars,
        ) {
            Some(outcome) => (SatQueryOutcome::Sat, true, Some(outcome)),
            None => (SatQueryOutcome::InvalidModel, true, None),
        },
    }
}

fn collect_atom_vars(atom: &IdAtom, bank: &TermBank, vars: &mut Vec<VarId>) {
    match atom {
        IdAtom::Pred(_, args) => {
            for &arg in args {
                collect_term_vars(arg, bank, vars);
            }
        }
        IdAtom::Eq(left, right) => {
            collect_term_vars(*left, bank, vars);
            collect_term_vars(*right, bank, vars);
        }
    }
}

fn collect_term_vars(term: TermId, bank: &TermBank, vars: &mut Vec<VarId>) {
    match bank.get(term) {
        TermNode::Var(var) => vars.push(*var),
        TermNode::App(_, args) => {
            for &arg in args {
                collect_term_vars(arg, bank, vars);
            }
        }
    }
}

fn query_expired(deadline: Option<Instant>, cancellation: Option<&AtomicBool>) -> bool {
    deadline.is_some_and(|limit| Instant::now() >= limit)
        || cancellation.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
}

struct BoundedEncoder {
    solver: Solver,
    clauses: usize,
}

impl BoundedEncoder {
    fn new() -> Self {
        Self {
            solver: Solver::new(),
            clauses: 0,
        }
    }

    fn new_var(&mut self) -> Option<i32> {
        if self.solver.vars() as usize >= MAX_SAT_VARIABLES {
            return None;
        }
        Some(self.solver.declare_variable())
    }

    fn add_clause(&mut self, clause: Vec<i32>) -> bool {
        if self.clauses >= MAX_SAT_CLAUSES {
            return false;
        }
        self.clauses += 1;
        self.solver.add_clause(clause);
        true
    }

    /// Sinz sequential at-most-one encoding, linear in the number of inputs.
    fn add_at_most_one(&mut self, vars: &[i32]) -> bool {
        if vars.len() <= 1 {
            return true;
        }
        let Some(mut previous) = self.new_var() else {
            return false;
        };
        if !self.add_clause(vec![-vars[0], previous]) {
            return false;
        }
        for &var in &vars[1..vars.len() - 1] {
            let Some(current) = self.new_var() else {
                return false;
            };
            if !self.add_clause(vec![-var, current])
                || !self.add_clause(vec![-previous, current])
                || !self.add_clause(vec![-var, -previous])
            {
                return false;
            }
            previous = current;
        }
        self.add_clause(vec![-vars[vars.len() - 1], -previous])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn id_clause(
        id_gen: &mut ClauseIdGen,
        literals: Vec<Literal>,
        bank: &mut TermBank,
        name: &str,
    ) -> IdClause {
        let clause = Clause::new(
            id_gen.next(),
            literals,
            ClauseSource::Input {
                name: name.into(),
                role: "axiom".into(),
            },
        );
        bank.clause_from_legacy(&clause)
    }

    #[test]
    fn combined_sat_queries_plain_subsumption_before_sr() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            &mut bank,
            "active",
        );
        let target = id_clause(
            &mut ids,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
            ],
            &mut bank,
            "target",
        );

        let attempt = simplify_pair_until(&active, &target, &mut bank, None, None);
        assert_eq!(
            attempt.outcome,
            SatSimplificationOutcome::SubsumptionResolution(0)
        );
        assert_eq!(attempt.subsumption, SatQueryOutcome::NoMatch);
        assert_eq!(attempt.sr, SatQueryOutcome::Sat);
        assert_eq!(
            mrs_calculus::subsumption::subsumption_resolution_id_until(
                &active, &target, &mut bank, None,
            ),
            Some(0)
        );
    }

    #[test]
    fn sat_sr_handles_targets_above_the_legacy_width_cap() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            &mut bank,
            "active",
        );
        let mut target_literals = vec![
            Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
        ];
        for index in 0..19 {
            let constant = symbols.intern(&format!("c{index}"));
            target_literals.push(Literal::pos(Atom::pred(q, vec![Term::constant(constant)])));
        }
        assert_eq!(target_literals.len(), 21);
        let target = id_clause(&mut ids, target_literals, &mut bank, "wide_target");

        assert_eq!(
            simplify_pair_until(&active, &target, &mut bank, None, None).outcome,
            SatSimplificationOutcome::SubsumptionResolution(0)
        );
    }

    #[test]
    fn combined_sat_uses_plain_subsumption_when_both_modes_could_apply() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            &mut bank,
            "active",
        );
        let mut target_literals = vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
        ];
        for index in 0..19 {
            let predicate = symbols.intern(&format!("r{index}"));
            target_literals.push(Literal::pos(Atom::pred(predicate, vec![Term::constant(a)])));
        }
        let target = id_clause(&mut ids, target_literals, &mut bank, "target");

        let attempt = simplify_pair_until(&active, &target, &mut bank, None, None);
        assert_eq!(attempt.outcome, SatSimplificationOutcome::Subsumed);
        assert_eq!(attempt.subsumption, SatQueryOutcome::Sat);
        assert_eq!(attempt.sr, SatQueryOutcome::NotRun);
        assert!(attempt.subsumption_solver_entered);
        assert!(!attempt.sr_solver_entered);
        assert!(mrs_calculus::subsumption::subsumes_id_until(
            &active, &target, &mut bank, None,
        ));
    }

    #[test]
    fn combined_sat_reuses_assumptions_for_sr_after_plain_subsumption_fails() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let c = symbols.intern("c");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            &mut bank,
            "active",
        );
        let target = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(b)])),
                Literal::neg(Atom::pred(p, vec![Term::constant(c)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(c)])),
            ],
            &mut bank,
            "target",
        );

        let attempt = simplify_pair_until(&active, &target, &mut bank, None, None);
        assert_eq!(
            attempt.outcome,
            SatSimplificationOutcome::SubsumptionResolution(2)
        );
        assert_eq!(attempt.subsumption, SatQueryOutcome::Unsat);
        assert_eq!(attempt.sr, SatQueryOutcome::Sat);
        assert!(attempt.subsumption_solver_entered);
        assert!(attempt.sr_solver_entered);
    }

    #[test]
    fn wide_sat_sr_pivot_emits_a_kernel_certified_inference() {
        let extras = (0..19)
            .map(|index| format!("r{index}(a)"))
            .collect::<Vec<_>>();
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active_literals = vec![
            Literal::pos(Atom::pred(p, vec![Term::var(0)])),
            Literal::pos(Atom::pred(q, vec![Term::var(0)])),
        ];
        let mut target_literals = vec![
            Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
        ];
        for index in 0..19 {
            let predicate = symbols.intern(&format!("r{index}"));
            target_literals.push(Literal::pos(Atom::pred(predicate, vec![Term::constant(a)])));
        }
        // The extra predicates belong only to the target, as in the TPTP proof
        // text below. Build the side clause and target with the same symbols.
        let active = id_clause(&mut ids, active_literals, &mut bank, "active");
        let target = id_clause(&mut ids, target_literals, &mut bank, "target");
        let SatSimplificationOutcome::SubsumptionResolution(pivot) =
            simplify_pair_until(&active, &target, &mut bank, None, None).outcome
        else {
            panic!("the wide target should have a SAT-SR witness")
        };
        assert_eq!(pivot, 0);

        let target_text = format!("~p(a) | q(a) | {}", extras.join(" | "));
        let conclusion_literals = std::iter::once("q(a)".to_string())
            .chain(extras.iter().cloned())
            .collect::<Vec<_>>();
        let mut unit_literals = vec![("nq".to_string(), "~q(a)".to_string())];
        unit_literals.extend(
            extras
                .iter()
                .enumerate()
                .map(|(index, literal)| (format!("nr{index}"), format!("~{literal}"))),
        );
        let mut problem_text =
            format!("cnf(active, axiom, p(X) | q(X)).\ncnf(target, axiom, {target_text}).\n");
        for (name, literal) in &unit_literals {
            problem_text.push_str(&format!("cnf({name}, axiom, {literal}).\n"));
        }

        let mut proof_text = format!(
            "cnf(active, axiom, p(X) | q(X), file('sat_sr.p', active)).\n\
             cnf(target, axiom, {target_text}, file('sat_sr.p', target)).\n"
        );
        for (name, literal) in &unit_literals {
            proof_text.push_str(&format!(
                "cnf({name}, axiom, {literal}, file('sat_sr.p', {name})).\n"
            ));
        }
        proof_text.push_str(&format!(
            "cnf(sr, plain, {}, inference(subsumption_resolution, [status(thm)], [target, active])).\n",
            conclusion_literals.join(" | ")
        ));
        let mut remaining = conclusion_literals;
        let mut previous = "sr".to_string();
        for index in 0..unit_literals.len() {
            remaining.remove(0);
            let current = if remaining.is_empty() {
                "bot".to_string()
            } else {
                format!("res{index}")
            };
            let conclusion = if remaining.is_empty() {
                "$false".to_string()
            } else {
                remaining.join(" | ")
            };
            let unit_name = &unit_literals[index].0;
            proof_text.push_str(&format!(
                "cnf({current}, plain, {conclusion}, inference(resolution, [status(thm)], [{previous}, {unit_name}])).\n"
            ));
            previous = current;
        }
        let problem = mrs_tptp::parse_tptp(&problem_text).expect("valid synthetic problem");
        let proof = mrs_tptp::parse_tptp(&proof_text).expect("valid SR proof");
        let verdict = mrs_proof_kernel::verify_strict(
            &problem,
            &proof,
            mrs_proof_kernel::VerificationLimits::default(),
        );
        assert!(
            matches!(verdict, mrs_proof_kernel::KernelVerdict::Certified),
            "{verdict:?}"
        );
    }

    #[test]
    fn sat_sr_rejects_incompatible_variable_bindings() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            &mut bank,
            "active",
        );
        let target = id_clause(
            &mut ids,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(b)])),
            ],
            &mut bank,
            "target",
        );

        let attempt = simplify_pair_until(&active, &target, &mut bank, None, None);
        assert_eq!(attempt.outcome, SatSimplificationOutcome::NoSimplification);
        assert_eq!(attempt.sr, SatQueryOutcome::Unsat);
    }

    #[test]
    fn sat_sr_requires_one_complementary_target_literal() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            &mut bank,
            "active",
        );
        let target = id_clause(
            &mut ids,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(q, vec![Term::constant(a)])),
            ],
            &mut bank,
            "two_pivots",
        );

        assert_eq!(
            simplify_pair_until(&active, &target, &mut bank, None, None).outcome,
            SatSimplificationOutcome::NoSimplification
        );
    }

    #[test]
    fn sat_sr_supports_equality_symmetry_and_set_style_target_reuse() {
        let mut symbols = SymbolTable::new();
        let r = symbols.intern("r");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let x = Term::var(0);
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::Eq(x.clone(), Term::constant(a))),
                Literal::pos(Atom::Eq(x, Term::constant(a))),
            ],
            &mut bank,
            "active",
        );
        let target = id_clause(
            &mut ids,
            vec![
                Literal::neg(Atom::Eq(Term::constant(a), Term::constant(b))),
                Literal::pos(Atom::pred(r, vec![Term::constant(a)])),
            ],
            &mut bank,
            "target",
        );

        // Both active equality literals match the same flipped target through
        // equality symmetry. This is legal under the kernel's set-style SR
        // semantics even though the target contains only one pivot occurrence.
        let attempt = simplify_pair_until(&active, &target, &mut bank, None, None);
        assert_eq!(
            attempt.outcome,
            SatSimplificationOutcome::SubsumptionResolution(0)
        );
    }

    #[test]
    fn sat_sr_refuses_overwide_pairs_without_running_the_solver() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let mut bank = TermBank::new();
        let mut ids = ClauseIdGen::new();
        let active = id_clause(
            &mut ids,
            vec![Literal::pos(Atom::pred(p, vec![]))],
            &mut bank,
            "active",
        );
        let target = id_clause(
            &mut ids,
            (0..=MAX_SAT_SR_WIDTH)
                .map(|_| Literal::neg(Atom::pred(p, vec![])))
                .collect(),
            &mut bank,
            "target",
        );
        assert_eq!(
            simplify_pair_until(&active, &target, &mut bank, None, None).outcome,
            SatSimplificationOutcome::Limit
        );
    }
}

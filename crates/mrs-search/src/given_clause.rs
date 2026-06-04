//! The given-clause loop (Otter-style).
//!
//! This is the main proof search algorithm. It alternates between:
//! 1. Selecting a clause from the unprocessed set
//! 2. Generating all inferences with the processed set
//! 3. Adding new clauses to the unprocessed set
//!
//! The loop terminates when:
//! - An empty clause is derived (refutation found)
//! - All clauses are processed (saturation)
//! - A time or clause limit is exceeded

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Instant;

use mrs_calculus::demodulation;
use mrs_calculus::equality;
use mrs_calculus::factoring;
use mrs_calculus::literal_selection::selected_literals_id;
use mrs_calculus::resolution;
use mrs_calculus::subsumption;
use mrs_calculus::superposition;
use mrs_core::SymbolId;
use mrs_core::clause::ClauseSource;
use mrs_core::term_bank::{IdAtom, IdClause, TermId, TermNode};
use mrs_index::fvi::FeatureVector;
use varisat::ExtendFormula;

use crate::select::select;
use crate::state::SearchState;
use crate::weight;
use crate::{SearchConfig, SearchResult};

/// After the SAT model changes, move clauses between active and dormant sets to
/// reflect the new assignment. Also updates the demodulation index for any
/// unit-equality clauses that cross the active/dormant boundary.
fn sync_active_dormant(state: &mut SearchState, ordering: &crate::TermOrdering) {
    // 1. Processed -> Dormant
    let to_remove: Vec<_> = state
        .processed
        .iter()
        .filter(|p| !state.is_active(p))
        .map(|p| p.id)
        .collect();
    for id in to_remove {
        if let Some(p) = state.processed.remove(id, &state.term_bank) {
            if is_unit_positive_equality_id(&p)
                && let IdAtom::Eq(l, r) = &p.literals[0].atom
            {
                use mrs_calculus::ordering::TermComparison;
                if ordering.compare_id(*l, *r, &state.term_bank) == TermComparison::Greater {
                    state
                        .demod_index
                        .remove(*l, &state.term_bank, &(*l, *r, p.id));
                } else if ordering.compare_id(*r, *l, &state.term_bank) == TermComparison::Greater {
                    state
                        .demod_index
                        .remove(*r, &state.term_bank, &(*r, *l, p.id));
                }
            }
            state.dormant_processed.insert(p.id, p);
        }
    }

    // 2. Unprocessed -> Dormant
    let inactive_unproc: HashSet<_> = state
        .unprocessed
        .iter()
        .filter(|id| !state.is_active(state.clause_store.get(id).unwrap()))
        .collect();
    state
        .unprocessed
        .retain(|id, _| !inactive_unproc.contains(&id));
    for id in inactive_unproc {
        let u = state.clause_store.get(&id).unwrap().clone();
        state.dormant_unprocessed.insert(id, u);
    }

    // 3. Dormant Processed -> Unprocessed
    let to_restore_proc: Vec<_> = state
        .dormant_processed
        .keys()
        .copied()
        .filter(|id| state.is_active(state.dormant_processed.get(id).unwrap()))
        .collect();
    for id in to_restore_proc {
        let p = state.dormant_processed.remove(&id).unwrap();
        state.unprocessed.push(&p, &state.term_bank);
    }

    // 4. Dormant Unprocessed -> Unprocessed
    let to_restore_unproc: Vec<_> = state
        .dormant_unprocessed
        .keys()
        .copied()
        .filter(|id| state.is_active(state.dormant_unprocessed.get(id).unwrap()))
        .collect();
    for id in to_restore_unproc {
        let u = state.dormant_unprocessed.remove(&id).unwrap();
        state.unprocessed.push(&u, &state.term_bank);
    }
}

/// Update the SAT model from the solver's current assignment.
fn update_model(state: &mut SearchState) {
    let model = state.avatar.solver.model().unwrap();
    state.avatar.current_model.clear();
    for lit in model {
        if lit.is_positive() {
            state
                .avatar
                .current_model
                .insert(lit.var().to_dimacs() as u32);
        }
    }
}

/// Add the negation of an AVATAR clause's assumptions to the SAT solver and
/// check satisfiability. Returns `true` if a new model was found (clause
/// is now dormant), `false` if UNSAT (full refutation).
fn avatar_refute_branch(
    state: &mut SearchState,
    avatar: &[u32],
    ordering: &crate::TermOrdering,
) -> bool {
    if state.search_deadline.is_some_and(|d| Instant::now() >= d)
        || state
            .stop_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Relaxed))
    {
        return true;
    }
    let sat_clause: Vec<varisat::Lit> = avatar
        .iter()
        .map(|&a| varisat::Lit::from_var(varisat::Var::from_dimacs(a as isize), false))
        .collect();
    state.avatar.solver.add_clause(&sat_clause);

    if matches!(state.avatar.solver.solve(), Ok(true)) {
        update_model(state);
        sync_active_dormant(state, ordering);
        true
    } else {
        false
    }
}

/// Scans the clause store for commutativity axioms of the form `f(X,Y) = f(Y,X)`
/// and returns the set of all such binary function symbols.
fn detect_comm_symbols(state: &crate::state::SearchState) -> HashSet<SymbolId> {
    let mut comm = HashSet::new();
    for clause in state.clause_store.values() {
        if clause.len() == 1
            && clause.literals[0].positive
            && let IdAtom::Eq(l, r) = &clause.literals[0].atom
            && let (TermNode::App(f1, args1), TermNode::App(f2, args2)) =
                (state.term_bank.get(*l), state.term_bank.get(*r))
            && f1 == f2
            && args1.len() == 2
            && args2.len() == 2
            && let (TermNode::Var(x1), TermNode::Var(y1), TermNode::Var(x2), TermNode::Var(y2)) = (
                state.term_bank.get(args1[0]),
                state.term_bank.get(args1[1]),
                state.term_bank.get(args2[0]),
                state.term_bank.get(args2[1]),
            )
            && x1 != y1
            && x1 == y2
            && y1 == x2
        {
            comm.insert(*f1);
        }
    }
    comm
}

/// Runs the given-clause proof search.
///
/// Returns `SearchResult::Refutation(id)` if the empty clause is derived,
/// `SearchResult::Saturated` if all clauses are processed without contradiction,
/// or `SearchResult::Timeout` on timeout.
pub fn search(state: &mut SearchState, config: &SearchConfig) -> SearchResult {
    let ordering = &config.ordering;
    let sym_config = ordering.symbol_config();

    state.comm_symbols = detect_comm_symbols(state);

    let start = Instant::now();
    state.search_deadline = Some(start + config.time_limit);

    // Initial SAT sync
    if config.use_avatar {
        state.avatar.current_model.clear();
        if matches!(state.avatar.solver.solve(), Ok(true)) {
            update_model(state);
        } else {
            return SearchResult::Refutation(mrs_core::clause::ClauseId(0), String::new());
        }
    }

    // Check for initial empty clauses
    let initial_ids: Vec<_> = state.unprocessed.iter().collect();
    for id in initial_ids {
        let clause = state.clause_store.get(&id).unwrap().clone();
        if clause.is_empty() {
            if clause.avatar.is_empty() || !config.use_avatar {
                return SearchResult::Refutation(clause.id, String::new());
            } else {
                let avatar = clause.avatar.clone();
                let cid = clause.id;
                if !avatar_refute_branch(state, &avatar, ordering) {
                    return SearchResult::Refutation(cid, String::new());
                }
            }
        }
    }

    let mut iteration: u64 = 0;

    while let Some(given_id) = select(&mut state.unprocessed, &config.selection, iteration) {
        let given = state.clause_store.get(&given_id).unwrap().clone();

        if !state.is_active(&given) {
            state.dormant_unprocessed.insert(given.id, given);
            continue;
        }

        // Check time limit (and parallel stop-flag)
        if start.elapsed() >= config.time_limit
            || state
                .stop_flag
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
        {
            return SearchResult::Timeout;
        }

        // Skip tautologies
        if given.is_tautology() {
            iteration += 1;
            continue;
        }

        let mut given = given;

        // Forward Subsumption Resolution
        loop {
            let given_fv = FeatureVector::from_id_clause(&given, &state.term_bank);
            let candidates = state
                .processed
                .get_subsumption_resolution_candidates(&given_fv);
            let mut changed = false;
            for p in candidates {
                if p.avatar_is_subset_of(&given)
                    && let Some(removed_idx) =
                        subsumption::subsumption_resolution_id(&p, &given, &mut state.term_bank)
                {
                    let mut new_lits = given.literals.clone();
                    new_lits.remove(removed_idx);
                    given = IdClause::new_avatar(
                        state.id_gen.next(),
                        new_lits,
                        ClauseSource::Inference {
                            rule: "subsumption_resolution".into(),
                            parents: vec![p.id, given.id],
                        },
                        given.avatar.clone(),
                    );
                    state.register_clause(&given.clone());
                    changed = true;
                    break;
                }
            }
            if !changed || given.is_empty() {
                break;
            }
        }

        if given.is_empty() {
            if given.avatar.is_empty() {
                if std::env::var("TRACE_AVATAR").is_ok() {
                    eprintln!(
                        "[AVATAR] empty given {} (no avatar) → Refutation",
                        given.id.0
                    );
                }
                return SearchResult::Refutation(given.id, String::new());
            } else {
                let avatar = given.avatar.clone();
                let id = given.id;
                if std::env::var("TRACE_AVATAR").is_ok() {
                    eprintln!(
                        "[AVATAR] empty given {} (avatar={:?}): calling avatar_refute_branch",
                        id.0, avatar
                    );
                }
                if !avatar_refute_branch(state, &avatar, ordering) {
                    if std::env::var("TRACE_AVATAR").is_ok() {
                        eprintln!(
                            "[AVATAR] empty given {}: avatar_refute_branch returned false → Refutation",
                            id.0
                        );
                    }
                    return SearchResult::Refutation(id, String::new());
                }
                continue;
            }
        }

        // Forward subsumption: skip if given is subsumed by a processed clause
        let given_fv = FeatureVector::from_id_clause(&given, &state.term_bank);
        {
            let candidates = state.processed.get_subsumption_candidates(&given_fv);
            if candidates.iter().any(|p| {
                p.avatar_is_subset_of(&given)
                    && subsumption::subsumes_id(p, &given, &mut state.term_bank)
            }) {
                iteration += 1;
                continue;
            }
        }

        // Forward demodulation: simplify given using unit equalities
        let given = {
            if let Some(simplified) = demodulation::demodulate_id(
                &given,
                &mut state.term_bank,
                &state.demod_index,
                &state.clause_store,
                &mut state.id_gen,
            ) {
                state.register_clause(&given);
                simplified
            } else {
                given
            }
        };

        // Condensation
        let given = if let Some(condensed) =
            subsumption::condense_id(&given, &mut state.term_bank, &mut state.id_gen)
        {
            state.register_clause(&given);
            condensed
        } else {
            given
        };

        // Compute selected literals for the given clause
        let given_sel = selected_literals_id(&given, &config.literal_selection, &state.term_bank);

        // Generate inferences
        let mut new_clauses = Vec::new();

        // --- Resolution ---
        {
            let mut resolution_partner_ids = HashSet::new();
            'resolution: for &lit_idx in &given_sel {
                let lit = &given.literals[lit_idx];
                let partners = state.processed.get_unifiable_resolution_partners(
                    &lit.atom,
                    lit.positive,
                    &state.term_bank,
                );
                for partner in partners {
                    if resolution_partner_ids.insert(partner.id) {
                        let active_sel = selected_literals_id(
                            &partner,
                            &config.literal_selection,
                            &state.term_bank,
                        );
                        let resolvents = resolution::resolve_selected_id(
                            &given,
                            &partner,
                            &mut state.term_bank,
                            &mut state.id_gen,
                            Some(&given_sel),
                            Some(&active_sel),
                            &state.comm_symbols,
                        );
                        new_clauses.extend(resolvents);
                        if start.elapsed() >= config.time_limit {
                            return SearchResult::Timeout;
                        }
                    }
                }
                if start.elapsed() >= config.time_limit {
                    break 'resolution;
                }
            }
        }

        // --- Superposition ---
        {
            // (1) Given as equation source, processed as targets
            let given_has_pos_eq = given
                .literals
                .iter()
                .any(|l| l.positive && matches!(&l.atom, IdAtom::Eq(_, _)));

            if given_has_pos_eq {
                let processed_clauses: Vec<IdClause> = state.processed.iter().cloned().collect();
                for active in &processed_clauses {
                    let active_sel =
                        selected_literals_id(active, &config.literal_selection, &state.term_bank);
                    let sp = superposition::superpose_selected_id(
                        &given,
                        active,
                        &mut state.term_bank,
                        ordering,
                        &mut state.id_gen,
                        Some(&active_sel),
                        &state.comm_symbols,
                    );
                    new_clauses.extend(sp);
                    if start.elapsed() >= config.time_limit {
                        return SearchResult::Timeout;
                    }
                }
                // self-superposition
                let given_sel_local =
                    selected_literals_id(&given, &config.literal_selection, &state.term_bank);
                let sp = superposition::superpose_selected_id(
                    &given,
                    &given,
                    &mut state.term_bank,
                    ordering,
                    &mut state.id_gen,
                    Some(&given_sel_local),
                    &state.comm_symbols,
                );
                new_clauses.extend(sp);
            }

            // (2) Processed clause as equation source, given as target
            {
                let eq_clauses = state.processed.get_positive_equality_clauses();
                for active in eq_clauses {
                    let sp = superposition::superpose_selected_id(
                        &active,
                        &given,
                        &mut state.term_bank,
                        ordering,
                        &mut state.id_gen,
                        Some(&given_sel),
                        &state.comm_symbols,
                    );
                    new_clauses.extend(sp);
                    if start.elapsed() >= config.time_limit {
                        return SearchResult::Timeout;
                    }
                }
            }
        }

        // Factor the given clause
        new_clauses.extend(factoring::factor_id(
            &given,
            &mut state.term_bank,
            &mut state.id_gen,
        ));

        // Equality resolution and factoring
        new_clauses.extend(equality::equality_resolve_id(
            &given,
            &mut state.term_bank,
            &mut state.id_gen,
        ));
        new_clauses.extend(equality::equality_factor_id(
            &given,
            &mut state.term_bank,
            ordering,
            &mut state.id_gen,
        ));

        // Backward subsumption: remove processed clauses subsumed by the given
        let mut to_remove_from_processed = Vec::new();
        {
            let candidates = state.processed.get_subsumed_candidates(&given_fv);
            for p in candidates {
                if given.avatar_is_subset_of(&p)
                    && subsumption::subsumes_id(&given, &p, &mut state.term_bank)
                {
                    to_remove_from_processed.push(p.id);
                }
            }
        }

        for id in to_remove_from_processed {
            state.remove_clause_and_orphans(id, ordering);
        }

        // Add given to processed set (indexed)
        state.register_clause(&given.clone());
        state.processed.insert(given.clone(), &state.term_bank);
        if is_unit_positive_equality_id(&given)
            && let IdAtom::Eq(l, r) = &given.literals[0].atom
        {
            use mrs_calculus::ordering::TermComparison;
            if ordering.compare_id(*l, *r, &state.term_bank) == TermComparison::Greater {
                state
                    .demod_index
                    .insert(*l, &state.term_bank, (*l, *r, given.id));
            } else if ordering.compare_id(*r, *l, &state.term_bank) == TermComparison::Greater {
                state
                    .demod_index
                    .insert(*r, &state.term_bank, (*r, *l, given.id));
            }
        }

        // Backward demodulation: if given is a unit positive equality,
        // rewrite all processed clauses using it. Iterate until fixpoint.
        if is_unit_positive_equality_id(&given) {
            let mut new_units: Vec<IdClause> = vec![given.clone()];
            let mut backward_demod_empty: Vec<IdClause> = Vec::new();

            loop {
                if new_units.is_empty() {
                    break;
                }

                if start.elapsed() >= config.time_limit {
                    break;
                }

                let mut temp_demod_index: mrs_index::dtree::DTreeId<(
                    TermId,
                    TermId,
                    mrs_core::clause::ClauseId,
                )> = mrs_index::dtree::DTreeId::new();
                for u in &new_units {
                    if let IdAtom::Eq(l, r) = &u.literals[0].atom {
                        use mrs_calculus::ordering::TermComparison;
                        if ordering.compare_id(*l, *r, &state.term_bank) == TermComparison::Greater
                        {
                            temp_demod_index.insert(*l, &state.term_bank, (*l, *r, u.id));
                        } else if ordering.compare_id(*r, *l, &state.term_bank)
                            == TermComparison::Greater
                        {
                            temp_demod_index.insert(*r, &state.term_bank, (*r, *l, u.id));
                        }
                    }
                }

                let all_processed = state.processed.drain();
                state.demod_index = mrs_index::dtree::DTreeId::new();
                let mut next_processed = Vec::new();
                let mut created_units = Vec::new();

                for proc in all_processed {
                    if start.elapsed() >= config.time_limit {
                        next_processed.push(proc);
                        continue;
                    }
                    if new_units.iter().any(|u| u.id == proc.id) {
                        next_processed.push(proc);
                        continue;
                    }
                    if let Some(simplified) = demodulation::demodulate_id(
                        &proc,
                        &mut state.term_bank,
                        &temp_demod_index,
                        &state.clause_store,
                        &mut state.id_gen,
                    ) {
                        state.register_clause(&proc);

                        // Build index from already-processed units for chained rewriting
                        let mut all_units_index: mrs_index::dtree::DTreeId<(
                            TermId,
                            TermId,
                            mrs_core::clause::ClauseId,
                        )> = mrs_index::dtree::DTreeId::new();
                        for c in &next_processed {
                            if is_unit_positive_equality_id(c)
                                && let IdAtom::Eq(l, r) = &c.literals[0].atom
                            {
                                use mrs_calculus::ordering::TermComparison;
                                if ordering.compare_id(*l, *r, &state.term_bank)
                                    == TermComparison::Greater
                                {
                                    all_units_index.insert(*l, &state.term_bank, (*l, *r, c.id));
                                } else if ordering.compare_id(*r, *l, &state.term_bank)
                                    == TermComparison::Greater
                                {
                                    all_units_index.insert(*r, &state.term_bank, (*r, *l, c.id));
                                }
                            }
                        }

                        let simplified = if !all_units_index.is_empty()
                            && let Some(further) = demodulation::demodulate_id(
                                &simplified,
                                &mut state.term_bank,
                                &all_units_index,
                                &state.clause_store,
                                &mut state.id_gen,
                            ) {
                            state.register_clause(&simplified);
                            further
                        } else {
                            simplified
                        };

                        if simplified.is_empty() {
                            state.register_clause(&simplified.clone());
                            backward_demod_empty.push(simplified);
                            continue;
                        }
                        if is_trivial_contradiction_id(&simplified) {
                            let empty = IdClause::new_avatar(
                                state.id_gen.next(),
                                vec![],
                                ClauseSource::Inference {
                                    rule: "equality_resolution".into(),
                                    parents: vec![simplified.id],
                                },
                                simplified.avatar.clone(),
                            );
                            state.register_clause(&simplified);
                            state.register_clause(&empty.clone());
                            backward_demod_empty.push(empty);
                            continue;
                        }
                        if !simplified.is_tautology() {
                            if is_unit_positive_equality_id(&simplified) {
                                created_units.push(simplified.clone());
                            }
                            state.register_clause(&simplified.clone());
                            next_processed.push(simplified);
                        }
                    } else {
                        next_processed.push(proc);
                    }
                }

                let time_ok = start.elapsed() < config.time_limit;
                for clause in next_processed {
                    state.processed.insert(clause.clone(), &state.term_bank);
                    if time_ok
                        && is_unit_positive_equality_id(&clause)
                        && let IdAtom::Eq(l, r) = &clause.literals[0].atom
                    {
                        use mrs_calculus::ordering::TermComparison;
                        if ordering.compare_id(*l, *r, &state.term_bank) == TermComparison::Greater
                        {
                            state
                                .demod_index
                                .insert(*l, &state.term_bank, (*l, *r, clause.id));
                        } else if ordering.compare_id(*r, *l, &state.term_bank)
                            == TermComparison::Greater
                        {
                            state
                                .demod_index
                                .insert(*r, &state.term_bank, (*r, *l, clause.id));
                        }
                    }
                }
                new_units = created_units;
            }

            for empty in backward_demod_empty {
                if empty.avatar.is_empty() || !config.use_avatar {
                    return SearchResult::Refutation(empty.id, String::new());
                }
                let avatar = empty.avatar.clone();
                let id = empty.id;
                if !avatar_refute_branch(state, &avatar, ordering) {
                    return SearchResult::Refutation(id, String::new());
                }
            }
        }

        // Split new inferences into AVATAR components
        let trace_avatar = std::env::var("TRACE_AVATAR").is_ok();
        let mut final_new_clauses = Vec::new();
        let mut model_violated = false;
        for clause in new_clauses {
            if config.use_avatar {
                if let Some(splits) =
                    state
                        .avatar
                        .split_clause_id(&clause, &mut state.id_gen, &state.term_bank)
                {
                    if trace_avatar {
                        eprintln!(
                            "[AVATAR] split clause {} ({} lits, avatar={:?}) -> {} components",
                            clause.id.0,
                            clause.literals.len(),
                            clause.avatar,
                            splits.len()
                        );
                        for s in &splits {
                            eprintln!(
                                "  component {} ({} lits, avatar={:?})",
                                s.id.0,
                                s.literals.len(),
                                s.avatar
                            );
                        }
                    }
                    let mut violated = clause
                        .avatar
                        .iter()
                        .all(|&a| state.avatar.current_model.contains(&a));
                    if violated {
                        violated = splits.iter().all(|s| {
                            !state
                                .avatar
                                .current_model
                                .contains(s.avatar.last().unwrap())
                        });
                    }
                    if violated {
                        model_violated = true;
                    }

                    for split in splits {
                        state.register_clause(&split.clone());
                        final_new_clauses.push(split);
                    }
                } else {
                    final_new_clauses.push(clause);
                }
            } else {
                final_new_clauses.push(clause);
            }
        }

        if config.use_avatar && model_violated {
            let past_deadline = state.search_deadline.is_some_and(|d| Instant::now() >= d);
            if !past_deadline {
                if trace_avatar {
                    eprintln!(
                        "[AVATAR] model_violated: re-querying SAT solver (given={})",
                        given.id.0
                    );
                }
                if matches!(state.avatar.solver.solve(), Ok(true)) {
                    update_model(state);
                    sync_active_dormant(state, ordering);
                } else {
                    if trace_avatar {
                        eprintln!(
                            "[AVATAR] model_violated path: SAT UNSAT → Refutation(given={})",
                            given.id.0
                        );
                    }
                    return SearchResult::Refutation(given.id, String::new());
                }
            }
        }

        for mut clause in final_new_clauses {
            clause.deduplicate();

            if clause.is_empty() {
                state.register_clause(&clause.clone());

                if clause.avatar.is_empty() || !config.use_avatar {
                    return SearchResult::Refutation(clause.id, String::new());
                } else {
                    let avatar = clause.avatar.clone();
                    let id = clause.id;
                    if trace_avatar {
                        eprintln!(
                            "[AVATAR] empty clause {} (avatar={:?}): calling avatar_refute_branch",
                            id.0, avatar
                        );
                    }
                    if !avatar_refute_branch(state, &avatar, ordering) {
                        if trace_avatar {
                            eprintln!(
                                "[AVATAR] avatar_refute_branch returned false → SAT UNSAT → Refutation({})",
                                id.0
                            );
                        }
                        return SearchResult::Refutation(id, String::new());
                    }
                    continue;
                }
            }

            if !clause.is_tautology() {
                // Forward demodulation on new clauses
                let clause = if let Some(simplified) = demodulation::demodulate_id(
                    &clause,
                    &mut state.term_bank,
                    &state.demod_index,
                    &state.clause_store,
                    &mut state.id_gen,
                ) {
                    state.register_clause(&clause);
                    simplified
                } else {
                    clause
                };

                if clause.is_empty() {
                    state.register_clause(&clause.clone());
                    if clause.avatar.is_empty() || !config.use_avatar {
                        return SearchResult::Refutation(clause.id, String::new());
                    }
                    let avatar = clause.avatar.clone();
                    let id = clause.id;
                    if !avatar_refute_branch(state, &avatar, ordering) {
                        return SearchResult::Refutation(id, String::new());
                    }
                    continue;
                }

                if is_trivial_contradiction_id(&clause) {
                    let empty = IdClause::new_avatar(
                        state.id_gen.next(),
                        vec![],
                        ClauseSource::Inference {
                            rule: "equality_resolution".into(),
                            parents: vec![clause.id],
                        },
                        clause.avatar.clone(),
                    );
                    state.register_clause(&clause.clone());
                    state.register_clause(&empty.clone());
                    if empty.avatar.is_empty() || !config.use_avatar {
                        return SearchResult::Refutation(empty.id, String::new());
                    }
                    let avatar = empty.avatar.clone();
                    let id = empty.id;
                    if !avatar_refute_branch(state, &avatar, ordering) {
                        return SearchResult::Refutation(id, String::new());
                    }
                    continue;
                }

                if clause.is_tautology() {
                    continue;
                }

                // Condensation
                let clause = if let Some(condensed) =
                    subsumption::condense_id(&clause, &mut state.term_bank, &mut state.id_gen)
                {
                    state.register_clause(&clause);
                    condensed
                } else {
                    clause
                };

                // Max term weight filter
                if let Some(max_w) = config.max_term_weight
                    && weight::clause_weight_exceeds_id(
                        &clause,
                        max_w,
                        &state.term_bank,
                        &sym_config,
                    )
                {
                    continue;
                }

                // Forward subsumption: skip if subsumed by a processed clause
                let clause_fv = FeatureVector::from_id_clause(&clause, &state.term_bank);
                {
                    let candidates = state.processed.get_subsumption_candidates(&clause_fv);
                    if candidates.iter().any(|p| {
                        p.avatar_is_subset_of(&clause)
                            && subsumption::subsumes_id(p, &clause, &mut state.term_bank)
                    }) {
                        continue;
                    }
                }

                state.register_clause(&clause.clone());
                state.unprocessed.push(&clause, &state.term_bank);
            }
        }

        iteration += 1;
    }

    // If the literal selection is incomplete, return GaveUp rather than Saturated.
    match config.literal_selection {
        crate::LiteralSelection::MaxNegativeOrMaxPositive => SearchResult::GaveUp,
        _ => SearchResult::Saturated,
    }
}

/// Returns true if a clause is a unit positive equality (used for demodulation).
fn is_unit_positive_equality_id(clause: &IdClause) -> bool {
    clause.len() == 1
        && clause.literals[0].positive
        && matches!(&clause.literals[0].atom, IdAtom::Eq(_, _))
}

/// Returns true if a clause is a trivial contradiction: a single negative equality
/// `s ≠ s` where both sides are the same `TermId` (structural equality via hash-consing).
fn is_trivial_contradiction_id(clause: &IdClause) -> bool {
    clause.len() == 1
        && !clause.literals[0].positive
        && matches!(&clause.literals[0].atom, IdAtom::Eq(l, r) if l == r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TermOrdering;
    use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn input_clause(
        id_gen: &mut ClauseIdGen,
        lits: Vec<Literal>,
        name: &str,
        role: &str,
    ) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.into(),
                role: role.into(),
            },
        )
    }

    #[test]
    fn prove_p_and_not_p() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax1",
            "axiom",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "ax2",
            "axiom",
        );

        let mut state = crate::state::SearchState::new(
            vec![c1, c2],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Refutation(..)));
    }

    #[test]
    fn prove_socrates() {
        let mut syms = SymbolTable::new();
        let human = syms.intern("human");
        let mortal = syms.intern("mortal");
        let socrates = syms.intern("socrates");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(human, vec![Term::var(0)])),
                Literal::pos(Atom::pred(mortal, vec![Term::var(0)])),
            ],
            "ax1",
            "axiom",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                human,
                vec![Term::constant(socrates)],
            ))],
            "ax2",
            "axiom",
        );
        let c3 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(
                mortal,
                vec![Term::constant(socrates)],
            ))],
            "goal",
            "negated_conjecture",
        );

        let mut state = crate::state::SearchState::new(
            vec![c1, c2, c3],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Refutation(..)));
    }

    #[test]
    fn saturates_on_satisfiable_ground() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax1",
            "axiom",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(q, vec![Term::constant(b)]))],
            "ax2",
            "axiom",
        );

        let mut state = crate::state::SearchState::new(
            vec![c1, c2],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Saturated));
    }

    #[test]
    fn pel27_literal_selection_all() {
        use crate::{LiteralSelection, SelectionStrategy};

        let mut syms = SymbolTable::new();
        let f_sym = syms.intern("f");
        let g_sym = syms.intern("g");
        let h_sym = syms.intern("h");
        let i_sym = syms.intern("i");
        let j_sym = syms.intern("j");
        let sk1 = syms.intern("sk_ax1_0");
        let sk2 = syms.intern("sk_goal_0");
        let mut id_gen = ClauseIdGen::new();

        let clauses = vec![
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(f_sym, vec![Term::constant(sk1)]))],
                "ax1_0",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![Literal::neg(Atom::pred(g_sym, vec![Term::constant(sk1)]))],
                "ax1_1",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(f_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(h_sym, vec![Term::var(0)])),
                ],
                "ax2",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(j_sym, vec![Term::var(0)])),
                    Literal::neg(Atom::pred(i_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(f_sym, vec![Term::var(0)])),
                ],
                "ax3",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(h_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(g_sym, vec![Term::var(0)])),
                    Literal::neg(Atom::pred(i_sym, vec![Term::var(1)])),
                    Literal::neg(Atom::pred(h_sym, vec![Term::var(1)])),
                ],
                "ax4",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(j_sym, vec![Term::constant(sk2)]))],
                "goal_0",
                "negated_conjecture",
            ),
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(i_sym, vec![Term::constant(sk2)]))],
                "goal_1",
                "negated_conjecture",
            ),
        ];

        let mut state = crate::state::SearchState::new(
            clauses.clone(),
            id_gen.clone(),
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig {
            time_limit: std::time::Duration::from_secs(5),
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::All,
            ordering: TermOrdering::KBO,
            ..SearchConfig::default()
        };
        let result = search(&mut state, &config);
        assert!(
            matches!(result, SearchResult::Refutation(..)),
            "Expected refutation with All selection, got {:?}",
            result
        );
    }

    /// Soundness regression test: a trivial contradiction produced by forward
    /// demodulation under AVATAR assumptions must NOT cause a global Refutation
    /// without first consulting the AVATAR SAT solver.
    #[test]
    fn avatar_forward_demod_no_false_refutation() {
        use crate::{LiteralSelection, SelectionStrategy};

        let mut syms = SymbolTable::new();
        let f_sym = syms.intern("f");
        let a_sym = syms.intern("a");
        let q_sym = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();

        let a = Term::constant(a_sym);
        let fa = Term::app(f_sym, vec![a.clone()]);

        let clause_a = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::eq(fa.clone(), a.clone())),
                Literal::pos(Atom::pred(q_sym, vec![a.clone()])),
            ],
            "ax1",
            "axiom",
        );

        let clause_b = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::eq(fa.clone(), a.clone()))],
            "ax2",
            "axiom",
        );

        let mut state = crate::state::SearchState::new(
            vec![clause_a, clause_b],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig {
            time_limit: std::time::Duration::from_secs(5),
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::All,
            ordering: TermOrdering::KBO,
            ..SearchConfig::default()
        };
        let result = search(&mut state, &config);
        assert!(
            !matches!(result, SearchResult::Refutation(..)),
            "Soundness bug: got false Refutation for a satisfiable problem (expected Saturated)"
        );
    }

    #[test]
    fn pel27_literal_selection_all_negative() {
        use crate::{LiteralSelection, SelectionStrategy};

        let mut syms = SymbolTable::new();
        let f_sym = syms.intern("f");
        let g_sym = syms.intern("g");
        let h_sym = syms.intern("h");
        let i_sym = syms.intern("i");
        let j_sym = syms.intern("j");
        let sk1 = syms.intern("sk_ax1_0");
        let sk2 = syms.intern("sk_goal_0");
        let mut id_gen = ClauseIdGen::new();

        let clauses = vec![
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(f_sym, vec![Term::constant(sk1)]))],
                "ax1_0",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![Literal::neg(Atom::pred(g_sym, vec![Term::constant(sk1)]))],
                "ax1_1",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(f_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(h_sym, vec![Term::var(0)])),
                ],
                "ax2",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(j_sym, vec![Term::var(0)])),
                    Literal::neg(Atom::pred(i_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(f_sym, vec![Term::var(0)])),
                ],
                "ax3",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(h_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(g_sym, vec![Term::var(0)])),
                    Literal::neg(Atom::pred(i_sym, vec![Term::var(1)])),
                    Literal::neg(Atom::pred(h_sym, vec![Term::var(1)])),
                ],
                "ax4",
                "axiom",
            ),
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(j_sym, vec![Term::constant(sk2)]))],
                "goal_0",
                "negated_conjecture",
            ),
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(i_sym, vec![Term::constant(sk2)]))],
                "goal_1",
                "negated_conjecture",
            ),
        ];

        let mut state = crate::state::SearchState::new(
            clauses,
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig {
            time_limit: std::time::Duration::from_secs(5),
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::AllNegative,
            ordering: TermOrdering::KBO,
            ..SearchConfig::default()
        };
        let result = search(&mut state, &config);
        let _ = result;
    }

    #[test]
    fn initial_empty_clause() {
        let mut id_gen = ClauseIdGen::new();
        let c = Clause::new(
            id_gen.next(),
            vec![],
            ClauseSource::Input {
                name: "empty".into(),
                role: "axiom".into(),
            },
        );
        let mut state = crate::state::SearchState::new(
            vec![c],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
            true,
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Refutation(..)));
    }
}

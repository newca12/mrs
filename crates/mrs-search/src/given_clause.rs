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
use std::time::Instant;

use mrs_calculus::demodulation;
use mrs_calculus::equality;
use mrs_calculus::factoring;
use mrs_calculus::literal_selection::selected_literals;
use mrs_calculus::resolution;
use mrs_calculus::subsumption;
use mrs_calculus::superposition;
use mrs_core::Atom;
use mrs_core::clause::{Clause, ClauseSource};
use mrs_index::fvi::FeatureVector;
use varisat::ExtendFormula;

use crate::select::select;
use crate::state::SearchState;
use crate::{SearchConfig, SearchResult};

/// Runs the given-clause proof search.
///
/// Returns `SearchResult::Refutation(id)` if the empty clause is derived,
/// `SearchResult::Saturated` if all clauses are processed without contradiction,
/// or `SearchResult::Timeout`/`SearchResult::ResourceOut` on resource limits.
pub fn search(state: &mut SearchState, config: &SearchConfig) -> SearchResult {
    // Initial SAT sync
    state.avatar.current_model.clear();
    if matches!(state.avatar.solver.solve(), Ok(true)) {
        let model = state.avatar.solver.model().unwrap();
        for lit in model {
            if lit.is_positive() {
                state
                    .avatar
                    .current_model
                    .insert(lit.var().to_dimacs() as u32);
            }
        }
    } else {
        return SearchResult::Refutation(mrs_core::clause::ClauseId(0), String::new()); // Should not happen on empty start
    }

    // Check for initial empty clauses
    for id in state.unprocessed.iter() {
        let clause = state.clause_store.get(&id).unwrap();
        if clause.is_empty() {
            if clause.avatar.is_empty() {
                return SearchResult::Refutation(clause.id, String::new());
            } else {
                let sat_clause: Vec<varisat::Lit> = clause
                    .avatar
                    .iter()
                    .map(|&a| varisat::Lit::from_var(varisat::Var::from_dimacs(a as isize), false))
                    .collect();
                state.avatar.solver.add_clause(&sat_clause);
                if !matches!(state.avatar.solver.solve(), Ok(true)) {
                    return SearchResult::Refutation(clause.id, String::new());
                } else {
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
            }
        }
    }

    let start = Instant::now();
    let mut iteration: u64 = 0;
    let ordering = &config.ordering;

    while let Some(given_id) = select(&mut state.unprocessed, &config.selection, iteration) {
        let given = state.clause_store.get(&given_id).unwrap().clone();

        if !state.is_active(&given) {
            state.dormant_unprocessed.insert(given.id, given);
            continue;
        }

        // Check time limit
        if start.elapsed() >= config.time_limit {
            return SearchResult::Timeout;
        }

        // Check clause limit
        if state.total_clauses() >= config.max_clauses {
            return SearchResult::ResourceOut;
        }

        // Skip tautologies
        if given.is_tautology() {
            iteration += 1;
            continue;
        }

        // Forward subsumption: skip if given is subsumed by a processed clause
        let given_fv = FeatureVector::from_clause(&given);
        if state
            .processed
            .get_subsumption_candidates(&given_fv)
            .into_iter()
            .any(|p| subsumption::subsumes(p, &given))
        {
            iteration += 1;
            continue;
        }

        // Forward demodulation: simplify the given clause using unit equalities
        let given = {
            if let Some(simplified) = demodulation::demodulate(
                &given,
                &state.demod_index,
                &state.clause_store,
                &mut state.id_gen,
            ) {
                // Store original clause so proof extraction can find it
                state.clause_store.insert(given.id, given);
                simplified
            } else {
                given
            }
        };

        // Condensation: simplify the given clause by removing redundant literals
        let given = if let Some(condensed) = subsumption::condense(&given, &mut state.id_gen) {
            state.clause_store.insert(given.id, given);
            condensed
        } else {
            given
        };

        // Compute selected literals for the given clause
        let given_sel = selected_literals(&given, &config.literal_selection);

        // Generate inferences
        let mut new_clauses = Vec::new();

        // --- Resolution: use index to find clauses with complementary predicates ---
        {
            let mut resolution_partner_ids = HashSet::new();
            for &lit_idx in &given_sel {
                let lit = &given.literals[lit_idx];
                let partners = state
                    .processed
                    .get_unifiable_resolution_partners(&lit.atom, lit.positive);
                for partner in partners {
                    if resolution_partner_ids.insert(partner.id) {
                        let active_sel = selected_literals(partner, &config.literal_selection);
                        let resolvents = resolution::resolve_selected(
                            &given,
                            partner,
                            &mut state.id_gen,
                            Some(&given_sel),
                            Some(&active_sel),
                        );
                        new_clauses.extend(resolvents);
                    }
                }
            }
        }

        // --- Superposition ---
        {
            // (1) Given as equation source, processed clauses as targets.
            // Only needed if the given clause has at least one positive equality.
            let given_has_pos_eq = given
                .literals
                .iter()
                .any(|l| l.is_positive() && matches!(&l.atom, Atom::Eq(_, _)));

            if given_has_pos_eq {
                let processed_clauses: Vec<Clause> = state.processed.iter().cloned().collect();
                for active in &processed_clauses {
                    let active_sel = selected_literals(active, &config.literal_selection);
                    let sp = superposition::superpose_selected(
                        &given,
                        active,
                        ordering,
                        &mut state.id_gen,
                        Some(&active_sel),
                    );
                    new_clauses.extend(sp);
                }
            }

            // (2) Processed clause as equation source, given as target.
            // Only consider processed clauses that have positive equalities.
            {
                // We don't need to add given here because given -> given is handled in step (1)
                let eq_clauses: Vec<Clause> = state
                    .processed
                    .get_positive_equality_clauses()
                    .into_iter()
                    .cloned()
                    .collect();
                for active in &eq_clauses {
                    let sp = superposition::superpose_selected(
                        active,
                        &given,
                        ordering,
                        &mut state.id_gen,
                        Some(&given_sel),
                    );
                    new_clauses.extend(sp);
                }
            }
        }

        // Factor the given clause
        let factors = factoring::factor(&given, &mut state.id_gen);
        new_clauses.extend(factors);

        // Equality resolution and factoring on the given clause
        new_clauses.extend(equality::equality_resolve(&given, &mut state.id_gen));
        new_clauses.extend(equality::equality_factor(
            &given,
            ordering,
            &mut state.id_gen,
        ));

        // Backward subsumption: remove processed clauses subsumed by the given
        let mut to_remove_from_demod = Vec::new();
        let candidates = state.processed.get_subsumed_candidates(&given_fv);
        let mut to_remove_from_processed = Vec::new();

        for p in candidates {
            if subsumption::subsumes(&given, p) {
                to_remove_from_processed.push(p.id);
                if is_unit_positive_equality(p) {
                    to_remove_from_demod.push(p.clone());
                }
            }
        }

        for id in to_remove_from_processed {
            state.processed.remove(id);
        }

        for p in to_remove_from_demod {
            if let Atom::Eq(l, r) = &p.literals[0].atom {
                use mrs_calculus::ordering::TermComparison;
                if ordering.compare(l, r) == TermComparison::Greater {
                    state.demod_index.remove(l, &(l.clone(), r.clone(), p.id));
                } else if ordering.compare(r, l) == TermComparison::Greater {
                    state.demod_index.remove(r, &(r.clone(), l.clone(), p.id));
                }
            }
        }

        // Backward subsumption of unprocessed: remove unprocessed clauses subsumed by the given
        state.unprocessed.retain(|id| {
            let u = state.clause_store.get(&id).unwrap();
            let u_fv = FeatureVector::from_clause(u);
            if given_fv.can_subsume(&u_fv) {
                !subsumption::subsumes(&given, u)
            } else {
                true
            }
        });

        // Add given to processed set (indexed)
        state.clause_store.insert(given.id, given.clone());
        state.processed.insert(given.clone());
        if is_unit_positive_equality(&given)
            && let Atom::Eq(l, r) = &given.literals[0].atom
        {
            use mrs_calculus::ordering::TermComparison;
            if ordering.compare(l, r) == TermComparison::Greater {
                state
                    .demod_index
                    .insert(l, (l.clone(), r.clone(), given.id));
            } else if ordering.compare(r, l) == TermComparison::Greater {
                state
                    .demod_index
                    .insert(r, (r.clone(), l.clone(), given.id));
            }
        }

        // Backward demodulation: if the given clause is a unit positive equality,
        // rewrite all processed clauses using it. Iterate until fixpoint.
        if is_unit_positive_equality(&given) {
            let mut new_units: Vec<Clause> = vec![given.clone()];

            loop {
                if new_units.is_empty() {
                    break;
                }

                let mut temp_demod_index = mrs_index::dtree::DTree::new();
                for u in &new_units {
                    if let Atom::Eq(l, r) = &u.literals[0].atom {
                        use mrs_calculus::ordering::TermComparison;
                        if ordering.compare(l, r) == TermComparison::Greater {
                            temp_demod_index.insert(l, (l.clone(), r.clone(), u.id));
                        } else if ordering.compare(r, l) == TermComparison::Greater {
                            temp_demod_index.insert(r, (r.clone(), l.clone(), u.id));
                        }
                    }
                }

                let all_processed = state.processed.drain();
                state.demod_index = mrs_index::dtree::DTree::new(); // Clear and rebuild later
                let mut next_processed = Vec::new();
                let mut created_units = Vec::new();

                for proc in all_processed {
                    // Don't rewrite the demod units themselves
                    if new_units.iter().any(|u| u.id == proc.id) {
                        next_processed.push(proc);
                        continue;
                    }
                    if let Some(simplified) = demodulation::demodulate(
                        &proc,
                        &temp_demod_index,
                        &state.clause_store,
                        &mut state.id_gen,
                    ) {
                        // Store original for proof extraction
                        state.clause_store.insert(proc.id, proc);
                        // Further simplify using all existing processed units
                        // Note: state.demod_index is currently empty, so we must rebuild it from next_processed to do this properly.
                        // Or we can just build a DTree of all_units so far.
                        let mut all_units_index = mrs_index::dtree::DTree::new();
                        for c in &next_processed {
                            if is_unit_positive_equality(c)
                                && let Atom::Eq(l, r) = &c.literals[0].atom
                            {
                                use mrs_calculus::ordering::TermComparison;
                                if ordering.compare(l, r) == TermComparison::Greater {
                                    all_units_index.insert(l, (l.clone(), r.clone(), c.id));
                                } else if ordering.compare(r, l) == TermComparison::Greater {
                                    all_units_index.insert(r, (r.clone(), l.clone(), c.id));
                                }
                            }
                        }

                        let simplified = if !all_units_index.is_empty()
                            && let Some(further) = demodulation::demodulate(
                                &simplified,
                                &all_units_index,
                                &state.clause_store,
                                &mut state.id_gen,
                            ) {
                            state.clause_store.insert(simplified.id, simplified);
                            further
                        } else {
                            simplified
                        };
                        if simplified.is_empty() {
                            state.clause_store.insert(simplified.id, simplified.clone());
                            return SearchResult::Refutation(simplified.id, String::new());
                        }
                        if is_trivial_contradiction(&simplified) {
                            let empty = Clause::new_avatar(
                                state.id_gen.next(),
                                vec![],
                                ClauseSource::Inference {
                                    rule: "equality_resolution".into(),
                                    parents: vec![simplified.id],
                                },
                                simplified.avatar.clone(),
                            );
                            state.clause_store.insert(simplified.id, simplified);
                            state.clause_store.insert(empty.id, empty.clone());
                            return SearchResult::Refutation(empty.id, String::new());
                        }
                        if is_trivial_contradiction(&simplified) {
                            let empty = Clause::new_avatar(
                                state.id_gen.next(),
                                vec![],
                                ClauseSource::Inference {
                                    rule: "equality_resolution".into(),
                                    parents: vec![simplified.id],
                                },
                                simplified.avatar.clone(),
                            );
                            state.clause_store.insert(simplified.id, simplified);
                            state.clause_store.insert(empty.id, empty.clone());
                            return SearchResult::Refutation(empty.id, String::new());
                        }
                        if !simplified.is_tautology() {
                            if is_unit_positive_equality(&simplified) {
                                created_units.push(simplified.clone());
                            }
                            state.clause_store.insert(simplified.id, simplified.clone());
                            next_processed.push(simplified);
                        }
                    } else {
                        next_processed.push(proc);
                    }
                }

                // Re-insert all clauses into the index
                for clause in next_processed {
                    state.processed.insert(clause.clone());
                    if is_unit_positive_equality(&clause)
                        && let Atom::Eq(l, r) = &clause.literals[0].atom
                    {
                        use mrs_calculus::ordering::TermComparison;
                        if ordering.compare(l, r) == TermComparison::Greater {
                            state
                                .demod_index
                                .insert(l, (l.clone(), r.clone(), clause.id));
                        } else if ordering.compare(r, l) == TermComparison::Greater {
                            state
                                .demod_index
                                .insert(r, (r.clone(), l.clone(), clause.id));
                        }
                    }
                }
                new_units = created_units;
            }
        }

        // Process generated clauses
        let mut final_new_clauses = Vec::new();
        for clause in new_clauses {
            if let Some(splits) = state.avatar.split_clause(&clause, &mut state.id_gen) {
                // println!("AVATAR SPLIT: {:?} into {} parts", clause.display(&state.symbols?), splits.len());
                for split in splits {
                    state.clause_store.insert(split.id, split.clone());
                    final_new_clauses.push(split);
                }
            } else {
                final_new_clauses.push(clause);
            }
        }

        for mut clause in final_new_clauses {
            // Remove duplicate literals
            clause.deduplicate();

            if clause.is_empty() {
                state.clause_store.insert(clause.id, clause.clone());

                if clause.avatar.is_empty() {
                    return SearchResult::Refutation(clause.id, String::new());
                } else {
                    let sat_clause: Vec<varisat::Lit> = clause
                        .avatar
                        .iter()
                        .map(|&a| {
                            varisat::Lit::from_var(varisat::Var::from_dimacs(a as isize), false)
                        })
                        .collect();
                    state.avatar.solver.add_clause(&sat_clause);

                    if matches!(state.avatar.solver.solve(), Ok(true)) {
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

                        // Update active/dormant status
                        // 1. Processed -> Dormant
                        let mut to_remove_processed = Vec::new();
                        for p in state.processed.iter() {
                            if !state.is_active(p) {
                                to_remove_processed.push(p.id);
                            }
                        }
                        for id in to_remove_processed {
                            if let Some(p) = state.processed.remove(id) {
                                state.dormant_processed.insert(p.id, p.clone());
                                if is_unit_positive_equality(&p)
                                    && let Atom::Eq(l, r) = &p.literals[0].atom
                                {
                                    use mrs_calculus::ordering::TermComparison;
                                    if ordering.compare(l, r) == TermComparison::Greater {
                                        state.demod_index.remove(l, &(l.clone(), r.clone(), p.id));
                                    } else if ordering.compare(r, l) == TermComparison::Greater {
                                        state.demod_index.remove(r, &(r.clone(), l.clone(), p.id));
                                    }
                                }
                            }
                        }

                        // 2. Unprocessed -> Dormant
                        let mut to_remove_unproc = Vec::new();
                        for id in state.unprocessed.iter() {
                            let u = state.clause_store.get(&id).unwrap();
                            if !state.is_active(u) {
                                to_remove_unproc.push(u.id);
                            }
                        }
                        let to_remove_set: std::collections::HashSet<_> =
                            to_remove_unproc.into_iter().collect();

                        state.unprocessed.retain(|id| !to_remove_set.contains(&id));

                        for id in to_remove_set {
                            let u = state.clause_store.get(&id).unwrap().clone();
                            state.dormant_unprocessed.insert(id, u);
                        }

                        // 3. Dormant -> Processed
                        let mut to_restore_processed = Vec::new();
                        for (id, p) in &state.dormant_processed {
                            if state.is_active(p) {
                                to_restore_processed.push(*id);
                            }
                        }
                        for id in to_restore_processed {
                            let p = state.dormant_processed.remove(&id).unwrap();
                            state.processed.insert(p.clone());
                            if is_unit_positive_equality(&p)
                                && let Atom::Eq(l, r) = &p.literals[0].atom
                            {
                                use mrs_calculus::ordering::TermComparison;
                                if ordering.compare(l, r) == TermComparison::Greater {
                                    state.demod_index.insert(l, (l.clone(), r.clone(), p.id));
                                } else if ordering.compare(r, l) == TermComparison::Greater {
                                    state.demod_index.insert(r, (r.clone(), l.clone(), p.id));
                                }
                            }
                        }

                        // 4. Dormant -> Unprocessed
                        let mut to_restore_unprocessed = Vec::new();
                        for (id, u) in &state.dormant_unprocessed {
                            if state.is_active(u) {
                                to_restore_unprocessed.push(*id);
                            }
                        }
                        for id in to_restore_unprocessed {
                            let u = state.dormant_unprocessed.remove(&id).unwrap();
                            state.unprocessed.push(&u);
                        }

                        continue;
                    } else {
                        return SearchResult::Refutation(clause.id, String::new());
                    }
                }
            }

            if !clause.is_tautology() {
                // Forward demodulation on new clauses
                let clause = {
                    if let Some(simplified) = demodulation::demodulate(
                        &clause,
                        &state.demod_index,
                        &state.clause_store,
                        &mut state.id_gen,
                    ) {
                        state.clause_store.insert(clause.id, clause);
                        if simplified.is_empty() {
                            state.clause_store.insert(simplified.id, simplified.clone());
                            return SearchResult::Refutation(simplified.id, String::new());
                        }
                        if is_trivial_contradiction(&simplified) {
                            let empty = Clause::new_avatar(
                                state.id_gen.next(),
                                vec![],
                                ClauseSource::Inference {
                                    rule: "equality_resolution".into(),
                                    parents: vec![simplified.id],
                                },
                                simplified.avatar.clone(),
                            );
                            state.clause_store.insert(simplified.id, simplified);
                            state.clause_store.insert(empty.id, empty.clone());
                            return SearchResult::Refutation(empty.id, String::new());
                        }
                        simplified
                    } else {
                        clause
                    }
                };

                if clause.is_tautology() {
                    continue;
                }

                // Condensation: simplify new clause by removing redundant literals
                let clause =
                    if let Some(condensed) = subsumption::condense(&clause, &mut state.id_gen) {
                        state.clause_store.insert(clause.id, clause);
                        condensed
                    } else {
                        clause
                    };

                // Forward subsumption: skip if subsumed by a processed clause
                let clause_fv = FeatureVector::from_clause(&clause);
                if state
                    .processed
                    .get_subsumption_candidates(&clause_fv)
                    .into_iter()
                    .any(|p| subsumption::subsumes(p, &clause))
                {
                    continue;
                }

                state.clause_store.insert(clause.id, clause.clone());
                state.unprocessed.push(&clause);
            }
        }

        iteration += 1;
    }

    // If we reach here, we processed all clauses.
    // If the literal selection is incomplete, we must return GaveUp.
    match config.literal_selection {
        crate::LiteralSelection::MaxNegativeOrMaxPositive => SearchResult::GaveUp,
        _ => SearchResult::Saturated,
    }
}

/// Returns true if a clause is a unit positive equality (used for demodulation).
fn is_unit_positive_equality(clause: &Clause) -> bool {
    clause.len() == 1
        && clause.literals[0].is_positive()
        && matches!(&clause.literals[0].atom, Atom::Eq(_, _))
}

/// Returns true if a clause is a trivial contradiction: a single negative equality
/// `s ≠ s` where both sides are syntactically identical.
fn is_trivial_contradiction(clause: &Clause) -> bool {
    clause.len() == 1
        && clause.literals[0].is_negative()
        && matches!(&clause.literals[0].atom, Atom::Eq(l, r) if l == r)
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

        let mut state = SearchState::new(
            vec![c1, c2],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Refutation(_)));
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

        let mut state = SearchState::new(
            vec![c1, c2, c3],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Refutation(_)));
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

        let mut state = SearchState::new(
            vec![c1, c2],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Saturated));
    }

    #[test]
    fn pel27_literal_selection_all() {
        // Test that pel27 clauses can be refuted with All literal selection
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
            // [0] f(sk1)
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(f_sym, vec![Term::constant(sk1)]))],
                "ax1_0",
                "axiom",
            ),
            // [1] ~g(sk1)
            input_clause(
                &mut id_gen,
                vec![Literal::neg(Atom::pred(g_sym, vec![Term::constant(sk1)]))],
                "ax1_1",
                "axiom",
            ),
            // [2] ~f(X0) | h(X0)
            input_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(f_sym, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(h_sym, vec![Term::var(0)])),
                ],
                "ax2",
                "axiom",
            ),
            // [3] ~j(X0) | ~i(X0) | f(X0)
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
            // [4] ~h(X0) | g(X0) | ~i(X1) | ~h(X1)
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
            // [5] j(sk2)
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(j_sym, vec![Term::constant(sk2)]))],
                "goal_0",
                "negated_conjecture",
            ),
            // [6] i(sk2)
            input_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::pred(i_sym, vec![Term::constant(sk2)]))],
                "goal_1",
                "negated_conjecture",
            ),
        ];

        // With All: should find refutation
        let mut state = SearchState::new(
            clauses.clone(),
            id_gen.clone(),
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        );
        let config = SearchConfig {
            time_limit: std::time::Duration::from_secs(5),
            max_clauses: 50_000,
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::All,
            ordering: TermOrdering::KBO,
        };
        let result = search(&mut state, &config);
        assert!(
            matches!(result, SearchResult::Refutation(_)),
            "Expected refutation with All selection, got {:?}",
            result
        );
    }

    #[test]
    fn pel27_literal_selection_all_negative() {
        // Test that pel27 fails with AllNegative literal selection
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

        // With AllNegative: check if it saturates
        let mut state = SearchState::new(
            clauses,
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        );
        let config = SearchConfig {
            time_limit: std::time::Duration::from_secs(5),
            max_clauses: 50_000,
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::AllNegative,
            ordering: TermOrdering::KBO,
        };
        let result = search(&mut state, &config);
        // Report what happens — if this saturates, AllNegative is too restrictive
        eprintln!("pel27 with AllNegative: {:?}", result);
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
        let mut state = SearchState::new(
            vec![c],
            id_gen,
            std::sync::Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
        );
        let config = SearchConfig::default();
        let result = search(&mut state, &config);
        assert!(matches!(result, SearchResult::Refutation(_)));
    }
}

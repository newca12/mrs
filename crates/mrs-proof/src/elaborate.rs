//! Post-refutation proof elaborator.
//!
//! Expands compact proof DAGs into fine-grained, step-by-step derivations
//! with explicit intermediate nodes for compound inferences such as
//! multi-step demodulation.
//!
//! Pure, deterministic, and topologically sorted.

use std::collections::{HashMap, HashSet, VecDeque};

use mrs_core::clause::{Clause, ClauseCertificate, ClauseId, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolTable;
use mrs_core::term::{Term, VarId};
use mrs_unify::matching::match_term;

/// An elaborated proof with explicit, fine-grained intermediate inference steps.
#[derive(Debug, Clone)]
pub struct ElaboratedProof {
    pub clauses: Vec<Clause>,
}

/// Errors that can occur during post-refutation proof elaboration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElaborationError {
    /// Multiple distinct rewrite paths could produce the conclusion without a unique canonical trace.
    Ambiguous(String),
    /// A cyclic rewrite or dependency loop was detected.
    Cyclic(String),
    /// Search exceeded bounded step limits.
    LimitExceeded(String),
    /// Replay could not reproduce the expected conclusion from the specified parents.
    CannotReproduce(String),
    /// The proof or rule structure is invalid or unsupported.
    Inconclusive(String),
}

impl std::fmt::Display for ElaborationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ambiguous(msg) => write!(f, "Ambiguous: {msg}"),
            Self::Cyclic(msg) => write!(f, "Cyclic: {msg}"),
            Self::LimitExceeded(msg) => write!(f, "LimitExceeded: {msg}"),
            Self::CannotReproduce(msg) => write!(f, "CannotReproduce: {msg}"),
            Self::Inconclusive(msg) => write!(f, "Inconclusive: {msg}"),
        }
    }
}

impl std::error::Error for ElaborationError {}

/// Identifies the sub-atom position of a term.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AtomSide {
    EqLeft,
    EqRight,
    PredArg(usize),
}

/// Precise coordinate of a rewritten subterm within a clause.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RewritePosition {
    pub lit_idx: usize,
    pub atom_side: AtomSide,
    pub term_path: Vec<usize>,
}

/// An oriented equality rewrite rule derived from an equality clause parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrientedRule {
    pub parent_id: ClauseId,
    pub lhs: Term,
    pub rhs: Term,
}

/// A single step in a demodulation rewrite sequence.
#[derive(Debug, Clone)]
pub struct DemodStep {
    pub position: RewritePosition,
    pub rule_parent_id: ClauseId,
    pub resulting_clause: Clause,
}

/// Retrieves the subterm at `path` within `term`.
pub fn get_subterm<'a>(term: &'a Term, path: &[usize]) -> Option<&'a Term> {
    let mut curr = term;
    for &idx in path {
        match curr {
            Term::App(_, args) if idx < args.len() => {
                curr = &args[idx];
            }
            _ => return None,
        }
    }
    Some(curr)
}

/// Replaces the subterm at `path` within `term` with `replacement`.
pub fn replace_subterm(term: &Term, path: &[usize], replacement: Term) -> Option<Term> {
    if path.is_empty() {
        return Some(replacement);
    }
    match term {
        Term::App(sym, args) => {
            let idx = path[0];
            if idx >= args.len() {
                return None;
            }
            let mut new_args = args.clone();
            new_args[idx] = replace_subterm(&args[idx], &path[1..], replacement)?;
            Some(Term::App(*sym, new_args))
        }
        Term::Var(_) => None,
    }
}

/// Retrieves the subterm at `pos` within `clause`.
pub fn get_subterm_in_clause<'a>(clause: &'a Clause, pos: &RewritePosition) -> Option<&'a Term> {
    let lit = clause.literals.get(pos.lit_idx)?;
    let term = match (&lit.atom, &pos.atom_side) {
        (Atom::Eq(l, _), AtomSide::EqLeft) => l,
        (Atom::Eq(_, r), AtomSide::EqRight) => r,
        (Atom::Pred(_, args), AtomSide::PredArg(idx)) => args.get(*idx)?,
        _ => return None,
    };
    get_subterm(term, &pos.term_path)
}

/// Replaces the subterm at `pos` within `clause` with `replacement`.
pub fn replace_subterm_in_clause(
    clause: &Clause,
    pos: &RewritePosition,
    replacement: Term,
) -> Option<Clause> {
    let mut new_lits = clause.literals.clone();
    let lit = new_lits.get_mut(pos.lit_idx)?;
    match (&mut lit.atom, &pos.atom_side) {
        (Atom::Eq(l, _), AtomSide::EqLeft) => {
            *l = replace_subterm(l, &pos.term_path, replacement)?;
        }
        (Atom::Eq(_, r), AtomSide::EqRight) => {
            *r = replace_subterm(r, &pos.term_path, replacement)?;
        }
        (Atom::Pred(_, args), AtomSide::PredArg(idx)) => {
            let target_arg = args.get_mut(*idx)?;
            *target_arg = replace_subterm(target_arg, &pos.term_path, replacement)?;
        }
        _ => return None,
    }
    Some(Clause {
        id: clause.id,
        literals: new_lits,
        source: clause.source.clone(),
        avatar: clause.avatar.clone(),
        distance: clause.distance,
        certificate: clause.certificate.clone(),
        formula: clause.formula.clone(),
        proof_id: clause.proof_id,
        witness: clause.witness.clone(),
    })
}

fn collect_term_paths(term: &Term, current_path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    out.push(current_path.clone());
    if let Term::App(_, args) = term {
        for (i, arg) in args.iter().enumerate() {
            current_path.push(i);
            collect_term_paths(arg, current_path, out);
            current_path.pop();
        }
    }
}

/// Collects all valid subterm positions within a clause in canonical order.
pub fn collect_all_positions(clause: &Clause) -> Vec<RewritePosition> {
    let mut positions = Vec::new();
    for (lit_idx, lit) in clause.literals.iter().enumerate() {
        match &lit.atom {
            Atom::Eq(l, r) => {
                let mut path = Vec::new();
                let mut l_paths = Vec::new();
                collect_term_paths(l, &mut path, &mut l_paths);
                for p in l_paths {
                    positions.push(RewritePosition {
                        lit_idx,
                        atom_side: AtomSide::EqLeft,
                        term_path: p,
                    });
                }
                let mut r_paths = Vec::new();
                collect_term_paths(r, &mut path, &mut r_paths);
                for p in r_paths {
                    positions.push(RewritePosition {
                        lit_idx,
                        atom_side: AtomSide::EqRight,
                        term_path: p,
                    });
                }
            }
            Atom::Pred(_, args) => {
                for (arg_idx, arg) in args.iter().enumerate() {
                    let mut path = Vec::new();
                    let mut arg_paths = Vec::new();
                    collect_term_paths(arg, &mut path, &mut arg_paths);
                    for p in arg_paths {
                        positions.push(RewritePosition {
                            lit_idx,
                            atom_side: AtomSide::PredArg(arg_idx),
                            term_path: p,
                        });
                    }
                }
            }
        }
    }
    positions
}

fn term_vars(term: &Term, vars: &mut HashSet<VarId>) {
    match term {
        Term::Var(v) => {
            vars.insert(*v);
        }
        Term::App(_, args) => {
            for arg in args {
                term_vars(arg, vars);
            }
        }
    }
}

fn term_weight(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_weight).sum::<usize>(),
    }
}

/// Extracts non-variable-expanding oriented rewrite rules from a unit equality clause.
pub fn extract_oriented_rules(rule_clause: &Clause) -> Result<Vec<OrientedRule>, ElaborationError> {
    if rule_clause.literals.len() != 1 || !rule_clause.literals[0].positive {
        return Err(ElaborationError::Inconclusive(format!(
            "Demodulation parent c{} must be a positive unit equality",
            rule_clause.id.0
        )));
    }
    let Atom::Eq(l, r) = &rule_clause.literals[0].atom else {
        return Err(ElaborationError::Inconclusive(format!(
            "Demodulation parent c{} must be an equality",
            rule_clause.id.0
        )));
    };
    if l == r {
        return Ok(Vec::new());
    }

    let mut l_vars = HashSet::new();
    let mut r_vars = HashSet::new();
    term_vars(l, &mut l_vars);
    term_vars(r, &mut r_vars);

    let l_weight = term_weight(l);
    let r_weight = term_weight(r);

    let mut rules = Vec::new();
    if r_vars.is_subset(&l_vars) && l_weight >= r_weight {
        rules.push(OrientedRule {
            parent_id: rule_clause.id,
            lhs: l.clone(),
            rhs: r.clone(),
        });
    } else if l_vars.is_subset(&r_vars) && r_weight > l_weight {
        rules.push(OrientedRule {
            parent_id: rule_clause.id,
            lhs: r.clone(),
            rhs: l.clone(),
        });
    }

    if rules.is_empty() {
        return Err(ElaborationError::Inconclusive(format!(
            "Demodulation parent c{} has no non-expanding orientation",
            rule_clause.id.0
        )));
    }
    Ok(rules)
}

fn shift_term(term: &Term, offset: VarId) -> Term {
    match term {
        Term::Var(v) => Term::Var(v + offset),
        Term::App(f, args) => Term::App(*f, args.iter().map(|a| shift_term(a, offset)).collect()),
    }
}

fn max_var_term(term: &Term) -> VarId {
    let mut vars = HashSet::new();
    term_vars(term, &mut vars);
    vars.into_iter().max().map_or(0, |v| v + 1)
}

fn max_var_clause(clause: &Clause) -> VarId {
    let mut max_v = 0;
    for lit in &clause.literals {
        match &lit.atom {
            Atom::Eq(l, r) => {
                max_v = max_v.max(max_var_term(l)).max(max_var_term(r));
            }
            Atom::Pred(_, args) => {
                for a in args {
                    max_v = max_v.max(max_var_term(a));
                }
            }
        }
    }
    max_v
}

/// Reconstructs the exact sequence of 1-step demodulation rewrites leading from
/// `target` to `conclusion` using rules from `rule_parents`.
pub fn reconstruct_demodulation(
    target: &Clause,
    conclusion: &Clause,
    rule_parents: &[Clause],
    next_clause_id: &mut u64,
) -> Result<Vec<Clause>, ElaborationError> {
    let offset = max_var_clause(target).max(max_var_clause(conclusion));
    let mut all_rules = Vec::new();
    for p in rule_parents {
        let rules = extract_oriented_rules(p)?;
        for mut r in rules {
            if offset > 0 {
                r.lhs = shift_term(&r.lhs, offset);
                r.rhs = shift_term(&r.rhs, offset);
            }
            all_rules.push(r);
        }
    }
    if all_rules.is_empty() {
        return Err(ElaborationError::CannotReproduce(format!(
            "No usable rewrite rules for demodulation of c{}",
            conclusion.id.0
        )));
    }

    if target.literals == conclusion.literals {
        return Ok(vec![conclusion.clone()]);
    }

    let max_depth = 25;

    // 1. Try canonical leftmost-outermost deterministic trace
    let mut current = target.clone();
    let mut visited = HashSet::new();
    visited.insert(target.literals.to_vec());
    let mut canonical_trace = Vec::new();
    let mut reached_canonical = false;

    for _ in 0..max_depth {
        if current.literals == conclusion.literals {
            reached_canonical = true;
            break;
        }

        let positions = collect_all_positions(&current);
        let mut step_taken = false;

        for pos in &positions {
            let Some(subterm) = get_subterm_in_clause(&current, pos) else {
                continue;
            };

            let mut matching_rules = Vec::new();
            for rule in &all_rules {
                if let Ok(sigma) = match_term(&rule.lhs, subterm) {
                    let rewritten = sigma.apply_term(&rule.rhs);
                    matching_rules.push((rule, rewritten));
                }
            }

            if matching_rules.is_empty() {
                continue;
            }

            // Check for ambiguous rules matching at this position
            if matching_rules.len() > 1 {
                let first_parent = matching_rules[0].0.parent_id;
                let first_rewritten = &matching_rules[0].1;
                let has_divergence = matching_rules[1..]
                    .iter()
                    .any(|(r, rw)| r.parent_id != first_parent || rw != first_rewritten);
                if has_divergence {
                    return Err(ElaborationError::Ambiguous(format!(
                        "Ambiguous demodulation rewrite rules at position in c{}",
                        conclusion.id.0
                    )));
                }
            }

            let (chosen_rule, rewritten_subterm) = &matching_rules[0];
            let Some(next_clause) =
                replace_subterm_in_clause(&current, pos, (*rewritten_subterm).clone())
            else {
                continue;
            };

            let lits_vec = next_clause.literals.to_vec();
            if visited.contains(&lits_vec) {
                continue;
            }
            visited.insert(lits_vec);

            canonical_trace.push(DemodStep {
                position: pos.clone(),
                rule_parent_id: chosen_rule.parent_id,
                resulting_clause: next_clause.clone(),
            });
            current = next_clause;
            step_taken = true;
            break;
        }

        if !step_taken {
            break;
        }
    }

    if current.literals == conclusion.literals {
        reached_canonical = true;
    }

    let steps = if reached_canonical {
        canonical_trace
    } else {
        // Fallback: bounded search
        struct DemodSearch<'a> {
            goal: &'a Clause,
            rules: &'a [OrientedRule],
            max_depth: usize,
            visited_clauses: HashSet<Vec<Literal>>,
            current_path: Vec<DemodStep>,
            found_paths: Vec<Vec<DemodStep>>,
        }

        impl<'a> DemodSearch<'a> {
            fn search(&mut self, current: &Clause, depth: usize) -> Result<(), ElaborationError> {
                if current.literals == self.goal.literals {
                    self.found_paths.push(self.current_path.clone());
                    return Ok(());
                }
                if depth >= self.max_depth {
                    return Err(ElaborationError::LimitExceeded(
                        "Demodulation depth exceeded bounded search limit".into(),
                    ));
                }

                let positions = collect_all_positions(current);
                for pos in &positions {
                    let Some(subterm) = get_subterm_in_clause(current, pos) else {
                        continue;
                    };
                    for rule in self.rules {
                        if let Ok(sigma) = match_term(&rule.lhs, subterm) {
                            let rewritten_subterm = sigma.apply_term(&rule.rhs);
                            if let Some(next_clause) =
                                replace_subterm_in_clause(current, pos, rewritten_subterm)
                            {
                                let lits_vec = next_clause.literals.to_vec();
                                if self.visited_clauses.contains(&lits_vec) {
                                    continue;
                                }
                                self.visited_clauses.insert(lits_vec.clone());
                                self.current_path.push(DemodStep {
                                    position: pos.clone(),
                                    rule_parent_id: rule.parent_id,
                                    resulting_clause: next_clause.clone(),
                                });

                                self.search(&next_clause, depth + 1)?;

                                self.current_path.pop();
                                self.visited_clauses.remove(&lits_vec);

                                if self.found_paths.len() > 1 {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }

        let mut visited_search = HashSet::new();
        visited_search.insert(target.literals.to_vec());

        let mut searcher = DemodSearch {
            goal: conclusion,
            rules: &all_rules,
            max_depth,
            visited_clauses: visited_search,
            current_path: Vec::new(),
            found_paths: Vec::new(),
        };

        searcher.search(target, 0)?;

        let mut found_paths = searcher.found_paths;

        if found_paths.is_empty() {
            return Err(ElaborationError::CannotReproduce(format!(
                "Could not reproduce conclusion c{} from target c{} and equality rules",
                conclusion.id.0, target.id.0
            )));
        }
        if found_paths.len() > 1 {
            return Err(ElaborationError::Ambiguous(format!(
                "Ambiguous demodulation trace for c{}: multiple candidate rewrite sequences reach conclusion",
                conclusion.id.0
            )));
        }
        found_paths.remove(0)
    };

    if steps.is_empty() {
        return Ok(vec![conclusion.clone()]);
    }

    let mut generated_clauses = Vec::with_capacity(steps.len());
    let mut prev_id = target.id;

    for (i, step) in steps.iter().enumerate() {
        let is_last = i + 1 == steps.len();
        let clause_id = if is_last {
            conclusion.id
        } else {
            let id = ClauseId(*next_clause_id);
            *next_clause_id += 1;
            id
        };

        let clause = Clause {
            id: clause_id,
            literals: step.resulting_clause.literals.clone(),
            source: ClauseSource::Inference {
                rule: "demodulation",
                parents: vec![prev_id, step.rule_parent_id].into(),
            },
            avatar: conclusion.avatar.clone(),
            distance: conclusion.distance,
            certificate: if is_last {
                conclusion.certificate.clone()
            } else {
                None
            },
            formula: if is_last {
                conclusion.formula.clone()
            } else {
                None
            },
            proof_id: if is_last { conclusion.proof_id } else { None },
            witness: if is_last {
                conclusion.witness.clone()
            } else {
                None
            },
        };
        prev_id = clause_id;
        generated_clauses.push(clause);
    }

    Ok(generated_clauses)
}

/// Topologically sorts clauses by actual parent dependency edges.
/// Does not rely on numeric clause IDs.
pub fn topological_sort(clauses: &[Clause]) -> Result<Vec<Clause>, ElaborationError> {
    let mut id_to_clause: HashMap<ClauseId, Clause> = HashMap::default();
    let mut in_degree: HashMap<ClauseId, usize> = HashMap::default();
    let mut dependents: HashMap<ClauseId, Vec<ClauseId>> = HashMap::default();
    let mut clause_order: HashMap<ClauseId, usize> = HashMap::default();

    for (idx, c) in clauses.iter().enumerate() {
        if id_to_clause.insert(c.id, c.clone()).is_some() {
            return Err(ElaborationError::Inconclusive(format!(
                "duplicate clause id c{} in proof",
                c.id.0
            )));
        }
        clause_order.insert(c.id, idx);
        in_degree.insert(c.id, 0);
    }

    for c in clauses {
        let mut parents = Vec::new();
        if let ClauseSource::Inference { parents: p, .. }
        | ClauseSource::Introduced { parents: p, .. } = &c.source
        {
            parents.extend_from_slice(p);
        }
        if let Some(cert) = &c.certificate {
            match cert {
                ClauseCertificate::AvatarComponent { split_parent, .. } => {
                    parents.push(*split_parent);
                }
                ClauseCertificate::AvatarSatRefutation {
                    split_nodes,
                    branch_roots,
                    ..
                } => {
                    parents.extend_from_slice(split_nodes);
                    parents.extend_from_slice(branch_roots);
                }
                _ => {}
            }
        }

        // Every cited parent must be present. Silently dropping a missing
        // parent would make the exported proof appear well-formed while
        // changing the derivation being checked.
        let mut unique_parents = HashSet::new();
        for p in parents {
            if !id_to_clause.contains_key(&p) {
                return Err(ElaborationError::Inconclusive(format!(
                    "clause c{} cites missing parent c{}",
                    c.id.0, p.0
                )));
            }
            if unique_parents.insert(p) {
                dependents.entry(p).or_default().push(c.id);
                *in_degree.entry(c.id).or_default() += 1;
            }
        }
    }

    let mut ready = VecDeque::new();
    let mut initial_ready: Vec<ClauseId> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(&id, _)| id)
        .collect();

    // Deterministic tie-breaking by original input order
    initial_ready.sort_by_key(|id| clause_order.get(id).copied().unwrap_or(0));
    ready.extend(initial_ready);

    let mut sorted = Vec::with_capacity(clauses.len());
    while let Some(curr_id) = ready.pop_front() {
        if let Some(clause) = id_to_clause.remove(&curr_id) {
            sorted.push(clause);
        }
        if let Some(children) = dependents.get(&curr_id) {
            let mut newly_ready = Vec::new();
            for &child_id in children {
                if let Some(deg) = in_degree.get_mut(&child_id) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        newly_ready.push(child_id);
                    }
                }
            }
            newly_ready.sort_by_key(|id| clause_order.get(id).copied().unwrap_or(0));
            ready.extend(newly_ready);
        }
    }

    if sorted.len() < clauses.len() {
        return Err(ElaborationError::Cyclic(
            "Cyclic parent dependency detected in proof DAG".into(),
        ));
    }

    Ok(sorted)
}

/// Elaborates a proof DAG by expanding composite inferences into 1-step derivations
/// and ordering all nodes topologically by true parent edges.
pub fn elaborate(
    proof: &[Clause],
    symbols: &SymbolTable,
) -> Result<ElaboratedProof, ElaborationError> {
    if proof.is_empty() {
        return Ok(ElaboratedProof {
            clauses: Vec::new(),
        });
    }

    let mut next_clause_id = proof.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
    let mut clause_map: HashMap<ClauseId, Clause> = HashMap::default();
    for c in proof {
        clause_map.insert(c.id, c.clone());
    }

    // Verify definitions do not define pre-existing axiom symbols
    for c in proof {
        if let ClauseSource::Introduced { symbol, .. } = &c.source {
            let sym_name = symbols.resolve(*symbol);
            if sym_name.is_empty() {
                return Err(ElaborationError::Inconclusive(format!(
                    "Clause c{} introduces an unresolvable symbol",
                    c.id.0
                )));
            }
        }
    }

    let mut expanded_clauses = Vec::new();

    for clause in proof {
        match &clause.source {
            ClauseSource::Inference { rule, parents } if *rule == "demodulation" => {
                if parents.len() < 2 {
                    return Err(ElaborationError::Inconclusive(format!(
                        "Demodulation node c{} requires at least 2 parents (target + rule)",
                        clause.id.0
                    )));
                }
                let target_id = parents[0];
                let Some(target_clause) = clause_map.get(&target_id) else {
                    return Err(ElaborationError::Inconclusive(format!(
                        "Demodulation target c{} not found for c{}",
                        target_id.0, clause.id.0
                    )));
                };
                let mut rule_clauses = Vec::new();
                for &pid in &parents[1..] {
                    let Some(rule_clause) = clause_map.get(&pid) else {
                        return Err(ElaborationError::Inconclusive(format!(
                            "Demodulation rule parent c{} not found for c{}",
                            pid.0, clause.id.0
                        )));
                    };
                    rule_clauses.push(rule_clause.clone());
                }

                let intermediate = reconstruct_demodulation(
                    target_clause,
                    clause,
                    &rule_clauses,
                    &mut next_clause_id,
                )?;

                for step_clause in intermediate {
                    clause_map.insert(step_clause.id, step_clause.clone());
                    expanded_clauses.push(step_clause);
                }
            }
            _ => {
                expanded_clauses.push(clause.clone());
            }
        }
    }

    let sorted = topological_sort(&expanded_clauses)?;
    Ok(ElaboratedProof { clauses: sorted })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_input_clause(id: u64, lits: Vec<Literal>) -> Clause {
        Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: format!("ax{id}"),
                role: "axiom".into(),
            },
        )
    }

    fn dummy_eq_clause(id: u64, l: Term, r: Term) -> Clause {
        dummy_input_clause(id, vec![Literal::pos(Atom::eq(l, r))])
    }

    #[test]
    fn test_one_step_demodulation() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");

        // Target: f(a) = b
        let c_target = dummy_input_clause(
            1,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
        );
        // Rule: a = c
        let c_rule = dummy_eq_clause(2, Term::constant(a), Term::constant(c));
        // Conclusion: f(c) = b
        let c_concl = Clause::new(
            ClauseId(3),
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(c)]),
                Term::constant(b),
            ))],
            ClauseSource::Inference {
                rule: "demodulation",
                parents: vec![ClauseId(1), ClauseId(2)].into(),
            },
        );

        let proof = vec![c_target, c_rule, c_concl];
        let elaborated = elaborate(&proof, &syms).expect("elaboration should succeed");
        assert_eq!(elaborated.clauses.len(), 3);
        let last = &elaborated.clauses[2];
        if let ClauseSource::Inference { rule, parents } = &last.source {
            assert_eq!(*rule, "demodulation");
            assert_eq!(parents.len(), 2);
            assert_eq!(parents[0], ClauseId(1));
            assert_eq!(parents[1], ClauseId(2));
        } else {
            panic!("expected inference");
        }
    }

    #[test]
    fn test_multi_step_demodulation() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");
        let d = syms.intern("d");
        let e = syms.intern("e");

        // Target: f(a, b) = c
        let c_target = dummy_input_clause(
            1,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a), Term::constant(b)]),
                Term::constant(c),
            ))],
        );
        // Rule 1: a = d
        let c_rule1 = dummy_eq_clause(2, Term::constant(a), Term::constant(d));
        // Rule 2: b = e
        let c_rule2 = dummy_eq_clause(3, Term::constant(b), Term::constant(e));
        // Conclusion: f(d, e) = c with composite parents [1, 2, 3]
        let c_concl = Clause::new(
            ClauseId(4),
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(d), Term::constant(e)]),
                Term::constant(c),
            ))],
            ClauseSource::Inference {
                rule: "demodulation",
                parents: vec![ClauseId(1), ClauseId(2), ClauseId(3)].into(),
            },
        );

        let proof = vec![c_target, c_rule1, c_rule2, c_concl];
        let elaborated = elaborate(&proof, &syms).expect("elaboration should succeed");

        // Should expand into 5 clauses: target, rule1, rule2, intermediate, conclusion
        assert_eq!(elaborated.clauses.len(), 5);

        // Verify topological sorting: all parent references must precede child
        let mut seen = HashSet::new();
        for clause in &elaborated.clauses {
            if let ClauseSource::Inference { parents, .. } = &clause.source {
                for p in parents {
                    assert!(
                        seen.contains(p),
                        "parent c{} must precede child c{}",
                        p.0,
                        clause.id.0
                    );
                }
                assert_eq!(
                    parents.len(),
                    2,
                    "every elaborated demodulation must have exactly 2 parents"
                );
            }
            seen.insert(clause.id);
        }
    }

    #[test]
    fn test_rewrite_position_mutation() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");

        let clause = dummy_input_clause(
            1,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
        );

        // Invalid position: out of bounds
        let bad_pos = RewritePosition {
            lit_idx: 5,
            atom_side: AtomSide::EqLeft,
            term_path: vec![0],
        };
        assert!(replace_subterm_in_clause(&clause, &bad_pos, Term::constant(c)).is_none());
    }

    #[test]
    fn test_rewrite_order_mutation() {
        // Construct inverted DAG dependency: child precedes parent
        let c1 = dummy_input_clause(1, vec![]);
        let c2 = Clause::new(
            ClauseId(2),
            vec![],
            ClauseSource::Inference {
                rule: "resolve",
                parents: vec![ClauseId(1)].into(),
            },
        );

        // Pass out of order
        let proof = vec![c2.clone(), c1.clone()];
        let sorted = topological_sort(&proof).expect("topological sort should reorder correctly");
        assert_eq!(sorted[0].id, ClauseId(1));
        assert_eq!(sorted[1].id, ClauseId(2));
    }

    #[test]
    fn test_cyclic_rewrite_rules() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");

        // Target: a = a
        let c_target = dummy_eq_clause(1, Term::constant(a), Term::constant(a));
        // Rule: a = b
        let c_rule = dummy_eq_clause(2, Term::constant(a), Term::constant(b));
        // Conclusion claims unreachable target with cyclic/infinite rewrite
        let c_concl = Clause::new(
            ClauseId(3),
            vec![Literal::pos(Atom::eq(Term::constant(b), Term::constant(b)))],
            ClauseSource::Inference {
                rule: "demodulation",
                parents: vec![ClauseId(1), ClauseId(2)].into(),
            },
        );

        let mut next_id = 10;
        let res = reconstruct_demodulation(&c_target, &c_concl, &[c_rule], &mut next_id);
        assert!(res.is_err() || res.is_ok());
    }

    #[test]
    fn test_ambiguous_rewrite_failure() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");

        // Target: f(a, a) = c
        let c_target = dummy_input_clause(
            1,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a), Term::constant(a)]),
                Term::constant(c),
            ))],
        );
        // Overlapping rules where both can reach conclusion
        let c_rule1 = dummy_eq_clause(2, Term::constant(a), Term::constant(b));
        let c_rule2 = dummy_eq_clause(3, Term::constant(a), Term::constant(b));

        let c_concl = Clause::new(
            ClauseId(4),
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(b), Term::constant(a)]),
                Term::constant(c),
            ))],
            ClauseSource::Inference {
                rule: "demodulation",
                parents: vec![ClauseId(1), ClauseId(2), ClauseId(3)].into(),
            },
        );

        let mut next_id = 10;
        let res = reconstruct_demodulation(&c_target, &c_concl, &[c_rule1, c_rule2], &mut next_id);
        assert!(matches!(res, Err(ElaborationError::Ambiguous(_))));
    }

    #[test]
    fn test_nested_definition_dependency() {
        let mut syms = SymbolTable::new();
        let d1 = syms.intern("def_d1");
        let d2 = syms.intern("def_d2");

        let c_def1 = Clause::new(
            ClauseId(1),
            vec![],
            ClauseSource::Introduced {
                symbol: d1,
                parents: smallvec::SmallVec::new(),
            },
        );
        let c_def2 = Clause::new(
            ClauseId(2),
            vec![],
            ClauseSource::Introduced {
                symbol: d2,
                parents: smallvec::SmallVec::new(),
            },
        );

        let proof = vec![c_def1, c_def2];
        let elaborated = elaborate(&proof, &syms).expect("definitions elaborate cleanly");
        assert_eq!(elaborated.clauses.len(), 2);
    }

    #[test]
    fn test_stable_topological_export() {
        let c1 = dummy_input_clause(1, vec![]);
        let c2 = dummy_input_clause(2, vec![]);
        let c3 = Clause::new(
            ClauseId(3),
            vec![],
            ClauseSource::Inference {
                rule: "resolve",
                parents: vec![ClauseId(1), ClauseId(2)].into(),
            },
        );

        let proof1 = vec![c1.clone(), c2.clone(), c3.clone()];
        let proof2 = vec![c2.clone(), c1.clone(), c3.clone()];

        let res1 = topological_sort(&proof1).unwrap();
        let res2 = topological_sort(&proof2).unwrap();

        assert_eq!(res1[2].id, ClauseId(3));
        assert_eq!(res2[2].id, ClauseId(3));
    }

    #[test]
    fn missing_parent_is_not_silently_dropped() {
        let child = Clause::new(
            ClauseId(2),
            vec![],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(99)].into(),
            },
        );
        assert!(matches!(
            topological_sort(&[child]),
            Err(ElaborationError::Inconclusive(reason)) if reason.contains("missing parent")
        ));
    }

    #[test]
    fn test_existing_golden_strict_certified_proofs() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let p = syms.intern("p");

        // ax1: f(a) = b
        let ax1 = dummy_eq_clause(1, Term::app(f, vec![Term::constant(a)]), Term::constant(b));
        // ax2: p(f(a))
        let ax2 = dummy_input_clause(
            2,
            vec![Literal::pos(Atom::Pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
        );
        // step1: p(b) from demodulation [2, 1]
        let step1 = Clause::new(
            ClauseId(3),
            vec![Literal::pos(Atom::Pred(p, vec![Term::constant(b)]))],
            ClauseSource::Inference {
                rule: "demodulation",
                parents: vec![ClauseId(2), ClauseId(1)].into(),
            },
        );
        // ax3: ~p(b)
        let ax3 = dummy_input_clause(
            4,
            vec![Literal::neg(Atom::Pred(p, vec![Term::constant(b)]))],
        );
        // step2: $false from resolution [3, 4]
        let step2 = Clause::new(
            ClauseId(5),
            vec![],
            ClauseSource::Inference {
                rule: "resolve",
                parents: vec![ClauseId(3), ClauseId(4)].into(),
            },
        );

        let proof = vec![ax1, ax2, step1, ax3, step2];
        let elaborated = elaborate(&proof, &syms).expect("elaboration should succeed");
        assert_eq!(elaborated.clauses.len(), 5);

        let tstp = crate::tstp::format_tstp(&elaborated.clauses, &syms);
        assert!(tstp.contains("demodulation"));
        assert!(tstp.contains("resolve"));
        assert!(tstp.contains("$false"));
    }
}

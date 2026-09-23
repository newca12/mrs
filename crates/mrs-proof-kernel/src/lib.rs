//! Deterministic, dependency-light proof kernel for a strict subset of TSTP.
//!
//! This crate deliberately does not depend on `mrs-search`, external ATPs, or
//! the competition verifier. It may use the workspace-owned CaDiCaL wrapper
//! for bounded propositional AVATAR roll-up checks after validating their
//! explicit certificate metadata. Unsupported proof rules return
//! `Inconclusive`; they are never accepted by guessing or by an inference-rule
//! name.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mrs_cadical::SolveResult;
use mrs_core::clause::avatar_sat_trace_digest;
use mrs_core::{Atom, Formula, Substitution, SymbolTable, Term, VarId};
use mrs_tptp::ast::common::{AtomicWord, GeneralTerm};
use mrs_tptp::proover::{
    AvatarBranchInfo, AvatarComponentInfo, AvatarSatInfo, AvatarSplitInfo, ParentRef, SatBackedInfo,
};
use mrs_tptp::{AnnotatedFormula, BinaryConnective, CNFFormula, CNFLiteral, CNFStatement};
use mrs_tptp::{FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, FormulaRole, Quantifier};

pub mod model;
pub use model::{EqualitySemantics, FunctionTable, ModelCertificate, ModelVerdict, PredicateTable};

/// Result of strict proof-kernel verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelVerdict {
    /// Every node in the proof was checked by a kernel rule.
    Certified,
    /// The proof is structurally or logically invalid.
    Rejected(String),
    /// The proof uses a rule or resource shape not implemented by the kernel.
    Inconclusive(String),
}

/// Verification measurements collected alongside a kernel verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationTelemetry {
    pub elapsed: Duration,
    pub problem_nodes: usize,
    pub proof_nodes: usize,
    pub proof_fof_nodes: usize,
    pub proof_cnf_nodes: usize,
    pub proof_clause_literals: usize,
    /// Structural measurements for proof nodes in topological order.
    pub steps: Vec<VerificationStepTelemetry>,
    pub verdict: KernelVerdict,
}

/// Structural measurements for one proof node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationStepTelemetry {
    pub name: String,
    pub rule: Option<String>,
    pub parent_count: usize,
    pub formula_nodes: usize,
    pub clause_literals: usize,
}

impl std::fmt::Display for KernelVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Certified => f.write_str("Certified"),
            Self::Rejected(reason) => write!(f, "Rejected: {reason}"),
            Self::Inconclusive(reason) => write!(f, "Inconclusive: {reason}"),
        }
    }
}

/// Explicit resource limits for strict verification.
#[derive(Debug, Clone, Copy)]
pub struct VerificationLimits {
    pub max_nodes: usize,
    pub max_formula_nodes: usize,
    pub max_clause_literals: usize,
    pub max_term_depth: usize,
    pub max_rewrite_steps: usize,
    pub max_subsumption_steps: usize,
    pub max_skolem_steps: usize,
    pub max_equivalence_steps: usize,
    pub max_avatar_steps: usize,
}

// CaDiCaL literals use signed i32 values. Keep parsed AVATAR variable names
// within that representation before any conversion to a SAT clause.
const MAX_CADICAL_VARIABLE: u32 = i32::MAX as u32;

impl Default for VerificationLimits {
    fn default() -> Self {
        Self {
            max_nodes: 100_000,
            max_formula_nodes: 100_000,
            max_clause_literals: 10_000,
            max_term_depth: 256,
            max_rewrite_steps: 64,
            max_subsumption_steps: 5_000,
            max_skolem_steps: 5_000,
            max_equivalence_steps: 5_000,
            max_avatar_steps: 100_000,
        }
    }
}

/// Verify a parsed FOF/CNF proof against its parsed problem.
pub fn verify_strict(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    limits: VerificationLimits,
) -> KernelVerdict {
    verify_strict_with_source(problem, proof, None, limits)
}

/// Verify a proof and return deterministic structural counts plus elapsed time.
pub fn verify_strict_with_telemetry(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    limits: VerificationLimits,
) -> VerificationTelemetry {
    verify_strict_with_telemetry_and_source(problem, proof, None, limits)
}

/// Telemetry variant of [`verify_strict_with_source`].
pub fn verify_strict_with_telemetry_and_source(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    expected_source: Option<&str>,
    limits: VerificationLimits,
) -> VerificationTelemetry {
    let start = Instant::now();
    let proof_nodes = proof
        .formulas
        .iter()
        .filter(|formula| formula.role() != FormulaRole::Type)
        .collect::<Vec<_>>();
    let problem_nodes = problem
        .formulas
        .iter()
        .filter(|formula| formula.role() != FormulaRole::Type)
        .count();
    let proof_fof_nodes = proof_nodes
        .iter()
        .filter(|formula| formula.is_fof())
        .count();
    let proof_cnf_nodes = proof_nodes
        .iter()
        .filter(|formula| formula.is_cnf())
        .count();
    let proof_clause_literals = proof_nodes
        .iter()
        .filter_map(|formula| formula.as_cnf())
        .map(|formula| cnf_literal_count(&formula.formula))
        .sum();
    let mut steps = Vec::with_capacity(proof_nodes.len());
    let verdict = verify_strict_with_source_internal(
        problem,
        proof,
        expected_source,
        limits,
        Some(&mut steps),
    );
    VerificationTelemetry {
        elapsed: start.elapsed(),
        problem_nodes,
        proof_nodes: proof_nodes.len(),
        proof_fof_nodes,
        proof_cnf_nodes,
        proof_clause_literals,
        steps,
        verdict,
    }
}

/// Verify a proof while requiring every `file(...)` leaf to cite the exact
/// source path recorded by its `% Proof : ...` header.
pub fn verify_strict_with_source(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    expected_source: Option<&str>,
    limits: VerificationLimits,
) -> KernelVerdict {
    verify_strict_with_source_internal(problem, proof, expected_source, limits, None)
}

fn verify_strict_with_source_internal(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    expected_source: Option<&str>,
    limits: VerificationLimits,
    mut telemetry: Option<&mut Vec<VerificationStepTelemetry>>,
) -> KernelVerdict {
    let dag = match build_dag(proof, limits) {
        Ok(dag) => dag,
        Err(v) => return v,
    };

    let mut symbols = SymbolTable::new();
    let mut proof_formulas = HashMap::with_capacity(dag.nodes.len());
    for &idx in &dag.topo {
        let node = &dag.nodes[idx];
        let formula = match lower_annotated(&mut symbols, node.formula, limits) {
            Ok(formula) => formula,
            Err(v) => return v,
        };
        proof_formulas.insert(idx, formula);
    }
    if let Some(telemetry) = telemetry.as_mut() {
        telemetry.extend(dag.topo.iter().map(|idx| {
            let node = &dag.nodes[*idx];
            let formula = proof_formulas.get(idx).expect("lowered proof formula");
            VerificationStepTelemetry {
                name: node.name.to_owned(),
                rule: node.rule.map(str::to_owned),
                parent_count: node.parents.len(),
                formula_nodes: formula_size(formula),
                clause_literals: lowered_clause_literal_count(formula, limits),
            }
        }));
    }
    let mut branch_contexts: HashMap<usize, BranchContext> = HashMap::new();
    let mut avatar_splits: HashMap<usize, AvatarSplitContext> = HashMap::new();
    let mut defined_symbols = HashSet::new();
    let mut skolem_axioms: HashMap<usize, OwnedSkolemAxiom> = HashMap::new();

    // NOTE: duplicate problem formula names are tolerated, not rejected.
    // The TPTP library contains includes that define the same name twice
    // with different bodies (e.g. LCL518+1 pulls `substitution_of_equivalents`
    // from both LCL006+0.ax and LCL006+5.ax). Leaf matching below is
    // existential over same-named formulas, which is sound: the cited
    // formula is asserted by the problem under that name either way.
    for formula in &problem.formulas {
        if formula.role() == FormulaRole::Type {
            continue;
        }
        if !formula.is_fof() && !formula.is_cnf() {
            return KernelVerdict::Inconclusive(format!(
                "problem formula `{}` uses an unsupported dialect",
                formula.name()
            ));
        }
    }

    let mut known_function_symbols: HashSet<String> = HashSet::new();
    let mut known_non_skolem_function_symbols: HashSet<String> = HashSet::new();
    let mut known_symbols: HashSet<String> = HashSet::new();
    let mut signatures = HashMap::new();
    // Background AC axioms straight from the problem (commutativity /
    // associativity shapes): the prover's superposition is AC-aware even
    // when a step is exported as plain `superposition` without AC
    // citations (SWV486+1 rewrites with commutativity of `plus`). The
    // fallback below replays such steps against these axioms; all of them
    // are problem premises, so accepting remains sound.
    let mut background_commutative: HashSet<mrs_core::SymbolId> = HashSet::new();
    let mut background_associative: HashSet<mrs_core::SymbolId> = HashSet::new();
    for formula in &problem.formulas {
        collect_function_symbols(formula, &mut known_function_symbols);
        collect_function_symbols(formula, &mut known_non_skolem_function_symbols);
        collect_all_symbols(formula, &mut known_symbols);
        if formula.role() != FormulaRole::Type {
            let lowered = match lower_annotated(&mut symbols, formula, limits) {
                Ok(formula) => formula,
                Err(v) => return v,
            };
            if let Err(reason) = register_signatures(&lowered, &mut signatures) {
                return KernelVerdict::Rejected(format!(
                    "problem has inconsistent symbol signature: {reason}"
                ));
            }
            let ac_candidate = if formula.role() == FormulaRole::Conjecture {
                Some(to_nnf(&Formula::neg(lowered.clone())))
            } else if formula.role().is_premise()
                || formula.role() == FormulaRole::NegatedConjecture
            {
                Some(lowered.clone())
            } else {
                None
            };
            if let Some(clause) = ac_candidate
                .as_ref()
                .and_then(|candidate| clause_from_formula(candidate, limits))
                && clause.len() == 1
                && clause[0].positive
                && let Atom::Eq(left, right) = &clause[0].atom
                && let Some((symbol, kind)) = classify_ac_axiom(left, right)
            {
                match kind {
                    AcAxiomKind::Commutative => {
                        background_commutative.insert(symbol);
                    }
                    AcAxiomKind::Associative => {
                        background_associative.insert(symbol);
                    }
                }
            }
        }
    }

    for &idx in &dag.topo {
        let node = &dag.nodes[idx];
        let conclusion = proof_formulas.get(&idx).expect("lowered proof formula");
        if let Err(reason) = register_signatures(conclusion, &mut signatures) {
            return KernelVerdict::Rejected(format!(
                "proof node `{}` has inconsistent symbol signature: {reason}",
                node.name
            ));
        }

        if node
            .formula
            .annotations()
            .and_then(|a| a.file_source())
            .is_some()
        {
            if !node.parents.is_empty() {
                return KernelVerdict::Rejected(format!(
                    "leaf `{}` also cites inference parents",
                    node.name
                ));
            }
            match verify_leaf(node, problem, expected_source, &mut symbols, limits) {
                Ok(()) => {
                    collect_function_symbols(node.formula, &mut known_function_symbols);
                    collect_function_symbols(node.formula, &mut known_non_skolem_function_symbols);
                    collect_all_symbols(node.formula, &mut known_symbols);
                    continue;
                }
                Err(v) => return v,
            }
        }

        if node.parents.is_empty() {
            if let Some(annotations) = node.formula.annotations()
                && is_introduced_definition(annotations)
            {
                let outcome = if is_skolem_symbol_introduction(annotations) {
                    match extract_skolem_axiom(
                        conclusion,
                        &symbols,
                        &known_non_skolem_function_symbols,
                        limits,
                    ) {
                        Ok(axiom) => {
                            let declared: HashSet<_> = annotations
                                .new_symbols()
                                .into_iter()
                                .filter_map(|name| symbols.resolve_name(name))
                                .collect();
                            if !declared.is_empty() && declared != axiom.fresh_symbols {
                                return KernelVerdict::Rejected(format!(
                                    "Skolem axiom `{}` declaration does not match its fresh symbols",
                                    node.name
                                ));
                            }
                            skolem_axioms.insert(idx, axiom);
                            KernelVerdict::Certified
                        }
                        Err(verdict) => verdict,
                    }
                } else {
                    verify_definition(node, annotations, &known_symbols, &mut defined_symbols)
                };
                if !matches!(outcome, KernelVerdict::Certified) {
                    return outcome;
                }
                collect_function_symbols(node.formula, &mut known_function_symbols);
                if !is_skolem_symbol_introduction(annotations) {
                    collect_function_symbols(node.formula, &mut known_non_skolem_function_symbols);
                }
                continue;
            }
            return KernelVerdict::Inconclusive(format!(
                "node `{}` has no provenance and no parents",
                node.name
            ));
        }

        let rule = match node.rule {
            Some(rule) => rule,
            None => {
                return KernelVerdict::Inconclusive(format!(
                    "node `{}` has no inference rule",
                    node.name
                ));
            }
        };

        let parents = match parent_formulas(
            &dag,
            node,
            &proof_formulas,
            !matches!(rule, "negated_conjecture" | "assume_negation"),
        ) {
            Ok(parents) => parents,
            Err(v) => return v,
        };

        let parent_role = node
            .parents
            .first()
            .and_then(|parent| dag.by_name.get(parent.name))
            .map(|idx| dag.nodes[*idx].role);
        let parent_indices = node
            .parents
            .iter()
            .map(|parent| *dag.by_name.get(parent.name).expect("validated parent"))
            .collect::<Vec<_>>();
        let outcome = match rule {
            "negated_conjecture" | "assume_negation" => {
                verify_negated_conjecture(node, &parents, parent_role, conclusion)
            }
            "fof_nnf" | "fof_nnf_transformation" | "nnf_transformation" => {
                verify_nnf(&parents, conclusion)
            }
            "variable_rename" | "rename_variable" | "rename" | "rectify" | "copy" | "assume"
            | "rewrite" | "duplicate" => verify_alpha_identity(&parents, conclusion),
            "alpha" => {
                let outcome = verify_alpha_identity(&parents, conclusion);
                if matches!(outcome, KernelVerdict::Certified) {
                    outcome
                } else {
                    let split = verify_split_conjunct(&parents, conclusion, limits);
                    if matches!(split, KernelVerdict::Certified) {
                        split
                    } else {
                        outcome
                    }
                }
            }
            "formula_equivalence"
            | "equivalence"
            | "fof_simplification"
            | "true_and_iff_removal"
            | "trivial_inequality_removal"
            | "evaluation"
            | "remove_duplicate_literals"
            | "duplicate_literal_removal"
            | "flattening"
            | "distribute"
            | "ennf_transformation"
            | "simplification"
            | "double_negation"
            | "remove_double_negation"
            | "reassociate"
            | "commute" => verify_formula_equivalence(&parents, conclusion, limits),
            "excluded_middle" => verify_excluded_middle(&parents, conclusion, limits),
            "modus_ponens" => verify_modus_ponens(&parents, conclusion, limits),
            "instantiate_mp" => verify_modus_ponens(&parents, conclusion, limits),
            "contrapositive" => verify_contrapositive(&parents, conclusion, limits),
            "disjunctive_syllogism" => verify_disjunctive_syllogism(&parents, conclusion, limits),
            "horn" => verify_horn(&parents, conclusion, limits),
            "consequence" => verify_consequence(&parents, conclusion, limits),
            "ex_falso" => verify_ex_falso(&parents, limits),
            "weaken" => verify_weakening(&parents, conclusion, limits),
            "reflexivity" => verify_reflexivity(&parents, conclusion),
            "transitivity" => verify_transitivity(&parents, conclusion, limits),
            "instantiate" | "instantiation" => verify_instantiation(&parents, conclusion, limits),
            "definition_renaming" => {
                verify_definition_renaming(&parents, conclusion, &dag, &parent_indices, limits)
            }
            "existential_gen" => verify_existential_generation(&parents, conclusion, limits),
            "conjunction" => verify_conjunction(&parents, conclusion, limits),
            "split_conjunct" => verify_split_conjunct(&parents, conclusion, limits),
            "skolemisation" | "skolemize" => verify_skolemisation(
                node,
                &dag,
                &parent_indices,
                &parents,
                conclusion,
                SkolemVerificationContext {
                    symbols: &symbols,
                    known_function_symbols: &known_function_symbols,
                    skolem_axioms: &skolem_axioms,
                    limits,
                },
            ),
            "cnf_transformation" => verify_cnf_transformation(
                node.name,
                &parents,
                conclusion,
                &dag,
                &parent_indices,
                limits,
            ),
            "resolution" => verify_resolution(&parents, conclusion, limits),
            "ac_resolution" => verify_ac_resolution(&parents, conclusion, limits),
            "subsumption_resolution" => verify_subsumption_resolution(&parents, conclusion, limits),
            "factoring" => verify_factoring(&parents, conclusion, limits),
            "equality_resolution" | "destructive_equality_resolution" => {
                verify_equality_resolution(&parents, conclusion, limits)
            }
            "ac_normalization" => verify_ac_normalization(&parents, conclusion, limits),
            "equality_factoring" => verify_equality_factoring(&parents, conclusion, limits),
            "equality_normalization" => verify_equality_normalization(&parents, conclusion, limits),
            "condensation" => verify_condensation(&parents, conclusion, limits),
            "demodulation" => match verify_demodulation(&parents, conclusion, limits) {
                KernelVerdict::Inconclusive(msg) => {
                    KernelVerdict::Inconclusive(format!("node {}: {}", node.name, msg))
                }
                other => other,
            },
            "goal_transformation" => verify_goal_transformation(&parents, conclusion, limits),
            "superposition" => verify_superposition(
                &parents,
                conclusion,
                limits,
                &background_commutative,
                &background_associative,
            ),
            "ac_superposition" => verify_ac_superposition(
                &parents,
                conclusion,
                limits,
                &background_commutative,
                &background_associative,
            ),
            "paramodulation" => verify_paramodulation(
                &parents,
                conclusion,
                limits,
                &background_commutative,
                &background_associative,
            ),
            "split_component" => verify_split_component(
                &parents,
                conclusion,
                parent_indices.first().copied(),
                limits,
            )
            .map(|context| {
                branch_contexts.insert(idx, context);
                KernelVerdict::Certified
            })
            .unwrap_or_else(|verdict| verdict),
            "avatar_split_clause" => verify_avatar_split_clause(
                &parents,
                conclusion,
                parent_indices.first().copied(),
                parent_indices
                    .first()
                    .and_then(|parent| branch_contexts.get(parent)),
                node.formula.annotations().and_then(|a| a.avatar_split()),
                &symbols,
                limits,
            )
            .map(|context| {
                let inherited = BranchContext {
                    assumptions: context.inherited_assumptions.clone(),
                    sat_context: context.inherited_vars.clone(),
                };
                avatar_splits.insert(idx, context);
                branch_contexts.insert(idx, inherited);
                KernelVerdict::Certified
            })
            .unwrap_or_else(|verdict| verdict),
            "avatar_component_clause" => verify_avatar_component_clause(
                &parents,
                conclusion,
                parent_indices.first().copied(),
                &avatar_splits,
                parent_indices
                    .first()
                    .and_then(|parent| branch_contexts.get(parent)),
                node.parents.first().map(|parent| parent.name),
                node.formula
                    .annotations()
                    .and_then(|a| a.avatar_component()),
                &symbols,
                limits,
            )
            .map(|context| {
                branch_contexts.insert(idx, context);
                KernelVerdict::Certified
            })
            .unwrap_or_else(|verdict| verdict),
            "avatar_branch_refutation" => verify_avatar_branch_refutation(
                &parents,
                conclusion,
                parent_indices.first().copied(),
                &branch_contexts,
                node.formula.annotations().and_then(|a| a.avatar_branch()),
                &symbols,
                limits,
            )
            .map(|context| {
                branch_contexts.insert(idx, context);
                KernelVerdict::Certified
            })
            .unwrap_or_else(|verdict| verdict),
            "sat_backed_refutation" => verify_sat_backed_refutation(
                node,
                &dag,
                &parent_indices,
                &parents,
                node.formula.annotations().and_then(|a| a.sat_backed()),
                &symbols,
                limits,
            ),
            "avatar_sat_refutation" => {
                let explicit = parent_indices
                    .first()
                    .is_some_and(|parent| dag.nodes[*parent].rule == Some("avatar_split_clause"));
                if explicit
                    || node
                        .formula
                        .annotations()
                        .and_then(|a| a.avatar_sat())
                        .is_some()
                {
                    verify_avatar_sat_refutation(
                        node,
                        &dag,
                        &parent_indices,
                        &branch_contexts,
                        &avatar_splits,
                        node.formula.annotations().and_then(|a| a.avatar_sat()),
                        limits,
                    )
                } else {
                    verify_case_split(
                        node,
                        &dag,
                        &parent_indices,
                        &proof_formulas,
                        &branch_contexts,
                        &avatar_splits,
                        &symbols,
                        limits,
                    )
                }
            }
            _ => KernelVerdict::Inconclusive(format!(
                "node `{}` uses unsupported strict rule `{rule}`",
                node.name
            )),
        };
        if let Some(expected_status) = expected_status(rule)
            && node.status() != Some(expected_status)
        {
            return KernelVerdict::Rejected(format!(
                "node `{}` rule `{rule}` must have status `{expected_status}`",
                node.name
            ));
        }
        match outcome {
            KernelVerdict::Certified => {}
            KernelVerdict::Rejected(msg) => {
                let msg = if msg.contains(node.name) {
                    msg
                } else {
                    format!("node {}: {}", node.name, msg)
                };
                return KernelVerdict::Rejected(msg);
            }
            KernelVerdict::Inconclusive(msg) => {
                let msg = if msg.contains(node.name) {
                    msg
                } else {
                    format!("node {}: {}", node.name, msg)
                };
                return KernelVerdict::Inconclusive(msg);
            }
        }
        if !matches!(
            rule,
            "split_component"
                | "avatar_split_clause"
                | "avatar_component_clause"
                | "avatar_branch_refutation"
                | "avatar_sat_refutation"
        ) {
            let mut context: Option<BranchContext> = None;
            for parent_idx in &parent_indices {
                let Some(parent_context) = branch_contexts.get(parent_idx) else {
                    continue;
                };
                context = Some(match context {
                    Some(existing) => merge_branch_contexts(&existing, parent_context),
                    None => parent_context.clone(),
                });
            }
            if let Some(context) = context {
                branch_contexts.insert(idx, context);
            }
        }
        collect_function_symbols(node.formula, &mut known_function_symbols);
        if !matches!(rule, "skolemisation" | "skolemize") {
            collect_function_symbols(node.formula, &mut known_non_skolem_function_symbols);
        }
        collect_all_symbols(node.formula, &mut known_symbols);
    }

    KernelVerdict::Certified
}

fn merge_branch_contexts(left: &BranchContext, right: &BranchContext) -> BranchContext {
    let mut assumptions = left.assumptions.clone();
    for assumption in &right.assumptions {
        if !assumptions.contains(assumption) {
            assumptions.push(assumption.clone());
        }
    }
    let mut sat_context = left.sat_context.clone();
    sat_context.extend(right.sat_context.iter().copied());
    sat_context.sort_unstable();
    sat_context.dedup();
    BranchContext {
        assumptions,
        sat_context,
    }
}

fn expected_status(rule: &str) -> Option<&'static str> {
    match rule {
        "negated_conjecture" | "assume_negation" => Some("cth"),
        "skolemisation"
        | "skolemize"
        | "avatar_split_clause"
        | "avatar_component_clause"
        | "avatar_branch_refutation"
        | "split_component" => Some("esa"),
        "fof_nnf"
        | "fof_nnf_transformation"
        | "nnf_transformation"
        | "variable_rename"
        | "rename_variable"
        | "rename"
        | "alpha"
        | "rectify"
        | "copy"
        | "assume"
        | "rewrite"
        | "duplicate"
        | "reassociate"
        | "formula_equivalence"
        | "equivalence"
        | "fof_simplification"
        | "true_and_iff_removal"
        | "trivial_inequality_removal"
        | "evaluation"
        | "remove_duplicate_literals"
        | "duplicate_literal_removal"
        | "flattening"
        | "distribute"
        | "ennf_transformation"
        | "simplification"
        | "double_negation"
        | "remove_double_negation"
        | "commute"
        | "excluded_middle"
        | "modus_ponens"
        | "instantiate_mp"
        | "contrapositive"
        | "disjunctive_syllogism"
        | "horn"
        | "consequence"
        | "ex_falso"
        | "weaken"
        | "reflexivity"
        | "transitivity"
        | "instantiate"
        | "instantiation"
        | "definition_renaming"
        | "existential_gen"
        | "conjunction"
        | "split_conjunct"
        | "cnf_transformation"
        | "resolution"
        | "subsumption_resolution"
        | "factoring"
        | "equality_resolution"
        | "destructive_equality_resolution"
        | "equality_factoring"
        | "equality_normalization"
        | "condensation"
        | "demodulation"
        | "goal_transformation"
        | "ac_normalization"
        | "avatar_sat_refutation"
        | "sat_backed_refutation"
        | "superposition"
        | "ac_superposition"
        | "ac_resolution"
        | "paramodulation" => Some("thm"),
        _ => None,
    }
}

fn cnf_literal_count(statement: &CNFStatement<'_>) -> usize {
    fn count(formula: &CNFFormula<'_>) -> usize {
        match formula {
            CNFFormula::Parens(inner) => count(inner),
            CNFFormula::Disjunction(literals) => literals.len(),
        }
    }
    match statement {
        CNFStatement::Logical(formula) => count(formula),
    }
}

fn register_signatures(
    formula: &Formula,
    signatures: &mut HashMap<mrs_core::SymbolId, usize>,
) -> Result<(), String> {
    fn visit_term(
        value: &Term,
        signatures: &mut HashMap<mrs_core::SymbolId, usize>,
    ) -> Result<(), String> {
        match value {
            Term::Var(_) => Ok(()),
            Term::App(symbol, args) => {
                register(*symbol, args.len(), signatures)?;
                for arg in args {
                    visit_term(arg, signatures)?;
                }
                Ok(())
            }
        }
    }
    fn atom(
        atom: &Atom,
        signatures: &mut HashMap<mrs_core::SymbolId, usize>,
    ) -> Result<(), String> {
        match atom {
            Atom::Pred(symbol, args) => {
                register(*symbol, args.len(), signatures)?;
                for arg in args {
                    visit_term(arg, signatures)?;
                }
                Ok(())
            }
            Atom::Eq(left, right) => {
                visit_term(left, signatures)?;
                visit_term(right, signatures)
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn visit(
        formula: &Formula,
        signatures: &mut HashMap<mrs_core::SymbolId, usize>,
    ) -> Result<(), String> {
        match formula {
            Formula::Atom(atom_value) => atom(atom_value, signatures),
            Formula::Neg(inner) | Formula::Forall(_, inner) | Formula::Exists(_, inner) => {
                visit(inner, signatures)
            }
            Formula::And(parts) | Formula::Or(parts) => {
                for part in parts {
                    visit(part, signatures)?;
                }
                Ok(())
            }
            Formula::Implies(left, right) | Formula::Iff(left, right) => {
                visit(left, signatures)?;
                visit(right, signatures)
            }
            Formula::True | Formula::False => Ok(()),
        }
    }
    fn register(
        symbol: mrs_core::SymbolId,
        arity: usize,
        signatures: &mut HashMap<mrs_core::SymbolId, usize>,
    ) -> Result<(), String> {
        if let Some(previous) = signatures.insert(symbol, arity)
            && previous != arity
        {
            return Err(format!(
                "symbol {} used with arities {} and {}",
                symbol.index(),
                previous,
                arity
            ));
        }
        Ok(())
    }
    visit(formula, signatures)
}

fn is_introduced_definition(annotations: &mrs_tptp::Annotations<'_>) -> bool {
    let GeneralTerm::Function(AtomicWord::Lower("introduced"), args) = &annotations.source else {
        return false;
    };
    args.first().is_some_and(|term| {
        matches!(
            term,
            GeneralTerm::Word(AtomicWord::Lower("definition"))
                | GeneralTerm::Word(AtomicWord::SingleQuoted("definition"))
        )
    })
}

fn is_skolem_symbol_introduction(annotations: &mrs_tptp::Annotations<'_>) -> bool {
    let GeneralTerm::Function(AtomicWord::Lower("introduced"), args) = &annotations.source else {
        return false;
    };
    matches!(
        args.get(2),
        Some(GeneralTerm::List(items)) if items.iter().any(|item| matches!(
            item,
            GeneralTerm::Word(AtomicWord::Lower("skolem_symbol_introduction"))
                | GeneralTerm::Word(AtomicWord::SingleQuoted("skolem_symbol_introduction"))
        ))
    )
}

fn declared_definition_symbol<'a>(annotations: &'a mrs_tptp::Annotations<'a>) -> Option<&'a str> {
    let GeneralTerm::Function(AtomicWord::Lower("introduced"), args) = &annotations.source else {
        return None;
    };
    let GeneralTerm::List(info) = args.get(1)? else {
        return None;
    };
    for item in info {
        let GeneralTerm::Function(AtomicWord::Lower("new_symbols"), args) = item else {
            continue;
        };
        let GeneralTerm::List(symbols) = args.get(1)? else {
            continue;
        };
        if symbols.len() == 1
            && let GeneralTerm::Word(AtomicWord::Lower(symbol) | AtomicWord::SingleQuoted(symbol)) =
                &symbols[0]
        {
            return Some(symbol);
        }
    }
    None
}

fn verify_definition(
    node: &Node<'_>,
    annotations: &mrs_tptp::Annotations<'_>,
    known_symbols: &HashSet<String>,
    defined_symbols: &mut HashSet<String>,
) -> KernelVerdict {
    if node.role != FormulaRole::Definition {
        return KernelVerdict::Rejected(format!(
            "definition `{}` must have role `definition`",
            node.name
        ));
    }
    let Some(declared) = declared_definition_symbol(annotations) else {
        return KernelVerdict::Inconclusive(format!(
            "definition `{}` lacks a single new_symbols declaration",
            node.name
        ));
    };
    if known_symbols.contains(declared) || !defined_symbols.insert(declared.to_string()) {
        return KernelVerdict::Rejected(format!(
            "definition `{}` reuses existing symbol `{declared}`",
            node.name
        ));
    }
    if let Some(fof) = node.formula.as_fof() {
        let FOFStatement::Logical(formula) = &fof.formula else {
            return KernelVerdict::Inconclusive("definition sequent is unsupported".into());
        };
        let body = strip_forall_fof(formula);
        match body {
            FOFFormula::Binary {
                left,
                connective: BinaryConnective::Iff,
                right,
            } => {
                let (head, rhs) = if let Some(head) = definition_head(left, declared) {
                    (head, right.as_ref())
                } else if let Some(head) = definition_head(right, declared) {
                    (head, left.as_ref())
                } else {
                    return KernelVerdict::Rejected(format!(
                        "definition `{}` does not define `{declared}`",
                        node.name
                    ));
                };
                if formula_contains_predicate(rhs, declared) {
                    return KernelVerdict::Rejected(format!(
                        "definition `{}` is recursive",
                        node.name
                    ));
                }
                if head
                    .iter()
                    .any(|term| !matches!(term, FOFTerm::Variable(_)))
                {
                    return KernelVerdict::Rejected(format!(
                        "definition `{}` has a non-variable predicate head",
                        node.name
                    ));
                }
                let mut free = HashSet::new();
                collect_free_variable_names(rhs, &mut free);
                let mut head_vars = HashSet::new();
                for term in head {
                    collect_term_variable_names(term, &mut head_vars);
                }
                if !free.is_subset(&head_vars) {
                    return KernelVerdict::Rejected(format!(
                        "definition `{}` leaves free variables outside its head",
                        node.name
                    ));
                }
                KernelVerdict::Certified
            }
            FOFFormula::Equality(left, right) => {
                verify_equational_definition(node.name, left, right, declared)
            }
            _ => KernelVerdict::Rejected(format!(
                "definition `{}` is not a biconditional or equality",
                node.name
            )),
        }
    } else if let Some(cnf) = node.formula.as_cnf() {
        let CNFStatement::Logical(cnf_formula) = &cnf.formula;
        let literals = match cnf_formula {
            CNFFormula::Disjunction(lits) => lits.as_slice(),
            CNFFormula::Parens(inner) => match inner.as_ref() {
                CNFFormula::Disjunction(lits) => lits.as_slice(),
                _ => return KernelVerdict::Inconclusive("unsupported CNF definition shape".into()),
            },
        };
        if literals.len() != 1 {
            return KernelVerdict::Rejected(format!(
                "CNF definition `{}` must be a unit clause",
                node.name
            ));
        }
        match &literals[0] {
            CNFLiteral::Equality(left, right) => {
                verify_equational_definition(node.name, left, right, declared)
            }
            _ => KernelVerdict::Rejected(format!(
                "CNF definition `{}` must be a positive equality",
                node.name
            )),
        }
    } else {
        KernelVerdict::Inconclusive("definition is neither FOF nor CNF".into())
    }
}

fn verify_equational_definition(
    node_name: &str,
    left: &FOFTerm<'_>,
    right: &FOFTerm<'_>,
    declared: &str,
) -> KernelVerdict {
    let (head_args, rhs) = if let Some(args) = fof_term_args_for_symbol(left, declared) {
        (args, right)
    } else if let Some(args) = fof_term_args_for_symbol(right, declared) {
        (args, left)
    } else {
        return KernelVerdict::Rejected(format!(
            "definition `{node_name}` does not define `{declared}`"
        ));
    };

    if term_contains_symbol(rhs, declared) {
        return KernelVerdict::Rejected(format!("definition `{node_name}` is recursive"));
    }

    let mut head_vars = HashSet::new();
    for term in head_args {
        match term {
            FOFTerm::Variable(v) => {
                if !head_vars.insert((*v).to_string()) {
                    return KernelVerdict::Rejected(format!(
                        "definition `{node_name}` has duplicate variable `{v}` in head"
                    ));
                }
            }
            _ => {
                return KernelVerdict::Rejected(format!(
                    "definition `{node_name}` has a non-variable function argument in head"
                ));
            }
        }
    }

    let mut rhs_vars = HashSet::new();
    collect_term_variable_names(rhs, &mut rhs_vars);
    if !rhs_vars.is_subset(&head_vars) {
        return KernelVerdict::Rejected(format!(
            "definition `{node_name}` leaves free variables outside its head"
        ));
    }

    KernelVerdict::Certified
}

fn term_contains_symbol(term: &FOFTerm<'_>, symbol: &str) -> bool {
    match term {
        FOFTerm::Function(name, args) => {
            name.as_str() == symbol || args.iter().any(|arg| term_contains_symbol(arg, symbol))
        }
        FOFTerm::DefinedFunction(name, args) => {
            format!("${}", name.0) == symbol
                || args.iter().any(|arg| term_contains_symbol(arg, symbol))
        }
        FOFTerm::SystemFunction(name, args) => {
            format!("$${}", name.0) == symbol
                || args.iter().any(|arg| term_contains_symbol(arg, symbol))
        }
        FOFTerm::Variable(_) | FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => false,
    }
}

fn fof_term_args_for_symbol<'a>(term: &'a FOFTerm<'a>, symbol: &str) -> Option<&'a [FOFTerm<'a>]> {
    match term {
        FOFTerm::Function(name, args) if name.as_str() == symbol => Some(args.as_slice()),
        _ => None,
    }
}

fn formula_contains_predicate(formula: &FOFFormula<'_>, symbol: &str) -> bool {
    match formula {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(name, _)) => name.as_str() == symbol,
        FOFFormula::Atomic(FOFAtomicFormula::Defined(name, _)) => format!("${}", name.0) == symbol,
        FOFFormula::Atomic(FOFAtomicFormula::System(name, _)) => format!("$${}", name.0) == symbol,
        FOFFormula::Atomic(FOFAtomicFormula::True | FOFAtomicFormula::False) => false,
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
            formula_contains_predicate(inner, symbol)
        }
        FOFFormula::Quantified { formula, .. } => formula_contains_predicate(formula, symbol),
        FOFFormula::Binary { left, right, .. } => {
            formula_contains_predicate(left, symbol) || formula_contains_predicate(right, symbol)
        }
        FOFFormula::Equality(_, _) | FOFFormula::Inequality(_, _) => false,
    }
}

fn verify_cnf_transformation(
    node_name: &str,
    parents: &[Formula],
    conclusion: &Formula,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.is_empty() {
        return KernelVerdict::Inconclusive(
            "strict CNF transformation requires a formula parent".into(),
        );
    }
    if parents.len() != parent_indices.len() {
        return KernelVerdict::Rejected(
            "CNF transformation parent metadata is inconsistent".into(),
        );
    }
    let source_origin_position = 0;
    let mut source_position = source_origin_position;
    if contains_exists(&parents[0]) {
        let candidates = parents
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, parent)| !contains_exists(parent))
            .filter(|(position, _)| {
                let parent_idx = parent_indices[*position];
                matches!(
                    dag.nodes[parent_idx].rule,
                    Some("skolemisation" | "skolemize")
                )
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return KernelVerdict::Inconclusive(
                "existential CNF transformation requires one cited Skolemization parent".into(),
            );
        }
        let (position, _) = candidates[0];
        let skolem_idx = parent_indices[position];
        if !dag_has_ancestor(dag, skolem_idx, parent_indices[source_origin_position]) {
            return KernelVerdict::Rejected(
                "CNF Skolemization parent does not cite the existential source".into(),
            );
        }
        source_position = position;
    }
    let source = &parents[source_position];
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive("CNF conclusion is not a supported clause".into());
    };

    let mut definitions = Vec::with_capacity(parents.len().saturating_sub(1));
    for (position, (parent, parent_idx)) in parents.iter().zip(parent_indices).enumerate() {
        if position == source_origin_position || position == source_position {
            continue;
        }
        if !dag.nodes[*parent_idx]
            .formula
            .annotations()
            .is_some_and(is_introduced_definition)
        {
            return KernelVerdict::Inconclusive(
                "CNF transformation extra parents must be introduced definitions".into(),
            );
        }
        let Some(definition) = core_definition(parent) else {
            return KernelVerdict::Inconclusive(
                "CNF transformation definition parent has an unsupported shape".into(),
            );
        };
        definitions.push(definition);
    }
    if let Err(verdict) = validate_definition_dependencies(source, &definitions) {
        return verdict;
    }

    let Some(named_source) = replace_definition_subformulas(source, &definitions, limits) else {
        return KernelVerdict::Inconclusive(
            "CNF definition replacement exceeded strict limits".into(),
        );
    };
    let normalized = match normalize_quantified_cnf(&named_source, limits) {
        Ok(formula) => formula,
        Err(verdict) => return verdict,
    };
    let normalized_matrix = strip_forall_core(&normalized);
    let mut expanded = Vec::new();
    if !cnf_expand(normalized_matrix, &mut expanded, limits) {
        return KernelVerdict::Inconclusive("CNF expansion exceeded strict limits".into());
    }
    if expanded.len() > limits.max_nodes {
        return KernelVerdict::Inconclusive("CNF expansion exceeded strict limits".into());
    }

    for definition in &definitions {
        let Some(direction) = definition_direction_clauses(definition, limits) else {
            return KernelVerdict::Inconclusive(
                "CNF definition parent does not contain clause-shaped conjuncts".into(),
            );
        };
        expanded.extend(direction);
        if expanded.len() > limits.max_nodes {
            return KernelVerdict::Inconclusive("CNF expansion exceeded strict limits".into());
        }
    }

    if expanded
        .iter()
        .any(|clause| clause_alpha_equiv(&condense_clause(clause), &condense_clause(&goal)))
    {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected(format!(
            "CNF node `{node_name}` is not a clause of the cited parents"
        ))
    }
}

#[derive(Clone)]
struct CoreDefinition {
    head: Atom,
    rhs: Formula,
}

fn core_definition(formula: &Formula) -> Option<CoreDefinition> {
    let body = strip_forall_core(formula);
    let Formula::Iff(left, right) = body else {
        return None;
    };
    let (head, rhs) = if let Some(head) = core_definition_head(left) {
        (head, right.as_ref())
    } else {
        let head = core_definition_head(right)?;
        (head, left.as_ref())
    };
    Some(CoreDefinition {
        head,
        rhs: rhs.clone(),
    })
}

fn is_flat_definition_clause(formula: &Formula) -> bool {
    match formula {
        Formula::Atom(Atom::Pred(..)) => true,
        Formula::Neg(inner) => matches!(inner.as_ref(), Formula::Atom(Atom::Pred(..))),
        Formula::Or(parts) => parts.iter().all(is_flat_definition_clause),
        _ => false,
    }
}

fn strip_forall_core(formula: &Formula) -> &Formula {
    let mut current = formula;
    while let Formula::Forall(_, body) = current {
        current = body;
    }
    current
}

fn dag_has_ancestor(dag: &Dag<'_>, start: usize, target: usize) -> bool {
    let mut stack = vec![start];
    let mut visited = HashSet::new();
    while let Some(index) = stack.pop() {
        if !visited.insert(index) {
            continue;
        }
        if index == target {
            return true;
        }
        for parent in &dag.nodes[index].parents {
            if let Some(parent_index) = dag.by_name.get(parent.name) {
                stack.push(*parent_index);
            }
        }
    }
    false
}

fn normalize_quantified_cnf(
    formula: &Formula,
    limits: VerificationLimits,
) -> Result<Formula, KernelVerdict> {
    let normalized = to_nnf(formula);
    if formula_size(&normalized) > limits.max_formula_nodes {
        return Err(KernelVerdict::Inconclusive(
            "quantified CNF normalization exceeded strict formula-size limit".into(),
        ));
    }
    let mut next_var = max_formula_var(&normalized).saturating_add(1);
    let mut steps = 0;
    let (prefix, matrix) = prenex_quantified_cnf(
        &normalized,
        &mut next_var,
        &mut steps,
        limits.max_equivalence_steps,
    )?;
    let mut result = matrix;
    for variable in prefix.into_iter().rev() {
        result = Formula::forall(variable, result);
    }
    Ok(result)
}

fn prenex_quantified_cnf(
    formula: &Formula,
    next_var: &mut VarId,
    steps: &mut usize,
    step_limit: usize,
) -> Result<(Vec<VarId>, Formula), KernelVerdict> {
    *steps += 1;
    if *steps > step_limit {
        return Err(KernelVerdict::Inconclusive(
            "quantified CNF normalization exceeded strict matching-step limit".into(),
        ));
    }
    match formula {
        Formula::Forall(variable, body) => {
            let (mut prefix, matrix) = prenex_quantified_cnf(body, next_var, steps, step_limit)?;
            prefix.insert(0, *variable);
            Ok((prefix, matrix))
        }
        Formula::Exists(_, _) => Err(KernelVerdict::Inconclusive(
            "quantified CNF source contains an uneliminated existential".into(),
        )),
        Formula::And(parts) | Formula::Or(parts) => {
            let is_conjunction = matches!(formula, Formula::And(_));
            let mut prefix = Vec::new();
            let mut matrices = Vec::with_capacity(parts.len());
            for part in parts {
                let (part_prefix, mut matrix) =
                    prenex_quantified_cnf(part, next_var, steps, step_limit)?;
                let mut fresh_prefix = Vec::with_capacity(part_prefix.len());
                for variable in part_prefix {
                    let fresh = *next_var;
                    *next_var = next_var.saturating_add(1);
                    matrix = rename_formula_variable(&matrix, variable, fresh);
                    fresh_prefix.push(fresh);
                }
                prefix.extend(fresh_prefix);
                matrices.push(matrix);
            }
            let matrix = if is_conjunction {
                Formula::and(matrices)
            } else {
                Formula::or(matrices)
            };
            Ok((prefix, matrix))
        }
        Formula::Neg(inner) => {
            let (prefix, matrix) = prenex_quantified_cnf(inner, next_var, steps, step_limit)?;
            Ok((prefix, Formula::neg(matrix)))
        }
        Formula::Implies(_, _) | Formula::Iff(_, _) => {
            let normalized = to_nnf(formula);
            prenex_quantified_cnf(&normalized, next_var, steps, step_limit)
        }
        Formula::Atom(_) | Formula::True | Formula::False => Ok((Vec::new(), formula.clone())),
    }
}

fn rename_formula_variable(formula: &Formula, from: VarId, to: VarId) -> Formula {
    fn rename_term(term: &Term, from: VarId, to: VarId) -> Term {
        match term {
            Term::Var(variable) if *variable == from => Term::Var(to),
            Term::Var(variable) => Term::Var(*variable),
            Term::App(symbol, args) => Term::App(
                *symbol,
                args.iter().map(|arg| rename_term(arg, from, to)).collect(),
            ),
        }
    }
    match formula {
        Formula::Atom(Atom::Pred(symbol, args)) => Formula::atom(Atom::Pred(
            *symbol,
            args.iter().map(|arg| rename_term(arg, from, to)).collect(),
        )),
        Formula::Atom(Atom::Eq(left, right)) => Formula::atom(Atom::Eq(
            rename_term(left, from, to),
            rename_term(right, from, to),
        )),
        Formula::Neg(inner) => Formula::neg(rename_formula_variable(inner, from, to)),
        Formula::And(parts) => Formula::and(
            parts
                .iter()
                .map(|part| rename_formula_variable(part, from, to))
                .collect(),
        ),
        Formula::Or(parts) => Formula::or(
            parts
                .iter()
                .map(|part| rename_formula_variable(part, from, to))
                .collect(),
        ),
        Formula::Implies(left, right) => Formula::implies(
            rename_formula_variable(left, from, to),
            rename_formula_variable(right, from, to),
        ),
        Formula::Iff(left, right) => Formula::iff(
            rename_formula_variable(left, from, to),
            rename_formula_variable(right, from, to),
        ),
        Formula::Forall(variable, body) => Formula::forall(
            if *variable == from { to } else { *variable },
            rename_formula_variable(body, from, to),
        ),
        Formula::Exists(variable, body) => Formula::exists(
            if *variable == from { to } else { *variable },
            rename_formula_variable(body, from, to),
        ),
        Formula::True => Formula::True,
        Formula::False => Formula::False,
    }
}

fn core_definition_head(formula: &Formula) -> Option<Atom> {
    let Formula::Atom(Atom::Pred(symbol, args)) = formula else {
        return None;
    };
    if args.iter().all(|term| matches!(term, Term::Var(_))) {
        Some(Atom::Pred(*symbol, args.clone()))
    } else {
        None
    }
}

fn validate_definition_dependencies(
    source: &Formula,
    definitions: &[CoreDefinition],
) -> Result<(), KernelVerdict> {
    let mut source_symbols = HashSet::new();
    collect_core_predicate_symbols(source, &mut source_symbols);
    let mut definition_indices = HashMap::new();
    for (index, definition) in definitions.iter().enumerate() {
        let Atom::Pred(symbol, _) = definition.head else {
            return Err(KernelVerdict::Inconclusive(
                "CNF definition head is not a predicate".into(),
            ));
        };
        if definition_indices.insert(symbol, index).is_some() {
            return Err(KernelVerdict::Rejected(
                "CNF transformation cites duplicate definition symbols".into(),
            ));
        }
        if !matches!(definition.rhs, Formula::And(_)) && !is_flat_definition_clause(&definition.rhs)
        {
            return Err(KernelVerdict::Inconclusive(
                "CNF transformation extra definitions must name conjunctions or flat clauses"
                    .into(),
            ));
        }
    }

    if definitions.iter().any(|definition| {
        let Atom::Pred(symbol, _) = definition.head else {
            return false;
        };
        source_symbols.contains(&symbol)
    }) {
        return Err(KernelVerdict::Rejected(
            "CNF transformation source contains a cited fresh definition symbol".into(),
        ));
    }

    let mut dependencies = vec![Vec::new(); definitions.len()];
    for (index, definition) in definitions.iter().enumerate() {
        let Atom::Pred(head, _) = definition.head else {
            unreachable!("validated definition head")
        };
        let mut rhs_symbols = HashSet::new();
        collect_core_predicate_symbols(&definition.rhs, &mut rhs_symbols);
        for symbol in rhs_symbols {
            if symbol == head || definition_indices.contains_key(&symbol) {
                if symbol == head {
                    continue;
                }
                let dependency = definition_indices
                    .get(&symbol)
                    .copied()
                    .expect("definition dependency exists");
                dependencies[index].push(dependency);
                continue;
            }
            if !source_symbols.contains(&symbol) {
                return Err(KernelVerdict::Inconclusive(
                    "CNF definition references an uncited fresh predicate".into(),
                ));
            };
        }
    }

    fn visit(index: usize, dependencies: &[Vec<usize>], marks: &mut [u8]) -> bool {
        if marks[index] == 1 {
            return false;
        }
        if marks[index] == 2 {
            return true;
        }
        marks[index] = 1;
        if dependencies[index]
            .iter()
            .all(|dependency| visit(*dependency, dependencies, marks))
        {
            marks[index] = 2;
            true
        } else {
            false
        }
    }

    let mut marks = vec![0; definitions.len()];
    if (0..definitions.len()).all(|index| visit(index, &dependencies, &mut marks)) {
        Ok(())
    } else {
        Err(KernelVerdict::Rejected(
            "CNF definition dependency graph contains a cycle".into(),
        ))
    }
}

fn collect_core_predicate_symbols(formula: &Formula, symbols: &mut HashSet<mrs_core::SymbolId>) {
    match formula {
        Formula::Atom(Atom::Pred(symbol, _)) => {
            symbols.insert(*symbol);
        }
        Formula::Atom(Atom::Eq(_, _)) | Formula::True | Formula::False => {}
        Formula::Neg(inner) | Formula::Forall(_, inner) | Formula::Exists(_, inner) => {
            collect_core_predicate_symbols(inner, symbols)
        }
        Formula::And(parts) | Formula::Or(parts) => {
            for part in parts {
                collect_core_predicate_symbols(part, symbols);
            }
        }
        Formula::Implies(left, right) | Formula::Iff(left, right) => {
            collect_core_predicate_symbols(left, symbols);
            collect_core_predicate_symbols(right, symbols);
        }
    }
}

fn definition_direction_clauses(
    definition: &CoreDefinition,
    limits: VerificationLimits,
) -> Option<Vec<Vec<Literal>>> {
    let definition_formula = Formula::iff(
        Formula::atom(definition.head.clone()),
        definition.rhs.clone(),
    );
    let mut clauses = Vec::new();
    if !cnf_expand(&to_nnf(&definition_formula), &mut clauses, limits) {
        None
    } else {
        Some(clauses)
    }
}

fn replace_definition_subformulas(
    source: &Formula,
    definitions: &[CoreDefinition],
    limits: VerificationLimits,
) -> Option<Formula> {
    let mut current = source.clone();
    let mut steps = 0;
    loop {
        if steps >= limits.max_rewrite_steps {
            return None;
        }
        steps += 1;
        let mut changed = false;
        // Hoist quantifiers that bind nothing outside their conjunct /
        // disjunct first: the prover generalises definition bodies across
        // quantifier boundaries (`(A & ![x]B(x))` with body `(A & B(x))`,
        // e.g. SYO606+1 `def_nc5_11`), so the source block only matches
        // after the vacuous placement is floated outward.
        let (moved, moved_changed) = pull_vacuous_quantifiers_once(&current);
        current = moved;
        changed |= moved_changed;
        // Canonical sites first: apply identity matches for every
        // definition before falling back to permuting matches, so
        // symmetric blocks are claimed by their own definitions.
        for definition in definitions {
            let (next, replaced) = replace_one_definition(&current, definition, true);
            current = next;
            changed |= replaced;
        }
        for definition in definitions {
            let (next, replaced) = replace_one_definition(&current, definition, false);
            current = next;
            changed |= replaced;
        }
        if !changed {
            return Some(current);
        }
    }
}

/// Floats one level of vacuously-placed quantifiers outward:
/// `(A & Qx.B(x)) ≡ Qx.(A & B(x))` (likewise `|`) when `x ∉ FV(A)`.
/// Recurses bottom-up so nested placements surface in one pass; the
/// caller iterates to a fixpoint. All rewrites are logical equivalences.
fn pull_vacuous_quantifiers_once(formula: &Formula) -> (Formula, bool) {
    match formula {
        Formula::And(parts) | Formula::Or(parts) => {
            let is_conjunction = matches!(formula, Formula::And(_));
            let mut rewritten: Vec<Formula> = Vec::with_capacity(parts.len());
            let mut changed = false;
            for part in parts {
                let (part, part_changed) = pull_vacuous_quantifiers_once(part);
                changed |= part_changed;
                rewritten.push(part);
            }
            // Hoist at most one quantifier per pass; the fixpoint loop in
            // the caller handles the rest.
            for index in 0..rewritten.len() {
                let (variable, body, universal) = match &rewritten[index] {
                    Formula::Forall(variable, body) => (*variable, body.as_ref(), true),
                    Formula::Exists(variable, body) => (*variable, body.as_ref(), false),
                    _ => continue,
                };
                let mut others_free = HashSet::new();
                for (other_index, other) in rewritten.iter().enumerate() {
                    if other_index != index {
                        others_free.extend(other.free_vars());
                    }
                }
                if others_free.contains(&variable) {
                    continue;
                }
                let mut hoisted: Vec<Formula> = Vec::with_capacity(rewritten.len());
                for (other_index, other) in rewritten.iter().enumerate() {
                    if other_index != index {
                        hoisted.push(other.clone());
                    }
                }
                hoisted.push(body.clone());
                let combined = if is_conjunction {
                    Formula::And(hoisted)
                } else {
                    Formula::Or(hoisted)
                };
                let lifted = if universal {
                    Formula::forall(variable, combined)
                } else {
                    Formula::exists(variable, combined)
                };
                return (lifted, true);
            }
            let rebuilt = if is_conjunction {
                Formula::And(rewritten)
            } else {
                Formula::Or(rewritten)
            };
            (rebuilt, changed)
        }
        Formula::Neg(inner) => {
            let (inner, changed) = pull_vacuous_quantifiers_once(inner);
            (Formula::neg(inner), changed)
        }
        Formula::Implies(left, right) => {
            let (left, left_changed) = pull_vacuous_quantifiers_once(left);
            let (right, right_changed) = pull_vacuous_quantifiers_once(right);
            (Formula::implies(left, right), left_changed || right_changed)
        }
        Formula::Iff(left, right) => {
            let (left, left_changed) = pull_vacuous_quantifiers_once(left);
            let (right, right_changed) = pull_vacuous_quantifiers_once(right);
            (Formula::iff(left, right), left_changed || right_changed)
        }
        Formula::Forall(variable, body) => {
            let (body, changed) = pull_vacuous_quantifiers_once(body);
            (Formula::forall(*variable, body), changed)
        }
        Formula::Exists(variable, body) => {
            let (body, changed) = pull_vacuous_quantifiers_once(body);
            (Formula::exists(*variable, body), changed)
        }
        _ => (formula.clone(), false),
    }
}

fn replace_one_definition(
    source: &Formula,
    definition: &CoreDefinition,
    identity_only: bool,
) -> (Formula, bool) {
    let (transformed, replaced) = match source {
        Formula::Atom(_) | Formula::True | Formula::False => (source.clone(), false),
        Formula::Neg(inner) => {
            let (inner, replaced) = replace_one_definition(inner, definition, identity_only);
            (Formula::neg(inner), replaced)
        }
        Formula::And(parts) => {
            let mut replaced = false;
            let parts = parts
                .iter()
                .map(|part| {
                    let (part, part_replaced) =
                        replace_one_definition(part, definition, identity_only);
                    replaced |= part_replaced;
                    part
                })
                .collect();
            (Formula::And(parts), replaced)
        }
        Formula::Or(parts) => {
            let mut replaced = false;
            let parts = parts
                .iter()
                .map(|part| {
                    let (part, part_replaced) =
                        replace_one_definition(part, definition, identity_only);
                    replaced |= part_replaced;
                    part
                })
                .collect();
            (Formula::Or(parts), replaced)
        }
        Formula::Implies(left, right) => {
            let (left, left_replaced) = replace_one_definition(left, definition, identity_only);
            let (right, right_replaced) = replace_one_definition(right, definition, identity_only);
            (
                Formula::implies(left, right),
                left_replaced || right_replaced,
            )
        }
        Formula::Iff(left, right) => {
            let (left, left_replaced) = replace_one_definition(left, definition, identity_only);
            let (right, right_replaced) = replace_one_definition(right, definition, identity_only);
            (Formula::iff(left, right), left_replaced || right_replaced)
        }
        Formula::Forall(var, body) => {
            let (body, replaced) = replace_one_definition(body, definition, identity_only);
            (Formula::forall(*var, body), replaced)
        }
        Formula::Exists(var, body) => {
            let (body, replaced) = replace_one_definition(body, definition, identity_only);
            (Formula::exists(*var, body), replaced)
        }
    };

    let mut mapping = HashMap::new();
    if match_core_formula(&definition.rhs, &transformed, &mut mapping)
        // In identity mode only accept matches that keep every pattern
        // variable fixed: the prover introduces each definition at its
        // canonical block, so canonical sites must win contested blocks
        // (e.g. GEO125+1's two definitions over symmetric blocks).
        && (!identity_only || is_identity_mapping(&mapping))
        && let Some(head) = apply_core_definition_head(&definition.head, &mapping)
    {
        (Formula::atom(head), true)
    } else {
        (transformed, replaced)
    }
}

/// Whether every binding maps a pattern variable to itself. Such matches
/// identify a definition's canonical block (same variable names as the
/// rendered definition), as opposed to permuted cross-matches.
fn is_identity_mapping(mapping: &HashMap<VarId, Term>) -> bool {
    mapping
        .iter()
        .all(|(variable, term)| matches!(term, Term::Var(other) if other == variable))
}

fn apply_core_definition_head(head: &Atom, mapping: &HashMap<VarId, Term>) -> Option<Atom> {
    fn apply_term(term: &Term, mapping: &HashMap<VarId, Term>) -> Option<Term> {
        match term {
            Term::Var(var) => mapping.get(var).cloned(),
            Term::App(symbol, args) => Some(Term::App(
                *symbol,
                args.iter()
                    .map(|arg| apply_term(arg, mapping))
                    .collect::<Option<Vec<_>>>()?,
            )),
        }
    }
    match head {
        Atom::Pred(symbol, args) => Some(Atom::Pred(
            *symbol,
            args.iter()
                .map(|arg| apply_term(arg, mapping))
                .collect::<Option<Vec<_>>>()?,
        )),
        Atom::Eq(_, _) => None,
    }
}

fn match_core_formula(
    pattern: &Formula,
    target: &Formula,
    mapping: &mut HashMap<VarId, Term>,
) -> bool {
    match (pattern, target) {
        (Formula::Atom(pattern), Formula::Atom(target)) => {
            match_core_atom(pattern, target, mapping)
        }
        (Formula::Neg(pattern), Formula::Neg(target)) => {
            match_core_formula(pattern, target, mapping)
        }
        (Formula::And(_), Formula::And(_)) | (Formula::Or(_), Formula::Or(_)) => {
            let connective = if matches!(pattern, Formula::And(_)) {
                CoreConnective::And
            } else {
                CoreConnective::Or
            };
            let patterns = flatten_core_parts(pattern, connective);
            let targets = flatten_core_parts(target, connective);
            match_core_multiset(&patterns, &targets, mapping)
        }
        (
            Formula::Implies(pattern_left, pattern_right),
            Formula::Implies(target_left, target_right),
        )
        | (Formula::Iff(pattern_left, pattern_right), Formula::Iff(target_left, target_right)) => {
            match_core_formula(pattern_left, target_left, mapping)
                && match_core_formula(pattern_right, target_right, mapping)
        }
        (Formula::Forall(pattern_var, pattern_body), Formula::Forall(target_var, target_body))
        | (Formula::Exists(pattern_var, pattern_body), Formula::Exists(target_var, target_body)) => {
            // Bound variables are alpha-equivalent, not substitution
            // variables. Temporarily record their correspondence while
            // matching the body and restore any outer mapping afterwards.
            let previous = mapping.insert(*pattern_var, Term::Var(*target_var));
            let matched = match_core_formula(pattern_body, target_body, mapping);
            match previous {
                Some(term) => {
                    mapping.insert(*pattern_var, term);
                }
                None => {
                    mapping.remove(pattern_var);
                }
            }
            matched
        }
        (Formula::True, Formula::True) | (Formula::False, Formula::False) => true,
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum CoreConnective {
    And,
    Or,
}

fn flatten_core_parts(formula: &Formula, connective: CoreConnective) -> Vec<&Formula> {
    let mut output = Vec::new();
    let mut pending = vec![formula];
    while let Some(current) = pending.pop() {
        let matching = match (connective, current) {
            (CoreConnective::And, Formula::And(parts))
            | (CoreConnective::Or, Formula::Or(parts)) => Some(parts),
            _ => None,
        };
        if let Some(parts) = matching {
            pending.extend(parts.iter());
        } else {
            output.push(current);
        }
    }
    output
}

fn match_core_multiset(
    patterns: &[&Formula],
    targets: &[&Formula],
    mapping: &mut HashMap<VarId, Term>,
) -> bool {
    if patterns.len() != targets.len() {
        return false;
    }
    fn visit(
        patterns: &[&Formula],
        targets: &[&Formula],
        index: usize,
        used: &mut [bool],
        mapping: &mut HashMap<VarId, Term>,
    ) -> bool {
        if index == patterns.len() {
            return true;
        }
        for target_idx in 0..targets.len() {
            if used[target_idx] {
                continue;
            }
            let mut next_mapping = mapping.clone();
            if match_core_formula(patterns[index], targets[target_idx], &mut next_mapping) {
                used[target_idx] = true;
                if visit(patterns, targets, index + 1, used, &mut next_mapping) {
                    *mapping = next_mapping;
                    return true;
                }
                used[target_idx] = false;
            }
        }
        false
    }
    visit(
        patterns,
        targets,
        0,
        &mut vec![false; targets.len()],
        mapping,
    )
}

fn match_core_atom(pattern: &Atom, target: &Atom, mapping: &mut HashMap<VarId, Term>) -> bool {
    match (pattern, target) {
        (Atom::Pred(pattern_symbol, pattern_args), Atom::Pred(target_symbol, target_args)) => {
            pattern_symbol == target_symbol
                && pattern_args.len() == target_args.len()
                && pattern_args
                    .iter()
                    .zip(target_args)
                    .all(|(pattern, target)| match_core_term(pattern, target, mapping))
        }
        (Atom::Eq(pattern_left, pattern_right), Atom::Eq(target_left, target_right)) => {
            match_core_term(pattern_left, target_left, mapping)
                && match_core_term(pattern_right, target_right, mapping)
        }
        _ => false,
    }
}

fn match_core_term(pattern: &Term, target: &Term, mapping: &mut HashMap<VarId, Term>) -> bool {
    match (pattern, target) {
        (Term::Var(pattern), target) => {
            if let Some(mapped) = mapping.get(pattern) {
                mapped == target
            } else {
                mapping.insert(*pattern, target.clone());
                true
            }
        }
        (Term::App(pattern_symbol, pattern_args), Term::App(target_symbol, target_args)) => {
            pattern_symbol == target_symbol
                && pattern_args.len() == target_args.len()
                && pattern_args
                    .iter()
                    .zip(target_args)
                    .all(|(pattern, target)| match_core_term(pattern, target, mapping))
        }
        _ => false,
    }
}

fn cnf_expand(
    formula: &Formula,
    output: &mut Vec<Vec<Literal>>,
    limits: VerificationLimits,
) -> bool {
    match formula {
        Formula::And(parts) => {
            for part in parts {
                if !cnf_expand(part, output, limits) {
                    return false;
                }
            }
            true
        }
        Formula::Or(parts) => {
            let mut clauses = vec![Vec::<Literal>::new()];
            for part in parts {
                let mut child = Vec::new();
                if !cnf_expand(part, &mut child, limits) {
                    return false;
                }
                let mut next = Vec::new();
                for left in &clauses {
                    for right in &child {
                        let mut merged = left.clone();
                        merged.extend(right.clone());
                        if merged.len() > limits.max_clause_literals {
                            return false;
                        }
                        next.push(merged);
                        if next.len() > limits.max_nodes {
                            return false;
                        }
                    }
                }
                clauses = next;
            }
            output.extend(clauses);
            true
        }
        Formula::Atom(atom) => {
            output.push(vec![Literal {
                positive: true,
                atom: atom.clone(),
            }]);
            true
        }
        Formula::Neg(inner) => match inner.as_ref() {
            Formula::Atom(atom) => {
                output.push(vec![Literal {
                    positive: false,
                    atom: atom.clone(),
                }]);
                true
            }
            _ => false,
        },
        Formula::False => {
            output.push(Vec::new());
            true
        }
        Formula::True => true,
        Formula::Forall(_, inner) => cnf_expand(inner, output, limits),
        Formula::Implies(_, _) | Formula::Iff(_, _) | Formula::Exists(_, _) => false,
    }
}

fn definition_head<'a>(formula: &'a FOFFormula<'a>, symbol: &str) -> Option<&'a [FOFTerm<'a>]> {
    match formula {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(name, args)) if name.as_str() == symbol => {
            Some(args)
        }
        _ => None,
    }
}

fn strip_forall_fof<'a>(formula: &'a FOFFormula<'a>) -> &'a FOFFormula<'a> {
    match formula {
        FOFFormula::Parens(inner) => strip_forall_fof(inner),
        FOFFormula::Quantified {
            quantifier: Quantifier::Forall,
            formula,
            ..
        } => strip_forall_fof(formula),
        _ => formula,
    }
}

fn collect_free_variable_names(formula: &FOFFormula<'_>, out: &mut HashSet<String>) {
    fn walk(formula: &FOFFormula<'_>, bound: &mut HashSet<String>, out: &mut HashSet<String>) {
        match formula {
            FOFFormula::Atomic(atom) => match atom {
                FOFAtomicFormula::Plain(_, terms)
                | FOFAtomicFormula::Defined(_, terms)
                | FOFAtomicFormula::System(_, terms) => {
                    for term in terms {
                        collect_free_term_names(term, bound, out);
                    }
                }
                FOFAtomicFormula::True | FOFAtomicFormula::False => {}
            },
            FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => walk(inner, bound, out),
            FOFFormula::Binary { left, right, .. } => {
                walk(left, bound, out);
                walk(right, bound, out);
            }
            FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
                collect_free_term_names(left, bound, out);
                collect_free_term_names(right, bound, out);
            }
            FOFFormula::Quantified {
                variables, formula, ..
            } => {
                let mut inserted = Vec::new();
                for variable in variables {
                    if bound.insert((*variable).to_string()) {
                        inserted.push(*variable);
                    }
                }
                walk(formula, bound, out);
                for variable in inserted {
                    bound.remove(variable);
                }
            }
        }
    }
    walk(formula, &mut HashSet::new(), out);
}

fn collect_free_term_names(term: &FOFTerm<'_>, bound: &HashSet<String>, out: &mut HashSet<String>) {
    match term {
        FOFTerm::Variable(variable) => {
            if !bound.contains(*variable) {
                out.insert((*variable).to_string());
            }
        }
        FOFTerm::Function(_, args)
        | FOFTerm::DefinedFunction(_, args)
        | FOFTerm::SystemFunction(_, args) => {
            for arg in args {
                collect_free_term_names(arg, bound, out);
            }
        }
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => {}
    }
}

fn collect_term_variable_names(term: &FOFTerm<'_>, out: &mut HashSet<String>) {
    match term {
        FOFTerm::Variable(variable) => {
            out.insert((*variable).to_string());
        }
        FOFTerm::Function(_, args)
        | FOFTerm::DefinedFunction(_, args)
        | FOFTerm::SystemFunction(_, args) => {
            for arg in args {
                collect_term_variable_names(arg, out);
            }
        }
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => {}
    }
}

struct Node<'a> {
    name: &'a str,
    role: FormulaRole,
    parents: Vec<ParentRef<'a>>,
    rule: Option<&'a str>,
    formula: &'a AnnotatedFormula<'a>,
    is_false: bool,
}

struct Dag<'a> {
    nodes: Vec<Node<'a>>,
    by_name: HashMap<&'a str, usize>,
    topo: Vec<usize>,
}

fn build_dag<'a>(
    proof: &'a mrs_tptp::TPTPProblem<'a>,
    limits: VerificationLimits,
) -> Result<Dag<'a>, KernelVerdict> {
    if proof.formulas.len() > limits.max_nodes {
        return Err(KernelVerdict::Inconclusive(format!(
            "proof has {} formulas, exceeding limit {}",
            proof.formulas.len(),
            limits.max_nodes
        )));
    }

    let mut nodes = Vec::with_capacity(proof.formulas.len());
    let mut by_name = HashMap::with_capacity(proof.formulas.len());
    for formula in &proof.formulas {
        if formula.role() == FormulaRole::Type {
            continue;
        }
        if !formula.is_fof() && !formula.is_cnf() {
            return Err(KernelVerdict::Inconclusive(format!(
                "proof node `{}` uses an unsupported dialect",
                formula.name()
            )));
        }
        let annotations = formula.annotations();
        let parents = annotations.map(|a| a.parent_refs()).unwrap_or_default();
        let rule = annotations.and_then(|a| a.inference_rule());
        let node = Node {
            name: formula.name(),
            role: formula.role(),
            parents,
            rule,
            formula,
            is_false: is_false_formula(formula),
        };
        if by_name.insert(node.name, nodes.len()).is_some() {
            return Err(KernelVerdict::Rejected(format!(
                "duplicate proof formula name `{}`",
                node.name
            )));
        }
        nodes.push(node);
    }

    if nodes.is_empty() {
        return Err(KernelVerdict::Inconclusive(
            "proof contains no FOF/CNF nodes".into(),
        ));
    }

    for node in &nodes {
        for parent in &node.parents {
            if !by_name.contains_key(parent.name) {
                return Err(KernelVerdict::Rejected(format!(
                    "node `{}` references unknown parent `{}`",
                    node.name, parent.name
                )));
            }
        }
    }

    let topo = topo_sort(&nodes, &by_name)?;
    let mut used_as_parent = HashSet::new();
    for node in &nodes {
        for parent in &node.parents {
            used_as_parent.insert(parent.name);
        }
    }
    let roots: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.is_false && !used_as_parent.contains(node.name))
        .map(|(idx, _)| idx)
        .collect();
    if roots.is_empty() {
        return Err(KernelVerdict::Rejected(
            "proof has no unparented `$false` root".into(),
        ));
    }

    // Nodes outside the root derivation are still validated individually
    // below, but they no longer invalidate the proof on their own:
    // unreachable nodes cannot affect the validity of the refutation
    // (typical cause: the exporter emits unused input axioms, e.g.
    // GEO127+1's `meet_defn`/`finish_point_defn` leaves). A forged
    // unreachable step is still rejected by its own verification.
    Ok(Dag {
        nodes,
        by_name,
        topo,
    })
}

fn topo_sort<'a>(
    nodes: &[Node<'a>],
    by_name: &HashMap<&'a str, usize>,
) -> Result<Vec<usize>, KernelVerdict> {
    let mut indegree = vec![0usize; nodes.len()];
    let mut children = vec![Vec::<usize>::new(); nodes.len()];
    for (idx, node) in nodes.iter().enumerate() {
        for parent in &node.parents {
            let parent_idx = *by_name.get(parent.name).expect("validated parent");
            indegree[idx] += 1;
            children[parent_idx].push(idx);
        }
    }
    let mut ready = std::collections::BinaryHeap::new();
    for (idx, &degree) in indegree.iter().enumerate() {
        if degree == 0 {
            ready.push(std::cmp::Reverse(idx));
        }
    }
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(std::cmp::Reverse(idx)) = ready.pop() {
        order.push(idx);
        for &child in &children[idx] {
            indegree[child] -= 1;
            if indegree[child] == 0 {
                ready.push(std::cmp::Reverse(child));
            }
        }
    }
    if order.len() != nodes.len() {
        return Err(KernelVerdict::Rejected(
            "cycle in proof parent graph".into(),
        ));
    }
    Ok(order)
}

fn parent_formulas<'a>(
    dag: &Dag<'a>,
    node: &Node<'a>,
    formulas: &HashMap<usize, Formula>,
    apply_pedigree_negation: bool,
) -> Result<Vec<Formula>, KernelVerdict> {
    let mut out = Vec::with_capacity(node.parents.len());
    for parent in &node.parents {
        let idx = *dag.by_name.get(parent.name).expect("validated parent");
        let mut formula = formulas.get(&idx).expect("lowered parent").clone();
        if apply_pedigree_negation && parent.negated {
            formula = Formula::neg(formula);
        }
        out.push(formula);
    }
    Ok(out)
}

fn verify_leaf<'a>(
    node: &Node<'a>,
    problem: &mrs_tptp::TPTPProblem<'_>,
    expected_source: Option<&str>,
    symbols: &mut SymbolTable,
    limits: VerificationLimits,
) -> Result<(), KernelVerdict> {
    let annotation = node
        .formula
        .annotations()
        .and_then(|annotations| annotations.file_source())
        .ok_or_else(|| KernelVerdict::Inconclusive("leaf has no file provenance".into()))?;
    if annotation.1 == "unknown" {
        return Err(KernelVerdict::Inconclusive(format!(
            "leaf `{}` has anonymous provenance",
            node.name
        )));
    }
    if let Some(expected_source) = expected_source
        && annotation.0 != expected_source
    {
        return Err(KernelVerdict::Rejected(format!(
            "leaf `{}` cites `{}` instead of proof source `{expected_source}`",
            node.name, annotation.0
        )));
    }
    // Existential over same-named problem formulas: includes may define
    // one name twice (see above), so a leaf is substantiated when ANY of
    // them matches in role and formula.
    let proof_formula = lower_annotated(symbols, node.formula, limits)?;
    let mut named_match = false;
    let mut role_match = false;
    for expected in problem
        .formulas
        .iter()
        .filter(|formula| formula.name() == annotation.1)
    {
        named_match = true;
        if !roles_compatible(node.role, expected.role()) {
            continue;
        }
        role_match = true;
        let expected_formula = lower_annotated(symbols, expected, limits)?;
        if alpha_equiv(&proof_formula, &expected_formula) {
            return Ok(());
        }
    }
    if !named_match {
        let mut provenance_match = false;
        for expected in problem.formulas.iter().filter(|formula| {
            formula
                .annotations()
                .and_then(|a| a.file_source())
                .map(|s| s.1 == annotation.1)
                .unwrap_or(false)
        }) {
            provenance_match = true;
            if !roles_compatible(node.role, expected.role()) {
                continue;
            }
            role_match = true;
            let expected_formula = lower_annotated(symbols, expected, limits)?;
            if alpha_equiv(&proof_formula, &expected_formula) {
                return Ok(());
            }
        }
        if !provenance_match {
            return Err(KernelVerdict::Rejected(format!(
                "leaf `{}` references missing problem formula `{}`",
                node.name, annotation.1
            )));
        }
    }
    if !role_match {
        return Err(KernelVerdict::Rejected(format!(
            "leaf `{}` role `{}` is incompatible with problem role `{}`",
            node.name,
            node.role.as_str(),
            annotation.1
        )));
    }
    // Keep the argument in the error so callers can diagnose a forged
    // provenance leaf without exposing the full formula in SZS output.
    Err(KernelVerdict::Rejected(format!(
        "leaf `{}` does not match problem formula `{}`",
        node.name, annotation.1
    )))
}

fn roles_compatible(proof: FormulaRole, problem: FormulaRole) -> bool {
    if proof == FormulaRole::Plain || proof.is_premise() {
        problem.is_premise()
    } else {
        proof == problem
    }
}

fn verify_negated_conjecture(
    node: &Node<'_>,
    parents: &[Formula],
    parent_role: Option<FormulaRole>,
    conclusion: &Formula,
) -> KernelVerdict {
    if node.role != FormulaRole::NegatedConjecture || node.status() != Some("cth") {
        return KernelVerdict::Rejected(format!(
            "node `{}` is not a correctly annotated negated conjecture",
            node.name
        ));
    }
    if parents.len() != 1 || parent_role != Some(FormulaRole::Conjecture) {
        return KernelVerdict::Rejected(format!(
            "node `{}` must have exactly one conjecture parent",
            node.name
        ));
    }
    let expected = to_nnf(&Formula::neg(parents[0].clone()));
    let actual = to_nnf(conclusion);
    if alpha_equiv(&expected, &actual) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected(format!(
            "node `{}` is not the negation of its conjecture",
            node.name
        ))
    }
}

fn verify_nnf(parents: &[Formula], conclusion: &Formula) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("NNF rule must have one parent".into());
    }
    if alpha_equiv(&to_nnf(&parents[0]), conclusion) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("conclusion is not the parent's NNF".into())
    }
}

fn verify_alpha_identity(parents: &[Formula], conclusion: &Formula) -> KernelVerdict {
    if parents.is_empty() {
        return KernelVerdict::Rejected("identity rule must have at least one parent".into());
    }
    if parents.iter().all(|p| alpha_equiv(p, conclusion)) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("conclusion is not alpha-equivalent to parent".into())
    }
}

fn verify_formula_equivalence(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("formula equivalence must have one parent".into());
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "formula equivalence exceeded strict formula-size limit".into(),
        );
    }
    let mut steps = 0;
    let left = match canonicalize_equivalence(&to_nnf(&parents[0]), &mut steps, limits) {
        Some(formula) => formula,
        None => {
            return KernelVerdict::Inconclusive(
                "formula equivalence exceeded strict matching-step limit".into(),
            );
        }
    };
    let right = match canonicalize_equivalence(&to_nnf(conclusion), &mut steps, limits) {
        Some(formula) => formula,
        None => {
            return KernelVerdict::Inconclusive(
                "formula equivalence exceeded strict matching-step limit".into(),
            );
        }
    };
    if alpha_equiv(&left, &right) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("conclusion is not equivalent to its parent".into())
    }
}

fn apply_subst_term_non_chaining(term: &Term, map: &HashMap<VarId, Term>) -> Term {
    match term {
        Term::Var(v) => map.get(v).cloned().unwrap_or(Term::Var(*v)),
        Term::App(sym, args) => Term::App(
            *sym,
            args.iter()
                .map(|arg| apply_subst_term_non_chaining(arg, map))
                .collect(),
        ),
    }
}

fn apply_subst_atom_non_chaining(atom: &Atom, map: &HashMap<VarId, Term>) -> Atom {
    match atom {
        Atom::Pred(sym, args) => Atom::Pred(
            *sym,
            args.iter()
                .map(|arg| apply_subst_term_non_chaining(arg, map))
                .collect(),
        ),
        Atom::Eq(l, r) => Atom::Eq(
            apply_subst_term_non_chaining(l, map),
            apply_subst_term_non_chaining(r, map),
        ),
    }
}

fn apply_subst_formula_non_chaining(formula: &Formula, map: &HashMap<VarId, Term>) -> Formula {
    match formula {
        Formula::Atom(a) => Formula::Atom(apply_subst_atom_non_chaining(a, map)),
        Formula::Neg(f) => Formula::Neg(Box::new(apply_subst_formula_non_chaining(f, map))),
        Formula::And(fs) => Formula::And(
            fs.iter()
                .map(|f| apply_subst_formula_non_chaining(f, map))
                .collect(),
        ),
        Formula::Or(fs) => Formula::Or(
            fs.iter()
                .map(|f| apply_subst_formula_non_chaining(f, map))
                .collect(),
        ),
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(apply_subst_formula_non_chaining(a, map)),
            Box::new(apply_subst_formula_non_chaining(b, map)),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(apply_subst_formula_non_chaining(a, map)),
            Box::new(apply_subst_formula_non_chaining(b, map)),
        ),
        Formula::Forall(v, body) => {
            if map.contains_key(v) {
                Formula::Forall(*v, body.clone())
            } else {
                Formula::Forall(*v, Box::new(apply_subst_formula_non_chaining(body, map)))
            }
        }
        Formula::Exists(v, body) => {
            if map.contains_key(v) {
                Formula::Exists(*v, body.clone())
            } else {
                Formula::Exists(*v, Box::new(apply_subst_formula_non_chaining(body, map)))
            }
        }
        Formula::True => Formula::True,
        Formula::False => Formula::False,
    }
}

fn verify_instantiation(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("instantiate must have one parent".into());
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "instantiate exceeded strict formula-size limit".into(),
        );
    }
    // Clausal multiset matching: if both parent and conclusion are clauses of the same length,
    // verify that conclusion is a substitution instance of parent modulo literal permutation.
    if let (Some(parent_clause), Some(conclusion_clause)) = (
        clause_from_formula(&parents[0], limits),
        clause_from_formula(conclusion, limits),
    ) && parent_clause.len() == conclusion_clause.len()
    {
        let mut steps = 0;
        match clause_subsumes(
            &parent_clause,
            &conclusion_clause,
            &mut steps,
            limits.max_subsumption_steps,
        ) {
            Ok(true) => return KernelVerdict::Certified,
            Ok(false) => {}
            Err(()) => {
                return KernelVerdict::Inconclusive(
                    "instantiate exceeded strict matching-step limit".into(),
                );
            }
        }
    }
    let (_, parent_body) = leading_forall_core(&parents[0]);
    let (_, target_body) = leading_forall_core(conclusion);
    let mut substitution = HashMap::new();
    let mut bound = HashMap::new();
    let mut steps = 0;
    if !match_universal_instance(
        parent_body,
        target_body,
        &mut substitution,
        &mut bound,
        &mut steps,
        limits,
    ) {
        if steps >= limits.max_equivalence_steps {
            return KernelVerdict::Inconclusive(
                "instantiate exceeded strict matching-step limit".into(),
            );
        }
        return KernelVerdict::Rejected("instantiate conclusion is not a parent instance".into());
    }
    let instantiated_parent = apply_subst_formula_non_chaining(parent_body, &substitution);
    if alpha_equiv(&instantiated_parent, target_body) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("instantiate conclusion is not a parent instance".into())
    }
}

fn substitute_definition_rhs(
    body: &Formula,
    params: &[VarId],
    args: &[Term],
    fresh: &Cell<VarId>,
) -> Option<Formula> {
    let mut map = HashMap::new();
    for (param, arg) in params.iter().copied().zip(args.iter()) {
        if let Some(previous) = map.get(&param) {
            if previous != arg {
                return None;
            }
        } else {
            map.insert(param, arg.clone());
        }
    }
    fn sub_term(term: &Term, map: &HashMap<VarId, Term>) -> Term {
        match term {
            Term::Var(v) => map.get(v).cloned().unwrap_or(Term::Var(*v)),
            Term::App(sym, args) => {
                Term::App(*sym, args.iter().map(|arg| sub_term(arg, map)).collect())
            }
        }
    }
    fn sub_atom(atom: &Atom, map: &HashMap<VarId, Term>) -> Atom {
        match atom {
            Atom::Pred(sym, args) => {
                Atom::Pred(*sym, args.iter().map(|arg| sub_term(arg, map)).collect())
            }
            Atom::Eq(l, r) => Atom::Eq(sub_term(l, map), sub_term(r, map)),
        }
    }
    fn sub_form(f: &Formula, map: &mut HashMap<VarId, Term>, fresh: &Cell<VarId>) -> Formula {
        match f {
            Formula::Atom(atom) => Formula::Atom(sub_atom(atom, map)),
            Formula::Neg(inner) => Formula::neg(sub_form(inner, map, fresh)),
            Formula::And(parts) => {
                Formula::And(parts.iter().map(|p| sub_form(p, map, fresh)).collect())
            }
            Formula::Or(parts) => {
                Formula::Or(parts.iter().map(|p| sub_form(p, map, fresh)).collect())
            }
            Formula::Implies(l, r) => {
                Formula::implies(sub_form(l, map, fresh), sub_form(r, map, fresh))
            }
            Formula::Iff(l, r) => Formula::iff(sub_form(l, map, fresh), sub_form(r, map, fresh)),
            Formula::Forall(v, inner) => {
                let nv = fresh.get();
                fresh.set(nv.saturating_add(1));
                let prev = map.insert(*v, Term::Var(nv));
                let sub_inner = sub_form(inner, map, fresh);
                match prev {
                    Some(old) => {
                        map.insert(*v, old);
                    }
                    None => {
                        map.remove(v);
                    }
                }
                Formula::forall(nv, sub_inner)
            }
            Formula::Exists(v, inner) => {
                let nv = fresh.get();
                fresh.set(nv.saturating_add(1));
                let prev = map.insert(*v, Term::Var(nv));
                let sub_inner = sub_form(inner, map, fresh);
                match prev {
                    Some(old) => {
                        map.insert(*v, old);
                    }
                    None => {
                        map.remove(v);
                    }
                }
                Formula::exists(nv, sub_inner)
            }
            Formula::True | Formula::False => f.clone(),
        }
    }
    Some(sub_form(body, &mut map, fresh))
}

fn unfold_definitions_once(
    formula: &Formula,
    defs: &HashMap<mrs_core::SymbolId, (Vec<VarId>, Formula)>,
    fresh: &Cell<VarId>,
    limits: VerificationLimits,
    steps: &mut usize,
) -> Option<(Formula, bool)> {
    if *steps >= limits.max_rewrite_steps {
        return None;
    }
    *steps += 1;
    match formula {
        Formula::Atom(Atom::Pred(sym, args)) => {
            if let Some((params, body)) = defs.get(sym) {
                if params.len() != args.len() {
                    return None;
                }
                let Some(expanded) = substitute_definition_rhs(body, params, args, fresh) else {
                    return Some((formula.clone(), false));
                };
                Some((expanded, true))
            } else {
                Some((formula.clone(), false))
            }
        }
        Formula::Atom(Atom::Eq(..)) | Formula::True | Formula::False => {
            Some((formula.clone(), false))
        }
        Formula::Neg(inner) => {
            let (sub, changed) = unfold_definitions_once(inner, defs, fresh, limits, steps)?;
            Some((Formula::neg(sub), changed))
        }
        Formula::And(parts) => {
            let mut changed = false;
            let mut sub_parts = Vec::with_capacity(parts.len());
            for part in parts {
                let (sub, c) = unfold_definitions_once(part, defs, fresh, limits, steps)?;
                changed |= c;
                sub_parts.push(sub);
            }
            Some((Formula::And(sub_parts), changed))
        }
        Formula::Or(parts) => {
            let mut changed = false;
            let mut sub_parts = Vec::with_capacity(parts.len());
            for part in parts {
                let (sub, c) = unfold_definitions_once(part, defs, fresh, limits, steps)?;
                changed |= c;
                sub_parts.push(sub);
            }
            Some((Formula::Or(sub_parts), changed))
        }
        Formula::Implies(l, r) => {
            let (sl, cl) = unfold_definitions_once(l, defs, fresh, limits, steps)?;
            let (sr, cr) = unfold_definitions_once(r, defs, fresh, limits, steps)?;
            Some((Formula::implies(sl, sr), cl || cr))
        }
        Formula::Iff(l, r) => {
            let (sl, cl) = unfold_definitions_once(l, defs, fresh, limits, steps)?;
            let (sr, cr) = unfold_definitions_once(r, defs, fresh, limits, steps)?;
            Some((Formula::iff(sl, sr), cl || cr))
        }
        Formula::Forall(v, inner) => {
            let (sub, changed) = unfold_definitions_once(inner, defs, fresh, limits, steps)?;
            Some((Formula::forall(*v, sub), changed))
        }
        Formula::Exists(v, inner) => {
            let (sub, changed) = unfold_definitions_once(inner, defs, fresh, limits, steps)?;
            Some((Formula::exists(*v, sub), changed))
        }
    }
}

fn unfold_definitions_all(
    formula: &Formula,
    defs: &HashMap<mrs_core::SymbolId, (Vec<VarId>, Formula)>,
    fresh: &Cell<VarId>,
    limits: VerificationLimits,
) -> Option<(Formula, bool)> {
    let mut current = formula.clone();
    let mut changed_any = false;
    let mut steps = 0;
    for _ in 0..=defs.len().saturating_add(1) {
        if formula_size(&current) > limits.max_formula_nodes {
            return None;
        }
        let (next, changed) = unfold_definitions_once(&current, defs, fresh, limits, &mut steps)?;
        current = next;
        changed_any |= changed;
        if !changed {
            return Some((current, changed_any));
        }
    }
    None
}

fn verify_definition_renaming(
    parents: &[Formula],
    conclusion: &Formula,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 2 {
        return KernelVerdict::Rejected(
            "definition_renaming requires the source and definition parents".into(),
        );
    }
    if parents
        .iter()
        .any(|formula| formula_size(formula) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "definition_renaming exceeded strict formula-size limit".into(),
        );
    }

    let source = &parents[0];
    let mut definitions = Vec::with_capacity(parents.len() - 1);
    for (definition, parent_idx) in parents[1..].iter().zip(&parent_indices[1..]) {
        if !dag.nodes[*parent_idx]
            .formula
            .annotations()
            .is_some_and(is_introduced_definition)
        {
            return KernelVerdict::Rejected(
                "definition_renaming parents must be introduced definitions".into(),
            );
        }
        let Some(definition) = core_definition(definition) else {
            return KernelVerdict::Inconclusive(
                "definition_renaming parent is not a supported definition".into(),
            );
        };
        definitions.push(definition);
    }

    // 1. Fast path: forward replacement (exact substitution of definition RHS in source)
    let forward_res = replace_definition_subformulas(source, &definitions, limits);
    if let Some(expected) = &forward_res
        && !alpha_equiv(expected, source)
        && alpha_equiv(expected, conclusion)
    {
        return KernelVerdict::Certified;
    }

    // 2. Unfolding fallback: unfold introduced definitions in the conclusion
    // to handle cases where multiple definitions have structurally identical
    // bodies. A valid renaming must actually use at least one fresh definition;
    // otherwise an unchanged source would be accepted as a forged no-op.
    let mut defs_by_symbol = HashMap::new();
    for def in &definitions {
        if let Atom::Pred(sym, args) = &def.head {
            let mut params = Vec::with_capacity(args.len());
            for arg in args {
                let Term::Var(v) = arg else {
                    return KernelVerdict::Inconclusive(
                        "definition_renaming head contains non-variable argument".into(),
                    );
                };
                params.push(*v);
            }
            defs_by_symbol.insert(*sym, (params, def.rhs.clone()));
        }
    }

    let mut conclusion_symbols = HashSet::new();
    collect_core_predicate_symbols(conclusion, &mut conclusion_symbols);
    let uses_definition = defs_by_symbol
        .keys()
        .any(|symbol| conclusion_symbols.contains(symbol));
    if !uses_definition {
        return if forward_res.is_none() {
            KernelVerdict::Inconclusive(
                "definition_renaming exceeded strict matching-step limit".into(),
            )
        } else {
            KernelVerdict::Rejected(
                "definition_renaming conclusion does not use a cited definition".into(),
            )
        };
    }

    let mut max_var = max_formula_var(source).max(max_formula_var(conclusion));
    for def in &definitions {
        max_var = max_var.max(max_formula_var(&def.rhs));
    }
    let fresh = Cell::new(max_var.saturating_add(1));

    let unfolded_conclusion_res =
        unfold_definitions_all(conclusion, &defs_by_symbol, &fresh, limits);

    if let Some((unfolded_conclusion, changed)) = &unfolded_conclusion_res
        && *changed
        && alpha_equiv(source, unfolded_conclusion)
    {
        return KernelVerdict::Certified;
    }

    if forward_res.is_none() || unfolded_conclusion_res.is_none() {
        return KernelVerdict::Inconclusive(
            "definition_renaming exceeded strict matching-step limit".into(),
        );
    }

    KernelVerdict::Rejected(
        "definition_renaming conclusion is not the source with definitions replaced".into(),
    )
}

fn verify_existential_generation(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("existential_gen must have one parent".into());
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "existential_gen exceeded strict formula-size limit".into(),
        );
    }
    let mut steps = 0;
    let mut state = ExistentialGenerationState::default();
    if match_existential_generation(&parents[0], conclusion, &mut state, &mut steps, limits)
        && state.introduced > 0
    {
        KernelVerdict::Certified
    } else if steps >= limits.max_equivalence_steps {
        KernelVerdict::Inconclusive("existential_gen exceeded strict matching-step limit".into())
    } else {
        KernelVerdict::Rejected("existential_gen conclusion is not a parent generalization".into())
    }
}

fn verify_conjunction(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 2 {
        return KernelVerdict::Rejected("conjunction must have at least two parents".into());
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "conjunction exceeded strict formula-size limit".into(),
        );
    }
    let mut conclusion_parts = Vec::new();
    flatten_conjunction(conclusion, &mut conclusion_parts);
    if conclusion_parts.len() != parents.len() {
        return KernelVerdict::Rejected(
            "conjunction conclusion does not contain exactly one part per parent".into(),
        );
    }
    let mut used = vec![false; conclusion_parts.len()];
    let mut steps = 0;
    if match_conjunction_parts(parents, &conclusion_parts, &mut used, 0, &mut steps, limits) {
        KernelVerdict::Certified
    } else if steps >= limits.max_equivalence_steps {
        KernelVerdict::Inconclusive("conjunction exceeded strict matching-step limit".into())
    } else {
        KernelVerdict::Rejected("conjunction conclusion does not match its parents".into())
    }
}

fn verify_split_conjunct(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("split_conjunct must have one parent".into());
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "split_conjunct exceeded strict formula-size limit".into(),
        );
    }
    let mut direct_parts = Vec::new();
    flatten_conjunction(&parents[0], &mut direct_parts);
    for (steps, part) in direct_parts.iter().enumerate() {
        if steps >= limits.max_equivalence_steps {
            return KernelVerdict::Inconclusive(
                "split_conjunct exceeded strict matching-step limit".into(),
            );
        }
        if alpha_equiv(part, conclusion) {
            return KernelVerdict::Certified;
        }
    }
    let mut parent_binders = Vec::new();
    let parent_body = strip_leading_foralls(&parents[0], &mut parent_binders);
    let mut conclusion_binders = Vec::new();
    let conclusion_body = strip_leading_foralls(conclusion, &mut conclusion_binders);
    if parent_binders.len() != conclusion_binders.len() {
        return KernelVerdict::Rejected(
            "split_conjunct conclusion changes the universal prefix".into(),
        );
    }
    let mut parts = Vec::new();
    flatten_conjunction(parent_body, &mut parts);
    for (steps, part) in parts.into_iter().enumerate() {
        if steps >= limits.max_equivalence_steps {
            return KernelVerdict::Inconclusive(
                "split_conjunct exceeded strict matching-step limit".into(),
            );
        }
        let mut parent_wrapped = part.clone();
        for binder in parent_binders.iter().rev() {
            parent_wrapped = Formula::forall(*binder, parent_wrapped);
        }
        if alpha_equiv(&parent_wrapped, conclusion)
            || (parent_binders.is_empty() && alpha_equiv(part, conclusion_body))
        {
            return KernelVerdict::Certified;
        }
    }
    KernelVerdict::Rejected("split_conjunct conclusion is not a parent conjunct".into())
}

fn verify_excluded_middle(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("excluded_middle must have one parent".into());
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "excluded_middle exceeded strict formula-size limit".into(),
        );
    }
    let expected = Formula::or(vec![parents[0].clone(), Formula::neg(parents[0].clone())]);
    if formula_equivalent_with_limit(&expected, conclusion, limits) {
        return KernelVerdict::Certified;
    }
    let mut parts = Vec::new();
    flatten_disjunction(conclusion, &mut parts);
    let parent_negation = Formula::neg(parents[0].clone());
    if parts.len() == 2
        && ((formula_equivalent_with_limit(&parents[0], parts[0], limits)
            && formula_equivalent_with_limit(&parent_negation, parts[1], limits))
            || (formula_equivalent_with_limit(&parents[0], parts[1], limits)
                && formula_equivalent_with_limit(&parent_negation, parts[0], limits)))
    {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("excluded_middle conclusion is not A | ~A".into())
    }
}

fn verify_modus_ponens(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("modus_ponens must have two parents".into());
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "modus_ponens exceeded strict formula-size limit".into(),
        );
    }

    let mut exhausted = false;
    for (implication_index, premise_index) in [(0, 1), (1, 0)] {
        let (_, implication) = leading_forall_core(&parents[implication_index]);
        if !matches!(implication, Formula::Implies(_, _)) {
            continue;
        }
        let target = Formula::implies(parents[premise_index].clone(), conclusion.clone());
        let mut substitution = HashMap::new();
        let mut bound = HashMap::new();
        let mut steps = 0;
        if !match_universal_instance(
            implication,
            &target,
            &mut substitution,
            &mut bound,
            &mut steps,
            limits,
        ) {
            exhausted |= steps >= limits.max_equivalence_steps;
            continue;
        }
        let mut core_substitution = Substitution::new();
        for (var, term) in substitution {
            core_substitution.bind(var, term);
        }
        if alpha_equiv(&core_substitution.apply_formula(implication), &target) {
            return KernelVerdict::Certified;
        }
    }
    if exhausted {
        KernelVerdict::Inconclusive("modus_ponens exceeded strict matching-step limit".into())
    } else {
        KernelVerdict::Rejected("modus_ponens conclusion does not follow from its parents".into())
    }
}

fn verify_contrapositive(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("contrapositive must have two parents".into());
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "contrapositive exceeded strict formula-size limit".into(),
        );
    }
    let Formula::Neg(conclusion_inner) = conclusion else {
        return KernelVerdict::Rejected("contrapositive conclusion is not negated".into());
    };
    let mut exhausted = false;
    for (implication_index, premise_index) in [(0, 1), (1, 0)] {
        let (_, implication) = leading_forall_core(&parents[implication_index]);
        if !matches!(implication, Formula::Implies(_, _)) {
            continue;
        }
        let Formula::Neg(premise_inner) = &parents[premise_index] else {
            continue;
        };
        let target = Formula::implies((**conclusion_inner).clone(), (**premise_inner).clone());
        let mut substitution = HashMap::new();
        let mut bound = HashMap::new();
        let mut steps = 0;
        if !match_universal_instance(
            implication,
            &target,
            &mut substitution,
            &mut bound,
            &mut steps,
            limits,
        ) {
            exhausted |= steps >= limits.max_equivalence_steps;
            continue;
        }
        let mut core_substitution = Substitution::new();
        for (variable, term) in substitution {
            core_substitution.bind(variable, term);
        }
        if alpha_equiv(&core_substitution.apply_formula(implication), &target) {
            return KernelVerdict::Certified;
        }
    }
    if exhausted {
        KernelVerdict::Inconclusive("contrapositive exceeded strict matching-step limit".into())
    } else {
        KernelVerdict::Rejected("contrapositive conclusion does not follow from its parents".into())
    }
}

fn verify_disjunctive_syllogism(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("disjunctive_syllogism must have two parents".into());
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "disjunctive_syllogism exceeded strict formula-size limit".into(),
        );
    }
    let mut steps = 0;
    let Some(disjunction) = canonicalize_equivalence(&to_nnf(&parents[0]), &mut steps, limits)
    else {
        return KernelVerdict::Inconclusive(
            "disjunctive_syllogism exceeded strict matching-step limit".into(),
        );
    };
    let Some(negative) = canonicalize_equivalence(&to_nnf(&parents[1]), &mut steps, limits) else {
        return KernelVerdict::Inconclusive(
            "disjunctive_syllogism exceeded strict matching-step limit".into(),
        );
    };
    let Formula::Neg(removed) = negative else {
        return KernelVerdict::Rejected(
            "disjunctive_syllogism second parent is not negated".into(),
        );
    };
    let mut disjuncts = Vec::new();
    flatten_disjunction(&disjunction, &mut disjuncts);
    if disjuncts.len() < 2 {
        return KernelVerdict::Rejected(
            "disjunctive_syllogism first parent is not a disjunction".into(),
        );
    }
    let Some(conclusion) = canonicalize_equivalence(&to_nnf(conclusion), &mut steps, limits) else {
        return KernelVerdict::Inconclusive(
            "disjunctive_syllogism exceeded strict matching-step limit".into(),
        );
    };
    for removed_index in 0..disjuncts.len() {
        if steps >= limits.max_equivalence_steps {
            return KernelVerdict::Inconclusive(
                "disjunctive_syllogism exceeded strict matching-step limit".into(),
            );
        }
        steps += 1;
        if !alpha_equiv(disjuncts[removed_index], &removed) {
            continue;
        }
        let remaining = disjuncts
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != removed_index)
            .map(|(_, part)| (*part).clone())
            .collect();
        let expected = Formula::or(remaining);
        if alpha_equiv(&expected, &conclusion) {
            return KernelVerdict::Certified;
        }
    }
    KernelVerdict::Rejected(
        "disjunctive_syllogism conclusion is not the remaining disjunction".into(),
    )
}

fn verify_horn(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 2 {
        return KernelVerdict::Rejected("horn must have at least two parents".into());
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive("horn exceeded strict formula-size limit".into());
    }

    let mut rules = Vec::new();
    let mut derived = Vec::new();
    for parent in parents {
        let (_, body) = leading_forall_core(parent);
        if matches!(body, Formula::Implies(_, _)) {
            rules.push(body);
        } else {
            derived.push(parent.clone());
        }
    }
    if rules.is_empty() || derived.is_empty() {
        return KernelVerdict::Rejected(
            "horn needs at least one implication and one fact parent".into(),
        );
    }

    let mut steps = 0;
    let mut changed = true;
    while changed {
        changed = false;
        let snapshot = derived.clone();
        for rule in &rules {
            for fact in &snapshot {
                let Some(candidate) = instantiate_horn_rule(rule, fact, &mut steps, limits) else {
                    if steps >= limits.max_equivalence_steps {
                        return KernelVerdict::Inconclusive(
                            "horn exceeded strict matching-step limit".into(),
                        );
                    }
                    continue;
                };
                if formula_size(&candidate) > limits.max_formula_nodes {
                    return KernelVerdict::Inconclusive(
                        "horn derived formula exceeded strict formula-size limit".into(),
                    );
                }
                if alpha_equiv(&candidate, conclusion) {
                    return KernelVerdict::Certified;
                }
                if derived.iter().any(|known| alpha_equiv(known, &candidate)) {
                    continue;
                }
                if derived.len() >= limits.max_equivalence_steps {
                    return KernelVerdict::Inconclusive(
                        "horn exceeded strict derived-formula limit".into(),
                    );
                }
                derived.push(candidate);
                changed = true;
            }
        }
    }
    KernelVerdict::Rejected("horn conclusion is not reachable from its parents".into())
}

fn verify_ex_falso(parents: &[Formula], limits: VerificationLimits) -> KernelVerdict {
    if parents.len() == 1 && matches!(parents[0], Formula::False) {
        return KernelVerdict::Certified;
    }
    if parents.len() != 2 {
        return KernelVerdict::Rejected(
            "ex_falso must have one `$false` parent or two contradiction parents".into(),
        );
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
    {
        return KernelVerdict::Inconclusive("ex_falso exceeded strict formula-size limit".into());
    }
    let verdict = verify_resolution(&parents[..2], &Formula::False, limits);
    if matches!(verdict, KernelVerdict::Certified) {
        KernelVerdict::Certified
    } else {
        verdict
    }
}

fn verify_weakening(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("weaken must have one parent".into());
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive("weaken exceeded strict formula-size limit".into());
    }
    let mut steps = 0;
    let Some(parent) = canonicalize_equivalence(&to_nnf(&parents[0]), &mut steps, limits) else {
        return KernelVerdict::Inconclusive("weaken exceeded strict matching-step limit".into());
    };
    let Some(conclusion) = canonicalize_equivalence(&to_nnf(conclusion), &mut steps, limits) else {
        return KernelVerdict::Inconclusive("weaken exceeded strict matching-step limit".into());
    };
    let mut parent_parts = Vec::new();
    flatten_disjunction(&parent, &mut parent_parts);
    let mut conclusion_parts = Vec::new();
    flatten_disjunction(&conclusion, &mut conclusion_parts);
    let mut used = vec![false; conclusion_parts.len()];
    if match_disjunction_parts(
        &parent_parts,
        &conclusion_parts,
        &mut used,
        0,
        &mut steps,
        limits,
    ) {
        KernelVerdict::Certified
    } else if steps >= limits.max_equivalence_steps {
        KernelVerdict::Inconclusive("weaken exceeded strict matching-step limit".into())
    } else {
        KernelVerdict::Rejected("weaken conclusion does not contain the parent disjuncts".into())
    }
}

fn verify_reflexivity(parents: &[Formula], conclusion: &Formula) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("reflexivity must have one parent".into());
    }
    if matches!(conclusion, Formula::Atom(Atom::Eq(left, right)) if left == right) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("reflexivity conclusion is not t = t".into())
    }
}

/// Verify the ground unit-equality normalization emitted by the certified
/// EPR+Eq path. One parent is the clause being normalized (the target);
/// the remaining parents are positive ground unit equalities that justify
/// congruence replacement. The target is identified by shape — the parent
/// that is not a positive ground unit equality — rather than by position,
/// and both the target and the conclusion are normalized with the same
/// classes before comparison, so the verdict does not depend on which
/// class representative either side happened to pick (the search-side
/// union-find is rank-based while this checker is left-biased; both agree
/// on connectivity but may elect different representatives). This
/// deliberately excludes non-ground and non-unit equality rewriting until
/// the broader superposition certificate is implemented.
fn verify_equality_normalization(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 2 {
        return KernelVerdict::Rejected(
            "equality_normalization requires a target and equality parents".into(),
        );
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "equality_normalization exceeded strict formula-size limit".into(),
        );
    }
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "equality_normalization conclusion is not a supported clause".into(),
        );
    };
    let mut parsed = Vec::with_capacity(parents.len());
    for parent in parents {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Inconclusive(
                "equality_normalization parent is not a supported clause".into(),
            );
        };
        parsed.push(clause);
    }
    let mut saw_inconclusive = false;
    for target_idx in 0..parsed.len() {
        let mut classes = GroundEqualityClasses::default();
        let mut others_valid = true;
        for (idx, clause) in parsed.iter().enumerate() {
            if idx == target_idx {
                continue;
            }
            if clause.len() != 1 || !clause[0].positive {
                others_valid = false;
                break;
            }
            let Atom::Eq(left, right) = &clause[0].atom else {
                others_valid = false;
                break;
            };
            if term_var_set(left).is_empty()
                && term_var_set(right).is_empty()
                && is_ground_constant_term(left)
                && is_ground_constant_term(right)
            {
                classes.union(left, right);
            } else {
                // A non-ground equality parent is outside the strict
                // fragment: fail open so the candidate is discarded without
                // claiming unsoundness.
                saw_inconclusive = true;
                others_valid = false;
                break;
            }
        }
        if !others_valid {
            continue;
        }
        let Ok(normalized_target) = normalize_eq_clause(&parsed[target_idx], &mut classes) else {
            // A positive reflexive target is a dropped tautology, never an
            // emitted normalization step.
            continue;
        };
        let Ok(normalized_goal) = normalize_eq_clause(&goal, &mut classes) else {
            continue;
        };
        if clause_alpha_equiv(&normalized_target, &normalized_goal) {
            return KernelVerdict::Certified;
        }
    }
    if saw_inconclusive {
        return KernelVerdict::Inconclusive(
            "equality_normalization requires ground constant equalities".into(),
        );
    }
    KernelVerdict::Rejected("equality_normalization conclusion is not the normalized target".into())
}

/// Normalize a clause by class representatives, recursing into nested
/// function arguments so congruence under function symbols is honored.
/// Negative reflexive equalities are deleted (false disjuncts); a positive
/// reflexive equality signals a tautology (`Err(())`), which the search
/// side drops silently instead of emitting as a step.
fn normalize_eq_clause(
    clause: &[Literal],
    classes: &mut GroundEqualityClasses,
) -> Result<Vec<Literal>, ()> {
    let mut normalized = Vec::with_capacity(clause.len());
    for literal in clause {
        let atom = match &literal.atom {
            Atom::Pred(predicate, args) => Atom::Pred(
                *predicate,
                args.iter()
                    .map(|term| normalize_eq_term(term, classes))
                    .collect(),
            ),
            Atom::Eq(left, right) => {
                let left = normalize_eq_term(left, classes);
                let right = normalize_eq_term(right, classes);
                if left == right {
                    if literal.positive {
                        return Err(());
                    }
                    continue;
                }
                Atom::Eq(left, right)
            }
        };
        normalized.push(Literal {
            positive: literal.positive,
            atom,
        });
    }
    Ok(normalized)
}

fn normalize_eq_term(term: &Term, classes: &mut GroundEqualityClasses) -> Term {
    match term {
        Term::Var(_) => term.clone(),
        Term::App(symbol, args) if args.is_empty() => classes.representative(term),
        Term::App(symbol, args) => Term::App(
            *symbol,
            args.iter()
                .map(|arg| normalize_eq_term(arg, classes))
                .collect(),
        ),
    }
}

#[derive(Default)]
struct GroundEqualityClasses {
    parent: HashMap<Term, Term>,
}

impl GroundEqualityClasses {
    fn find(&mut self, term: &Term) -> Term {
        let Some(parent) = self.parent.get(term).cloned() else {
            return term.clone();
        };
        if parent == *term {
            return parent;
        }
        let root = self.find(&parent);
        self.parent.insert(term.clone(), root.clone());
        root
    }

    fn union(&mut self, left: &Term, right: &Term) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        self.parent
            .entry(left_root.clone())
            .or_insert(left_root.clone());
        self.parent
            .entry(right_root.clone())
            .or_insert(right_root.clone());
        if left_root != right_root {
            self.parent.insert(right_root, left_root);
        }
    }

    fn representative(&mut self, term: &Term) -> Term {
        self.find(term)
    }
}

fn is_ground_constant_term(term: &Term) -> bool {
    matches!(term, Term::App(_, args) if args.is_empty())
}

fn verify_transitivity(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("transitivity must have two parents".into());
    }
    if parents
        .iter()
        .any(|parent| formula_size(parent) > limits.max_formula_nodes)
        || formula_size(conclusion) > limits.max_formula_nodes
    {
        return KernelVerdict::Inconclusive(
            "transitivity exceeded strict formula-size limit".into(),
        );
    }
    let Some((left_a, left_b)) = positive_equality(&parents[0]) else {
        return KernelVerdict::Rejected("transitivity parent is not a positive equality".into());
    };
    let Some((right_a, right_b)) = positive_equality(&parents[1]) else {
        return KernelVerdict::Rejected("transitivity parent is not a positive equality".into());
    };
    let Some((goal_left, goal_right)) = positive_equality(conclusion) else {
        return KernelVerdict::Rejected(
            "transitivity conclusion is not a positive equality".into(),
        );
    };
    if [left_a, left_b, right_a, right_b, goal_left, goal_right]
        .iter()
        .any(|term| !term_var_set(term).is_empty())
    {
        return KernelVerdict::Inconclusive(
            "transitivity strict fragment requires ground equalities".into(),
        );
    }

    for (first_left, first_right) in [(left_a, left_b), (left_b, left_a)] {
        for (second_left, second_right) in [(right_a, right_b), (right_b, right_a)] {
            if first_right != second_left {
                continue;
            }
            if (goal_left == first_left && goal_right == second_right)
                || (goal_left == second_right && goal_right == first_left)
            {
                return KernelVerdict::Certified;
            }
        }
    }
    KernelVerdict::Rejected("transitivity conclusion is not a parent equality chain".into())
}

fn positive_equality(formula: &Formula) -> Option<(&Term, &Term)> {
    match formula {
        Formula::Atom(Atom::Eq(left, right)) => Some((left, right)),
        _ => None,
    }
}

fn instantiate_horn_rule(
    rule: &Formula,
    fact: &Formula,
    steps: &mut usize,
    limits: VerificationLimits,
) -> Option<Formula> {
    let Formula::Implies(antecedent, consequent) = rule else {
        return None;
    };
    let mut substitution = HashMap::new();
    let mut bound = HashMap::new();
    if !match_universal_instance(
        antecedent,
        fact,
        &mut substitution,
        &mut bound,
        steps,
        limits,
    ) {
        return None;
    }
    if consequent
        .free_vars()
        .iter()
        .any(|variable| !substitution.contains_key(variable))
    {
        return None;
    }
    let mut core_substitution = Substitution::new();
    for (variable, term) in substitution {
        core_substitution.bind(variable, term);
    }
    Some(core_substitution.apply_formula(consequent))
}

fn formula_equivalent_with_limit(
    left: &Formula,
    right: &Formula,
    limits: VerificationLimits,
) -> bool {
    let mut steps = 0;
    let Some(left) = canonicalize_equivalence(&to_nnf(left), &mut steps, limits) else {
        return false;
    };
    let Some(right) = canonicalize_equivalence(&to_nnf(right), &mut steps, limits) else {
        return false;
    };
    alpha_equiv(&left, &right)
}

fn strip_leading_foralls<'a>(formula: &'a Formula, binders: &mut Vec<VarId>) -> &'a Formula {
    let mut current = formula;
    while let Formula::Forall(variable, body) = current {
        binders.push(*variable);
        current = body;
    }
    current
}

fn flatten_conjunction<'a>(formula: &'a Formula, parts: &mut Vec<&'a Formula>) {
    match formula {
        Formula::And(children) => {
            for child in children {
                flatten_conjunction(child, parts);
            }
        }
        _ => parts.push(formula),
    }
}

fn flatten_disjunction<'a>(formula: &'a Formula, parts: &mut Vec<&'a Formula>) {
    match formula {
        Formula::Or(children) => {
            for child in children {
                flatten_disjunction(child, parts);
            }
        }
        _ => parts.push(formula),
    }
}

fn match_conjunction_parts(
    parents: &[Formula],
    conclusion: &[&Formula],
    used: &mut [bool],
    index: usize,
    steps: &mut usize,
    limits: VerificationLimits,
) -> bool {
    *steps += 1;
    if *steps > limits.max_equivalence_steps {
        return false;
    }
    if index == parents.len() {
        return true;
    }
    for target_index in 0..conclusion.len() {
        if used[target_index] {
            continue;
        }
        if alpha_equiv(&parents[index], conclusion[target_index]) {
            used[target_index] = true;
            if match_conjunction_parts(parents, conclusion, used, index + 1, steps, limits) {
                return true;
            }
            used[target_index] = false;
        }
    }
    false
}

fn match_disjunction_parts(
    parents: &[&Formula],
    conclusion: &[&Formula],
    used: &mut [bool],
    index: usize,
    steps: &mut usize,
    limits: VerificationLimits,
) -> bool {
    *steps += 1;
    if *steps > limits.max_equivalence_steps {
        return false;
    }
    if index == parents.len() {
        return true;
    }
    for target_index in 0..conclusion.len() {
        if used[target_index] {
            continue;
        }
        if alpha_equiv(parents[index], conclusion[target_index]) {
            used[target_index] = true;
            if match_disjunction_parts(parents, conclusion, used, index + 1, steps, limits) {
                return true;
            }
            used[target_index] = false;
        }
    }
    false
}

#[derive(Clone, Default)]
struct ExistentialGenerationState {
    bound: HashMap<VarId, VarId>,
    witnesses: HashMap<VarId, Term>,
    existential_vars: HashSet<VarId>,
    introduced: usize,
}

fn match_existential_generation(
    parent: &Formula,
    conclusion: &Formula,
    state: &mut ExistentialGenerationState,
    steps: &mut usize,
    limits: VerificationLimits,
) -> bool {
    *steps += 1;
    if *steps > limits.max_equivalence_steps {
        return false;
    }
    if let Formula::Exists(target_var, target_body) = conclusion {
        if let Formula::Exists(parent_var, parent_body) = parent {
            let mut preserved = state.clone();
            preserved.bound.insert(*parent_var, *target_var);
            if match_existential_generation(parent_body, target_body, &mut preserved, steps, limits)
            {
                *state = preserved;
                return true;
            }
        }
        let mut introduced = state.clone();
        introduced.existential_vars.insert(*target_var);
        introduced.introduced += 1;
        if match_existential_generation(parent, target_body, &mut introduced, steps, limits) {
            *state = introduced;
            return true;
        }
        return false;
    }

    let mut candidate = state.clone();
    let matched = match (parent, conclusion) {
        (Formula::Forall(parent_var, parent_body), Formula::Forall(target_var, target_body)) => {
            candidate.bound.insert(*parent_var, *target_var);
            match_existential_generation(parent_body, target_body, &mut candidate, steps, limits)
        }
        (Formula::Exists(parent_var, parent_body), Formula::Exists(target_var, target_body)) => {
            candidate.bound.insert(*parent_var, *target_var);
            match_existential_generation(parent_body, target_body, &mut candidate, steps, limits)
        }
        (Formula::Neg(parent_inner), Formula::Neg(target_inner)) => {
            match_existential_generation(parent_inner, target_inner, &mut candidate, steps, limits)
        }
        (Formula::And(parent_parts), Formula::And(target_parts))
        | (Formula::Or(parent_parts), Formula::Or(target_parts)) => {
            parent_parts.len() == target_parts.len()
                && parent_parts
                    .iter()
                    .zip(target_parts)
                    .all(|(parent, target)| {
                        match_existential_generation(parent, target, &mut candidate, steps, limits)
                    })
        }
        (
            Formula::Implies(parent_left, parent_right),
            Formula::Implies(target_left, target_right),
        )
        | (Formula::Iff(parent_left, parent_right), Formula::Iff(target_left, target_right)) => {
            match_existential_generation(parent_left, target_left, &mut candidate, steps, limits)
                && match_existential_generation(
                    parent_right,
                    target_right,
                    &mut candidate,
                    steps,
                    limits,
                )
        }
        (Formula::Atom(parent_atom), Formula::Atom(target_atom)) => {
            match_existential_atom(parent_atom, target_atom, &mut candidate)
        }
        (Formula::True, Formula::True) | (Formula::False, Formula::False) => true,
        _ => false,
    };
    if matched {
        *state = candidate;
    }
    matched
}

fn match_existential_atom(
    parent: &Atom,
    conclusion: &Atom,
    state: &mut ExistentialGenerationState,
) -> bool {
    match (parent, conclusion) {
        (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| match_existential_term(left, right, state))
        }
        (Atom::Eq(left_a, left_b), Atom::Eq(right_a, right_b)) => {
            match_existential_term(left_a, right_a, state)
                && match_existential_term(left_b, right_b, state)
        }
        _ => false,
    }
}

fn match_existential_term(
    parent: &Term,
    conclusion: &Term,
    state: &mut ExistentialGenerationState,
) -> bool {
    match (parent, conclusion) {
        (parent, Term::Var(conclusion_var)) if state.existential_vars.contains(conclusion_var) => {
            match state.witnesses.get(conclusion_var) {
                Some(witness) => witness == parent,
                None => {
                    state.witnesses.insert(*conclusion_var, parent.clone());
                    true
                }
            }
        }
        (Term::Var(left), Term::Var(right)) => {
            state.bound.get(left).copied().unwrap_or(*left) == *right
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| match_existential_term(left, right, state))
        }
        _ => false,
    }
}

fn leading_forall_core(formula: &Formula) -> (Vec<VarId>, &Formula) {
    let mut variables = Vec::new();
    let mut current = formula;
    while let Formula::Forall(variable, body) = current {
        variables.push(*variable);
        current = body;
    }
    (variables, current)
}

fn match_universal_instance(
    parent: &Formula,
    target: &Formula,
    substitution: &mut HashMap<VarId, Term>,
    bound: &mut HashMap<VarId, VarId>,
    steps: &mut usize,
    limits: VerificationLimits,
) -> bool {
    *steps += 1;
    if *steps > limits.max_equivalence_steps {
        return false;
    }
    match (parent, target) {
        (Formula::Atom(left), Formula::Atom(right)) => {
            match_instance_atom(left, right, substitution, bound)
        }
        (Formula::Neg(left), Formula::Neg(right)) => {
            match_universal_instance(left, right, substitution, bound, steps, limits)
        }
        (Formula::And(left), Formula::And(right)) | (Formula::Or(left), Formula::Or(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    match_universal_instance(left, right, substitution, bound, steps, limits)
                })
        }
        (Formula::Implies(left_a, left_b), Formula::Implies(right_a, right_b))
        | (Formula::Iff(left_a, left_b), Formula::Iff(right_a, right_b)) => {
            match_universal_instance(left_a, right_a, substitution, bound, steps, limits)
                && match_universal_instance(left_b, right_b, substitution, bound, steps, limits)
        }
        (Formula::Forall(left_var, left_body), Formula::Forall(right_var, right_body))
        | (Formula::Exists(left_var, left_body), Formula::Exists(right_var, right_body)) => {
            let mut nested = substitution.clone();
            let mut nested_bound = bound.clone();
            nested.remove(left_var);
            nested_bound.insert(*left_var, *right_var);
            let matched = match_universal_instance(
                left_body,
                right_body,
                &mut nested,
                &mut nested_bound,
                steps,
                limits,
            );
            if matched {
                *substitution = nested;
                *bound = nested_bound;
            }
            matched
        }
        (Formula::True, Formula::True) | (Formula::False, Formula::False) => true,
        _ => false,
    }
}

fn match_instance_atom(
    parent: &Atom,
    target: &Atom,
    substitution: &mut HashMap<VarId, Term>,
    bound: &HashMap<VarId, VarId>,
) -> bool {
    match (parent, target) {
        (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| match_instance_term(left, right, substitution, bound))
        }
        (Atom::Eq(left_a, left_b), Atom::Eq(right_a, right_b)) => {
            match_instance_term(left_a, right_a, substitution, bound)
                && match_instance_term(left_b, right_b, substitution, bound)
        }
        _ => false,
    }
}

fn match_instance_term(
    parent: &Term,
    target: &Term,
    substitution: &mut HashMap<VarId, Term>,
    bound: &HashMap<VarId, VarId>,
) -> bool {
    match parent {
        Term::Var(var) if bound.contains_key(var) => {
            matches!(target, Term::Var(target_var) if bound.get(var) == Some(target_var))
        }
        Term::Var(var) => match substitution.get(var) {
            Some(existing) => existing == target,
            None => {
                substitution.insert(*var, target.clone());
                true
            }
        },
        Term::App(left_symbol, left_args) => match target {
            Term::App(right_symbol, right_args) => {
                left_symbol == right_symbol
                    && left_args.len() == right_args.len()
                    && left_args
                        .iter()
                        .zip(right_args)
                        .all(|(left, right)| match_instance_term(left, right, substitution, bound))
            }
            Term::Var(_) => false,
        },
    }
}

fn formula_size(formula: &Formula) -> usize {
    match formula {
        Formula::Atom(Atom::Pred(_, terms)) => 1 + terms.iter().map(term_size).sum::<usize>(),
        Formula::Atom(Atom::Eq(left, right)) => 1 + term_size(left) + term_size(right),
        Formula::Neg(inner) | Formula::Forall(_, inner) | Formula::Exists(_, inner) => {
            1 + formula_size(inner)
        }
        Formula::And(parts) | Formula::Or(parts) => {
            1 + parts.iter().map(formula_size).sum::<usize>()
        }
        Formula::Implies(left, right) | Formula::Iff(left, right) => {
            1 + formula_size(left) + formula_size(right)
        }
        Formula::True | Formula::False => 1,
    }
}

fn lowered_clause_literal_count(formula: &Formula, limits: VerificationLimits) -> usize {
    clause_from_formula(formula, limits).map_or(0, |clause| clause.len())
}

fn term_size(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_size).sum::<usize>(),
    }
}

fn canonicalize_equivalence(
    formula: &Formula,
    steps: &mut usize,
    limits: VerificationLimits,
) -> Option<Formula> {
    *steps += 1;
    if *steps > limits.max_equivalence_steps {
        return None;
    }
    let result = match formula {
        Formula::And(parts) => {
            let mut flattened = Vec::new();
            for part in parts {
                let part = canonicalize_equivalence(part, steps, limits)?;
                match part {
                    Formula::And(nested) => flattened.extend(nested),
                    other => flattened.push(other),
                }
            }
            deduplicate_and_sort_formulas(&mut flattened);
            match flattened.len() {
                0 => Formula::True,
                1 => flattened.pop().expect("one canonical conjunct"),
                _ => Formula::And(flattened),
            }
        }
        Formula::Or(parts) => {
            let mut flattened = Vec::new();
            for part in parts {
                let part = canonicalize_equivalence(part, steps, limits)?;
                match part {
                    Formula::Or(nested) => flattened.extend(nested),
                    other => flattened.push(other),
                }
            }
            deduplicate_and_sort_formulas(&mut flattened);
            match flattened.len() {
                0 => Formula::False,
                1 => flattened.pop().expect("one canonical disjunct"),
                _ => Formula::Or(flattened),
            }
        }
        Formula::Neg(inner) => Formula::neg(canonicalize_equivalence(inner, steps, limits)?),
        Formula::Forall(var, inner) => {
            Formula::forall(*var, canonicalize_equivalence(inner, steps, limits)?)
        }
        Formula::Exists(var, inner) => {
            Formula::exists(*var, canonicalize_equivalence(inner, steps, limits)?)
        }
        Formula::Implies(left, right) => Formula::implies(
            canonicalize_equivalence(left, steps, limits)?,
            canonicalize_equivalence(right, steps, limits)?,
        ),
        Formula::Iff(left, right) => {
            let mut parts = vec![
                Formula::or(vec![Formula::neg((**left).clone()), (**right).clone()]),
                Formula::or(vec![Formula::neg((**right).clone()), (**left).clone()]),
            ];
            for part in &mut parts {
                *part = canonicalize_equivalence(part, steps, limits)?;
            }
            deduplicate_and_sort_formulas(&mut parts);
            Formula::And(parts)
        }
        Formula::Atom(_) | Formula::True | Formula::False => formula.clone(),
    };
    Some(result)
}

fn deduplicate_and_sort_formulas(formulas: &mut Vec<Formula>) {
    formulas.sort_by_key(|formula| format!("{formula:?}"));
    formulas.dedup();
}

fn verify_existential_free_identity(parents: &[Formula], conclusion: &Formula) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Inconclusive(
            "strict kernel currently supports only single-parent skolemisation".into(),
        );
    }
    if contains_exists(&parents[0]) {
        return KernelVerdict::Inconclusive(
            "existential Skolemization is not yet implemented by the strict kernel".into(),
        );
    }
    verify_alpha_identity(parents, conclusion)
}

fn verify_skolemisation(
    node: &Node<'_>,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    parents: &[Formula],
    conclusion: &Formula,
    context: SkolemVerificationContext<'_>,
) -> KernelVerdict {
    let SkolemVerificationContext {
        symbols,
        known_function_symbols,
        skolem_axioms,
        limits,
    } = context;
    let declared_symbols = node
        .formula
        .annotations()
        .map(|annotations| annotations.new_symbols())
        .unwrap_or_default();
    let is_e_skolemize = node.rule == Some("skolemize");
    if is_e_skolemize && parent_indices.len() > 1 {
        return KernelVerdict::Inconclusive(
            "strict `skolemize` verification currently supports one parent".into(),
        );
    }
    if parent_indices.len() > 1 && parents.len() > 1 {
        return verify_multi_parent_skolemisation(
            node,
            parent_indices,
            parents,
            conclusion,
            symbols,
            skolem_axioms,
            limits,
        );
    }
    let annotation = if is_e_skolemize {
        let Some(annotations) = node.formula.annotations() else {
            return KernelVerdict::Inconclusive("`skolemize` node has no annotations".into());
        };
        let Some(info) = annotations.skolemize_info() else {
            return KernelVerdict::Inconclusive(
                "strict `skolemize` verification requires skolemize(...) metadata".into(),
            );
        };
        if declared_symbols.len() != 1 || declared_symbols[0] != info.skolem_symbol {
            return KernelVerdict::Rejected(
                "`skolemize` metadata must declare exactly its witness symbol".into(),
            );
        }
        Some(SkolemAnnotation {
            variable: info.var.to_string(),
            symbol: info.skolem_symbol.to_string(),
            arguments: info.args.iter().map(|arg| (*arg).to_string()).collect(),
        })
    } else {
        None
    };
    if !declared_symbols.is_empty() && !is_e_skolemize {
        return KernelVerdict::Rejected(
            "Skolemization with declared new symbols must cite its Skolem axioms".into(),
        );
    }
    if parent_indices.len() != 1 || parents.len() != 1 {
        return KernelVerdict::Inconclusive(
            "strict kernel currently supports single-parent Skolemization".into(),
        );
    }
    let parent_ast = dag.nodes[parent_indices[0]].formula;
    let Some(parent_fof) = parent_ast.as_fof() else {
        return KernelVerdict::Inconclusive("Skolemization parent is not FOF".into());
    };
    let Some(node_fof) = node.formula.as_fof() else {
        return KernelVerdict::Inconclusive("Skolemization conclusion is not FOF".into());
    };
    let (FOFStatement::Logical(parent_formula), FOFStatement::Logical(step_formula)) =
        (&parent_fof.formula, &node_fof.formula)
    else {
        return KernelVerdict::Inconclusive("Skolemization sequents are unsupported".into());
    };
    let mut parent_symbols = HashSet::new();
    collect_function_symbols_formula(parent_formula, &mut parent_symbols);
    let mut step_symbols = HashSet::new();
    collect_function_symbols_formula(step_formula, &mut step_symbols);
    let fresh: HashSet<String> = step_symbols
        .difference(&parent_symbols)
        .filter(|symbol| !known_function_symbols.contains(*symbol))
        .cloned()
        .collect();
    if fresh.is_empty() {
        if is_e_skolemize {
            return KernelVerdict::Inconclusive(
                "`skolemize` conclusion introduces no fresh witness symbol".into(),
            );
        }
        return verify_existential_free_identity(parents, conclusion);
    }
    if is_e_skolemize && !contains_skolemizable_existential(parent_formula, true) {
        return KernelVerdict::Inconclusive(
            "`skolemize` metadata names no eliminable existential".into(),
        );
    }
    if is_e_skolemize
        && (fresh.len() != 1 || !fresh.contains(&annotation.as_ref().expect("E metadata").symbol))
    {
        return KernelVerdict::Rejected(
            "`skolemize` metadata must describe the sole fresh witness".into(),
        );
    }
    if !contains_skolemizable_existential(parent_formula, true) {
        return KernelVerdict::Inconclusive(
            "Skolemization introduced symbols without an existential parent".into(),
        );
    }
    let mut state = SkolemMatch::new(fresh.clone(), limits.max_skolem_steps);
    state.annotation = annotation;
    if !match_skolem_formula(parent_formula, step_formula, &mut state) {
        if state.exhausted.get() {
            return KernelVerdict::Inconclusive(
                "Skolemization exceeded strict matching-step limit".into(),
            );
        }
        return KernelVerdict::Rejected(format!(
            "node `{}` is not an exact Skolemization of its parent",
            node.name
        ));
    }
    if let Some(annotation) = &state.annotation
        && !validate_skolem_annotation(annotation, &state)
    {
        return KernelVerdict::Rejected(format!(
            "node `{}` has inconsistent skolemize(...) metadata",
            node.name
        ));
    }
    if state.used_symbols != fresh {
        return KernelVerdict::Rejected(format!(
            "node `{}` does not use exactly its fresh Skolem symbols",
            node.name
        ));
    }
    KernelVerdict::Certified
}

fn validate_skolem_annotation(annotation: &SkolemAnnotation, state: &SkolemMatch) -> bool {
    let Some((symbol, arguments)) = state.existential_witnesses.get(&annotation.variable) else {
        return false;
    };
    if symbol != &annotation.symbol {
        return false;
    }
    if arguments == &annotation.arguments {
        return true;
    }
    let mapped_args: Vec<_> = annotation
        .arguments
        .iter()
        .map(|arg| state.universal_history.get(arg).unwrap_or(arg).clone())
        .collect();
    arguments == &mapped_args
}

struct SkolemVerificationContext<'a> {
    symbols: &'a SymbolTable,
    known_function_symbols: &'a HashSet<String>,
    skolem_axioms: &'a HashMap<usize, OwnedSkolemAxiom>,
    limits: VerificationLimits,
}

#[derive(Clone)]
struct OwnedSkolemAxiom {
    universals: Vec<VarId>,
    existentials: Vec<VarId>,
    antecedent: Formula,
    consequent: Formula,
    fresh_symbols: HashSet<mrs_core::SymbolId>,
}

fn extract_skolem_axiom(
    formula: &Formula,
    symbols: &SymbolTable,
    known_function_symbols: &HashSet<String>,
    limits: VerificationLimits,
) -> Result<OwnedSkolemAxiom, KernelVerdict> {
    if formula_size(formula) > limits.max_formula_nodes {
        return Err(KernelVerdict::Inconclusive(
            "Skolem axiom exceeds strict formula-size limit".into(),
        ));
    }
    let (universals, body) = leading_foralls_owned(formula);
    let Formula::Implies(left, right) = body else {
        return Err(KernelVerdict::Inconclusive(
            "Skolem axiom must be an implication".into(),
        ));
    };
    let (existentials, antecedent) = leading_exists_owned(&left);
    if existentials.is_empty() {
        return Err(KernelVerdict::Inconclusive(
            "Skolem axiom has no existential antecedent".into(),
        ));
    }

    let allowed_antecedent_vars = universals
        .iter()
        .chain(&existentials)
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let allowed_consequent_vars = universals
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if !antecedent
        .free_vars()
        .iter()
        .all(|var| allowed_antecedent_vars.contains(var))
        || !right
            .free_vars()
            .iter()
            .all(|var| allowed_consequent_vars.contains(var))
    {
        return Err(KernelVerdict::Rejected(
            "Skolem axiom contains variables outside its declared scope".into(),
        ));
    }

    let antecedent_symbols = formula_function_symbols(&antecedent);
    let consequent_symbols = formula_function_symbols(&right);
    let fresh_symbols: HashSet<_> = consequent_symbols
        .difference(&antecedent_symbols)
        .copied()
        .collect();
    if fresh_symbols.len() != existentials.len() || fresh_symbols.is_empty() {
        return Err(KernelVerdict::Rejected(
            "Skolem axiom must introduce one fresh function per existential".into(),
        ));
    }
    if fresh_symbols
        .iter()
        .any(|symbol| known_function_symbols.contains(symbols.resolve(*symbol)))
    {
        return Err(KernelVerdict::Rejected(
            "Skolem axiom reuses a problem function symbol".into(),
        ));
    }

    let universals_set = universals.iter().copied().collect::<HashSet<_>>();
    let existentials_set = existentials.iter().copied().collect::<HashSet<_>>();
    let mut state = AxiomMatch::default();
    let mut universal_map = universals
        .iter()
        .copied()
        .map(|var| (var, var))
        .collect::<HashMap<_, _>>();
    if !match_axiom_formula(
        &antecedent,
        &right,
        &universals_set,
        &existentials_set,
        &mut universal_map,
        &mut state,
    ) {
        return Err(KernelVerdict::Rejected(
            "Skolem axiom consequent does not preserve its matrix".into(),
        ));
    }
    if state
        .universal_terms
        .iter()
        .any(|(var, term)| term != &Term::Var(*universal_map.get(var).unwrap_or(var)))
    {
        return Err(KernelVerdict::Rejected(
            "Skolem axiom consequent changes its universal matrix variables".into(),
        ));
    }
    if state.existential_terms.len() != existentials.len()
        || state.existential_terms.values().any(|term| {
            let Some((symbol, args)) = function_application(term) else {
                return true;
            };
            if !fresh_symbols.contains(&symbol) || args.len() != universals.len() {
                return true;
            }
            args.iter()
                .zip(&universals)
                .any(|(arg, universal)| !matches!(arg, Term::Var(var) if var == universal))
        })
    {
        return Err(KernelVerdict::Rejected(
            "Skolem axiom witness has invalid symbol, arity, or scope".into(),
        ));
    }
    Ok(OwnedSkolemAxiom {
        universals,
        existentials,
        antecedent,
        consequent: *right,
        fresh_symbols,
    })
}

fn verify_multi_parent_skolemisation(
    node: &Node<'_>,
    parent_indices: &[usize],
    parents: &[Formula],
    conclusion: &Formula,
    symbols: &SymbolTable,
    skolem_axioms: &HashMap<usize, OwnedSkolemAxiom>,
    limits: VerificationLimits,
) -> KernelVerdict {
    let Some(annotations) = node.formula.annotations() else {
        return KernelVerdict::Inconclusive("multi-parent Skolemization has no annotations".into());
    };
    let declared = annotations
        .new_symbols()
        .into_iter()
        .filter_map(|symbol| symbols.resolve_name(symbol))
        .collect::<HashSet<_>>();
    if declared.is_empty() {
        return KernelVerdict::Inconclusive(
            "multi-parent Skolemization must declare new Skolem symbols".into(),
        );
    }
    let sources = parent_indices
        .iter()
        .enumerate()
        .filter_map(|(position, index)| (!skolem_axioms.contains_key(index)).then_some(position))
        .collect::<Vec<_>>();
    if sources.len() != 1 {
        return KernelVerdict::Inconclusive(
            "multi-parent Skolemization requires exactly one source parent".into(),
        );
    }
    let source_position = sources[0];
    let mut axiom_ids = parent_indices
        .iter()
        .enumerate()
        .filter_map(|(position, index)| (position != source_position).then_some(*index))
        .collect::<Vec<_>>();
    if axiom_ids.is_empty() {
        return KernelVerdict::Inconclusive(
            "multi-parent Skolemization requires at least one Skolem axiom".into(),
        );
    }
    let mut union = HashSet::new();
    for id in &axiom_ids {
        let Some(axiom) = skolem_axioms.get(id) else {
            return KernelVerdict::Inconclusive("Skolem axiom parent was not validated".into());
        };
        union.extend(axiom.fresh_symbols.iter().copied());
    }
    if union != declared {
        return KernelVerdict::Rejected(
            "multi-parent Skolemization declaration does not match axiom symbols".into(),
        );
    }

    let mut current = parents[source_position].clone();
    let mut steps = 0;
    let mut exhausted = false;
    while !axiom_ids.is_empty() {
        let mut progressed = false;
        let mut remaining = Vec::new();
        for id in axiom_ids {
            let axiom = skolem_axioms.get(&id).expect("validated axiom parent");
            if let Some(rewritten) =
                apply_owned_skolem_axiom(&current, axiom, limits, &mut steps, &mut exhausted)
            {
                current = rewritten;
                progressed = true;
            } else {
                remaining.push(id);
            }
        }
        if !progressed {
            if exhausted {
                return KernelVerdict::Inconclusive(
                    "multi-parent Skolemization exceeded strict matching-step limit".into(),
                );
            }
            return KernelVerdict::Rejected(format!(
                "multi-parent Skolemization node `{}` cannot apply all cited axioms",
                node.name
            ));
        }
        axiom_ids = remaining;
    }
    if alpha_equiv(&current, conclusion) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected(format!(
            "multi-parent Skolemization node `{}` conclusion mismatch",
            node.name
        ))
    }
}

fn apply_owned_skolem_axiom(
    formula: &Formula,
    axiom: &OwnedSkolemAxiom,
    limits: VerificationLimits,
    steps: &mut usize,
    exhausted: &mut bool,
) -> Option<Formula> {
    let shift = max_formula_var(formula)
        .max(max_formula_var(&axiom.consequent))
        .max(max_formula_var(&axiom.antecedent))
        .saturating_add(1);
    let axiom = shift_owned_axiom(axiom, shift);
    apply_owned_skolem_axiom_walk(formula, &axiom, steps, limits.max_skolem_steps, exhausted)
}

fn apply_owned_skolem_axiom_walk(
    formula: &Formula,
    axiom: &OwnedSkolemAxiom,
    steps: &mut usize,
    limit: usize,
    exhausted: &mut bool,
) -> Option<Formula> {
    *steps += 1;
    if *steps > limit {
        *exhausted = true;
        return None;
    }
    let (target_existentials, target_body) = leading_exists_owned(formula);
    if target_existentials.len() == axiom.existentials.len() {
        let mut existential_map = HashMap::new();
        for (pattern, target) in axiom.existentials.iter().zip(&target_existentials) {
            existential_map.insert(*pattern, *target);
        }
        let mut state = AxiomMatch::default();
        let universals = axiom.universals.iter().copied().collect::<HashSet<_>>();
        let existentials = axiom.existentials.iter().copied().collect::<HashSet<_>>();
        if match_axiom_formula(
            &axiom.antecedent,
            &target_body,
            &universals,
            &existentials,
            &mut existential_map,
            &mut state,
        ) && state.universal_terms.len() == axiom.universals.len()
            && state
                .existential_terms
                .iter()
                .all(|(pattern, term)| matches!(term, Term::Var(var) if existential_map.get(pattern) == Some(var)))
        {
            let mut substitution = Substitution::new();
            for (var, term) in state.universal_terms {
                substitution.bind(var, term);
            }
            return Some(substitution.apply_formula(&axiom.consequent));
        }
    }

    match formula {
        Formula::Neg(inner) => Some(Formula::neg(apply_owned_skolem_axiom_walk(
            inner, axiom, steps, limit, exhausted,
        )?)),
        Formula::And(parts) => rewrite_owned_children(parts, axiom, steps, limit, exhausted, true),
        Formula::Or(parts) => rewrite_owned_children(parts, axiom, steps, limit, exhausted, false),
        Formula::Implies(left, right) => {
            if let Some(left) = apply_owned_skolem_axiom_walk(left, axiom, steps, limit, exhausted)
            {
                Some(Formula::implies(left, (**right).clone()))
            } else {
                Some(Formula::implies(
                    (**left).clone(),
                    apply_owned_skolem_axiom_walk(right, axiom, steps, limit, exhausted)?,
                ))
            }
        }
        Formula::Iff(left, right) => {
            if let Some(left) = apply_owned_skolem_axiom_walk(left, axiom, steps, limit, exhausted)
            {
                Some(Formula::iff(left, (**right).clone()))
            } else {
                Some(Formula::iff(
                    (**left).clone(),
                    apply_owned_skolem_axiom_walk(right, axiom, steps, limit, exhausted)?,
                ))
            }
        }
        Formula::Forall(var, body) => Some(Formula::forall(
            *var,
            apply_owned_skolem_axiom_walk(body, axiom, steps, limit, exhausted)?,
        )),
        Formula::Exists(var, body) => Some(Formula::exists(
            *var,
            apply_owned_skolem_axiom_walk(body, axiom, steps, limit, exhausted)?,
        )),
        Formula::Atom(_) | Formula::True | Formula::False => None,
    }
}

fn rewrite_owned_children(
    parts: &[Formula],
    axiom: &OwnedSkolemAxiom,
    steps: &mut usize,
    limit: usize,
    exhausted: &mut bool,
    conjunction: bool,
) -> Option<Formula> {
    for index in 0..parts.len() {
        if let Some(rewritten) =
            apply_owned_skolem_axiom_walk(&parts[index], axiom, steps, limit, exhausted)
        {
            let mut next = parts.to_vec();
            next[index] = rewritten;
            return Some(if conjunction {
                Formula::And(next)
            } else {
                Formula::Or(next)
            });
        }
    }
    None
}

fn leading_foralls_owned(formula: &Formula) -> (Vec<VarId>, Formula) {
    let mut vars = Vec::new();
    let mut current = formula.clone();
    while let Formula::Forall(var, body) = current {
        vars.push(var);
        current = *body;
    }
    (vars, current)
}

fn leading_exists_owned(formula: &Formula) -> (Vec<VarId>, Formula) {
    let mut vars = Vec::new();
    let mut current = formula.clone();
    while let Formula::Exists(var, body) = current {
        vars.push(var);
        current = *body;
    }
    (vars, current)
}

fn max_formula_var(formula: &Formula) -> VarId {
    fn visit_term(term: &Term, max: &mut VarId) {
        match term {
            Term::Var(var) => *max = (*max).max(*var),
            Term::App(_, args) => args.iter().for_each(|arg| visit_term(arg, max)),
        }
    }
    fn visit(formula: &Formula, max: &mut VarId) {
        match formula {
            Formula::Atom(Atom::Pred(_, args)) => args.iter().for_each(|arg| visit_term(arg, max)),
            Formula::Atom(Atom::Eq(left, right)) => {
                visit_term(left, max);
                visit_term(right, max);
            }
            Formula::Neg(inner) | Formula::Forall(_, inner) | Formula::Exists(_, inner) => {
                visit(inner, max)
            }
            Formula::And(parts) | Formula::Or(parts) => {
                parts.iter().for_each(|part| visit(part, max))
            }
            Formula::Implies(left, right) | Formula::Iff(left, right) => {
                visit(left, max);
                visit(right, max);
            }
            Formula::True | Formula::False => {}
        }
    }
    let mut max = 0;
    visit(formula, &mut max);
    max
}

fn shift_owned_axiom(axiom: &OwnedSkolemAxiom, shift: VarId) -> OwnedSkolemAxiom {
    OwnedSkolemAxiom {
        universals: axiom
            .universals
            .iter()
            .map(|var| var.saturating_add(shift))
            .collect(),
        existentials: axiom
            .existentials
            .iter()
            .map(|var| var.saturating_add(shift))
            .collect(),
        antecedent: shift_formula(&axiom.antecedent, shift),
        consequent: shift_formula(&axiom.consequent, shift),
        fresh_symbols: axiom.fresh_symbols.clone(),
    }
}

fn shift_formula(formula: &Formula, shift: VarId) -> Formula {
    match formula {
        Formula::Atom(Atom::Pred(symbol, args)) => Formula::atom(Atom::Pred(
            *symbol,
            args.iter()
                .map(|arg| shift_owned_term(arg, shift))
                .collect(),
        )),
        Formula::Atom(Atom::Eq(left, right)) => Formula::atom(Atom::Eq(
            shift_owned_term(left, shift),
            shift_owned_term(right, shift),
        )),
        Formula::Neg(inner) => Formula::neg(shift_formula(inner, shift)),
        Formula::And(parts) => Formula::And(
            parts
                .iter()
                .map(|part| shift_formula(part, shift))
                .collect(),
        ),
        Formula::Or(parts) => Formula::Or(
            parts
                .iter()
                .map(|part| shift_formula(part, shift))
                .collect(),
        ),
        Formula::Implies(left, right) => {
            Formula::implies(shift_formula(left, shift), shift_formula(right, shift))
        }
        Formula::Iff(left, right) => {
            Formula::iff(shift_formula(left, shift), shift_formula(right, shift))
        }
        Formula::Forall(var, body) => {
            Formula::forall(var.saturating_add(shift), shift_formula(body, shift))
        }
        Formula::Exists(var, body) => {
            Formula::exists(var.saturating_add(shift), shift_formula(body, shift))
        }
        Formula::True => Formula::True,
        Formula::False => Formula::False,
    }
}

fn shift_owned_term(term: &Term, shift: VarId) -> Term {
    match term {
        Term::Var(var) => Term::Var(var.saturating_add(shift)),
        Term::App(symbol, args) => Term::App(
            *symbol,
            args.iter()
                .map(|arg| shift_owned_term(arg, shift))
                .collect(),
        ),
    }
}

fn formula_function_symbols(formula: &Formula) -> HashSet<mrs_core::SymbolId> {
    fn visit_term(value: &Term, symbols: &mut HashSet<mrs_core::SymbolId>) {
        if let Term::App(symbol, args) = value {
            symbols.insert(*symbol);
            for arg in args {
                visit_term(arg, symbols);
            }
        }
    }
    fn visit(formula: &Formula, symbols: &mut HashSet<mrs_core::SymbolId>) {
        match formula {
            Formula::Atom(Atom::Pred(_, args)) => {
                for arg in args {
                    visit_term(arg, symbols);
                }
            }
            Formula::Atom(Atom::Eq(left, right)) => {
                visit_term(left, symbols);
                visit_term(right, symbols);
            }
            Formula::Neg(inner) | Formula::Forall(_, inner) | Formula::Exists(_, inner) => {
                visit(inner, symbols)
            }
            Formula::And(parts) | Formula::Or(parts) => {
                for part in parts {
                    visit(part, symbols);
                }
            }
            Formula::Implies(left, right) | Formula::Iff(left, right) => {
                visit(left, symbols);
                visit(right, symbols);
            }
            Formula::True | Formula::False => {}
        }
    }
    let mut symbols = HashSet::new();
    visit(formula, &mut symbols);
    symbols
}

fn function_application(term: &Term) -> Option<(mrs_core::SymbolId, &[Term])> {
    match term {
        Term::App(symbol, args) => Some((*symbol, args)),
        Term::Var(_) => None,
    }
}

#[derive(Clone, Default)]
struct AxiomMatch {
    existential_terms: HashMap<VarId, Term>,
    universal_terms: HashMap<VarId, Term>,
}

fn match_axiom_formula(
    pattern: &Formula,
    target: &Formula,
    universals: &HashSet<VarId>,
    existentials: &HashSet<VarId>,
    universal_map: &mut HashMap<VarId, VarId>,
    state: &mut AxiomMatch,
) -> bool {
    match (pattern, target) {
        (Formula::Atom(left), Formula::Atom(right)) => {
            match_axiom_atom(left, right, universals, existentials, universal_map, state)
        }
        (Formula::Neg(left), Formula::Neg(right)) => {
            match_axiom_formula(left, right, universals, existentials, universal_map, state)
        }
        (Formula::And(left), Formula::And(right)) | (Formula::Or(left), Formula::Or(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    match_axiom_formula(left, right, universals, existentials, universal_map, state)
                })
        }
        (Formula::Implies(left_a, left_b), Formula::Implies(right_a, right_b))
        | (Formula::Iff(left_a, left_b), Formula::Iff(right_a, right_b)) => {
            match_axiom_formula(
                left_a,
                right_a,
                universals,
                existentials,
                universal_map,
                state,
            ) && match_axiom_formula(
                left_b,
                right_b,
                universals,
                existentials,
                universal_map,
                state,
            )
        }
        (Formula::Forall(..), Formula::Forall(..)) | (Formula::Exists(..), Formula::Exists(..)) => {
            let (left_forall, left_vars, left_body) = quantifier_block(pattern);
            let (right_forall, right_vars, right_body) = quantifier_block(target);
            let mut context = AxiomMatchContext {
                universals,
                existentials,
                universal_map: universal_map.clone(),
                state: state.clone(),
            };
            left_forall == right_forall
                && left_vars.len() == right_vars.len()
                && match_axiom_quantifier_binders(
                    &left_vars,
                    &right_vars,
                    left_body,
                    right_body,
                    &mut context,
                )
                && {
                    *universal_map = context.universal_map;
                    *state = context.state;
                    true
                }
        }
        (Formula::True, Formula::True) | (Formula::False, Formula::False) => true,
        _ => false,
    }
}

struct AxiomMatchContext<'a> {
    universals: &'a HashSet<VarId>,
    existentials: &'a HashSet<VarId>,
    universal_map: HashMap<VarId, VarId>,
    state: AxiomMatch,
}

fn quantifier_block(formula: &Formula) -> (bool, Vec<VarId>, &Formula) {
    let mut current = formula;
    let forall = matches!(current, Formula::Forall(_, _));
    let mut vars = Vec::new();
    loop {
        match (forall, current) {
            (true, Formula::Forall(var, body)) | (false, Formula::Exists(var, body)) => {
                vars.push(*var);
                current = body;
            }
            _ => return (forall, vars, current),
        }
    }
}

fn match_axiom_quantifier_binders(
    left_vars: &[VarId],
    right_vars: &[VarId],
    left_body: &Formula,
    right_body: &Formula,
    context: &mut AxiomMatchContext<'_>,
) -> bool {
    fn visit(
        index: usize,
        left_vars: &[VarId],
        right_vars: &[VarId],
        left_body: &Formula,
        right_body: &Formula,
        context: &mut AxiomMatchContext<'_>,
        used: &mut [bool],
    ) -> bool {
        if index == left_vars.len() {
            return match_axiom_formula(
                left_body,
                right_body,
                context.universals,
                context.existentials,
                &mut context.universal_map,
                &mut context.state,
            );
        }
        for target_index in 0..right_vars.len() {
            if used[target_index] {
                continue;
            }
            used[target_index] = true;
            let mut candidate = AxiomMatchContext {
                universals: context.universals,
                existentials: context.existentials,
                universal_map: context.universal_map.clone(),
                state: context.state.clone(),
            };
            candidate
                .universal_map
                .insert(left_vars[index], right_vars[target_index]);
            if visit(
                index + 1,
                left_vars,
                right_vars,
                left_body,
                right_body,
                &mut candidate,
                used,
            ) {
                *context = candidate;
                return true;
            }
            used[target_index] = false;
        }
        false
    }

    let mut used = vec![false; right_vars.len()];
    visit(
        0, left_vars, right_vars, left_body, right_body, context, &mut used,
    )
}

fn match_axiom_atom(
    pattern: &Atom,
    target: &Atom,
    universals: &HashSet<VarId>,
    existentials: &HashSet<VarId>,
    universal_map: &mut HashMap<VarId, VarId>,
    state: &mut AxiomMatch,
) -> bool {
    match (pattern, target) {
        (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args.iter().zip(right_args).all(|(left, right)| {
                    match_axiom_term(left, right, universals, existentials, universal_map, state)
                })
        }
        (Atom::Eq(left_a, left_b), Atom::Eq(right_a, right_b)) => {
            match_axiom_term(
                left_a,
                right_a,
                universals,
                existentials,
                universal_map,
                state,
            ) && match_axiom_term(
                left_b,
                right_b,
                universals,
                existentials,
                universal_map,
                state,
            )
        }
        _ => false,
    }
}

fn match_axiom_term(
    pattern: &Term,
    target: &Term,
    universals: &HashSet<VarId>,
    existentials: &HashSet<VarId>,
    universal_map: &mut HashMap<VarId, VarId>,
    state: &mut AxiomMatch,
) -> bool {
    match pattern {
        Term::Var(var) if existentials.contains(var) => {
            if let Some(previous) = state.existential_terms.get(var) {
                previous == target
            } else {
                state.existential_terms.insert(*var, target.clone());
                true
            }
        }
        Term::Var(var) if universals.contains(var) => {
            if let Some(previous) = state.universal_terms.get(var) {
                previous == target
            } else {
                state.universal_terms.insert(*var, target.clone());
                true
            }
        }
        Term::Var(var) => target == &Term::Var(*universal_map.get(var).unwrap_or(var)),
        Term::App(symbol, args) => match target {
            Term::App(target_symbol, target_args) => {
                symbol == target_symbol
                    && args.len() == target_args.len()
                    && args.iter().zip(target_args).all(|(left, right)| {
                        match_axiom_term(
                            left,
                            right,
                            universals,
                            existentials,
                            universal_map,
                            state,
                        )
                    })
            }
            Term::Var(_) => false,
        },
    }
}

fn collect_function_symbols(formula: &AnnotatedFormula<'_>, symbols: &mut HashSet<String>) {
    match formula {
        AnnotatedFormula::FOF(formula) => match &formula.formula {
            FOFStatement::Logical(formula) => collect_function_symbols_formula(formula, symbols),
            FOFStatement::Sequent(left, right) => {
                for formula in left.iter().chain(right) {
                    collect_function_symbols_formula(formula, symbols);
                }
            }
        },
        AnnotatedFormula::CNF(formula) => match &formula.formula {
            CNFStatement::Logical(formula) => collect_function_symbols_cnf(formula, symbols),
        },
        _ => {}
    }
}

fn collect_function_symbols_formula(formula: &FOFFormula<'_>, symbols: &mut HashSet<String>) {
    match formula {
        FOFFormula::Atomic(atom) => match atom {
            FOFAtomicFormula::Plain(_, terms)
            | FOFAtomicFormula::Defined(_, terms)
            | FOFAtomicFormula::System(_, terms) => {
                for term in terms {
                    collect_function_symbols_term(term, symbols);
                }
            }
            FOFAtomicFormula::True | FOFAtomicFormula::False => {}
        },
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
            collect_function_symbols_formula(inner, symbols)
        }
        FOFFormula::Quantified { formula, .. } => {
            collect_function_symbols_formula(formula, symbols)
        }
        FOFFormula::Binary { left, right, .. } => {
            collect_function_symbols_formula(left, symbols);
            collect_function_symbols_formula(right, symbols);
        }
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
            collect_function_symbols_term(left, symbols);
            collect_function_symbols_term(right, symbols);
        }
    }
}

fn collect_function_symbols_cnf(formula: &CNFFormula<'_>, symbols: &mut HashSet<String>) {
    match formula {
        CNFFormula::Parens(inner) => collect_function_symbols_cnf(inner, symbols),
        CNFFormula::Disjunction(literals) => {
            for literal in literals {
                match literal {
                    CNFLiteral::Positive(atom) | CNFLiteral::Negative(atom) => match atom {
                        mrs_tptp::CNFAtomicFormula::Plain(_, terms)
                        | mrs_tptp::CNFAtomicFormula::Defined(_, terms)
                        | mrs_tptp::CNFAtomicFormula::System(_, terms) => {
                            for term in terms {
                                collect_function_symbols_term(term, symbols);
                            }
                        }
                        mrs_tptp::CNFAtomicFormula::True | mrs_tptp::CNFAtomicFormula::False => {}
                    },
                    CNFLiteral::Equality(left, right) | CNFLiteral::Inequality(left, right) => {
                        collect_function_symbols_term(left, symbols);
                        collect_function_symbols_term(right, symbols);
                    }
                }
            }
        }
    }
}

fn collect_function_symbols_term(term: &FOFTerm<'_>, symbols: &mut HashSet<String>) {
    match term {
        FOFTerm::Function(name, args) => {
            symbols.insert(name.as_str().to_string());
            for arg in args {
                collect_function_symbols_term(arg, symbols);
            }
        }
        FOFTerm::DefinedFunction(name, args) => {
            symbols.insert(format!("${}", name.0));
            for arg in args {
                collect_function_symbols_term(arg, symbols);
            }
        }
        FOFTerm::SystemFunction(name, args) => {
            symbols.insert(format!("$${}", name.0));
            for arg in args {
                collect_function_symbols_term(arg, symbols);
            }
        }
        FOFTerm::Variable(_) | FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => {}
    }
}

fn collect_all_symbols(formula: &AnnotatedFormula<'_>, symbols: &mut HashSet<String>) {
    match formula {
        AnnotatedFormula::FOF(formula) => {
            if let FOFStatement::Logical(formula) = &formula.formula {
                collect_all_formula_symbols(formula, symbols);
            }
        }
        AnnotatedFormula::CNF(formula) => {
            let CNFStatement::Logical(formula) = &formula.formula;
            collect_all_cnf_symbols(formula, symbols);
        }
        _ => {}
    }
}

fn collect_all_formula_symbols(formula: &FOFFormula<'_>, symbols: &mut HashSet<String>) {
    match formula {
        FOFFormula::Atomic(atom) => match atom {
            FOFAtomicFormula::Plain(name, terms) => {
                symbols.insert(name.as_str().to_string());
                for term in terms {
                    collect_all_term_symbols(term, symbols);
                }
            }
            FOFAtomicFormula::Defined(name, terms) => {
                symbols.insert(format!("${}", name.0));
                for term in terms {
                    collect_all_term_symbols(term, symbols);
                }
            }
            FOFAtomicFormula::System(name, terms) => {
                symbols.insert(format!("$${}", name.0));
                for term in terms {
                    collect_all_term_symbols(term, symbols);
                }
            }
            FOFAtomicFormula::True | FOFAtomicFormula::False => {}
        },
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
            collect_all_formula_symbols(inner, symbols)
        }
        FOFFormula::Quantified { formula, .. } => collect_all_formula_symbols(formula, symbols),
        FOFFormula::Binary { left, right, .. } => {
            collect_all_formula_symbols(left, symbols);
            collect_all_formula_symbols(right, symbols);
        }
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
            collect_all_term_symbols(left, symbols);
            collect_all_term_symbols(right, symbols);
        }
    }
}

fn collect_all_cnf_symbols(formula: &CNFFormula<'_>, symbols: &mut HashSet<String>) {
    match formula {
        CNFFormula::Parens(inner) => collect_all_cnf_symbols(inner, symbols),
        CNFFormula::Disjunction(literals) => {
            for literal in literals {
                match literal {
                    CNFLiteral::Positive(atom) | CNFLiteral::Negative(atom) => match atom {
                        mrs_tptp::CNFAtomicFormula::Plain(name, terms) => {
                            symbols.insert(name.as_str().to_string());
                            for term in terms {
                                collect_all_term_symbols(term, symbols);
                            }
                        }
                        mrs_tptp::CNFAtomicFormula::Defined(name, terms) => {
                            symbols.insert(format!("${}", name.0));
                            for term in terms {
                                collect_all_term_symbols(term, symbols);
                            }
                        }
                        mrs_tptp::CNFAtomicFormula::System(name, terms) => {
                            symbols.insert(format!("$${}", name.0));
                            for term in terms {
                                collect_all_term_symbols(term, symbols);
                            }
                        }
                        mrs_tptp::CNFAtomicFormula::True | mrs_tptp::CNFAtomicFormula::False => {}
                    },
                    CNFLiteral::Equality(left, right) | CNFLiteral::Inequality(left, right) => {
                        collect_all_term_symbols(left, symbols);
                        collect_all_term_symbols(right, symbols);
                    }
                }
            }
        }
    }
}

fn collect_all_term_symbols(term: &FOFTerm<'_>, symbols: &mut HashSet<String>) {
    collect_function_symbols_term(term, symbols);
}

#[derive(Clone)]
struct SkolemScope {
    allowed: HashSet<String>,
    required: HashSet<String>,
}

#[derive(Clone)]
struct SkolemMatch {
    fresh_symbols: HashSet<String>,
    used_symbols: HashSet<String>,
    universal_map: HashMap<String, String>,
    universal_history: HashMap<String, String>,
    existential_terms: HashMap<String, String>,
    witness_owners: HashMap<String, String>,
    active_existentials: HashMap<String, SkolemScope>,
    active_universals: Vec<String>,
    steps: Rc<Cell<usize>>,
    step_limit: usize,
    exhausted: Rc<Cell<bool>>,
    annotation: Option<SkolemAnnotation>,
    existential_witnesses: HashMap<String, (String, Vec<String>)>,
}

#[derive(Clone)]
struct SkolemAnnotation {
    variable: String,
    symbol: String,
    arguments: Vec<String>,
}

impl SkolemMatch {
    fn new(fresh_symbols: HashSet<String>, step_limit: usize) -> Self {
        Self {
            fresh_symbols,
            used_symbols: HashSet::new(),
            universal_map: HashMap::new(),
            universal_history: HashMap::new(),
            existential_terms: HashMap::new(),
            witness_owners: HashMap::new(),
            active_existentials: HashMap::new(),
            active_universals: Vec::new(),
            steps: Rc::new(Cell::new(0)),
            step_limit,
            exhausted: Rc::new(Cell::new(false)),
            annotation: None,
            existential_witnesses: HashMap::new(),
        }
    }

    fn charge(&self) -> bool {
        let steps = self.steps.get();
        if steps >= self.step_limit {
            self.exhausted.set(true);
            false
        } else {
            self.steps.set(steps + 1);
            true
        }
    }
}

fn match_skolem_formula(
    parent: &FOFFormula<'_>,
    step: &FOFFormula<'_>,
    state: &mut SkolemMatch,
) -> bool {
    match_skolem_formula_with_polarity(parent, step, state, true)
}

fn match_skolem_formula_with_polarity(
    parent: &FOFFormula<'_>,
    step: &FOFFormula<'_>,
    state: &mut SkolemMatch,
    polarity: bool,
) -> bool {
    if !state.charge() {
        return false;
    }
    let mut candidate = state.clone();
    if match_skolem_formula_inner(parent, step, &mut candidate, polarity) {
        *state = candidate;
        true
    } else {
        false
    }
}

fn match_skolem_formula_inner(
    parent: &FOFFormula<'_>,
    step: &FOFFormula<'_>,
    state: &mut SkolemMatch,
    polarity: bool,
) -> bool {
    let (parent_prefix, parent_matrix) = leading_quantifiers(parent);
    let (step_prefix, step_matrix) = leading_quantifiers(step);
    let step_universals: Vec<String> = step_prefix
        .iter()
        .filter(|(quantifier, _)| is_effective_universal(*quantifier, polarity))
        .flat_map(|(_, variables)| variables.iter().cloned())
        .collect();
    let step_existentials: HashSet<String> = step_prefix
        .iter()
        .filter(|(quantifier, _)| !is_effective_universal(*quantifier, polarity))
        .flat_map(|(_, variables)| variables.iter().cloned())
        .collect();
    let step_existential_count = step_prefix
        .iter()
        .filter(|(quantifier, _)| !is_effective_universal(*quantifier, polarity))
        .map(|(_, variables)| variables.len())
        .sum::<usize>();

    // A Skolem step may remove effective existential binders, but it must not
    // reorder the binders that remain. In particular, moving a preserved
    // existential in front of a universal changes the quantifier scope and is
    // not a Skolemization of the parent. Preserved existential names are kept
    // as-is here; rejecting alpha-renamed variants is conservative, while
    // accepting a reordered scope would be unsound.
    let parent_prefix_vars = parent_prefix
        .iter()
        .flat_map(|(_, variables)| variables.iter())
        .collect::<HashSet<_>>();
    if step_existentials
        .iter()
        .any(|variable| !parent_prefix_vars.contains(variable))
    {
        return false;
    }
    let expected_prefix = parent_prefix
        .iter()
        .flat_map(|(quantifier, variables)| {
            variables
                .iter()
                .map(move |variable| (*quantifier, variable))
        })
        .filter(|(quantifier, variable)| {
            is_effective_universal(*quantifier, polarity) || step_existentials.contains(*variable)
        })
        .collect::<Vec<_>>();
    let step_prefix_vars = step_prefix
        .iter()
        .flat_map(|(quantifier, variables)| {
            variables
                .iter()
                .map(move |variable| (*quantifier, variable))
        })
        .collect::<Vec<_>>();
    if expected_prefix.len() != step_prefix_vars.len()
        || expected_prefix.iter().zip(&step_prefix_vars).any(
            |((parent_quantifier, parent_variable), (step_quantifier, step_variable))| {
                is_effective_universal(*parent_quantifier, polarity)
                    != is_effective_universal(*step_quantifier, polarity)
                    || (!is_effective_universal(*parent_quantifier, polarity)
                        && parent_variable != step_variable)
            },
        )
    {
        return false;
    }

    let parent_universal_count = parent_prefix
        .iter()
        .filter(|(quantifier, _)| is_effective_universal(*quantifier, polarity))
        .map(|(_, variables)| variables.len())
        .sum::<usize>();
    if parent_universal_count != step_universals.len() {
        return false;
    }

    let mut free_in_parent_matrix = HashSet::new();
    collect_free_variable_names(parent_matrix, &mut free_in_parent_matrix);

    let mut step_universal_idx = 0;
    let mut local_universals = Vec::new();
    let mut local_preserved = Vec::new();
    let mut local_existentials = Vec::new();
    for (quantifier, variables) in &parent_prefix {
        if is_effective_universal(*quantifier, polarity) {
            for parent_var in variables {
                let Some(step_var) = step_universals.get(step_universal_idx) else {
                    return false;
                };
                if state.universal_map.contains_key(parent_var)
                    || state.active_existentials.contains_key(parent_var)
                    || state.active_universals.contains(step_var)
                {
                    return false;
                }
                state
                    .universal_map
                    .insert(parent_var.clone(), step_var.clone());
                state
                    .universal_history
                    .insert(parent_var.clone(), step_var.clone());
                state.active_universals.push(step_var.clone());
                local_universals.push(parent_var.clone());
                step_universal_idx += 1;
            }
        } else {
            for parent_var in variables {
                if state.universal_map.contains_key(parent_var)
                    || state.active_existentials.contains_key(parent_var)
                {
                    return false;
                }
                if step_existentials.contains(parent_var) {
                    state
                        .universal_map
                        .insert(parent_var.clone(), parent_var.clone());
                    state
                        .universal_history
                        .insert(parent_var.clone(), parent_var.clone());
                    local_preserved.push(parent_var.clone());
                } else {
                    let allowed: HashSet<String> =
                        state.active_universals.iter().cloned().collect();
                    let required: HashSet<String> = state
                        .active_universals
                        .iter()
                        .filter(|s_var| {
                            state.universal_map.iter().any(|(p_var, s)| {
                                s == *s_var && free_in_parent_matrix.contains(p_var)
                            })
                        })
                        .cloned()
                        .collect();
                    state
                        .active_existentials
                        .insert(parent_var.clone(), SkolemScope { allowed, required });
                    local_existentials.push(parent_var.clone());
                }
            }
        }
    }

    if local_preserved.len() != step_existential_count {
        for parent_var in local_existentials.into_iter().rev() {
            state.active_existentials.remove(&parent_var);
        }
        for parent_var in local_preserved.into_iter().rev() {
            state.universal_map.remove(&parent_var);
        }
        for parent_var in local_universals.into_iter().rev() {
            state.universal_map.remove(&parent_var);
            state.active_universals.pop();
        }
        return false;
    }

    let matched = match_skolem_matrix(parent_matrix, step_matrix, state, polarity);
    for parent_var in local_existentials.into_iter().rev() {
        state.active_existentials.remove(&parent_var);
    }
    for parent_var in local_preserved.into_iter().rev() {
        state.universal_map.remove(&parent_var);
    }
    for parent_var in local_universals.into_iter().rev() {
        state.universal_map.remove(&parent_var);
        state.active_universals.pop();
    }
    matched
}

fn is_effective_universal(quantifier: Quantifier, polarity: bool) -> bool {
    matches!(
        (quantifier, polarity),
        (Quantifier::Forall, true) | (Quantifier::Exists, false)
    )
}

fn contains_skolemizable_existential(formula: &FOFFormula<'_>, polarity: bool) -> bool {
    let formula = strip_skolem_parens(formula);
    match formula {
        FOFFormula::Negation(inner) => contains_skolemizable_existential(inner, !polarity),
        FOFFormula::Quantified {
            quantifier,
            formula,
            ..
        } => {
            if !is_effective_universal(*quantifier, polarity) {
                true
            } else {
                contains_skolemizable_existential(formula, polarity)
            }
        }
        FOFFormula::Binary { left, right, .. } => {
            contains_skolemizable_existential(left, polarity)
                || contains_skolemizable_existential(right, polarity)
        }
        FOFFormula::Equality(_, _) | FOFFormula::Inequality(_, _) => false,
        FOFFormula::Atomic(_) => false,
        FOFFormula::Parens(_) => unreachable!("parentheses are stripped above"),
    }
}

fn leading_quantifiers<'a, 'p>(
    formula: &'a FOFFormula<'p>,
) -> (Vec<(Quantifier, Vec<String>)>, &'a FOFFormula<'p>) {
    let mut current = formula;
    let mut prefix = Vec::new();
    loop {
        while let FOFFormula::Parens(inner) = current {
            current = inner;
        }
        let FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } = current
        else {
            break;
        };
        prefix.push((
            *quantifier,
            variables
                .iter()
                .map(|variable| (*variable).to_string())
                .collect(),
        ));
        current = formula;
    }
    (prefix, current)
}

fn match_skolem_matrix(
    parent: &FOFFormula<'_>,
    step: &FOFFormula<'_>,
    state: &mut SkolemMatch,
    polarity: bool,
) -> bool {
    let parent = strip_skolem_parens(parent);
    let step = strip_skolem_parens(step);
    if matches!(parent, FOFFormula::Quantified { .. })
        || matches!(step, FOFFormula::Quantified { .. })
    {
        return match_skolem_formula_with_polarity(parent, step, state, polarity);
    }
    match (parent, step) {
        (FOFFormula::Atomic(parent), FOFFormula::Atomic(step)) => {
            match_skolem_atom(parent, step, state)
        }
        (FOFFormula::Negation(parent), FOFFormula::Negation(step)) => {
            match_skolem_formula_with_polarity(parent, step, state, !polarity)
        }
        (
            FOFFormula::Binary {
                left: parent_left,
                connective: parent_connective,
                right: parent_right,
            },
            FOFFormula::Binary {
                left: step_left,
                connective: step_connective,
                right: step_right,
            },
        ) if parent_connective == step_connective => {
            if matches!(
                parent_connective,
                BinaryConnective::And | BinaryConnective::Or
            ) {
                let parent_parts = flatten_skolem_associative(parent, *parent_connective);
                let step_parts = flatten_skolem_associative(step, *step_connective);
                let mut candidate_state = state.clone();
                if match_skolem_multiset(&parent_parts, &step_parts, &mut candidate_state, polarity)
                {
                    *state = candidate_state;
                    true
                } else {
                    let mut fallback_state = state.clone();
                    if match_skolem_formula_with_polarity(
                        parent_left,
                        step_left,
                        &mut fallback_state,
                        polarity,
                    ) && match_skolem_formula_with_polarity(
                        parent_right,
                        step_right,
                        &mut fallback_state,
                        polarity,
                    ) {
                        *state = fallback_state;
                        true
                    } else {
                        false
                    }
                }
            } else {
                match_skolem_formula_with_polarity(parent_left, step_left, state, polarity)
                    && match_skolem_formula_with_polarity(parent_right, step_right, state, polarity)
            }
        }
        (
            FOFFormula::Equality(parent_left, parent_right),
            FOFFormula::Equality(step_left, step_right),
        )
        | (
            FOFFormula::Inequality(parent_left, parent_right),
            FOFFormula::Inequality(step_left, step_right),
        ) => {
            match_skolem_term(parent_left, step_left, state)
                && match_skolem_term(parent_right, step_right, state)
        }
        _ => false,
    }
}

fn strip_skolem_parens<'a, 'p>(formula: &'a FOFFormula<'p>) -> &'a FOFFormula<'p> {
    let mut current = formula;
    while let FOFFormula::Parens(inner) = current {
        current = inner;
    }
    current
}

fn flatten_skolem_associative<'a, 'p>(
    formula: &'a FOFFormula<'p>,
    connective: BinaryConnective,
) -> Vec<&'a FOFFormula<'p>> {
    let mut result = Vec::new();
    let mut pending = vec![formula];
    while let Some(current) = pending.pop() {
        let current = strip_skolem_parens(current);
        if let FOFFormula::Binary {
            left,
            connective: current_connective,
            right,
        } = current
            && *current_connective == connective
        {
            pending.push(right);
            pending.push(left);
        } else {
            result.push(current);
        }
    }
    result
}

fn match_skolem_multiset(
    parent: &[&FOFFormula<'_>],
    step: &[&FOFFormula<'_>],
    state: &mut SkolemMatch,
    polarity: bool,
) -> bool {
    if parent.len() != step.len() {
        return false;
    }
    fn visit(
        parent: &[&FOFFormula<'_>],
        step: &[&FOFFormula<'_>],
        parent_idx: usize,
        used: &mut [bool],
        state: &mut SkolemMatch,
        polarity: bool,
    ) -> bool {
        if !state.charge() {
            return false;
        }
        if parent_idx == parent.len() {
            return true;
        }
        for step_idx in 0..step.len() {
            if used[step_idx] {
                continue;
            }
            let mut candidate = state.clone();
            if match_skolem_formula_with_polarity(
                parent[parent_idx],
                step[step_idx],
                &mut candidate,
                polarity,
            ) {
                used[step_idx] = true;
                if visit(parent, step, parent_idx + 1, used, &mut candidate, polarity) {
                    *state = candidate;
                    return true;
                }
                used[step_idx] = false;
            }
        }
        false
    }

    visit(
        parent,
        step,
        0,
        &mut vec![false; step.len()],
        state,
        polarity,
    )
}

fn match_skolem_atom(
    parent: &FOFAtomicFormula<'_>,
    step: &FOFAtomicFormula<'_>,
    state: &mut SkolemMatch,
) -> bool {
    match (parent, step) {
        (
            FOFAtomicFormula::Plain(parent_name, parent_args),
            FOFAtomicFormula::Plain(step_name, step_args),
        ) => {
            parent_name == step_name
                && parent_args.len() == step_args.len()
                && parent_args
                    .iter()
                    .zip(step_args)
                    .all(|(parent, step)| match_skolem_term(parent, step, state))
        }
        (
            FOFAtomicFormula::Defined(parent_name, parent_args),
            FOFAtomicFormula::Defined(step_name, step_args),
        ) => {
            parent_name == step_name
                && parent_args.len() == step_args.len()
                && parent_args
                    .iter()
                    .zip(step_args)
                    .all(|(parent, step)| match_skolem_term(parent, step, state))
        }
        (
            FOFAtomicFormula::System(parent_name, parent_args),
            FOFAtomicFormula::System(step_name, step_args),
        ) => {
            parent_name == step_name
                && parent_args.len() == step_args.len()
                && parent_args
                    .iter()
                    .zip(step_args)
                    .all(|(parent, step)| match_skolem_term(parent, step, state))
        }
        (FOFAtomicFormula::True, FOFAtomicFormula::True)
        | (FOFAtomicFormula::False, FOFAtomicFormula::False) => true,
        _ => false,
    }
}

fn match_skolem_term(parent: &FOFTerm<'_>, step: &FOFTerm<'_>, state: &mut SkolemMatch) -> bool {
    match parent {
        FOFTerm::Variable(parent_var) => {
            let parent_var = (*parent_var).to_string();
            if let Some(scope) = state.active_existentials.get(&parent_var).cloned() {
                let step_repr = format!("{step:?}");
                if let Some(previous) = state.existential_terms.get(&parent_var) {
                    return previous == &step_repr;
                }
                let Some((symbol, arguments)) = skolem_application(step) else {
                    return false;
                };
                let actual: Option<Vec<&str>> = arguments
                    .iter()
                    .map(|argument| match argument {
                        FOFTerm::Variable(variable) => Some(*variable),
                        _ => None,
                    })
                    .collect();
                let Some(actual) = actual else {
                    return false;
                };
                let actual_set: HashSet<&str> = actual.iter().copied().collect();
                let allowed_refs: HashSet<&str> =
                    scope.allowed.iter().map(String::as_str).collect();
                let required_refs: HashSet<&str> =
                    scope.required.iter().map(String::as_str).collect();
                if actual.len() != actual_set.len()
                    || !actual_set.is_subset(&allowed_refs)
                    || !required_refs.is_subset(&actual_set)
                    || !state.fresh_symbols.contains(&symbol)
                {
                    return false;
                }
                if let Some(owner) = state.witness_owners.get(&symbol)
                    && owner != &parent_var
                {
                    return false;
                }
                let witness_symbol = symbol.clone();
                state
                    .witness_owners
                    .insert(symbol.clone(), parent_var.clone());
                state.used_symbols.insert(symbol);
                state.existential_witnesses.insert(
                    parent_var.clone(),
                    (
                        witness_symbol,
                        actual.iter().map(|arg| (*arg).to_string()).collect(),
                    ),
                );
                state.existential_terms.insert(parent_var, step_repr);
                true
            } else if let Some(mapped) = state.universal_map.get(&parent_var) {
                matches!(step, FOFTerm::Variable(step_var) if *step_var == mapped)
            } else {
                matches!(step, FOFTerm::Variable(step_var) if *step_var == parent_var)
            }
        }
        FOFTerm::Function(parent_name, parent_args) => match step {
            FOFTerm::Function(step_name, step_args) => {
                parent_name == step_name
                    && parent_args.len() == step_args.len()
                    && parent_args
                        .iter()
                        .zip(step_args)
                        .all(|(parent, step)| match_skolem_term(parent, step, state))
            }
            _ => false,
        },
        FOFTerm::DefinedFunction(parent_name, parent_args) => match step {
            FOFTerm::DefinedFunction(step_name, step_args) => {
                parent_name == step_name
                    && parent_args.len() == step_args.len()
                    && parent_args
                        .iter()
                        .zip(step_args)
                        .all(|(parent, step)| match_skolem_term(parent, step, state))
            }
            _ => false,
        },
        FOFTerm::SystemFunction(parent_name, parent_args) => match step {
            FOFTerm::SystemFunction(step_name, step_args) => {
                parent_name == step_name
                    && parent_args.len() == step_args.len()
                    && parent_args
                        .iter()
                        .zip(step_args)
                        .all(|(parent, step)| match_skolem_term(parent, step, state))
            }
            _ => false,
        },
        FOFTerm::Number(parent) => {
            matches!(step, FOFTerm::Number(step) if parent.as_str() == step.as_str())
        }
        FOFTerm::DistinctObject(parent) => {
            matches!(step, FOFTerm::DistinctObject(step) if parent == step)
        }
    }
}

fn skolem_application<'a, 'p>(term: &'a FOFTerm<'p>) -> Option<(String, &'a [FOFTerm<'p>])> {
    match term {
        FOFTerm::Function(symbol, args) => Some((symbol.as_str().to_string(), args)),
        FOFTerm::DefinedFunction(symbol, args) => Some((format!("${}", symbol.0), args)),
        FOFTerm::SystemFunction(symbol, args) => Some((format!("$${}", symbol.0), args)),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Literal {
    positive: bool,
    atom: Atom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BranchContext {
    assumptions: Vec<BranchAssumption>,
    sat_context: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BranchAssumption {
    split_parent: usize,
    branch_index: usize,
    literal: Literal,
    sat_var: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AvatarSplitContext {
    split_parent: usize,
    parent_literals: Vec<Literal>,
    branch_vars: Vec<u32>,
    component_literal_indices: Vec<Vec<usize>>,
    inherited_vars: Vec<u32>,
    inherited_assumptions: Vec<BranchAssumption>,
}

fn verify_resolution(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("resolution rule must have two parents".into());
    }
    let Some(left) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive("resolution parent is not a supported clause".into());
    };
    let Some(mut right) = clause_from_formula(&parents[1], limits) else {
        return KernelVerdict::Inconclusive("resolution parent is not a supported clause".into());
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "resolution conclusion is not a supported clause".into(),
        );
    };
    if goal.is_empty()
        && parents.iter().any(|p| {
            matches!(p, Formula::False)
                || clause_from_formula(p, limits)
                    .map(|c| c.is_empty())
                    .unwrap_or(false)
        })
    {
        return KernelVerdict::Certified;
    }
    let shift = max_var_clause(&left).saturating_add(1);
    shift_clause(&mut right, shift);
    for (left_idx, left_literal) in left.iter().enumerate() {
        for (right_idx, right_literal) in right.iter().enumerate() {
            if left_literal.positive == right_literal.positive {
                continue;
            }
            let mut substitution = HashMap::new();
            let unified = match (&left_literal.atom, &right_literal.atom) {
                (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) => {
                    if left_symbol != right_symbol || left_args.len() != right_args.len() {
                        false
                    } else {
                        left_args
                            .iter()
                            .zip(right_args)
                            .all(|(left, right)| unify_terms(left, right, &mut substitution))
                    }
                }
                (Atom::Eq(ls, lt), Atom::Eq(rs, rt)) => {
                    let mut sub1 = HashMap::new();
                    if unify_terms(ls, rs, &mut sub1) && unify_terms(lt, rt, &mut sub1) {
                        substitution = sub1;
                        true
                    } else {
                        let mut sub2 = HashMap::new();
                        if unify_terms(ls, rt, &mut sub2) && unify_terms(lt, rs, &mut sub2) {
                            substitution = sub2;
                            true
                        } else {
                            false
                        }
                    }
                }
                _ => false,
            };
            if !unified {
                continue;
            }
            let mut resolvent = Vec::with_capacity(left.len() + right.len() - 2);
            for (idx, literal) in left.iter().enumerate() {
                if idx != left_idx {
                    resolvent.push(apply_substitution_literal(literal, &substitution));
                }
            }
            for (idx, literal) in right.iter().enumerate() {
                if idx != right_idx {
                    resolvent.push(apply_substitution_literal(literal, &substitution));
                }
            }
            if clause_alpha_equiv(&resolvent, &goal) {
                return KernelVerdict::Certified;
            }
            let mut deduplicated = Vec::with_capacity(resolvent.len());
            for lit in &resolvent {
                if !deduplicated.contains(lit) {
                    deduplicated.push(lit.clone());
                }
            }
            let mut deduplicated_goal = Vec::with_capacity(goal.len());
            for lit in &goal {
                if !deduplicated_goal.contains(lit) {
                    deduplicated_goal.push(lit.clone());
                }
            }
            if deduplicated.len() != resolvent.len()
                && clause_alpha_equiv(&deduplicated, &deduplicated_goal)
            {
                return KernelVerdict::Certified;
            }
        }
    }
    KernelVerdict::Rejected("resolution conclusion is not a parent resolvent".into())
}

fn verify_consequence(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() == 2 {
        return verify_resolution(parents, conclusion, limits);
    }
    if parents.is_empty() {
        return KernelVerdict::Rejected("consequence must have at least one parent".into());
    }
    let mut attempts = 0usize;
    let is_conclusion_false = matches!(conclusion, Formula::False)
        || clause_from_formula(conclusion, limits)
            .map(|c| c.is_empty())
            .unwrap_or(false);
    if is_conclusion_false
        && parents.iter().any(|p| {
            matches!(p, Formula::False)
                || clause_from_formula(p, limits)
                    .map(|c| c.is_empty())
                    .unwrap_or(false)
        })
    {
        return KernelVerdict::Certified;
    }
    for i in 0..parents.len() {
        for j in (i + 1)..parents.len() {
            // Pairwise resolution is quadratic in the parent count, and the
            // parent count is bounded only by proof-text size. Cap the
            // attempts so one crafted node cannot consume the whole
            // verification budget; exhaustion fails open.
            if attempts >= limits.max_equivalence_steps {
                return KernelVerdict::Inconclusive(
                    "consequence exceeded strict matching-step limit".into(),
                );
            }
            attempts += 1;
            let pair = [parents[i].clone(), parents[j].clone()];
            if matches!(
                verify_resolution(&pair, conclusion, limits),
                KernelVerdict::Certified
            ) {
                return KernelVerdict::Certified;
            }
        }
    }
    KernelVerdict::Rejected("consequence conclusion is not justified by parents".into())
}

fn verify_subsumption_resolution(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected(
            "subsumption_resolution must have a target and an active parent".into(),
        );
    }
    let Some(c0) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive(
            "subsumption_resolution parent 0 is not a supported clause".into(),
        );
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "subsumption_resolution conclusion is not a supported clause".into(),
        );
    };
    let Some(c1) = clause_from_formula(&parents[1], limits) else {
        return KernelVerdict::Inconclusive(
            "subsumption_resolution parent 1 is not a supported clause".into(),
        );
    };

    // A conclusion identical to either cited parent is entailed by that
    // parent alone, regardless of rule naming (COM130+1 c45780 restates
    // its active parent after chained simplifications whose intermediate
    // steps were subsumed away).
    if clause_alpha_equiv(&c0, &goal) || clause_alpha_equiv(&c1, &goal) {
        return KernelVerdict::Certified;
    }

    // Try both orderings: (target=c0, active=c1) and (target=c1, active=c0)
    for (target, active) in [(&c0, &c1), (&c1, &c0)] {
        if active.is_empty() {
            continue;
        }

        let mut matching_steps = 0;
        for removed_idx in 0..target.len() {
            let mut modified_target = target.clone();
            modified_target[removed_idx].positive = !modified_target[removed_idx].positive;
            match clause_subsumes_set(
                active,
                &modified_target,
                &mut matching_steps,
                limits.max_subsumption_steps,
            ) {
                Ok(false) => continue,
                Err(()) => {
                    return KernelVerdict::Inconclusive(
                        "subsumption_resolution exceeded strict matching-step limit".into(),
                    );
                }
                Ok(true) => {}
            }

            let expected: Vec<Literal> = target
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != removed_idx)
                .map(|(_, literal)| literal.clone())
                .collect();
            // The prover does not always condense intermediate clauses, so
            // the target may contain duplicate literals while the conclusion
            // is condensed (`C | L | L` vs `C | L`). Condense both sides
            // before comparing; duplicate literals are logically inert.
            if clause_alpha_equiv(&condense_clause(&expected), &condense_clause(&goal)) {
                return KernelVerdict::Certified;
            }
        }
    }

    KernelVerdict::Rejected(
        "subsumption_resolution conclusion is not the target with a justified literal removed"
            .into(),
    )
}

fn clause_subsumes(
    pattern: &[Literal],
    target: &[Literal],
    steps: &mut usize,
    step_limit: usize,
) -> Result<bool, ()> {
    match_subsumes(pattern, target, steps, step_limit, false)
}

/// Set-style subsumption for `subsumption_resolution`: the same target
/// literal may witness several pattern literals once they coincide under σ.
/// Search's `match_literals` already allows this; multiset matching rejects
/// valid steps such as MGT005+1 c1709 where both active literals become
/// `~greater(X7,X7)` and the flipped target holds only one copy. Sound:
/// set inclusion of σ(active) in the flipped target still yields
/// `active ∧ target ⊨ target \ {L}`.
fn clause_subsumes_set(
    pattern: &[Literal],
    target: &[Literal],
    steps: &mut usize,
    step_limit: usize,
) -> Result<bool, ()> {
    match_subsumes(pattern, target, steps, step_limit, true)
}

fn match_subsumes(
    pattern: &[Literal],
    target: &[Literal],
    steps: &mut usize,
    step_limit: usize,
    allow_target_reuse: bool,
) -> Result<bool, ()> {
    if pattern.len() > target.len() && !allow_target_reuse {
        return Ok(false);
    }

    // Keep target variables rigid while allowing only the standardized-apart
    // active-clause variables to receive matching substitutions.
    let shift = max_var_clause(target).saturating_add(1);
    let mut pattern = pattern.to_vec();
    shift_clause(&mut pattern, shift);
    let mut state = SubsumptionMatch {
        min_bindable: shift,
        used: vec![false; target.len()],
        steps,
        step_limit,
        allow_target_reuse,
    };
    match_subsumption_literals(&pattern, target, &HashMap::new(), &mut state)
}

/// Shared mutable state for one subsumption search.
struct SubsumptionMatch<'a> {
    min_bindable: VarId,
    used: Vec<bool>,
    steps: &'a mut usize,
    step_limit: usize,
    allow_target_reuse: bool,
}

fn match_subsumption_literals(
    remaining: &[Literal],
    target: &[Literal],
    substitution: &HashMap<VarId, Term>,
    state: &mut SubsumptionMatch<'_>,
) -> Result<bool, ()> {
    let Some((literal, rest)) = remaining.split_first() else {
        return Ok(true);
    };
    if *state.steps >= state.step_limit {
        return Err(());
    }
    *state.steps += 1;

    for (target_idx, target_literal) in target.iter().enumerate() {
        if !state.allow_target_reuse && state.used[target_idx] {
            continue;
        }
        if literal.positive != target_literal.positive {
            continue;
        }
        if let Some(next) = match_subsumption_atom(
            &literal.atom,
            &target_literal.atom,
            substitution,
            state.min_bindable,
        ) {
            state.used[target_idx] = true;
            let matched = match_subsumption_literals(rest, target, &next, state);
            state.used[target_idx] = false;
            match matched {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(()) => return Err(()),
            }
        }
    }
    Ok(false)
}

fn match_subsumption_atom(
    pattern: &Atom,
    target: &Atom,
    substitution: &HashMap<VarId, Term>,
    min_bindable: VarId,
) -> Option<HashMap<VarId, Term>> {
    match (pattern, target) {
        (Atom::Pred(pattern_symbol, pattern_args), Atom::Pred(target_symbol, target_args))
            if pattern_symbol == target_symbol && pattern_args.len() == target_args.len() =>
        {
            let mut next = substitution.clone();
            for (pattern, target) in pattern_args.iter().zip(target_args) {
                let pattern = apply_substitution_term(pattern, &next);
                if !match_subsumption_term(&pattern, target, &mut next, min_bindable) {
                    return None;
                }
            }
            Some(next)
        }
        (Atom::Eq(pattern_left, pattern_right), Atom::Eq(target_left, target_right)) => {
            let original_left = pattern_left;
            let original_right = pattern_right;
            let mut next = substitution.clone();
            let pattern_left = apply_substitution_term(original_left, &next);
            if match_subsumption_term(&pattern_left, target_left, &mut next, min_bindable) {
                let pattern_right = apply_substitution_term(original_right, &next);
                if match_subsumption_term(&pattern_right, target_right, &mut next, min_bindable) {
                    return Some(next);
                }
            }

            let mut next = substitution.clone();
            let pattern_left = apply_substitution_term(original_left, &next);
            if match_subsumption_term(&pattern_left, target_right, &mut next, min_bindable) {
                let pattern_right = apply_substitution_term(original_right, &next);
                if match_subsumption_term(&pattern_right, target_left, &mut next, min_bindable) {
                    return Some(next);
                }
            }
            None
        }
        _ => None,
    }
}

fn match_subsumption_term(
    pattern: &Term,
    target: &Term,
    substitution: &mut HashMap<VarId, Term>,
    min_bindable: VarId,
) -> bool {
    let pattern = apply_substitution_term(pattern, substitution);
    match (&pattern, target) {
        (Term::Var(var), target) if *var >= min_bindable => {
            if let Some(bound) = substitution.get(var) {
                bound == target
            } else {
                substitution.insert(*var, target.clone());
                true
            }
        }
        (Term::Var(_), Term::Var(target_var)) => pattern == Term::Var(*target_var),
        (Term::App(pattern_symbol, pattern_args), Term::App(target_symbol, target_args)) => {
            pattern_symbol == target_symbol
                && pattern_args.len() == target_args.len()
                && pattern_args
                    .iter()
                    .zip(target_args)
                    .all(|(pattern, target)| {
                        match_subsumption_term(pattern, target, substitution, min_bindable)
                    })
        }
        _ => false,
    }
}

fn verify_factoring(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("factoring rule must have one parent".into());
    }
    let Some(parent) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive("factoring parent is not a supported clause".into());
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "factoring conclusion is not a supported clause".into(),
        );
    };
    // Factoring may merge more than one pair of literals in a single
    // exported step (e.g. `X1 = X2 = X3` collapses three literals at
    // once). Accept any conclusion reachable by repeated single-factor
    // steps: each step removes exactly one literal, so the search depth is
    // bounded by the parent size.
    let goal_condensed = condense_clause(&goal);
    let step_cap = limits.max_subsumption_steps.max(5_000);
    let mut visited: usize = 0;
    let mut stack: Vec<Vec<Literal>> = Vec::new();
    // Seed with all one-step factors of the parent (at least one merge is
    // required; a verbatim repeat of the parent is not factoring).
    for first in 0..parent.len() {
        for second in (first + 1)..parent.len() {
            let left = &parent[first];
            let right = &parent[second];
            if left.positive != right.positive {
                continue;
            }
            let (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) =
                (&left.atom, &right.atom)
            else {
                continue;
            };
            if left_symbol != right_symbol || left_args.len() != right_args.len() {
                continue;
            }
            let mut substitution = HashMap::new();
            if !left_args
                .iter()
                .zip(right_args)
                .all(|(left, right)| unify_terms(left, right, &mut substitution))
            {
                continue;
            }
            let expected: Vec<Literal> = parent
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != second)
                .map(|(_, literal)| apply_substitution_literal(literal, &substitution))
                .collect();
            if condense_clause(&expected).len() < goal_condensed.len() {
                continue;
            }
            stack.push(expected);
        }
    }
    while let Some(state) = stack.pop() {
        visited += 1;
        if visited > step_cap {
            return KernelVerdict::Inconclusive("factoring search exceeded step limit".into());
        }
        // The prover condenses intermediate factors before export (MGT079+1
        // c9135 collapses a duplicate after the predicate merge).
        let condensed_state = condense_clause(&state);
        if clause_alpha_equiv(&state, &goal)
            || clause_alpha_equiv(&condensed_state, &goal_condensed)
        {
            return KernelVerdict::Certified;
        }
        if condensed_state.len() <= goal_condensed.len() {
            continue;
        }
        for first in 0..state.len() {
            for second in (first + 1)..state.len() {
                let left = &state[first];
                let right = &state[second];
                if left.positive != right.positive {
                    continue;
                }
                let (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) =
                    (&left.atom, &right.atom)
                else {
                    continue;
                };
                if left_symbol != right_symbol || left_args.len() != right_args.len() {
                    continue;
                }
                let mut substitution = HashMap::new();
                if !left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| unify_terms(left, right, &mut substitution))
                {
                    continue;
                }
                let expected: Vec<Literal> = state
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| *idx != second)
                    .map(|(_, literal)| apply_substitution_literal(literal, &substitution))
                    .collect();
                if condense_clause(&expected).len() < goal_condensed.len() {
                    continue;
                }
                stack.push(expected);
            }
        }
    }
    KernelVerdict::Rejected("factoring conclusion is not a valid factor".into())
}

fn verify_equality_resolution(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("equality_resolution must have one parent".into());
    }
    let Some(parent) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive(
            "equality_resolution parent is not a supported clause".into(),
        );
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "equality_resolution conclusion is not a supported clause".into(),
        );
    };
    for (removed, literal) in parent.iter().enumerate() {
        if literal.positive {
            continue;
        }
        let Atom::Eq(left, right) = &literal.atom else {
            continue;
        };
        let mut substitution = HashMap::new();
        if !unify_terms(left, right, &mut substitution) {
            continue;
        }
        let expected: Vec<Literal> = parent
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != removed)
            .map(|(_, literal)| apply_substitution_literal(literal, &substitution))
            .collect();
        // The prover may emit a condensed conclusion while the parent still
        // carries duplicate literals (CSR117+1); compare modulo condensation.
        if clause_alpha_equiv(&expected, &goal)
            || clause_alpha_equiv(&condense_clause(&expected), &condense_clause(&goal))
        {
            return KernelVerdict::Certified;
        }
    }
    KernelVerdict::Rejected(
        "equality_resolution conclusion is not a valid equality resolvent".into(),
    )
}

fn verify_equality_factoring(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("equality_factoring must have one parent".into());
    }
    let Some(parent) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive(
            "equality_factoring parent is not a supported clause".into(),
        );
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "equality_factoring conclusion is not a supported clause".into(),
        );
    };
    for first in 0..parent.len() {
        let Literal {
            positive: true,
            atom: Atom::Eq(first_left, first_right),
        } = &parent[first]
        else {
            continue;
        };
        for second in (first + 1)..parent.len() {
            let Literal {
                positive: true,
                atom: Atom::Eq(second_left, second_right),
            } = &parent[second]
            else {
                continue;
            };
            for (left, right, other_left, other_right) in [
                (first_left, first_right, second_left, second_right),
                (first_left, first_right, second_right, second_left),
                (first_right, first_left, second_left, second_right),
                (first_right, first_left, second_right, second_left),
            ] {
                let mut substitution = HashMap::new();
                if !unify_terms(left, other_left, &mut substitution) {
                    continue;
                }
                let mut expected = Vec::with_capacity(parent.len());
                expected.push(apply_substitution_literal(&parent[first], &substitution));
                expected.push(Literal {
                    positive: false,
                    atom: Atom::Eq(
                        apply_substitution_term(right, &substitution),
                        apply_substitution_term(other_right, &substitution),
                    ),
                });
                for (index, literal) in parent.iter().enumerate() {
                    if index != first && index != second {
                        expected.push(apply_substitution_literal(literal, &substitution));
                    }
                }
                if clause_alpha_equiv(&expected, &goal) {
                    return KernelVerdict::Certified;
                }
            }
        }
    }
    KernelVerdict::Rejected("equality_factoring conclusion is not a valid factor".into())
}

fn verify_condensation(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() != 1 {
        return KernelVerdict::Rejected("condensation must have one parent".into());
    }
    let Some(parent) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive("condensation parent is not a supported clause".into());
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "condensation conclusion is not a supported clause".into(),
        );
    };
    if parent.len() < 2 || goal.len() >= parent.len() {
        return KernelVerdict::Rejected("condensation must remove at least one literal".into());
    }

    let goal_condensed = condense_clause(&goal);
    for removed in 0..parent.len() {
        for matched in 0..parent.len() {
            if removed == matched || parent[removed].positive != parent[matched].positive {
                continue;
            }
            // Try every unifier of the removed/matched pair (both Eq
            // orientations). The prover matches one-way like search, so a
            // direct unifier can succeed with a substitution that does not
            // reach the goal while the flipped unifier does (CSR015+1).
            for substitution in
                unifiers_for_condensation(&parent[removed].atom, &parent[matched].atom)
            {
                let mut expected = Vec::with_capacity(parent.len() - 1);
                for (index, literal) in parent.iter().enumerate() {
                    if index == removed {
                        continue;
                    }
                    expected.push(apply_substitution_literal(literal, &substitution));
                }
                // Search deduplicates the factor before the subsumption and
                // goal checks; keep the same shape here so conclusions that
                // retain duplicate literals still compare equal after
                // condensing both sides (CSR115+6, SEV606+1, GEO111+1).
                let condensed_expected = condense_clause(&expected);
                let mut matching_steps = 0;
                let subsumes_parent = match clause_subsumes(
                    &condensed_expected,
                    &parent,
                    &mut matching_steps,
                    limits.max_subsumption_steps,
                ) {
                    Ok(value) => value,
                    Err(()) => {
                        return KernelVerdict::Inconclusive(
                            "condensation exceeded strict matching-step limit".into(),
                        );
                    }
                };
                if subsumes_parent
                    && (clause_alpha_equiv(&expected, &goal)
                        || clause_alpha_equiv(&condensed_expected, &goal_condensed))
                {
                    return KernelVerdict::Certified;
                }
            }
        }
    }
    KernelVerdict::Rejected("condensation conclusion is not a valid condensed clause".into())
}

/// All unifiers of `left` and `right` worth trying for condensation.
/// Equality tries both orientations independently: the direct unifier may
/// bind a variable in a way that misses the goal while the flipped unifier
/// is the identity the prover used (CSR015+1).
fn unifiers_for_condensation(left: &Atom, right: &Atom) -> Vec<HashMap<VarId, Term>> {
    match (left, right) {
        (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args))
            if left_symbol == right_symbol && left_args.len() == right_args.len() =>
        {
            let mut substitution = HashMap::new();
            if left_args
                .iter()
                .zip(right_args)
                .all(|(left, right)| unify_terms(left, right, &mut substitution))
            {
                vec![substitution]
            } else {
                Vec::new()
            }
        }
        (Atom::Eq(left_left, left_right), Atom::Eq(right_left, right_right)) => {
            let mut unifiers = Vec::new();
            let mut substitution = HashMap::new();
            if unify_terms(left_left, right_left, &mut substitution)
                && unify_terms(left_right, right_right, &mut substitution)
            {
                unifiers.push(substitution);
            }
            let mut substitution = HashMap::new();
            if unify_terms(left_left, right_right, &mut substitution)
                && unify_terms(left_right, right_left, &mut substitution)
            {
                unifiers.push(substitution);
            }
            unifiers
        }
        _ => Vec::new(),
    }
}

fn verify_demodulation(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 2 {
        return KernelVerdict::Rejected(
            "demodulation requires a target and at least one equality parent".into(),
        );
    }
    let Some(target) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive("demodulation target is not a supported clause".into());
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "demodulation conclusion is not a supported clause".into(),
        );
    };
    let mut orientation_choices: Vec<Vec<(Term, Term)>> = Vec::new();
    let mut condition_literals = Vec::new();
    for parent in parents[1..].iter() {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Inconclusive(
                "demodulation equality parent is not a supported clause".into(),
            );
        };
        let mut eq_idx = None;
        for (idx, lit) in clause.iter().enumerate() {
            if lit.positive && matches!(lit.atom, Atom::Eq(_, _)) {
                if eq_idx.is_some() {
                    return KernelVerdict::Rejected(
                        "demodulation parents must contain at most one positive equality".into(),
                    );
                }
                eq_idx = Some(idx);
            }
        }
        let Some(eq_idx) = eq_idx else {
            return KernelVerdict::Rejected(
                "demodulation parents must contain a positive equality".into(),
            );
        };
        for (idx, lit) in clause.iter().enumerate() {
            if idx == eq_idx || target.iter().any(|target_lit| target_lit == lit) {
                continue;
            }
            if literal_var_set(lit).is_empty() && goal.iter().any(|goal_lit| goal_lit == lit) {
                if !condition_literals.contains(lit) {
                    condition_literals.push(lit.clone());
                }
                continue;
            }
            return KernelVerdict::Rejected(
                "demodulation condition literal not satisfied in target or conclusion".into(),
            );
        }
        let Atom::Eq(left, right) = &clause[eq_idx].atom else {
            unreachable!();
        };
        if left == right {
            continue;
        }
        let left_vars = term_var_set(left);
        let right_vars = term_var_set(right);
        let left_weight = term_weight(left);
        let right_weight = term_weight(right);
        let mut parent_orientations = Vec::new();
        if right_vars.is_subset(&left_vars) && left_weight >= right_weight {
            parent_orientations.push((left.clone(), right.clone()));
        }
        if left_vars.is_subset(&right_vars) && right_weight >= left_weight {
            let rev = (right.clone(), left.clone());
            if !parent_orientations.contains(&rev) {
                parent_orientations.push(rev);
            }
        }
        if parent_orientations.is_empty() {
            return KernelVerdict::Rejected("demodulation has no non-trivial rewrite rule".into());
        }
        orientation_choices.push(parent_orientations);
    }
    if orientation_choices.is_empty() {
        return KernelVerdict::Rejected("demodulation has no non-trivial rewrite rule".into());
    }

    let mut candidate_rule_sets: Vec<Vec<(Term, Term)>> = vec![Vec::new()];
    for choices in orientation_choices {
        let mut next = Vec::new();
        for prev in &candidate_rule_sets {
            for choice in &choices {
                let mut combo = prev.clone();
                combo.push(choice.clone());
                next.push(combo);
                if next.len() >= 16 {
                    break;
                }
            }
            if next.len() >= 16 {
                break;
            }
        }
        candidate_rule_sets = next;
    }

    let mut hit_step_limit = false;
    for rules in candidate_rule_sets {
        let mut current = target.clone();
        current.extend(condition_literals.iter().cloned());
        let shift = max_var_clause(&current)
            .max(max_var_clause(&goal))
            .max(
                rules
                    .iter()
                    .flat_map(|(left, right)| [max_var_term(left), max_var_term(right)])
                    .max()
                    .unwrap_or(0),
            )
            .saturating_add(1);
        shift_clause(&mut current, shift);
        let mut steps = 0usize;
        let mut visited: Vec<Vec<Literal>> = Vec::new();
        loop {
            if clause_alpha_equiv(&current, &goal) {
                return KernelVerdict::Certified;
            }
            if steps >= limits.max_rewrite_steps {
                hit_step_limit = true;
                break;
            }
            if visited.iter().any(|v| v == &current) {
                break;
            }
            visited.push(current.clone());
            let mut changed = false;
            for literal in &mut current {
                if rewrite_atom(&mut literal.atom, &rules, &mut steps, limits) {
                    changed = true;
                    break;
                }
            }
            if !changed {
                break;
            }
        }
    }
    if hit_step_limit {
        KernelVerdict::Inconclusive("demodulation exceeded strict rewrite-step limit".into())
    } else {
        KernelVerdict::Inconclusive(
            "demodulation replay could not reach the conclusion within the implemented search"
                .into(),
        )
    }
}

fn verify_ac_normalization(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 2 {
        return KernelVerdict::Rejected(
            "ac_normalization requires a source clause and AC axioms".into(),
        );
    }
    let Some(source) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive("ac_normalization source is not a clause".into());
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive("ac_normalization conclusion is not a clause".into());
    };

    // AC axioms are cited as ordinary positive unit equalities. Their
    // presence makes the conclusion an equational consequence of the source;
    // the parent list is intentionally explicit so this rule is not a hidden
    // modulo-AC acceptance path.
    let mut commutative = HashSet::new();
    let mut associative = HashSet::new();
    for (parent_index, parent) in parents[1..].iter().enumerate() {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Inconclusive("ac_normalization axiom is not a clause".into());
        };
        if clause.len() != 1 || !clause[0].positive || !matches!(clause[0].atom, Atom::Eq(_, _)) {
            return KernelVerdict::Rejected(
                "ac_normalization parents must be positive unit equalities".into(),
            );
        }
        let Atom::Eq(left, right) = &clause[0].atom else {
            unreachable!()
        };
        let Some((symbol, kind)) = classify_ac_axiom(left, right) else {
            return KernelVerdict::Rejected(format!(
                "ac_normalization parent {parent_index} is not a commutativity or associativity axiom: {:?}",
                clause[0].atom
            ));
        };
        match kind {
            AcAxiomKind::Commutative => {
                commutative.insert(symbol);
            }
            AcAxiomKind::Associative => {
                associative.insert(symbol);
            }
        }
    }

    if (!commutative.is_empty() || !associative.is_empty())
        && ac_clause_alpha_equiv(&source, &goal, &commutative, &associative, limits)
    {
        return KernelVerdict::Certified;
    }
    if clause_alpha_equiv(&source, &goal) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Inconclusive(
            "ac_normalization replay could not establish AC-equivalence within the implemented search".into(),
        )
    }
}

/// Shape of a unit equality axiom recognised as commutativity or associativity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcAxiomKind {
    Commutative,
    Associative,
}

/// Classify a binary-function equation as commutativity or associativity.
///
/// Returns the function symbol and the axiom kind when `left = right` matches
/// `f(X,Y) = f(Y,X)` (commutative) or the nested associativity pattern.
pub fn classify_ac_axiom(left: &Term, right: &Term) -> Option<(mrs_core::SymbolId, AcAxiomKind)> {
    let (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args)) = (left, right)
    else {
        return None;
    };
    if left_symbol != right_symbol || left_args.len() != 2 || right_args.len() != 2 {
        return None;
    }
    if let (Term::Var(x), Term::Var(y), Term::Var(y2), Term::Var(x2)) =
        (&left_args[0], &left_args[1], &right_args[0], &right_args[1])
        && x != y
        && x == x2
        && y == y2
    {
        return Some((*left_symbol, AcAxiomKind::Commutative));
    }

    let associative =
        |left_nested: &Term, left_last: &Term, right_first: &Term, right_nested: &Term| -> bool {
            let (
                Term::App(left_inner_symbol, left_inner_args),
                left_last,
                right_first,
                Term::App(right_inner_symbol, right_inner_args),
            ) = (left_nested, left_last, right_first, right_nested)
            else {
                return false;
            };
            let (Term::Var(z), Term::Var(x2)) = (left_last, right_first) else {
                return false;
            };
            if left_inner_symbol != left_symbol
                || right_inner_symbol != left_symbol
                || left_inner_args.len() != 2
                || right_inner_args.len() != 2
            {
                return false;
            }
            let (Term::Var(x), Term::Var(y)) = (&left_inner_args[0], &left_inner_args[1]) else {
                return false;
            };
            let (Term::Var(y2), Term::Var(z2)) = (&right_inner_args[0], &right_inner_args[1])
            else {
                return false;
            };
            *x != *y && *y != *z && *x != *z && *x == *x2 && *y == *y2 && *z == *z2
        };

    if associative(&left_args[0], &left_args[1], &right_args[0], &right_args[1])
        || associative(&right_args[0], &right_args[1], &left_args[0], &left_args[1])
    {
        return Some((*left_symbol, AcAxiomKind::Associative));
    }
    None
}

fn ac_clause_alpha_equiv(
    left: &[Literal],
    right: &[Literal],
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    limits: VerificationLimits,
) -> bool {
    if left.len() != right.len() {
        return false;
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        index: usize,
        left: &[Literal],
        right: &[Literal],
        used: &mut [bool],
        mapping: &mut HashMap<VarId, VarId>,
        reverse: &mut HashMap<VarId, VarId>,
        commutative: &HashSet<mrs_core::SymbolId>,
        associative: &HashSet<mrs_core::SymbolId>,
        steps: &mut usize,
        limits: VerificationLimits,
    ) -> bool {
        if *steps >= limits.max_equivalence_steps {
            return false;
        }
        if index == left.len() {
            return true;
        }
        for right_index in 0..right.len() {
            if used[right_index] || left[index].positive != right[right_index].positive {
                continue;
            }
            used[right_index] = true;
            let mut branch_mapping = mapping.clone();
            let mut branch_reverse = reverse.clone();
            let matched = ac_atom_matches(
                &left[index].atom,
                &right[right_index].atom,
                &mut branch_mapping,
                &mut branch_reverse,
                commutative,
                associative,
                steps,
                limits,
                &mut |next_m, next_r, steps| {
                    let mut m = next_m.clone();
                    let mut r = next_r.clone();
                    visit(
                        index + 1,
                        left,
                        right,
                        used,
                        &mut m,
                        &mut r,
                        commutative,
                        associative,
                        steps,
                        limits,
                    )
                },
            );
            used[right_index] = false;
            if matched {
                return true;
            }
        }
        false
    }

    visit(
        0,
        left,
        right,
        &mut vec![false; right.len()],
        &mut HashMap::new(),
        &mut HashMap::new(),
        commutative,
        associative,
        &mut 0,
        limits,
    )
}

type AcCallback<'a> =
    &'a mut dyn FnMut(&HashMap<VarId, VarId>, &HashMap<VarId, VarId>, &mut usize) -> bool;

#[allow(clippy::too_many_arguments)]
fn ac_atom_matches(
    left: &Atom,
    right: &Atom,
    mapping: &mut HashMap<VarId, VarId>,
    reverse: &mut HashMap<VarId, VarId>,
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    steps: &mut usize,
    limits: VerificationLimits,
    callback: AcCallback<'_>,
) -> bool {
    match (left, right) {
        (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) => {
            if left_symbol != right_symbol || left_args.len() != right_args.len() {
                false
            } else {
                ac_args_matches(
                    left_args,
                    right_args,
                    0,
                    mapping,
                    reverse,
                    commutative,
                    associative,
                    steps,
                    limits,
                    callback,
                )
            }
        }
        (Atom::Eq(left_left, left_right), Atom::Eq(right_left, right_right)) => {
            let mut direct_mapping = mapping.clone();
            let mut direct_reverse = reverse.clone();
            let direct_matched = ac_term_matches(
                left_left,
                right_left,
                &mut direct_mapping,
                &mut direct_reverse,
                commutative,
                associative,
                steps,
                limits,
                &mut |m, r, steps| {
                    let mut next_m = m.clone();
                    let mut next_r = r.clone();
                    ac_term_matches(
                        left_right,
                        right_right,
                        &mut next_m,
                        &mut next_r,
                        commutative,
                        associative,
                        steps,
                        limits,
                        &mut *callback,
                    )
                },
            );
            if direct_matched {
                return true;
            }
            let mut flipped_mapping = mapping.clone();
            let mut flipped_reverse = reverse.clone();
            ac_term_matches(
                left_left,
                right_right,
                &mut flipped_mapping,
                &mut flipped_reverse,
                commutative,
                associative,
                steps,
                limits,
                &mut |m, r, steps| {
                    let mut next_m = m.clone();
                    let mut next_r = r.clone();
                    ac_term_matches(
                        left_right,
                        right_left,
                        &mut next_m,
                        &mut next_r,
                        commutative,
                        associative,
                        steps,
                        limits,
                        &mut *callback,
                    )
                },
            )
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn ac_args_matches(
    left_args: &[Term],
    right_args: &[Term],
    index: usize,
    mapping: &mut HashMap<VarId, VarId>,
    reverse: &mut HashMap<VarId, VarId>,
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    steps: &mut usize,
    limits: VerificationLimits,
    callback: AcCallback<'_>,
) -> bool {
    if *steps >= limits.max_equivalence_steps {
        return false;
    }
    if index == left_args.len() {
        return callback(mapping, reverse, steps);
    }
    ac_term_matches(
        &left_args[index],
        &right_args[index],
        mapping,
        reverse,
        commutative,
        associative,
        steps,
        limits,
        &mut |m, r, steps| {
            let mut next_m = m.clone();
            let mut next_r = r.clone();
            ac_args_matches(
                left_args,
                right_args,
                index + 1,
                &mut next_m,
                &mut next_r,
                commutative,
                associative,
                steps,
                limits,
                &mut *callback,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn ac_term_matches(
    left: &Term,
    right: &Term,
    mapping: &mut HashMap<VarId, VarId>,
    reverse: &mut HashMap<VarId, VarId>,
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    steps: &mut usize,
    limits: VerificationLimits,
    callback: AcCallback<'_>,
) -> bool {
    if *steps >= limits.max_equivalence_steps {
        return false;
    }
    *steps += 1;
    match (left, right) {
        (Term::Var(left_var), Term::Var(right_var)) => {
            if let Some(mapped) = mapping.get(left_var) {
                if mapped == right_var {
                    callback(mapping, reverse, steps)
                } else {
                    false
                }
            } else if reverse.contains_key(right_var) {
                false
            } else {
                mapping.insert(*left_var, *right_var);
                reverse.insert(*right_var, *left_var);
                let res = callback(mapping, reverse, steps);
                mapping.remove(left_var);
                reverse.remove(right_var);
                res
            }
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args))
            if left_symbol == right_symbol =>
        {
            if associative.contains(left_symbol) && commutative.contains(left_symbol) {
                let left_leaves = flatten_ac_term(left, *left_symbol, associative);
                let right_leaves = flatten_ac_term(right, *right_symbol, associative);
                if left_leaves.len() != right_leaves.len() {
                    return false;
                }
                match_ac_leaves(
                    &left_leaves,
                    &right_leaves,
                    0,
                    &mut vec![false; right_leaves.len()],
                    mapping,
                    reverse,
                    commutative,
                    associative,
                    steps,
                    limits,
                    callback,
                )
            } else if commutative.contains(left_symbol)
                && left_args.len() == 2
                && right_args.len() == 2
            {
                let mut direct_mapping = mapping.clone();
                let mut direct_reverse = reverse.clone();
                let direct_matched = ac_term_matches(
                    &left_args[0],
                    &right_args[0],
                    &mut direct_mapping,
                    &mut direct_reverse,
                    commutative,
                    associative,
                    steps,
                    limits,
                    &mut |m, r, steps| {
                        let mut next_m = m.clone();
                        let mut next_r = r.clone();
                        ac_term_matches(
                            &left_args[1],
                            &right_args[1],
                            &mut next_m,
                            &mut next_r,
                            commutative,
                            associative,
                            steps,
                            limits,
                            &mut *callback,
                        )
                    },
                );
                if direct_matched {
                    return true;
                }
                let mut flipped_mapping = mapping.clone();
                let mut flipped_reverse = reverse.clone();
                ac_term_matches(
                    &left_args[0],
                    &right_args[1],
                    &mut flipped_mapping,
                    &mut flipped_reverse,
                    commutative,
                    associative,
                    steps,
                    limits,
                    &mut |m, r, steps| {
                        let mut next_m = m.clone();
                        let mut next_r = r.clone();
                        ac_term_matches(
                            &left_args[1],
                            &right_args[0],
                            &mut next_m,
                            &mut next_r,
                            commutative,
                            associative,
                            steps,
                            limits,
                            &mut *callback,
                        )
                    },
                )
            } else {
                if left_args.len() != right_args.len() {
                    return false;
                }
                ac_args_matches(
                    left_args,
                    right_args,
                    0,
                    mapping,
                    reverse,
                    commutative,
                    associative,
                    steps,
                    limits,
                    callback,
                )
            }
        }
        _ => false,
    }
}

fn flatten_ac_term<'a>(
    term: &'a Term,
    symbol: mrs_core::SymbolId,
    associative: &HashSet<mrs_core::SymbolId>,
) -> Vec<&'a Term> {
    match term {
        Term::App(current, args) if *current == symbol && associative.contains(current) => args
            .iter()
            .flat_map(|arg| flatten_ac_term(arg, symbol, associative))
            .collect(),
        _ => vec![term],
    }
}

#[allow(clippy::too_many_arguments)]
fn match_ac_leaves(
    left: &[&Term],
    right: &[&Term],
    index: usize,
    used: &mut [bool],
    mapping: &mut HashMap<VarId, VarId>,
    reverse: &mut HashMap<VarId, VarId>,
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    steps: &mut usize,
    limits: VerificationLimits,
    callback: AcCallback<'_>,
) -> bool {
    if *steps >= limits.max_equivalence_steps {
        return false;
    }
    if index == left.len() {
        return callback(mapping, reverse, steps);
    }
    let mut candidates: Vec<_> = (0..right.len()).filter(|&i| !used[i]).collect();
    candidates.sort_by_key(|&right_index| {
        ac_match_priority(left[index], right[right_index], mapping, reverse)
    });
    for right_index in candidates {
        used[right_index] = true;
        let matched = ac_term_matches(
            left[index],
            right[right_index],
            mapping,
            reverse,
            commutative,
            associative,
            steps,
            limits,
            &mut |next_m, next_r, steps| {
                let mut m = next_m.clone();
                let mut r = next_r.clone();
                match_ac_leaves(
                    left,
                    right,
                    index + 1,
                    used,
                    &mut m,
                    &mut r,
                    commutative,
                    associative,
                    steps,
                    limits,
                    &mut *callback,
                )
            },
        );
        used[right_index] = false;
        if matched {
            return true;
        }
    }
    false
}

fn ac_match_priority(
    left: &Term,
    right: &Term,
    mapping: &HashMap<VarId, VarId>,
    reverse: &HashMap<VarId, VarId>,
) -> u8 {
    if left == right {
        return 0;
    }
    match (left, right) {
        (Term::Var(left), Term::Var(right)) if mapping.get(left) == Some(right) => 1,
        (Term::Var(left), Term::Var(right)) if left == right && !reverse.contains_key(right) => 2,
        _ => 3,
    }
}

fn verify_goal_transformation(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.is_empty() {
        return KernelVerdict::Rejected("goal_transformation requires at least one parent".into());
    }
    if parents.len() == 1 {
        return verify_alpha_identity(parents, conclusion);
    }
    verify_demodulation(parents, conclusion, limits)
}

fn verify_superposition(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
    background_commutative: &HashSet<mrs_core::SymbolId>,
    background_associative: &HashSet<mrs_core::SymbolId>,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("superposition must have two parents".into());
    }
    let Some(equation_clause) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive(
            "superposition equality parent is not a supported clause".into(),
        );
    };
    let Some(mut target_clause) = clause_from_formula(&parents[1], limits) else {
        return KernelVerdict::Inconclusive(
            "superposition target parent is not a supported clause".into(),
        );
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "superposition conclusion is not a supported clause".into(),
        );
    };

    let target_shift = max_var_clause(&equation_clause).saturating_add(1);
    shift_clause(&mut target_clause, target_shift);
    for (equation_idx, equation_literal) in equation_clause.iter().enumerate() {
        if !equation_literal.positive {
            continue;
        }
        let Atom::Eq(left, right) = &equation_literal.atom else {
            continue;
        };
        for (from, to) in [(left, right), (right, left)] {
            if matches!(from, Term::Var(_)) {
                continue;
            }
            for (target_idx, target_literal) in target_clause.iter().enumerate() {
                let positions = atom_term_positions(&target_literal.atom);
                for (side, position) in positions {
                    let base = match &target_literal.atom {
                        Atom::Pred(_, args) => args.get(side),
                        Atom::Eq(left, right) => {
                            if side == 0 {
                                Some(left)
                            } else {
                                Some(right)
                            }
                        }
                    };
                    let Some(base) = base else { continue };
                    let Some(subterm) = term_at_position(base, &position) else {
                        continue;
                    };
                    let mut substitution = HashMap::new();
                    if !unify_terms(from, subterm, &mut substitution) {
                        continue;
                    }
                    let replacement = apply_substitution_term(to, &substitution);
                    let replaced_base = replace_term_at(base, &position, replacement);
                    let replaced_atom =
                        replace_atom_side(&target_literal.atom, side, replaced_base);
                    let mut expected = Vec::with_capacity(
                        equation_clause.len() + target_clause.len().saturating_sub(1),
                    );
                    for (idx, literal) in equation_clause.iter().enumerate() {
                        if idx != equation_idx {
                            expected.push(apply_substitution_literal(literal, &substitution));
                        }
                    }
                    for (idx, literal) in target_clause.iter().enumerate() {
                        if idx != target_idx {
                            expected.push(apply_substitution_literal(literal, &substitution));
                        } else {
                            expected.push(Literal {
                                positive: literal.positive,
                                atom: apply_substitution_atom(&replaced_atom, &substitution),
                            });
                        }
                    }
                    if clause_alpha_equiv(&expected, &goal) {
                        return KernelVerdict::Certified;
                    }
                    // The prover condenses duplicate literals (e.g. shared
                    // AVATAR assumptions from both parents), so also accept
                    // the conclusion modulo condensation (GEO127+1 c11104
                    // merges a duplicated `~spl0_1`).
                    if clause_alpha_equiv(&condense_clause(&expected), &condense_clause(&goal)) {
                        return KernelVerdict::Certified;
                    }
                    // Folded demodulation: the prover simplifies
                    // superposition conclusions with unit equalities, citing
                    // only the original parents (SET637+1 c21169 rewrites
                    // `intersection(empty_set, empty_set)` with the cited
                    // equation itself). Accept conclusions reachable by
                    // bounded rewriting with the cited unit equation.
                    if equation_clause.len() == 1
                        && let Atom::Eq(unit_left, unit_right) = &equation_literal.atom
                        && unit_rewrite_closure_matches(
                            unit_left, unit_right, &expected, &goal, limits,
                        )
                    {
                        return KernelVerdict::Certified;
                    }
                }
            }
        }
    }
    // Background-theory AC replay: the prover's superposition is AC-aware
    // even when exported as plain `superposition` (SWV486+1 rewrites with
    // commutativity of `plus`, cited nowhere). Retrying against problem
    // AC axioms is sound: they are premises of the problem itself.
    if background_ac_fallback(
        parents,
        &goal,
        limits,
        background_commutative,
        background_associative,
    ) {
        return KernelVerdict::Certified;
    }
    KernelVerdict::Rejected("superposition conclusion is not a valid rewrite".into())
}

/// Background-AC fallback shared by the superposition entry points below.
/// Retries a failed plain check as AC superposition against problem-theory
/// axioms (see the collection site in the main loop).
fn background_ac_fallback(
    parents: &[Formula],
    goal: &[Literal],
    limits: VerificationLimits,
    background_commutative: &HashSet<mrs_core::SymbolId>,
    background_associative: &HashSet<mrs_core::SymbolId>,
) -> bool {
    if background_commutative.is_empty() && background_associative.is_empty() {
        return false;
    }
    let Ok([first, second]) = <&[Formula; 2]>::try_from(parents) else {
        return false;
    };
    for (equation_parent, target_parent) in [(first, second), (second, first)] {
        let (Some(equation_clause), Some(target_clause)) = (
            clause_from_formula(equation_parent, limits),
            clause_from_formula(target_parent, limits),
        ) else {
            continue;
        };
        if ac_superposition_replay(
            &equation_clause,
            &target_clause,
            goal,
            background_commutative,
            background_associative,
            limits,
        ) {
            return true;
        }
    }
    false
}

/// Whether `goal` is reachable from `start` by bounded rewriting with the
/// unit equation `left = right` (either orientation), comparing modulo
/// condensation. Every rewrite step is an instance of a cited unit
/// premise, hence sound; the state cap keeps the search decidable and
/// exceeding it simply fails closed (`false`, never unsound).
fn unit_rewrite_closure_matches(
    left: &Term,
    right: &Term,
    start: &[Literal],
    goal: &[Literal],
    limits: VerificationLimits,
) -> bool {
    let condensed_goal = condense_clause(goal);
    let mut visited: Vec<Vec<Literal>> = Vec::with_capacity(limits.max_rewrite_steps.max(1));
    let mut stack = vec![start.to_vec(), condense_clause(start)];
    while let Some(state) = stack.pop() {
        if visited.len() >= limits.max_rewrite_steps.max(1) {
            return false;
        }
        if visited.iter().any(|seen| clause_alpha_equiv(seen, &state)) {
            continue;
        }
        visited.push(state.clone());
        if clause_alpha_equiv(&condense_clause(&state), &condensed_goal) {
            return true;
        }
        for (literal_idx, literal) in state.iter().enumerate() {
            for (from, to) in [(left, right), (right, left)] {
                if matches!(from, Term::Var(_)) {
                    continue;
                }
                for (side, position) in atom_term_positions(&literal.atom) {
                    let base = match &literal.atom {
                        Atom::Pred(_, args) => args.get(side),
                        Atom::Eq(left_term, right_term) => {
                            if side == 0 {
                                Some(left_term)
                            } else {
                                Some(right_term)
                            }
                        }
                    };
                    let Some(base) = base else { continue };
                    let Some(subterm) = term_at_position(base, &position) else {
                        continue;
                    };
                    let mut substitution = HashMap::new();
                    if !unify_terms(from, subterm, &mut substitution) {
                        continue;
                    }
                    // Simplifications only: the replacement instance must
                    // be strictly smaller than the matched instance.
                    // Besides mirroring demodulation, this guarantees
                    // termination (no rewrite cycles) and keeps the
                    // expanding direction from starving the search.
                    let from_instance = apply_substitution_term(from, &substitution);
                    let to_instance = apply_substitution_term(to, &substitution);
                    if term_size(&to_instance) >= term_size(&from_instance) {
                        continue;
                    }
                    let replacement = to_instance;
                    let replaced_base = replace_term_at(base, &position, replacement);
                    let replaced_atom = replace_atom_side(&literal.atom, side, replaced_base);
                    let mut next: Vec<Literal> = state
                        .iter()
                        .enumerate()
                        .filter(|(idx, _)| *idx != literal_idx)
                        .map(|(_, other)| apply_substitution_literal(other, &substitution))
                        .collect();
                    next.push(Literal {
                        positive: literal.positive,
                        atom: apply_substitution_atom(&replaced_atom, &substitution),
                    });
                    stack.push(next);
                }
            }
        }
    }
    false
}

fn verify_ac_superposition(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
    background_commutative: &HashSet<mrs_core::SymbolId>,
    background_associative: &HashSet<mrs_core::SymbolId>,
) -> KernelVerdict {
    if parents.len() < 3 {
        return KernelVerdict::Rejected(
            "ac_superposition requires two inference parents and AC axioms".into(),
        );
    }
    let inference = verify_superposition(
        &parents[..2],
        conclusion,
        limits,
        background_commutative,
        background_associative,
    );
    let source = match clause_from_formula(&parents[0], limits) {
        Some(clause) => clause,
        None => return KernelVerdict::Inconclusive("AC source is not a clause".into()),
    };
    let target = match clause_from_formula(&parents[1], limits) {
        Some(clause) => clause,
        None => return KernelVerdict::Inconclusive("AC target is not a clause".into()),
    };
    let goal = match clause_from_formula(conclusion, limits) {
        Some(clause) => clause,
        None => return KernelVerdict::Inconclusive("AC conclusion is not a clause".into()),
    };
    let mut commutative = HashSet::new();
    let mut associative = HashSet::new();
    for parent in &parents[2..] {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Inconclusive("AC axiom is not a clause".into());
        };
        if clause.len() != 1 || !clause[0].positive {
            return KernelVerdict::Rejected("AC axiom is not a positive unit equality".into());
        }
        let Atom::Eq(left, right) = &clause[0].atom else {
            return KernelVerdict::Rejected("AC axiom is not an equality".into());
        };
        let Some((symbol, kind)) = classify_ac_axiom(left, right) else {
            return KernelVerdict::Rejected("AC axiom has unsupported shape".into());
        };
        match kind {
            AcAxiomKind::Commutative => {
                commutative.insert(symbol);
            }
            AcAxiomKind::Associative => {
                associative.insert(symbol);
            }
        }
    }

    if ac_superposition_replay(&source, &target, &goal, &commutative, &associative, limits) {
        KernelVerdict::Certified
    } else {
        match inference {
            KernelVerdict::Rejected(reason) => KernelVerdict::Inconclusive(format!(
                "ac_superposition replay is incomplete: {reason}"
            )),
            other => other,
        }
    }
}

fn verify_ac_resolution(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.len() < 3 {
        return KernelVerdict::Rejected(
            "ac_resolution requires two inference parents and AC axioms".into(),
        );
    }
    let left = match clause_from_formula(&parents[0], limits) {
        Some(clause) => clause,
        None => return KernelVerdict::Inconclusive("AC left parent is not a clause".into()),
    };
    let right = match clause_from_formula(&parents[1], limits) {
        Some(clause) => clause,
        None => return KernelVerdict::Inconclusive("AC right parent is not a clause".into()),
    };
    let goal = match clause_from_formula(conclusion, limits) {
        Some(clause) => clause,
        None => return KernelVerdict::Inconclusive("AC conclusion is not a clause".into()),
    };
    let mut commutative = HashSet::new();
    let mut associative = HashSet::new();
    for parent in &parents[2..] {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Inconclusive("AC axiom is not a clause".into());
        };
        if clause.len() != 1 || !clause[0].positive {
            return KernelVerdict::Rejected("AC axiom is not a positive unit equality".into());
        }
        let Atom::Eq(left, right) = &clause[0].atom else {
            return KernelVerdict::Rejected("AC axiom is not an equality".into());
        };
        let Some((symbol, kind)) = classify_ac_axiom(left, right) else {
            return KernelVerdict::Rejected("AC axiom has unsupported shape".into());
        };
        match kind {
            AcAxiomKind::Commutative => {
                commutative.insert(symbol);
            }
            AcAxiomKind::Associative => {
                associative.insert(symbol);
            }
        }
    }

    if ac_resolution_replay(&left, &right, &goal, &commutative, &associative, limits) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Inconclusive(
            "ac_resolution replay could not establish equivalence within limits".into(),
        )
    }
}

fn ac_resolution_replay(
    left: &[Literal],
    right: &[Literal],
    goal: &[Literal],
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    limits: VerificationLimits,
) -> bool {
    let shift = max_var_clause(left).saturating_add(1);
    let mut shifted_right = right.to_vec();
    shift_clause(&mut shifted_right, shift);

    for (left_idx, left_literal) in left.iter().enumerate() {
        for (right_idx, right_literal) in shifted_right.iter().enumerate() {
            if left_literal.positive == right_literal.positive {
                continue;
            }
            let (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) =
                (&left_literal.atom, &right_literal.atom)
            else {
                continue;
            };
            if left_symbol != right_symbol || left_args.len() != right_args.len() {
                continue;
            }
            let mut substitution = HashMap::new();
            if !left_args.iter().zip(right_args).all(|(l, r)| {
                ac_unify_terms(l, r, &mut substitution, commutative, associative, limits)
            }) {
                continue;
            }
            let mut resolvent = Vec::with_capacity(left.len() + shifted_right.len() - 2);
            for (idx, literal) in left.iter().enumerate() {
                if idx != left_idx {
                    resolvent.push(apply_substitution_literal(literal, &substitution));
                }
            }
            for (idx, literal) in shifted_right.iter().enumerate() {
                if idx != right_idx {
                    resolvent.push(apply_substitution_literal(literal, &substitution));
                }
            }
            if ac_clause_alpha_equiv(&resolvent, goal, commutative, associative, limits) {
                return true;
            }
            let mut deduplicated = Vec::with_capacity(resolvent.len());
            for lit in &resolvent {
                if !deduplicated.contains(lit) {
                    deduplicated.push(lit.clone());
                }
            }
            if ac_clause_alpha_equiv(&deduplicated, goal, commutative, associative, limits) {
                return true;
            }
        }
    }
    false
}

fn ac_superposition_replay(
    equation_clause: &[Literal],
    target_clause: &[Literal],
    goal: &[Literal],
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    limits: VerificationLimits,
) -> bool {
    let target_shift = max_var_clause(equation_clause).saturating_add(1);
    let mut shifted_target = target_clause.to_vec();
    shift_clause(&mut shifted_target, target_shift);
    for (equation_index, equation_literal) in equation_clause.iter().enumerate() {
        if !equation_literal.positive {
            continue;
        }
        let Atom::Eq(left, right) = &equation_literal.atom else {
            continue;
        };
        for (from, to) in [(left, right), (right, left)] {
            if matches!(from, Term::Var(_)) {
                continue;
            }
            for (target_index, target_literal) in shifted_target.iter().enumerate() {
                for (side, position) in atom_term_positions(&target_literal.atom) {
                    let base = match &target_literal.atom {
                        Atom::Pred(_, args) => args.get(side),
                        Atom::Eq(left, right) => Some(if side == 0 { left } else { right }),
                    };
                    let Some(base) = base else { continue };
                    let Some(subterm) = term_at_position(base, &position) else {
                        continue;
                    };
                    let mut substitution = HashMap::new();
                    if !ac_unify_terms(
                        from,
                        subterm,
                        &mut substitution,
                        commutative,
                        associative,
                        limits,
                    ) {
                        continue;
                    }
                    let replacement = apply_substitution_term(to, &substitution);
                    let replaced_base = replace_term_at(base, &position, replacement);
                    let replaced_atom =
                        replace_atom_side(&target_literal.atom, side, replaced_base);
                    let mut expected = Vec::new();
                    for (index, literal) in equation_clause.iter().enumerate() {
                        if index != equation_index {
                            expected.push(apply_substitution_literal(literal, &substitution));
                        }
                    }
                    for (index, literal) in shifted_target.iter().enumerate() {
                        if index != target_index {
                            expected.push(apply_substitution_literal(literal, &substitution));
                        } else {
                            expected.push(Literal {
                                positive: literal.positive,
                                atom: apply_substitution_atom(&replaced_atom, &substitution),
                            });
                        }
                    }
                    if clause_alpha_equiv(&expected, goal)
                        || ac_clause_alpha_equiv(&expected, goal, commutative, associative, limits)
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn ac_unify_terms(
    left: &Term,
    right: &Term,
    substitution: &mut HashMap<VarId, Term>,
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    limits: VerificationLimits,
) -> bool {
    let left = apply_substitution_term(left, substitution);
    let right = apply_substitution_term(right, substitution);
    if left == right {
        return true;
    }
    match (&left, &right) {
        (Term::Var(var), term) | (term, Term::Var(var)) => {
            if occurs(*var, term, substitution) {
                false
            } else {
                substitution.insert(*var, term.clone());
                true
            }
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args))
            if left_symbol == right_symbol
                && associative.contains(left_symbol)
                && commutative.contains(left_symbol) =>
        {
            let left_leaves = flatten_ac_term(&left, *left_symbol, associative);
            let right_leaves = flatten_ac_term(&right, *right_symbol, associative);
            if left_leaves.len() != right_leaves.len() {
                return false;
            }
            let mut used = vec![false; right_leaves.len()];
            ac_unify_multiset(
                &left_leaves,
                &right_leaves,
                0,
                &mut used,
                substitution,
                commutative,
                associative,
                limits,
            )
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args))
            if left_symbol == right_symbol && associative.contains(left_symbol) =>
        {
            left_args.len() == right_args.len()
                && left_args.iter().zip(right_args).all(|(left, right)| {
                    ac_unify_terms(left, right, substitution, commutative, associative, limits)
                })
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args))
            if left_symbol == right_symbol
                && commutative.contains(left_symbol)
                && left_args.len() == 2
                && right_args.len() == 2 =>
        {
            let mut direct = substitution.clone();
            if ac_unify_terms(
                &left_args[0],
                &right_args[0],
                &mut direct,
                commutative,
                associative,
                limits,
            ) && ac_unify_terms(
                &left_args[1],
                &right_args[1],
                &mut direct,
                commutative,
                associative,
                limits,
            ) {
                *substitution = direct;
                true
            } else {
                let mut swapped = substitution.clone();
                if ac_unify_terms(
                    &left_args[0],
                    &right_args[1],
                    &mut swapped,
                    commutative,
                    associative,
                    limits,
                ) && ac_unify_terms(
                    &left_args[1],
                    &right_args[0],
                    &mut swapped,
                    commutative,
                    associative,
                    limits,
                ) {
                    *substitution = swapped;
                    true
                } else {
                    false
                }
            }
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args.iter().zip(right_args).all(|(left, right)| {
                    ac_unify_terms(left, right, substitution, commutative, associative, limits)
                })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn ac_unify_multiset(
    left: &[&Term],
    right: &[&Term],
    index: usize,
    used: &mut [bool],
    substitution: &mut HashMap<VarId, Term>,
    commutative: &HashSet<mrs_core::SymbolId>,
    associative: &HashSet<mrs_core::SymbolId>,
    limits: VerificationLimits,
) -> bool {
    if index == left.len() {
        return true;
    }
    for right_index in 0..right.len() {
        if used[right_index] {
            continue;
        }
        let mut next = substitution.clone();
        if ac_unify_terms(
            left[index],
            right[right_index],
            &mut next,
            commutative,
            associative,
            limits,
        ) {
            used[right_index] = true;
            if ac_unify_multiset(
                left,
                right,
                index + 1,
                used,
                &mut next,
                commutative,
                associative,
                limits,
            ) {
                *substitution = next;
                return true;
            }
            used[right_index] = false;
        }
    }
    false
}

fn verify_paramodulation(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
    background_commutative: &HashSet<mrs_core::SymbolId>,
    background_associative: &HashSet<mrs_core::SymbolId>,
) -> KernelVerdict {
    if parents.len() != 2 {
        return KernelVerdict::Rejected("paramodulation must have two parents".into());
    }
    let first = verify_superposition(
        parents,
        conclusion,
        limits,
        background_commutative,
        background_associative,
    );
    if matches!(first, KernelVerdict::Certified) {
        return first;
    }
    let reversed = [parents[1].clone(), parents[0].clone()];
    let second = verify_superposition(
        &reversed,
        conclusion,
        limits,
        background_commutative,
        background_associative,
    );
    if matches!(second, KernelVerdict::Certified) {
        second
    } else if matches!(first, KernelVerdict::Inconclusive(_)) {
        first
    } else {
        second
    }
}

fn verify_split_component(
    parents: &[Formula],
    conclusion: &Formula,
    split_parent: Option<usize>,
    limits: VerificationLimits,
) -> Result<BranchContext, KernelVerdict> {
    if parents.len() != 1 {
        return Err(KernelVerdict::Rejected(
            "split_component must have one parent".into(),
        ));
    }
    let Some(parent_clause) = clause_from_formula(&parents[0], limits) else {
        return Err(KernelVerdict::Inconclusive(
            "split_component parent is not a supported clause".into(),
        ));
    };
    let Some(conclusion_clause) = clause_from_formula(conclusion, limits) else {
        return Err(KernelVerdict::Inconclusive(
            "split_component conclusion is not a supported clause".into(),
        ));
    };
    if conclusion_clause.len() != 1 {
        return Err(KernelVerdict::Rejected(
            "split_component conclusion must be a unit literal".into(),
        ));
    }
    let literal = conclusion_clause[0].clone();
    let branch_index = parent_clause
        .iter()
        .position(|parent_literal| {
            clause_alpha_equiv(
                std::slice::from_ref(parent_literal),
                std::slice::from_ref(&literal),
            )
        })
        .ok_or_else(|| {
            KernelVerdict::Rejected(
                "split_component conclusion is not a literal of its parent".into(),
            )
        })?;
    Ok(BranchContext {
        assumptions: vec![BranchAssumption {
            split_parent: split_parent.ok_or_else(|| {
                KernelVerdict::Rejected("split_component has no parent index".into())
            })?,
            branch_index,
            literal,
            sat_var: None,
        }],
        sat_context: Vec::new(),
    })
}

fn verify_avatar_split_clause(
    parents: &[Formula],
    conclusion: &Formula,
    split_parent: Option<usize>,
    inherited_context: Option<&BranchContext>,
    annotation: Option<AvatarSplitInfo<'_>>,
    symbols: &SymbolTable,
    limits: VerificationLimits,
) -> Result<AvatarSplitContext, KernelVerdict> {
    if parents.len() != 1 {
        return Err(KernelVerdict::Rejected(
            "avatar_split_clause must have one parent".into(),
        ));
    }
    let split_parent = split_parent
        .ok_or_else(|| KernelVerdict::Rejected("avatar_split_clause has no parent index".into()))?;
    let Some(parent_formula) = clause_from_formula(&parents[0], limits) else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_split_clause parent is not a supported clause".into(),
        ));
    };
    let Some(split_formula) = clause_from_formula(conclusion, limits) else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_split_clause conclusion is not a supported clause".into(),
        ));
    };
    let (parent, parent_context, parent_positive) = split_avatar_markers(parent_formula, symbols)?;
    let (split, split_context, split_positive) = split_avatar_markers(split_formula, symbols)?;
    if !parent_positive.is_empty()
        || parent.len() < 2
        || !split.is_empty()
        || split_positive.len() < 2
        || split_context != parent_context
    {
        return Err(KernelVerdict::Rejected(
            "avatar_split_clause has invalid parent or split marker clauses".into(),
        ));
    }
    let Some(annotation) = annotation else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_split_clause lacks explicit branch metadata".into(),
        ));
    };
    if annotation.components.len() != split_positive.len()
        || annotation.inherited.iter().any(|name| {
            let Some(var) = parse_avatar_var(name) else {
                return true;
            };
            var == 0
        })
    {
        return Err(KernelVerdict::Rejected(
            "avatar_split_clause metadata does not cover the parent clause".into(),
        ));
    }
    let mut branch_vars = vec![0; split_positive.len()];
    let mut seen_vars = HashSet::new();
    let mut component_literal_indices = vec![Vec::new(); split_positive.len()];
    let mut seen_indices = HashSet::new();
    for component in &annotation.components {
        if component.branch_index >= split_positive.len()
            || !seen_indices.insert(component.branch_index)
            || component.literal_indices.is_empty()
        {
            return Err(KernelVerdict::Rejected(
                "avatar_split_clause has invalid branch indices".into(),
            ));
        }
        let Some(var) = parse_avatar_var(component.sat_var) else {
            return Err(KernelVerdict::Rejected(
                "avatar_split_clause has an invalid SAT variable".into(),
            ));
        };
        if !seen_vars.insert(var) {
            return Err(KernelVerdict::Rejected(
                "avatar_split_clause reuses a SAT variable".into(),
            ));
        }
        if split_positive[component.branch_index] != var {
            return Err(KernelVerdict::Rejected(
                "avatar_split_clause metadata does not match its split literal".into(),
            ));
        }
        if component
            .literal_indices
            .iter()
            .any(|index| *index >= parent.len())
        {
            return Err(KernelVerdict::Rejected(
                "avatar_split_clause references an out-of-range literal".into(),
            ));
        }
        branch_vars[component.branch_index] = var;
        component_literal_indices[component.branch_index] = component.literal_indices.clone();
    }
    if branch_vars.contains(&0) {
        return Err(KernelVerdict::Rejected(
            "avatar_split_clause contains duplicate or missing branches".into(),
        ));
    }
    let mut covered = HashSet::new();
    for component in &annotation.components {
        for index in &component.literal_indices {
            if !covered.insert(*index) {
                return Err(KernelVerdict::Rejected(
                    "avatar_split_clause assigns a literal to multiple components".into(),
                ));
            }
        }
    }
    if covered.len() != parent.len() {
        return Err(KernelVerdict::Rejected(
            "avatar_split_clause does not cover every parent literal".into(),
        ));
    }
    let inherited_vars = annotation
        .inherited
        .iter()
        .map(|name| {
            parse_avatar_var(name).ok_or_else(|| {
                KernelVerdict::Rejected("avatar_split_clause has invalid inherited context".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inherited_assumptions = inherited_context
        .map(|context| context.assumptions.clone())
        .unwrap_or_default();
    if let Some(context) = inherited_context {
        let mut expected = context.sat_context.clone();
        normalize_avatar_vars(&mut expected);
        let mut actual = inherited_vars.clone();
        normalize_avatar_vars(&mut actual);
        if expected != actual {
            return Err(KernelVerdict::Rejected(
                "avatar_split_clause inherited context does not match its parent".into(),
            ));
        }
    } else if !inherited_vars.is_empty() {
        return Err(KernelVerdict::Rejected(
            "avatar_split_clause has inherited context without a branch parent".into(),
        ));
    }
    Ok(AvatarSplitContext {
        split_parent,
        parent_literals: parent,
        branch_vars,
        component_literal_indices,
        inherited_vars,
        inherited_assumptions,
    })
}

#[allow(clippy::too_many_arguments)]
fn verify_avatar_component_clause(
    parents: &[Formula],
    conclusion: &Formula,
    split_parent: Option<usize>,
    splits: &HashMap<usize, AvatarSplitContext>,
    inherited_context: Option<&BranchContext>,
    split_parent_name: Option<&str>,
    annotation: Option<AvatarComponentInfo<'_>>,
    symbols: &SymbolTable,
    limits: VerificationLimits,
) -> Result<BranchContext, KernelVerdict> {
    if parents.len() != 1 {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause must have one parent".into(),
        ));
    }
    let split_id = split_parent.ok_or_else(|| {
        KernelVerdict::Rejected("avatar_component_clause has no parent index".into())
    })?;
    let Some(split) = splits.get(&split_id) else {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause parent is not a validated split".into(),
        ));
    };
    let Some(annotation) = annotation else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_component_clause lacks explicit branch metadata".into(),
        ));
    };
    if Some(annotation.split_parent) != split_parent_name {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause cites the wrong split parent".into(),
        ));
    }
    let Some(goal_formula) = clause_from_formula(conclusion, limits) else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_component_clause conclusion is not a supported clause".into(),
        ));
    };
    let (goal, goal_context, goal_positive) = split_avatar_markers(goal_formula, symbols)?;
    if !goal_positive.is_empty() {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause contains a positive SAT marker".into(),
        ));
    }
    let branch_index = annotation.branch_index;
    let Some(branch_var) = split.branch_vars.get(branch_index).copied() else {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause references an unknown branch".into(),
        ));
    };
    let mut expected_context = split.inherited_vars.clone();
    expected_context.push(branch_var);
    normalize_avatar_vars(&mut expected_context);
    // The prover condenses duplicate literals, so a component derived
    // from duplicated parent literals (e.g. COM133+1's split of
    // `[visFreeVar, visFreeVar, vtcheck]` yields a one-literal
    // `visFreeVar` component) must compare modulo condensation.
    let expected_indices = &split.component_literal_indices[branch_index];
    let expected_component = expected_indices
        .iter()
        .map(|index| split.parent_literals[*index].clone())
        .collect::<Vec<_>>();
    if split.branch_vars.get(branch_index).copied() != parse_avatar_var(annotation.sat_var)
        || normalize_avatar_vars_copy(goal_context) != expected_context
        || !clause_alpha_equiv(
            &condense_clause(&expected_component),
            &condense_clause(&goal),
        )
    {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause metadata does not match its branch".into(),
        ));
    }
    let Some(literal) = goal.first().cloned() else {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause has an empty component".into(),
        ));
    };
    if !clause_alpha_equiv(
        &condense_clause(&expected_component),
        &condense_clause(&goal),
    ) {
        return Err(KernelVerdict::Rejected(
            "avatar_component_clause does not contain its declared component".into(),
        ));
    }
    let mut assumptions = inherited_context
        .map(|context| context.assumptions.clone())
        .unwrap_or_default();
    assumptions.push(BranchAssumption {
        split_parent: split_id,
        branch_index,
        literal,
        sat_var: Some(split.branch_vars[branch_index]),
    });
    let mut sat_context = inherited_context
        .map(|context| context.sat_context.clone())
        .unwrap_or_else(|| split.inherited_vars.clone());
    sat_context.push(split.branch_vars[branch_index]);
    sat_context.sort_unstable();
    sat_context.dedup();
    Ok(BranchContext {
        assumptions,
        sat_context,
    })
}

fn verify_avatar_branch_refutation(
    parents: &[Formula],
    conclusion: &Formula,
    parent_index: Option<usize>,
    contexts: &HashMap<usize, BranchContext>,
    annotation: Option<AvatarBranchInfo<'_>>,
    symbols: &SymbolTable,
    limits: VerificationLimits,
) -> Result<BranchContext, KernelVerdict> {
    if parents.len() != 1 || !matches!(conclusion, Formula::False) {
        return Err(KernelVerdict::Rejected(
            "avatar_branch_refutation must derive `$false` from one branch root".into(),
        ));
    }
    let parent_index = parent_index.ok_or_else(|| {
        KernelVerdict::Rejected("avatar_branch_refutation has no branch parent".into())
    })?;
    let Some(context) = contexts.get(&parent_index) else {
        return Err(KernelVerdict::Rejected(
            "avatar_branch_refutation parent has no branch context".into(),
        ));
    };
    let Some(annotation) = annotation else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_branch_refutation lacks explicit SAT context".into(),
        ));
    };
    let annotated_context = annotation
        .context
        .iter()
        .map(|name| {
            parse_avatar_var(name).ok_or_else(|| {
                KernelVerdict::Rejected("avatar_branch_refutation has invalid SAT context".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if normalize_avatar_vars_copy(annotated_context) != context.sat_context {
        return Err(KernelVerdict::Rejected(
            "avatar_branch_refutation SAT context does not match its branch".into(),
        ));
    }
    if formula_size(&parents[0]) > limits.max_formula_nodes {
        return Err(KernelVerdict::Inconclusive(
            "avatar_branch_refutation exceeded strict formula-size limit".into(),
        ));
    }
    let Some(parent_clause) = clause_from_formula(&parents[0], limits) else {
        return Err(KernelVerdict::Inconclusive(
            "avatar_branch_refutation parent is not a supported clause".into(),
        ));
    };
    let (ordinary, negative, positive) =
        split_avatar_markers(parent_clause, symbols).map_err(|_| {
            KernelVerdict::Rejected("avatar_branch_refutation parent has invalid markers".into())
        })?;
    if !ordinary.is_empty() || !positive.is_empty() || negative != context.sat_context {
        return Err(KernelVerdict::Rejected(
            "avatar_branch_refutation parent is not false under its SAT context".into(),
        ));
    }
    Ok(context.clone())
}

/// Tier-2 proof-size bounds for `sat_backed_refutation` (raised kernel
/// limits, scoped to this rule so the shared defaults stay conservative).
/// Sized against the small-PHP measurement (615k manifest entries, ~1.2M
/// trace events): the manifest bound covers Tier-2 grounding scale with
/// headroom, the trace-byte bound covers hex-doubled FRAT payloads, and the
/// event bound covers deletions-heavy proofs at similar scale.
const SAT_BACKED_MAX_MANIFEST: usize = 4_000_000;
const SAT_BACKED_MAX_TRACE_BYTES: usize = 512 * 1024 * 1024;
const SAT_BACKED_MAX_TRACE_EVENTS: usize = 64_000_000;

/// One recomputed parent literal: polarity plus the canonical atom key.
type SatBackedLiteral = (bool, (String, Vec<String>));

/// Canonical sort key for a ground atom, mirroring the producer
/// (`certified_sat::atom_sort_key`): predicate name plus argument constant
/// names. The two implementations must agree exactly — cross-tested by the
/// end-to-end accept tests below, which fail loudly on any drift.
fn sat_backed_atom_key(atom: &Atom, symbols: &SymbolTable) -> Option<(String, Vec<String>)> {
    match atom {
        Atom::Pred(predicate, args) => {
            let mut arg_names = Vec::with_capacity(args.len());
            for arg in args {
                let Term::App(symbol, inner) = arg else {
                    return None;
                };
                if !inner.is_empty() {
                    return None;
                }
                arg_names.push(symbols.resolve(*symbol).to_string());
            }
            Some((symbols.resolve(*predicate).to_string(), arg_names))
        }
        Atom::Eq(_, _) => None,
    }
}

fn verify_sat_backed_refutation(
    node: &Node<'_>,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    parents: &[Formula],
    annotation: Option<SatBackedInfo<'_>>,
    symbols: &SymbolTable,
    limits: VerificationLimits,
) -> KernelVerdict {
    if !node.is_false {
        return KernelVerdict::Rejected("sat_backed_refutation must conclude `$false`".into());
    }
    let Some(annotation) = annotation else {
        return KernelVerdict::Inconclusive("sat_backed_refutation lacks SAT metadata".into());
    };
    let Some(format) = annotation.trace_format else {
        return KernelVerdict::Inconclusive("sat_backed_refutation lacks a trace format".into());
    };
    if format != "frat-lrat" && format != "lrat" {
        return KernelVerdict::Inconclusive(format!(
            "unsupported sat-backed trace format `{format}`"
        ));
    }
    // Every cited parent must be a plain input or a kernel-validated
    // instantiation thereof (the main loop verifies each node's own rule;
    // this only restricts the shapes allowed here — no equality, no
    // rewriting, no AVATAR contexts).
    if parent_indices.is_empty() {
        return KernelVerdict::Rejected("sat_backed_refutation cites no parents".into());
    }
    for &parent_index in parent_indices {
        match dag.nodes[parent_index].rule {
            None | Some("instantiate") | Some("instantiation") => {}
            Some(other) => {
                return KernelVerdict::Rejected(format!(
                    "sat_backed_refutation parent `{}` uses unsupported rule `{other}`",
                    dag.nodes[parent_index].name
                ));
            }
        }
    }
    // The annotation's input list must name exactly the cited parents:
    // otherwise the trace binding below could be checked against a
    // different clause set than the proof actually depends on.
    let parent_names: HashSet<&str> = parent_indices
        .iter()
        .map(|&parent_index| dag.nodes[parent_index].name)
        .collect();
    let input_names: HashSet<&str> = annotation.inputs.iter().copied().collect();
    if parent_names != input_names {
        return KernelVerdict::Rejected(
            "sat_backed_refutation inputs do not match its parent list".into(),
        );
    }
    // Recompute the propositional encoding from the cited parents with the
    // producer's ordering, then require set-equality with the manifest.
    let mut atom_keys: HashSet<(String, Vec<String>)> = HashSet::new();
    let mut parent_clauses: Vec<Vec<SatBackedLiteral>> = Vec::new();
    for parent in parents {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Rejected(
                "sat_backed_refutation parent is not a supported clause".into(),
            );
        };
        if clause.is_empty() {
            return KernelVerdict::Rejected(
                "sat_backed_refutation parent is an empty clause".into(),
            );
        }
        let mut encoded = Vec::with_capacity(clause.len());
        for literal in &clause {
            let Some(key) = sat_backed_atom_key(&literal.atom, symbols) else {
                return KernelVerdict::Rejected(
                    "sat_backed_refutation supports ground atoms only".into(),
                );
            };
            atom_keys.insert(key.clone());
            encoded.push((literal.positive, key));
        }
        parent_clauses.push(encoded);
    }
    let mut sorted_keys: Vec<(String, Vec<String>)> = atom_keys.into_iter().collect();
    sorted_keys.sort();
    let var_of: HashMap<&(String, Vec<String>), i32> = sorted_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key, index as i32 + 1))
        .collect();
    let mut parent_set = HashSet::new();
    for clause in &parent_clauses {
        let mut lits: Vec<i32> = clause
            .iter()
            .map(|(positive, key)| {
                let var = var_of[key];
                if *positive { var } else { -var }
            })
            .collect();
        lits.sort_unstable();
        parent_set.insert(lits);
    }
    // Manifest binding: cited positions must be exactly all entries, and
    // the manifest set must equal the recomputed parent set.
    if annotation.trace_clauses.len() > SAT_BACKED_MAX_MANIFEST {
        return KernelVerdict::Inconclusive(
            "sat-backed manifest exceeds the Tier-2 entry limit".into(),
        );
    }
    let expected_cited: Vec<usize> = (0..annotation.trace_clauses.len()).collect();
    if annotation.trace_cited_indices != expected_cited {
        return KernelVerdict::Rejected(
            "sat_backed_refutation must cite every manifest entry in order".into(),
        );
    }
    let mut manifest_set = HashSet::new();
    for clause in &annotation.trace_clauses {
        let mut normalized = clause.clone();
        normalized.sort_unstable();
        manifest_set.insert(normalized);
    }
    if parent_set != manifest_set {
        return KernelVerdict::Rejected(
            "sat_backed_refutation manifest does not match its parents".into(),
        );
    }
    let Some(variables) = annotation.trace_variables else {
        return KernelVerdict::Inconclusive(
            "sat_backed_refutation omits its variable bound".into(),
        );
    };
    if variables != sorted_keys.len() {
        return KernelVerdict::Rejected(
            "sat_backed_refutation variable bound does not match its atoms".into(),
        );
    }
    if variables > u32::MAX as usize {
        return KernelVerdict::Inconclusive(
            "sat_backed_refutation variable bound exceeds the supported integer range".into(),
        );
    }
    let Some(digest_text) = annotation.trace_digest else {
        return KernelVerdict::Inconclusive("sat_backed_refutation omits its digest".into());
    };
    let Some(expected_digest) = decode_hex_digest(digest_text) else {
        return KernelVerdict::Rejected(
            "sat_backed_refutation has an invalid SHA-256 digest".into(),
        );
    };
    let Some(trace_hex) = annotation.trace_bytes else {
        return KernelVerdict::Inconclusive("sat_backed_refutation omits its proof bytes".into());
    };
    if trace_hex.len() > SAT_BACKED_MAX_TRACE_BYTES.saturating_mul(2) {
        return KernelVerdict::Inconclusive(
            "sat-backed trace exceeds the Tier-2 byte limit".into(),
        );
    }
    let Some(trace) = decode_hex(trace_hex) else {
        return KernelVerdict::Rejected(
            "sat_backed_refutation has invalid hexadecimal proof bytes".into(),
        );
    };
    if annotation.trace_original_ids.is_empty() {
        return KernelVerdict::Rejected("sat_backed_refutation has no original clause IDs".into());
    }
    let actual_digest = avatar_sat_trace_digest(
        format,
        variables as u32,
        &annotation.trace_original_ids,
        &annotation.trace_cited_indices,
        &annotation.trace_clauses,
        &trace,
    );
    if actual_digest != expected_digest {
        return KernelVerdict::Rejected(
            "sat_backed_refutation digest does not match its payload".into(),
        );
    }
    let tier2_limits = VerificationLimits {
        max_nodes: SAT_BACKED_MAX_MANIFEST,
        max_avatar_steps: SAT_BACKED_MAX_TRACE_EVENTS,
        ..limits
    };
    // The replay functions return Result with the verdict as the error
    // payload; a clean replay is the only path to Certified.
    let replayed = match format {
        "frat-lrat" => replay_frat_lrat(
            &annotation.trace_clauses,
            &annotation.trace_original_ids,
            &trace,
            variables,
            tier2_limits,
        ),
        "lrat" => replay_lrat(
            &annotation.trace_clauses,
            &annotation.trace_original_ids,
            &trace,
            variables,
            tier2_limits,
        ),
        _ => Err(KernelVerdict::Inconclusive(format!(
            "unsupported sat-backed trace format `{format}`"
        ))),
    };
    match replayed {
        Ok(()) => KernelVerdict::Certified,
        Err(verdict) => verdict,
    }
}

fn verify_avatar_sat_refutation(
    node: &Node<'_>,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    contexts: &HashMap<usize, BranchContext>,
    splits: &HashMap<usize, AvatarSplitContext>,
    annotation: Option<AvatarSatInfo<'_>>,
    limits: VerificationLimits,
) -> KernelVerdict {
    if !node.is_false || parent_indices.len() < 2 {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation must conclude `$false` from a split and branches".into(),
        );
    }
    let Some(annotation) = annotation else {
        return KernelVerdict::Inconclusive(
            "avatar_sat_refutation lacks explicit SAT metadata".into(),
        );
    };
    let has_trace_payload = annotation.trace_format.is_some()
        || annotation.trace_variables.is_some()
        || annotation.trace_digest.is_some()
        || !annotation.trace_original_ids.is_empty()
        || !annotation.trace_cited_indices.is_empty()
        || !annotation.trace_clauses.is_empty()
        || annotation.trace_bytes.is_some();
    if has_trace_payload && let Err(verdict) = verify_avatar_sat_trace(&annotation, limits) {
        return verdict;
    }
    let Some(split_indices) = annotation
        .split_nodes
        .iter()
        .map(|name| dag.by_name.get(name).copied())
        .collect::<Option<Vec<_>>>()
    else {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation references an unknown split node".into(),
        );
    };
    let Some(branch_indices) = annotation
        .branch_roots
        .iter()
        .map(|name| dag.by_name.get(name).copied())
        .collect::<Option<Vec<_>>>()
    else {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation references an unknown branch root".into(),
        );
    };
    if split_indices.is_empty() || branch_indices.is_empty() {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation must cite split nodes and branch roots".into(),
        );
    }
    let parent_set: HashSet<_> = parent_indices.iter().copied().collect();
    let metadata_set: HashSet<_> = split_indices
        .iter()
        .chain(&branch_indices)
        .copied()
        .collect();
    if parent_indices.len() != metadata_set.len() || parent_set != metadata_set {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation metadata does not match its parent list".into(),
        );
    }

    let mut split_contexts: Vec<&AvatarSplitContext> = Vec::new();
    for split_index in &split_indices {
        if dag.nodes[*split_index].rule != Some("avatar_split_clause") {
            return KernelVerdict::Rejected("avatar_sat_refutation cites a non-split node".into());
        }
        let Some(split) = splits.get(split_index) else {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation cites an unvalidated split".into(),
            );
        };
        split_contexts.push(split);
    }

    let split_index_set: HashSet<_> = split_indices.iter().copied().collect();
    let mut branch_context_list = Vec::new();
    let mut seen_contexts = HashSet::new();
    for branch_root in &branch_indices {
        let Some(context) = contexts.get(branch_root) else {
            return KernelVerdict::Rejected(format!(
                "avatar_sat_refutation branch `{}` has no context",
                dag.nodes[*branch_root].name
            ));
        };
        if !dag.nodes[*branch_root].is_false {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation branch root is not `$false`".into(),
            );
        }
        if dag.nodes[*branch_root].rule != Some("avatar_branch_refutation") {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation branch root is not an AVATAR branch refutation".into(),
            );
        }
        if context
            .assumptions
            .iter()
            .any(|assumption| !split_index_set.contains(&assumption.split_parent))
        {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation branch descends from an uncited split".into(),
            );
        }
        for assumption in &context.assumptions {
            let Some(split) = splits.get(&assumption.split_parent) else {
                return KernelVerdict::Rejected(
                    "avatar_sat_refutation branch cites an unknown split context".into(),
                );
            };
            if assumption.branch_index >= split.branch_vars.len()
                || assumption.sat_var != Some(split.branch_vars[assumption.branch_index])
            {
                return KernelVerdict::Rejected(
                    "avatar_sat_refutation branch has an invalid SAT assumption".into(),
                );
            }
        }
        let mut normalized_context = context.sat_context.clone();
        normalize_avatar_vars(&mut normalized_context);
        if !seen_contexts.insert(normalized_context.clone()) {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation contains a duplicate branch context".into(),
            );
        }
        branch_context_list.push(normalized_context);
    }
    if has_trace_payload
        && let Err(verdict) = verify_avatar_sat_manifest_binding(
            &annotation,
            &split_indices,
            &branch_indices,
            splits,
            contexts,
        )
    {
        return verdict;
    }
    // With a verified LRAT trace bound to this certificate's manifest, the
    // SAT instance is already proved unsatisfiable; enumeration would only
    // re-check the same fact.
    if has_trace_payload {
        return KernelVerdict::Certified;
    }
    // No stored trace: reconstruct the SAT instance from the validated
    // splits/branch contexts and check unsatisfiability with CaDiCaL.
    // Structure validation above still binds every variable to a real split
    // component, so a forged certificate cannot invent free variables.
    let mut variables = HashSet::new();
    for split in &split_contexts {
        variables.extend(split.branch_vars.iter().copied());
        variables.extend(split.inherited_vars.iter().copied());
    }
    for context in &branch_context_list {
        variables.extend(context.iter().copied());
    }
    if variables.len() > limits.max_avatar_steps {
        return KernelVerdict::Inconclusive(
            "avatar_sat_refutation exceeded strict SAT-variable limit".into(),
        );
    }
    let mut solver = mrs_cadical::Solver::new();
    for split in &split_contexts {
        let mut clause = Vec::with_capacity(split.branch_vars.len() + split.inherited_vars.len());
        for &var in &split.branch_vars {
            clause.push(var as i32);
        }
        for &var in &split.inherited_vars {
            clause.push(-(var as i32));
        }
        solver.add_clause(&clause);
    }
    for context in &branch_context_list {
        let mut clause = Vec::with_capacity(context.len());
        for &var in context {
            clause.push(-(var as i32));
        }
        solver.add_clause(&clause);
    }
    match solver.solve() {
        SolveResult::Unsat => KernelVerdict::Certified,
        SolveResult::Unknown => KernelVerdict::Inconclusive(
            "avatar_sat_refutation SAT verification inconclusive".into(),
        ),
        SolveResult::Sat => KernelVerdict::Rejected(
            "avatar_sat_refutation leaves a satisfiable SAT assignment unrefuted".into(),
        ),
    }
}

fn verify_avatar_sat_manifest_binding(
    annotation: &AvatarSatInfo<'_>,
    split_indices: &[usize],
    branch_indices: &[usize],
    splits: &HashMap<usize, AvatarSplitContext>,
    contexts: &HashMap<usize, BranchContext>,
) -> Result<(), KernelVerdict> {
    let mut expected = Vec::with_capacity(split_indices.len() + branch_indices.len());
    for split_index in split_indices {
        let split = splits.get(split_index).ok_or_else(|| {
            KernelVerdict::Rejected("avatar SAT trace references an unvalidated split".into())
        })?;
        let mut clause = split
            .branch_vars
            .iter()
            .map(|variable| *variable as i32)
            .collect::<Vec<_>>();
        clause.extend(
            split
                .inherited_vars
                .iter()
                .map(|variable| -(*variable as i32)),
        );
        expected.push(clause);
    }
    for branch_index in branch_indices {
        let context = contexts.get(branch_index).ok_or_else(|| {
            KernelVerdict::Rejected("avatar SAT trace references an unknown branch".into())
        })?;
        expected.push(
            context
                .sat_context
                .iter()
                .map(|variable| -(*variable as i32))
                .collect(),
        );
    }
    if expected.len() != annotation.trace_cited_indices.len() {
        return Err(KernelVerdict::Rejected(
            "avatar SAT trace citation indices do not match cited split and branch count".into(),
        ));
    }
    let mut seen = HashSet::new();
    for (expected_clause, index) in expected.iter().zip(&annotation.trace_cited_indices) {
        if !seen.insert(*index) {
            return Err(KernelVerdict::Rejected(
                "avatar SAT trace cites one manifest clause more than once".into(),
            ));
        }
        if annotation.trace_clauses.get(*index) != Some(expected_clause) {
            return Err(KernelVerdict::Rejected(
                "avatar SAT trace citation does not match its AVATAR certificate".into(),
            ));
        }
    }
    Ok(())
}

fn verify_avatar_sat_trace(
    annotation: &AvatarSatInfo<'_>,
    limits: VerificationLimits,
) -> Result<(), KernelVerdict> {
    let format = annotation.trace_format.ok_or_else(|| {
        KernelVerdict::Inconclusive("avatar SAT trace metadata is incomplete".into())
    })?;
    if format != "frat-lrat" && format != "lrat" {
        return Err(KernelVerdict::Inconclusive(format!(
            "unsupported AVATAR SAT trace format `{format}`"
        )));
    }
    let variables = annotation.trace_variables.ok_or_else(|| {
        KernelVerdict::Inconclusive("avatar SAT trace omits its variable bound".into())
    })?;
    if variables > u32::MAX as usize {
        return Err(KernelVerdict::Inconclusive(
            "avatar SAT trace variable bound exceeds the supported integer range".into(),
        ));
    }
    if variables > limits.max_avatar_steps {
        return Err(KernelVerdict::Inconclusive(
            "avatar SAT trace declares too many variables".into(),
        ));
    }
    let digest_text = annotation
        .trace_digest
        .ok_or_else(|| KernelVerdict::Inconclusive("avatar SAT trace omits its digest".into()))?;
    let expected_digest = decode_hex_digest(digest_text).ok_or_else(|| {
        KernelVerdict::Rejected("avatar SAT trace has an invalid SHA-256 digest".into())
    })?;
    let trace_hex = annotation.trace_bytes.ok_or_else(|| {
        KernelVerdict::Inconclusive("avatar SAT trace omits its proof bytes".into())
    })?;
    let trace = decode_hex(trace_hex).ok_or_else(|| {
        KernelVerdict::Rejected("avatar SAT trace has invalid hexadecimal proof bytes".into())
    })?;
    if trace.len() > limits.max_clause_literals.saturating_mul(1024) {
        return Err(KernelVerdict::Inconclusive(
            "avatar SAT trace exceeds the strict byte limit".into(),
        ));
    }
    if annotation.trace_original_ids.len() != annotation.trace_clauses.len()
        || annotation.trace_original_ids.is_empty()
        || annotation.trace_cited_indices.is_empty()
    {
        return Err(KernelVerdict::Rejected(
            "avatar SAT trace clause IDs do not match its manifest".into(),
        ));
    }
    if annotation.trace_clauses.len() > limits.max_nodes {
        return Err(KernelVerdict::Inconclusive(
            "avatar SAT trace manifest exceeds the strict node limit".into(),
        ));
    }
    if annotation
        .trace_cited_indices
        .iter()
        .any(|index| *index >= annotation.trace_clauses.len())
    {
        return Err(KernelVerdict::Rejected(
            "avatar SAT trace cites an out-of-range manifest index".into(),
        ));
    }
    if annotation
        .trace_clauses
        .iter()
        .flatten()
        .any(|literal| literal == &0 || literal.unsigned_abs() as usize > variables)
    {
        return Err(KernelVerdict::Rejected(
            "avatar SAT trace manifest contains an invalid literal".into(),
        ));
    }
    let actual_digest = avatar_sat_trace_digest(
        format,
        variables as u32,
        &annotation.trace_original_ids,
        &annotation.trace_cited_indices,
        &annotation.trace_clauses,
        &trace,
    );
    if actual_digest != expected_digest {
        return Err(KernelVerdict::Rejected(
            "avatar SAT trace digest does not match its payload".into(),
        ));
    }
    match format {
        "frat-lrat" => replay_frat_lrat(
            &annotation.trace_clauses,
            &annotation.trace_original_ids,
            &trace,
            variables,
            limits,
        ),
        "lrat" => replay_lrat(
            &annotation.trace_clauses,
            &annotation.trace_original_ids,
            &trace,
            variables,
            limits,
        ),
        _ => Err(KernelVerdict::Inconclusive(format!(
            "unsupported AVATAR SAT trace format `{format}`"
        ))),
    }
}

fn decode_hex_digest(value: &str) -> Option<[u8; 32]> {
    let bytes = decode_hex(value)?;
    bytes.try_into().ok()
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len() / 2);
    let (chunks, _) = bytes.as_chunks::<2>();
    for pair in chunks {
        let high = hex_digit(pair[0])?;
        let low = hex_digit(pair[1])?;
        output.push((high << 4) | low);
    }
    Some(output)
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn replay_frat_lrat(
    manifest: &[Vec<i32>],
    original_ids: &[i64],
    trace: &[u8],
    variables: usize,
    limits: VerificationLimits,
) -> Result<(), KernelVerdict> {
    let text = std::str::from_utf8(trace)
        .map_err(|_| KernelVerdict::Rejected("avatar SAT trace is not valid UTF-8 FRAT".into()))?;
    let mut clauses = HashMap::<i64, Vec<i32>>::new();
    let mut original_index = 0usize;
    let mut finalized_empty = false;
    let mut events = 0usize;
    let mut total_literals = 0usize;
    for (line_number, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('c') {
            continue;
        }
        events += 1;
        if events > limits.max_nodes.saturating_mul(32) {
            return Err(KernelVerdict::Inconclusive(
                "avatar SAT trace exceeds the strict event limit".into(),
            ));
        }
        let mut tokens = line.split_whitespace();
        let kind = tokens
            .next()
            .ok_or_else(|| trace_error(line_number, "missing record kind"))?;
        let id = tokens
            .next()
            .ok_or_else(|| trace_error(line_number, "missing clause ID"))?
            .parse::<i64>()
            .map_err(|_| trace_error(line_number, "invalid clause ID"))?;
        let clause = parse_trace_clause(&mut tokens, line_number, limits.max_clause_literals)?;
        total_literals = total_literals.saturating_add(clause.len());
        if total_literals > limits.max_clause_literals.saturating_mul(limits.max_nodes) {
            return Err(KernelVerdict::Inconclusive(
                "avatar SAT trace exceeds the strict literal limit".into(),
            ));
        }
        if clause
            .iter()
            .any(|literal| literal.unsigned_abs() as usize > variables)
        {
            return Err(trace_error(
                line_number,
                "trace clause uses a literal outside the declared variable bound",
            ));
        }
        match kind {
            "o" => {
                if tokens.next().is_some() {
                    return Err(trace_error(line_number, "unexpected original record data"));
                }
                let expected_id = original_ids.get(original_index).ok_or_else(|| {
                    trace_error(
                        line_number,
                        "trace has more original clauses than its manifest",
                    )
                })?;
                let expected_clause = manifest.get(original_index).ok_or_else(|| {
                    trace_error(line_number, "trace original clause index exceeds manifest")
                })?;
                if id != *expected_id || clause != *expected_clause {
                    return Err(trace_error(
                        line_number,
                        "trace original clause does not match its manifest entry",
                    ));
                }
                if id <= 0 || clauses.insert(id, clause).is_some() {
                    return Err(trace_error(line_number, "duplicate original clause ID"));
                }
                original_index += 1;
            }
            "a" => {
                let mut antecedents = Vec::new();
                if let Some(marker) = tokens.next() {
                    if marker != "l" {
                        return Err(trace_error(line_number, "expected LRAT antecedent marker"));
                    }
                    loop {
                        let token = tokens.next().ok_or_else(|| {
                            trace_error(line_number, "unterminated antecedent list")
                        })?;
                        if token == "0" {
                            break;
                        }
                        antecedents.push(token.parse::<i64>().map_err(|_| {
                            trace_error(line_number, "invalid antecedent clause ID")
                        })?);
                    }
                }
                if tokens.next().is_some()
                    || id <= 0
                    || clauses.contains_key(&id)
                    || (antecedents.is_empty() && !is_tautology_kernel(&clause))
                    || (!antecedents.is_empty()
                        && !rup_check_kernel(&clauses, &clause, &antecedents))
                {
                    return Err(trace_error(line_number, "invalid LRAT/RUP clause addition"));
                }
                clauses.insert(id, clause);
            }
            "d" => {
                if tokens.next().is_some() {
                    return Err(trace_error(line_number, "unexpected deletion record data"));
                }
                let Some(existing) = clauses.remove(&id) else {
                    return Err(trace_error(
                        line_number,
                        "deletion references an unknown clause",
                    ));
                };
                // Multiset comparison (not order-sensitive): deletion
                // records, like finalizations, may list literals in
                // CaDiCaL's internal order rather than insertion order.
                let mut expected = existing;
                let mut actual = clause;
                expected.sort_unstable();
                actual.sort_unstable();
                if expected != actual {
                    return Err(trace_error(line_number, "deletion clause does not match"));
                }
            }
            "f" => {
                // Multiset comparison (not order-sensitive): CaDiCaL
                // reports finalized clauses in internal literal order,
                // which may differ from the manifest order.
                let same = clauses.get(&id).is_some_and(|tracked| {
                    let mut expected = tracked.clone();
                    let mut actual = clause.clone();
                    expected.sort_unstable();
                    actual.sort_unstable();
                    expected == actual
                });
                if tokens.next().is_some() || !same {
                    return Err(trace_error(line_number, "invalid clause finalization"));
                }
                finalized_empty |= clause.is_empty();
            }
            _ => return Err(trace_error(line_number, "unsupported FRAT record")),
        }
    }
    if original_index != manifest.len() {
        return Err(KernelVerdict::Rejected(
            "SAT trace does not contain every manifest clause".into(),
        ));
    }
    if finalized_empty {
        Ok(())
    } else {
        Err(KernelVerdict::Rejected(
            "SAT trace does not finalize the empty clause".into(),
        ))
    }
}

fn replay_lrat(
    manifest: &[Vec<i32>],
    original_ids: &[i64],
    trace: &[u8],
    variables: usize,
    limits: VerificationLimits,
) -> Result<(), KernelVerdict> {
    let text = std::str::from_utf8(trace)
        .map_err(|_| KernelVerdict::Rejected("avatar SAT trace is not valid UTF-8 LRAT".into()))?;
    let mut clauses = HashMap::<i64, Vec<i32>>::new();

    // Seed original clauses from manifest and original_ids
    for (i, clause) in manifest.iter().enumerate() {
        let id = if let Some(&id) = original_ids.get(i) {
            id
        } else {
            (i + 1) as i64
        };
        if id <= 0 || clauses.insert(id, clause.clone()).is_some() {
            return Err(KernelVerdict::Rejected(
                "duplicate or invalid original clause ID in manifest".into(),
            ));
        }
    }

    let mut derived_empty = false;
    let mut events = 0usize;
    let mut total_literals = 0usize;

    for (line_number, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('c') {
            continue;
        }
        events += 1;
        if events > limits.max_nodes.saturating_mul(32) {
            return Err(KernelVerdict::Inconclusive(
                "avatar SAT trace exceeds the strict event limit".into(),
            ));
        }
        let mut tokens = line.split_whitespace();
        let first_token = tokens
            .next()
            .ok_or_else(|| trace_error(line_number, "missing record token"))?;

        if first_token == "d" {
            // Deletion line without step ID: `d del1 del2 ... 0`
            for tok in tokens.by_ref() {
                if tok == "0" {
                    break;
                }
                let del_id = tok
                    .parse::<i64>()
                    .map_err(|_| trace_error(line_number, "invalid deletion ID"))?;
                clauses.remove(&del_id);
            }
        } else {
            let id = first_token
                .parse::<i64>()
                .map_err(|_| trace_error(line_number, "invalid clause ID or step ID"))?;

            // Check if this is a deletion line starting with `<step_id> d ...`
            if let Some(second) = tokens.next() {
                if second == "d" {
                    // Deletion line: `<step_id> d del1 del2 ... 0`
                    for tok in tokens.by_ref() {
                        if tok == "0" {
                            break;
                        }
                        let del_id = tok
                            .parse::<i64>()
                            .map_err(|_| trace_error(line_number, "invalid deletion ID"))?;
                        clauses.remove(&del_id);
                    }
                    continue;
                }

                // Clause addition: `second` is the first literal or "0" if empty clause
                let mut clause = Vec::new();
                if second != "0" {
                    let lit = second
                        .parse::<i32>()
                        .map_err(|_| trace_error(line_number, "invalid clause literal"))?;
                    if lit == 0 {
                        return Err(trace_error(line_number, "zero is not a clause literal"));
                    }
                    clause.push(lit);
                    // Parse rest of clause literals until 0
                    for tok in tokens.by_ref() {
                        if tok == "0" {
                            break;
                        }
                        let lit = tok
                            .parse::<i32>()
                            .map_err(|_| trace_error(line_number, "invalid clause literal"))?;
                        if lit == 0 {
                            return Err(trace_error(line_number, "zero is not a clause literal"));
                        }
                        if clause.len() >= limits.max_clause_literals {
                            return Err(KernelVerdict::Inconclusive(
                                "clause literal limit exceeded".into(),
                            ));
                        }
                        clause.push(lit);
                    }
                }

                total_literals = total_literals.saturating_add(clause.len());
                if total_literals > limits.max_clause_literals.saturating_mul(limits.max_nodes) {
                    return Err(KernelVerdict::Inconclusive(
                        "avatar SAT trace exceeds the strict literal limit".into(),
                    ));
                }
                if clause
                    .iter()
                    .any(|literal| literal.unsigned_abs() as usize > variables)
                {
                    return Err(trace_error(
                        line_number,
                        "trace clause uses a literal outside the declared variable bound",
                    ));
                }

                // Parse antecedents until 0
                let mut antecedents = Vec::new();
                for tok in tokens.by_ref() {
                    if tok == "0" {
                        break;
                    }
                    let ant = tok
                        .parse::<i64>()
                        .map_err(|_| trace_error(line_number, "invalid antecedent ID"))?;
                    antecedents.push(ant.abs());
                }

                if id <= 0
                    || clauses.contains_key(&id)
                    || (antecedents.is_empty() && !is_tautology_kernel(&clause))
                    || (!antecedents.is_empty()
                        && !rup_check_kernel(&clauses, &clause, &antecedents))
                {
                    return Err(trace_error(line_number, "invalid LRAT/RUP clause addition"));
                }

                if clause.is_empty() {
                    derived_empty = true;
                }
                clauses.insert(id, clause);
            } else {
                return Err(trace_error(line_number, "incomplete LRAT line"));
            }
        }
    }

    if derived_empty {
        Ok(())
    } else {
        Err(KernelVerdict::Rejected(
            "LRAT SAT trace does not derive the empty clause".into(),
        ))
    }
}

fn parse_trace_clause<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    line_number: usize,
    max_literals: usize,
) -> Result<Vec<i32>, KernelVerdict> {
    let mut clause = Vec::new();
    loop {
        let token = tokens
            .next()
            .ok_or_else(|| trace_error(line_number, "unterminated clause"))?;
        if token == "0" {
            return Ok(clause);
        }
        let literal = token
            .parse::<i32>()
            .map_err(|_| trace_error(line_number, "invalid clause literal"))?;
        if literal == 0 {
            return Err(trace_error(line_number, "zero is not a clause literal"));
        }
        if clause.len() >= max_literals {
            return Err(KernelVerdict::Inconclusive(
                "avatar SAT trace clause exceeds the strict literal limit".into(),
            ));
        }
        clause.push(literal);
    }
}

fn trace_error(line_number: usize, reason: &str) -> KernelVerdict {
    KernelVerdict::Rejected(format!(
        "malformed AVATAR SAT trace at line {}: {reason}",
        line_number + 1
    ))
}

fn rup_check_kernel(
    clauses: &HashMap<i64, Vec<i32>>,
    conclusion: &[i32],
    antecedents: &[i64],
) -> bool {
    if antecedents.iter().any(|id| !clauses.contains_key(id)) {
        return false;
    }
    let mut assignment = HashSet::new();
    for &literal in conclusion {
        assignment.insert(-literal);
    }
    for &id in antecedents {
        let Some(clause) = clauses.get(&id) else {
            return false;
        };
        let mut satisfied = false;
        let mut unit = None;
        let mut multiple = false;
        for &literal in clause {
            if assignment.contains(&literal) {
                satisfied = true;
                break;
            }
            if !assignment.contains(&-literal) && unit.replace(literal).is_some() {
                multiple = true;
            }
        }
        if satisfied {
            continue;
        }
        if clause.is_empty() {
            return true;
        }
        if multiple {
            return false;
        }
        if let Some(unit) = unit {
            assignment.insert(unit);
        } else {
            return true;
        }
    }
    false
}

fn is_tautology_kernel(clause: &[i32]) -> bool {
    clause.iter().any(|literal| clause.contains(&-*literal))
}

fn parse_avatar_var(name: &str) -> Option<u32> {
    name.strip_prefix("spl0_")
        .or_else(|| name.strip_prefix("spl_"))?
        .parse()
        .ok()
        .filter(|var| *var > 0 && *var <= MAX_CADICAL_VARIABLE)
}

fn normalize_avatar_vars(vars: &mut Vec<u32>) {
    vars.sort_unstable();
    vars.dedup();
}

fn normalize_avatar_vars_copy(mut vars: Vec<u32>) -> Vec<u32> {
    normalize_avatar_vars(&mut vars);
    vars
}

#[allow(clippy::type_complexity)]
fn split_avatar_markers(
    clause: Vec<Literal>,
    symbols: &SymbolTable,
) -> Result<(Vec<Literal>, Vec<u32>, Vec<u32>), KernelVerdict> {
    let mut ordinary = Vec::new();
    let mut negative = Vec::new();
    let mut positive = Vec::new();
    for literal in clause {
        let marker = match &literal.atom {
            Atom::Pred(symbol, args)
                if args.is_empty() && {
                    let name = symbols.resolve(*symbol);
                    name.starts_with("spl0_") || name.starts_with("spl_")
                } =>
            {
                Some(symbols.resolve(match &literal.atom {
                    Atom::Pred(symbol, _) => *symbol,
                    Atom::Eq(_, _) => unreachable!(),
                }))
            }
            _ => None,
        };
        let Some(marker) = marker else {
            ordinary.push(literal);
            continue;
        };
        let Some(var) = parse_avatar_var(marker) else {
            return Err(KernelVerdict::Rejected(
                "AVATAR marker has an invalid SAT variable".into(),
            ));
        };
        if literal.positive {
            positive.push(var);
        } else {
            negative.push(var);
        }
    }
    normalize_avatar_vars(&mut negative);
    Ok((ordinary, negative, positive))
}

fn is_avatar_split_literal(literal: &Literal, symbols: &SymbolTable) -> bool {
    match &literal.atom {
        Atom::Pred(symbol, args) => {
            literal.positive && args.is_empty() && {
                let name = symbols.resolve(*symbol);
                name.starts_with("spl0_") || name.starts_with("spl_")
            }
        }
        Atom::Eq(_, _) => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_case_split(
    node: &Node<'_>,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    formulas: &HashMap<usize, Formula>,
    branch_contexts: &HashMap<usize, BranchContext>,
    splits: &HashMap<usize, AvatarSplitContext>,
    symbols: &SymbolTable,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parent_indices.len() < 2 {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation requires a split parent and branch roots".into(),
        );
    }
    if !node.is_false {
        return KernelVerdict::Rejected("avatar_sat_refutation must conclude `$false`".into());
    }
    let split_parent = parent_indices[0];
    let split_definition = splits.get(&split_parent);
    let Some(top_clause) = formulas
        .get(&split_parent)
        .and_then(|formula| clause_from_formula(formula, limits))
    else {
        return KernelVerdict::Inconclusive(
            "avatar_sat_refutation split parent is not a supported clause".into(),
        );
    };
    if top_clause.len() < 2 {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation split parent must be a disjunction".into(),
        );
    }
    let mut seen = HashSet::new();
    for branch_root in &parent_indices[1..] {
        let Some(context) = branch_contexts.get(branch_root) else {
            return KernelVerdict::Rejected(format!(
                "avatar_sat_refutation branch `{}` has no case context",
                dag.nodes[*branch_root].name
            ));
        };
        for assumption in &context.assumptions {
            if assumption.split_parent != split_parent {
                return KernelVerdict::Rejected(format!(
                    "avatar_sat_refutation branch `{}` cites a different split parent",
                    dag.nodes[*branch_root].name
                ));
            }
            if !seen.insert(assumption.branch_index) {
                return KernelVerdict::Rejected(
                    "avatar_sat_refutation contains a duplicate branch".into(),
                );
            }
            let matches_top = top_clause
                .get(assumption.branch_index)
                .is_some_and(|top_literal| {
                    clause_alpha_equiv(
                        std::slice::from_ref(top_literal),
                        std::slice::from_ref(&assumption.literal),
                    )
                });
            if !matches_top {
                return KernelVerdict::Rejected(
                    "avatar_sat_refutation branch literal does not match split parent".into(),
                );
            }
        }
        if !dag.nodes[*branch_root].is_false {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation branch root is not `$false`".into(),
            );
        }
    }
    if seen.len() != top_clause.len() {
        return KernelVerdict::Rejected(format!(
            "avatar_sat_refutation covers {} of {} branches",
            seen.len(),
            top_clause.len()
        ));
    }
    if split_definition.is_none()
        && !top_clause
            .iter()
            .all(|literal| !is_avatar_split_literal(literal, symbols))
    {
        return KernelVerdict::Rejected(
            "avatar_sat_refutation lacks an explicit validated split certificate".into(),
        );
    }
    KernelVerdict::Certified
}

fn atom_term_positions(atom: &Atom) -> Vec<(usize, Vec<usize>)> {
    let mut positions = Vec::new();
    let terms: Vec<&Term> = match atom {
        Atom::Pred(_, args) => args.iter().collect(),
        Atom::Eq(left, right) => vec![left, right],
    };
    for (side, term) in terms.into_iter().enumerate() {
        collect_term_positions(term, Vec::new(), side, &mut positions);
    }
    positions
}

fn collect_term_positions(
    term: &Term,
    path: Vec<usize>,
    side: usize,
    out: &mut Vec<(usize, Vec<usize>)>,
) {
    if !matches!(term, Term::Var(_)) {
        out.push((side, path.clone()));
    }
    if let Term::App(_, args) = term {
        for (idx, arg) in args.iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(idx);
            collect_term_positions(arg, child_path, side, out);
        }
    }
}

fn term_at_position<'a>(term: &'a Term, path: &[usize]) -> Option<&'a Term> {
    if path.is_empty() {
        return Some(term);
    }
    match term {
        Term::App(_, args) => args
            .get(path[0])
            .and_then(|arg| term_at_position(arg, &path[1..])),
        Term::Var(_) => None,
    }
}

fn replace_term_at(term: &Term, path: &[usize], replacement: Term) -> Term {
    if path.is_empty() {
        return replacement;
    }
    match term {
        Term::App(symbol, args) => {
            let mut args = args.clone();
            if let Some(arg) = args.get_mut(path[0]) {
                *arg = replace_term_at(arg, &path[1..], replacement);
            }
            Term::App(*symbol, args)
        }
        Term::Var(_) => term.clone(),
    }
}

fn replace_atom_side(atom: &Atom, side: usize, replacement: Term) -> Atom {
    match atom {
        Atom::Pred(symbol, args) => {
            let mut args = args.clone();
            if let Some(arg) = args.get_mut(side) {
                *arg = replacement;
            }
            Atom::Pred(*symbol, args)
        }
        Atom::Eq(left, right) => {
            if side == 0 {
                Atom::Eq(replacement, right.clone())
            } else {
                Atom::Eq(left.clone(), replacement)
            }
        }
    }
}

fn rewrite_atom(
    atom: &mut Atom,
    rules: &[(Term, Term)],
    steps: &mut usize,
    limits: VerificationLimits,
) -> bool {
    match atom {
        Atom::Pred(_, args) => {
            for term in args {
                if rewrite_term(term, rules, steps, limits) {
                    return true;
                }
            }
            false
        }
        Atom::Eq(left, right) => {
            rewrite_term(left, rules, steps, limits) || rewrite_term(right, rules, steps, limits)
        }
    }
}

fn rewrite_term(
    term: &mut Term,
    rules: &[(Term, Term)],
    steps: &mut usize,
    limits: VerificationLimits,
) -> bool {
    for (left, right) in rules {
        if *steps >= limits.max_rewrite_steps {
            return false;
        }
        let mut substitution = HashMap::new();
        if match_pattern(left, term, &mut substitution) {
            *term = apply_substitution_term(right, &substitution);
            *steps += 1;
            return true;
        }
    }
    if let Term::App(_, args) = term {
        for arg in args {
            if rewrite_term(arg, rules, steps, limits) {
                return true;
            }
        }
    }
    false
}

fn match_pattern(pattern: &Term, target: &Term, substitution: &mut HashMap<VarId, Term>) -> bool {
    match (pattern, target) {
        (Term::Var(var), target) => match substitution.get(var) {
            Some(existing) => existing == target,
            None => {
                substitution.insert(*var, target.clone());
                true
            }
        },
        (Term::App(pattern_symbol, pattern_args), Term::App(target_symbol, target_args)) => {
            pattern_symbol == target_symbol
                && pattern_args.len() == target_args.len()
                && pattern_args
                    .iter()
                    .zip(target_args)
                    .all(|(pattern, target)| match_pattern(pattern, target, substitution))
        }
        _ => false,
    }
}

fn term_var_set(term: &Term) -> HashSet<VarId> {
    let mut vars = HashSet::new();
    collect_term_vars_set(term, &mut vars);
    vars
}

fn collect_term_vars_set(term: &Term, vars: &mut HashSet<VarId>) {
    match term {
        Term::Var(var) => {
            vars.insert(*var);
        }
        Term::App(_, args) => {
            for arg in args {
                collect_term_vars_set(arg, vars);
            }
        }
    }
}

fn max_var_term(term: &Term) -> VarId {
    term_var_set(term).into_iter().max().unwrap_or(0)
}

fn literal_var_set(literal: &Literal) -> HashSet<VarId> {
    let mut vars = HashSet::new();
    match &literal.atom {
        Atom::Pred(_, args) => {
            for arg in args {
                collect_term_vars_set(arg, &mut vars);
            }
        }
        Atom::Eq(left, right) => {
            collect_term_vars_set(left, &mut vars);
            collect_term_vars_set(right, &mut vars);
        }
    }
    vars
}

fn clause_from_formula(formula: &Formula, limits: VerificationLimits) -> Option<Vec<Literal>> {
    let mut body = formula;
    while let Formula::Forall(_, inner) = body {
        body = inner;
    }
    let mut literals = Vec::new();
    collect_disjunction(body, &mut literals)?;
    (literals.len() <= limits.max_clause_literals).then_some(literals)
}

fn collect_disjunction(formula: &Formula, out: &mut Vec<Literal>) -> Option<()> {
    match formula {
        Formula::Or(parts) => {
            for part in parts {
                collect_disjunction(part, out)?;
            }
            Some(())
        }
        Formula::Atom(atom) => {
            out.push(Literal {
                positive: true,
                atom: atom.clone(),
            });
            Some(())
        }
        Formula::Neg(inner) => match inner.as_ref() {
            Formula::Atom(atom) => {
                out.push(Literal {
                    positive: false,
                    atom: atom.clone(),
                });
                Some(())
            }
            _ => None,
        },
        Formula::False => Some(()),
        _ => None,
    }
}

fn shift_clause(clause: &mut [Literal], shift: VarId) {
    for literal in clause {
        shift_atom(&mut literal.atom, shift);
    }
}

fn shift_atom(atom: &mut Atom, shift: VarId) {
    match atom {
        Atom::Pred(_, terms) => terms.iter_mut().for_each(|term| shift_term(term, shift)),
        Atom::Eq(left, right) => {
            shift_term(left, shift);
            shift_term(right, shift);
        }
    }
}

fn shift_term(term: &mut Term, shift: VarId) {
    match term {
        Term::Var(var) => *var = var.saturating_add(shift),
        Term::App(_, args) => args.iter_mut().for_each(|arg| shift_term(arg, shift)),
    }
}

fn max_var_clause(clause: &[Literal]) -> VarId {
    clause
        .iter()
        .flat_map(|literal| atom_vars(&literal.atom))
        .max()
        .unwrap_or(0)
}

fn atom_vars(atom: &Atom) -> Vec<VarId> {
    let mut out = Vec::new();
    match atom {
        Atom::Pred(_, args) => args
            .iter()
            .for_each(|term| collect_term_vars(term, &mut out)),
        Atom::Eq(left, right) => {
            collect_term_vars(left, &mut out);
            collect_term_vars(right, &mut out);
        }
    }
    out
}

fn collect_term_vars(term: &Term, out: &mut Vec<VarId>) {
    match term {
        Term::Var(var) => out.push(*var),
        Term::App(_, args) => args.iter().for_each(|arg| collect_term_vars(arg, out)),
    }
}

fn unify_terms(left: &Term, right: &Term, substitution: &mut HashMap<VarId, Term>) -> bool {
    let left = resolve_term(left, substitution);
    let right = resolve_term(right, substitution);
    match (&left, &right) {
        (Term::Var(left), Term::Var(right)) if left == right => true,
        (Term::Var(var), term) | (term, Term::Var(var)) => {
            if occurs(*var, term, substitution) {
                false
            } else {
                substitution.insert(*var, term.clone());
                true
            }
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| unify_terms(left, right, substitution))
        }
    }
}

fn resolve_term(term: &Term, substitution: &HashMap<VarId, Term>) -> Term {
    match term {
        Term::Var(var) => substitution
            .get(var)
            .map(|term| resolve_term(term, substitution))
            .unwrap_or_else(|| term.clone()),
        _ => term.clone(),
    }
}

fn occurs(var: VarId, term: &Term, substitution: &HashMap<VarId, Term>) -> bool {
    match term {
        Term::Var(other) => {
            *other == var
                || substitution
                    .get(other)
                    .is_some_and(|term| occurs(var, term, substitution))
        }
        Term::App(_, args) => args.iter().any(|arg| occurs(var, arg, substitution)),
    }
}

fn apply_substitution_literal(literal: &Literal, substitution: &HashMap<VarId, Term>) -> Literal {
    Literal {
        positive: literal.positive,
        atom: apply_substitution_atom(&literal.atom, substitution),
    }
}

fn apply_substitution_atom(atom: &Atom, substitution: &HashMap<VarId, Term>) -> Atom {
    match atom {
        Atom::Pred(symbol, args) => Atom::Pred(
            *symbol,
            args.iter()
                .map(|term| apply_substitution_term(term, substitution))
                .collect(),
        ),
        Atom::Eq(left, right) => Atom::Eq(
            apply_substitution_term(left, substitution),
            apply_substitution_term(right, substitution),
        ),
    }
}

fn apply_substitution_term(term: &Term, substitution: &HashMap<VarId, Term>) -> Term {
    match term {
        Term::Var(var) => substitution
            .get(var)
            .map(|term| apply_substitution_term(term, substitution))
            .unwrap_or_else(|| term.clone()),
        Term::App(symbol, args) => Term::App(
            *symbol,
            args.iter()
                .map(|arg| apply_substitution_term(arg, substitution))
                .collect(),
        ),
    }
}

fn clause_alpha_equiv(left: &[Literal], right: &[Literal]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    fn go(
        idx: usize,
        left: &[Literal],
        right: &[Literal],
        used: &mut [bool],
        mapping: &mut HashMap<VarId, VarId>,
        reverse: &mut HashMap<VarId, VarId>,
    ) -> bool {
        if idx == left.len() {
            return true;
        }
        for right_idx in 0..right.len() {
            if used[right_idx] || left[idx].positive != right[right_idx].positive {
                continue;
            }
            let mut next_mapping = mapping.clone();
            let mut next_reverse = reverse.clone();
            if atom_alpha_equiv(
                &left[idx].atom,
                &right[right_idx].atom,
                &mut next_mapping,
                &mut next_reverse,
            ) {
                used[right_idx] = true;
                if go(
                    idx + 1,
                    left,
                    right,
                    used,
                    &mut next_mapping,
                    &mut next_reverse,
                ) {
                    return true;
                }
                used[right_idx] = false;
            }
        }
        false
    }
    go(
        0,
        left,
        right,
        &mut vec![false; right.len()],
        &mut HashMap::new(),
        &mut HashMap::new(),
    )
}

/// Remove syntactically duplicate literals, keeping the first of each
/// class. Duplicate literals are logically inert (`C | L | L` ≡ `C | L`),
/// but the prover does not always condense intermediate clauses while
/// conclusions are condensed, so strict checks must compare modulo
/// condensation.
///
/// NOTE: only *syntactic* duplicates are removed. Merging
/// alpha-variant literals (`p(X,Y) | p(U,V)` to `p(X,Y)`) is unsound in
/// general (it strengthens the clause) and must go through real
/// condensation (unify + global substitution), never silent deletion.
fn condense_clause(clause: &[Literal]) -> Vec<Literal> {
    let mut kept: Vec<Literal> = Vec::with_capacity(clause.len());
    for literal in clause {
        if !kept.contains(literal) {
            kept.push(literal.clone());
        }
    }
    kept
}

fn atom_alpha_equiv(
    left: &Atom,
    right: &Atom,
    mapping: &mut HashMap<VarId, VarId>,
    reverse: &mut HashMap<VarId, VarId>,
) -> bool {
    match (left, right) {
        (Atom::Pred(left_symbol, left_args), Atom::Pred(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| term_alpha_equiv(left, right, mapping, reverse))
        }
        (Atom::Eq(left_l, left_r), Atom::Eq(right_l, right_r)) => {
            let mut direct_mapping = mapping.clone();
            let mut direct_reverse = reverse.clone();
            if term_alpha_equiv(left_l, right_l, &mut direct_mapping, &mut direct_reverse)
                && term_alpha_equiv(left_r, right_r, &mut direct_mapping, &mut direct_reverse)
            {
                *mapping = direct_mapping;
                *reverse = direct_reverse;
                true
            } else {
                let mut flip_mapping = mapping.clone();
                let mut flip_reverse = reverse.clone();
                if term_alpha_equiv(left_l, right_r, &mut flip_mapping, &mut flip_reverse)
                    && term_alpha_equiv(left_r, right_l, &mut flip_mapping, &mut flip_reverse)
                {
                    *mapping = flip_mapping;
                    *reverse = flip_reverse;
                    true
                } else {
                    false
                }
            }
        }
        _ => false,
    }
}

fn term_weight(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_weight).sum::<usize>(),
    }
}

fn term_alpha_equiv(
    left: &Term,
    right: &Term,
    mapping: &mut HashMap<VarId, VarId>,
    reverse: &mut HashMap<VarId, VarId>,
) -> bool {
    match (left, right) {
        (Term::Var(left), Term::Var(right)) => {
            if let Some(mapped) = mapping.get(left) {
                mapped == right
            } else if reverse.contains_key(right) {
                false
            } else {
                mapping.insert(*left, *right);
                reverse.insert(*right, *left);
                true
            }
        }
        (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args)) => {
            left_symbol == right_symbol
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| term_alpha_equiv(left, right, mapping, reverse))
        }
        _ => false,
    }
}

fn contains_exists(formula: &Formula) -> bool {
    match formula {
        Formula::Exists(_, _) => true,
        Formula::Forall(_, body) | Formula::Neg(body) => contains_exists(body),
        Formula::And(parts) | Formula::Or(parts) => parts.iter().any(contains_exists),
        Formula::Implies(left, right) | Formula::Iff(left, right) => {
            contains_exists(left) || contains_exists(right)
        }
        Formula::Atom(_) | Formula::True | Formula::False => false,
    }
}

fn alpha_equiv(left: &Formula, right: &Formula) -> bool {
    mrs_core::alpha::alpha_equiv(left, right)
}

fn to_nnf(formula: &Formula) -> Formula {
    fn visit(formula: &Formula, negated: bool) -> Formula {
        match formula {
            Formula::Atom(atom) => {
                if negated {
                    Formula::neg(Formula::Atom(atom.clone()))
                } else {
                    Formula::Atom(atom.clone())
                }
            }
            Formula::True => {
                if negated {
                    Formula::False
                } else {
                    Formula::True
                }
            }
            Formula::False => {
                if negated {
                    Formula::True
                } else {
                    Formula::False
                }
            }
            Formula::Neg(inner) => visit(inner, !negated),
            Formula::And(parts) => {
                let parts = parts.iter().map(|part| visit(part, negated)).collect();
                if negated {
                    Formula::or(parts)
                } else {
                    Formula::and(parts)
                }
            }
            Formula::Or(parts) => {
                let parts = parts.iter().map(|part| visit(part, negated)).collect();
                if negated {
                    Formula::and(parts)
                } else {
                    Formula::or(parts)
                }
            }
            Formula::Implies(left, right) => {
                if negated {
                    Formula::and(vec![visit(left, false), visit(right, true)])
                } else {
                    Formula::or(vec![visit(left, true), visit(right, false)])
                }
            }
            Formula::Iff(left, right) => {
                if negated {
                    Formula::and(vec![
                        Formula::or(vec![visit(left, false), visit(right, false)]),
                        Formula::or(vec![visit(left, true), visit(right, true)]),
                    ])
                } else {
                    Formula::and(vec![
                        Formula::or(vec![visit(left, true), visit(right, false)]),
                        Formula::or(vec![visit(left, false), visit(right, true)]),
                    ])
                }
            }
            Formula::Forall(var, body) => {
                if negated {
                    Formula::exists(*var, visit(body, true))
                } else {
                    Formula::forall(*var, visit(body, false))
                }
            }
            Formula::Exists(var, body) => {
                if negated {
                    Formula::forall(*var, visit(body, true))
                } else {
                    Formula::exists(*var, visit(body, false))
                }
            }
        }
    }
    visit(formula, false)
}

fn lower_annotated(
    symbols: &mut SymbolTable,
    formula: &AnnotatedFormula<'_>,
    limits: VerificationLimits,
) -> Result<Formula, KernelVerdict> {
    let mut ctx = LowerCtx::new(symbols, limits);
    let formula = match formula {
        AnnotatedFormula::FOF(formula) => lower_fof_statement(&mut ctx, &formula.formula),
        AnnotatedFormula::CNF(formula) => lower_cnf_statement(&mut ctx, &formula.formula),
        _ => {
            return Err(KernelVerdict::Inconclusive(
                "unsupported proof dialect".into(),
            ));
        }
    }?;
    Ok(formula)
}

struct LowerCtx<'a> {
    symbols: &'a mut SymbolTable,
    vars: HashMap<String, VarId>,
    next_var: VarId,
    limits: VerificationLimits,
    nodes: usize,
}

impl<'a> LowerCtx<'a> {
    fn new(symbols: &'a mut SymbolTable, limits: VerificationLimits) -> Self {
        Self {
            symbols,
            vars: HashMap::new(),
            next_var: 0,
            limits,
            nodes: 0,
        }
    }

    fn step(&mut self) -> Result<(), KernelVerdict> {
        self.nodes += 1;
        if self.nodes > self.limits.max_formula_nodes {
            Err(KernelVerdict::Inconclusive(
                "formula exceeds strict kernel node limit".into(),
            ))
        } else {
            Ok(())
        }
    }

    fn symbol(&mut self, name: &str) -> mrs_core::SymbolId {
        self.symbols.intern(name)
    }

    fn term_depth(&self, term: &Term) -> usize {
        match term {
            Term::Var(_) => 0,
            Term::App(_, args) => {
                1 + args
                    .iter()
                    .map(|arg| self.term_depth(arg))
                    .max()
                    .unwrap_or(0)
            }
        }
    }
}

fn lower_fof_statement(
    ctx: &mut LowerCtx<'_>,
    statement: &FOFStatement<'_>,
) -> Result<Formula, KernelVerdict> {
    match statement {
        FOFStatement::Logical(formula) => lower_fof_formula(ctx, formula),
        FOFStatement::Sequent(_, _) => Err(KernelVerdict::Inconclusive(
            "FOF sequents are not supported by the strict kernel".into(),
        )),
    }
}

fn lower_fof_formula(
    ctx: &mut LowerCtx<'_>,
    formula: &FOFFormula<'_>,
) -> Result<Formula, KernelVerdict> {
    ctx.step()?;
    match formula {
        FOFFormula::Atomic(atom) => lower_fof_atom(ctx, atom),
        FOFFormula::Negation(inner) => Ok(Formula::neg(lower_fof_formula(ctx, inner)?)),
        FOFFormula::Parens(inner) => lower_fof_formula(ctx, inner),
        FOFFormula::Equality(left, right) => Ok(Formula::atom(Atom::Eq(
            lower_fof_term(ctx, left)?,
            lower_fof_term(ctx, right)?,
        ))),
        FOFFormula::Inequality(left, right) => Ok(Formula::neg(Formula::atom(Atom::Eq(
            lower_fof_term(ctx, left)?,
            lower_fof_term(ctx, right)?,
        )))),
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => {
            let left = lower_fof_formula(ctx, left)?;
            let right = lower_fof_formula(ctx, right)?;
            Ok(match connective {
                BinaryConnective::And => Formula::and(vec![left, right]),
                BinaryConnective::Or => Formula::or(vec![left, right]),
                BinaryConnective::Impl => Formula::implies(left, right),
                BinaryConnective::RevImpl => Formula::implies(right, left),
                BinaryConnective::Iff => Formula::iff(left, right),
                BinaryConnective::Xor => Formula::neg(Formula::iff(left, right)),
                BinaryConnective::Nor => Formula::neg(Formula::or(vec![left, right])),
                BinaryConnective::Nand => Formula::neg(Formula::and(vec![left, right])),
            })
        }
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            let mut ids = Vec::with_capacity(variables.len());
            let mut saved = Vec::with_capacity(variables.len());
            for variable in variables {
                let name = (*variable).to_string();
                saved.push((name.clone(), ctx.vars.get(&name).copied()));
                let id = ctx.next_var;
                ctx.next_var += 1;
                ctx.vars.insert(name, id);
                ids.push(id);
            }
            let body = lower_fof_formula(ctx, formula)?;
            for (name, old) in saved {
                if let Some(old) = old {
                    ctx.vars.insert(name, old);
                } else {
                    ctx.vars.remove(&name);
                }
            }
            let mut result = body;
            for id in ids.into_iter().rev() {
                result = match quantifier {
                    Quantifier::Forall => Formula::forall(id, result),
                    Quantifier::Exists => Formula::exists(id, result),
                };
            }
            Ok(result)
        }
    }
}

fn lower_fof_atom(
    ctx: &mut LowerCtx<'_>,
    atom: &FOFAtomicFormula<'_>,
) -> Result<Formula, KernelVerdict> {
    let result = match atom {
        FOFAtomicFormula::Plain(name, args) => Formula::atom(Atom::Pred(
            ctx.symbol(name.as_str()),
            lower_terms(ctx, args)?,
        )),
        FOFAtomicFormula::Defined(name, args) => Formula::atom(Atom::Pred(
            ctx.symbol(&format!("${}", name.0)),
            lower_terms(ctx, args)?,
        )),
        FOFAtomicFormula::System(name, args) => Formula::atom(Atom::Pred(
            ctx.symbol(&format!("$${}", name.0)),
            lower_terms(ctx, args)?,
        )),
        FOFAtomicFormula::True => Formula::True,
        FOFAtomicFormula::False => Formula::False,
    };
    Ok(result)
}

fn lower_terms(ctx: &mut LowerCtx<'_>, terms: &[FOFTerm<'_>]) -> Result<Vec<Term>, KernelVerdict> {
    terms.iter().map(|term| lower_fof_term(ctx, term)).collect()
}

fn lower_fof_term(ctx: &mut LowerCtx<'_>, term: &FOFTerm<'_>) -> Result<Term, KernelVerdict> {
    let result = match term {
        FOFTerm::Variable(name) => {
            let id = if let Some(id) = ctx.vars.get(*name) {
                *id
            } else {
                let id = ctx.next_var;
                ctx.next_var += 1;
                ctx.vars.insert((*name).to_string(), id);
                id
            };
            Term::Var(id)
        }
        FOFTerm::Function(name, args) => {
            Term::App(ctx.symbol(name.as_str()), lower_terms(ctx, args)?)
        }
        FOFTerm::DefinedFunction(name, args) => {
            Term::App(ctx.symbol(&format!("${}", name.0)), lower_terms(ctx, args)?)
        }
        FOFTerm::SystemFunction(name, args) => Term::App(
            ctx.symbol(&format!("$${}", name.0)),
            lower_terms(ctx, args)?,
        ),
        FOFTerm::Number(number) => Term::App(ctx.symbol(number.as_str()), Vec::new()),
        FOFTerm::DistinctObject(value) => {
            Term::App(ctx.symbol(&format!("\"{value}\"")), Vec::new())
        }
    };
    if ctx.term_depth(&result) > ctx.limits.max_term_depth {
        return Err(KernelVerdict::Inconclusive(
            "term exceeds strict kernel depth limit".into(),
        ));
    }
    Ok(result)
}

fn lower_cnf_statement(
    ctx: &mut LowerCtx<'_>,
    statement: &CNFStatement<'_>,
) -> Result<Formula, KernelVerdict> {
    match statement {
        CNFStatement::Logical(formula) => lower_cnf_formula(ctx, formula),
    }
}

fn lower_cnf_formula(
    ctx: &mut LowerCtx<'_>,
    formula: &CNFFormula<'_>,
) -> Result<Formula, KernelVerdict> {
    ctx.step()?;
    match formula {
        CNFFormula::Parens(inner) => lower_cnf_formula(ctx, inner),
        CNFFormula::Disjunction(literals) => {
            let mut variables = Vec::new();
            let parts = literals
                .iter()
                .map(|literal| lower_cnf_literal(ctx, literal, &mut variables))
                .collect::<Result<Vec<_>, _>>()?;
            let mut result = if parts.is_empty() {
                Formula::False
            } else {
                Formula::or(parts)
            };
            for variable in variables.into_iter().rev() {
                result = Formula::forall(variable, result);
            }
            Ok(result)
        }
    }
}

fn lower_cnf_literal(
    ctx: &mut LowerCtx<'_>,
    literal: &CNFLiteral<'_>,
    variables: &mut Vec<VarId>,
) -> Result<Formula, KernelVerdict> {
    let result = match literal {
        CNFLiteral::Positive(atom) => lower_cnf_atom(ctx, atom, variables)?,
        CNFLiteral::Negative(atom) => Formula::neg(lower_cnf_atom(ctx, atom, variables)?),
        CNFLiteral::Equality(left, right) => Formula::atom(Atom::Eq(
            lower_fof_term(ctx, left)?,
            lower_fof_term(ctx, right)?,
        )),
        CNFLiteral::Inequality(left, right) => Formula::neg(Formula::atom(Atom::Eq(
            lower_fof_term(ctx, left)?,
            lower_fof_term(ctx, right)?,
        ))),
    };
    for var in result.free_vars() {
        if !variables.contains(&var) {
            variables.push(var);
        }
    }
    Ok(result)
}

fn lower_cnf_atom(
    ctx: &mut LowerCtx<'_>,
    atom: &mrs_tptp::CNFAtomicFormula<'_>,
    variables: &mut Vec<VarId>,
) -> Result<Formula, KernelVerdict> {
    let result = match atom {
        mrs_tptp::CNFAtomicFormula::Plain(name, args) => Formula::atom(Atom::Pred(
            ctx.symbol(name.as_str()),
            lower_terms(ctx, args)?,
        )),
        mrs_tptp::CNFAtomicFormula::Defined(name, args) => Formula::atom(Atom::Pred(
            ctx.symbol(&format!("${}", name.0)),
            lower_terms(ctx, args)?,
        )),
        mrs_tptp::CNFAtomicFormula::System(name, args) => Formula::atom(Atom::Pred(
            ctx.symbol(&format!("$${}", name.0)),
            lower_terms(ctx, args)?,
        )),
        mrs_tptp::CNFAtomicFormula::True => Formula::True,
        mrs_tptp::CNFAtomicFormula::False => Formula::False,
    };
    variables.extend(result.free_vars());
    variables.sort_unstable();
    variables.dedup();
    Ok(result)
}

fn is_false_formula(formula: &AnnotatedFormula<'_>) -> bool {
    match formula {
        AnnotatedFormula::FOF(formula) => matches!(
            &formula.formula,
            FOFStatement::Logical(formula) if is_false_fof(formula)
        ),
        AnnotatedFormula::CNF(formula) => matches!(
            &formula.formula,
            CNFStatement::Logical(formula) if is_false_cnf(formula)
        ),
        _ => false,
    }
}

fn is_false_cnf(formula: &CNFFormula<'_>) -> bool {
    match formula {
        CNFFormula::Parens(inner) => is_false_cnf(inner),
        CNFFormula::Disjunction(literals) => {
            literals.is_empty()
                || literals.iter().all(|literal| {
                    matches!(
                        literal,
                        CNFLiteral::Positive(mrs_tptp::CNFAtomicFormula::False)
                            | CNFLiteral::Negative(mrs_tptp::CNFAtomicFormula::True)
                    )
                })
        }
    }
}

fn is_false_fof(formula: &FOFFormula<'_>) -> bool {
    match formula {
        FOFFormula::Parens(inner) => is_false_fof(inner),
        FOFFormula::Atomic(FOFAtomicFormula::False) => true,
        FOFFormula::Negation(inner) => {
            matches!(inner.as_ref(), FOFFormula::Atomic(FOFAtomicFormula::True))
        }
        _ => false,
    }
}

impl Node<'_> {
    fn status(&self) -> Option<&str> {
        self.formula
            .annotations()
            .and_then(|annotations| annotations.status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    fn check(problem: &str, proof: &str) -> KernelVerdict {
        let problem = parse_tptp(problem).expect("problem parses");
        let proof = parse_tptp(proof).expect("proof parses");
        verify_strict(&problem, &proof, VerificationLimits::default())
    }

    #[test]
    fn rejects_model_with_missing_predicate_interpretation() {
        let problem = parse_tptp("fof(a, axiom, p(a)).").expect("problem parses");
        let mut certificate = ModelCertificate {
            domain_size: 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: Default::default(),
            predicates: Default::default(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        certificate.digest = certificate.compute_digest();
        assert!(matches!(
            certificate.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Rejected(reason) if reason.contains("predicate `p`")
        ));
    }

    #[test]
    fn classifies_commutativity_axiom() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let left = Term::app(f, vec![Term::var(0), Term::var(1)]);
        let right = Term::app(f, vec![Term::var(1), Term::var(0)]);
        let (Term::App(left_symbol, left_args), Term::App(right_symbol, right_args)) =
            (&left, &right)
        else {
            panic!("not apps")
        };
        assert_eq!(left_symbol, right_symbol);
        assert_eq!(left_args.len(), 2);
        assert_eq!(right_args.len(), 2);
        let (Term::Var(x), Term::Var(y)) = (&left_args[0], &left_args[1]) else {
            panic!("left vars")
        };
        let (Term::Var(y2), Term::Var(x2)) = (&right_args[0], &right_args[1]) else {
            panic!("right vars")
        };
        assert_ne!(x, y);
        assert_eq!(x, x2);
        assert_eq!(y, y2);
        assert!(matches!(
            classify_ac_axiom(&left, &right),
            Some((symbol, AcAxiomKind::Commutative)) if symbol == f
        ));

        let assoc_left = Term::app(
            f,
            vec![Term::app(f, vec![Term::var(0), Term::var(1)]), Term::var(2)],
        );
        let assoc_right = Term::app(
            f,
            vec![Term::var(0), Term::app(f, vec![Term::var(1), Term::var(2)])],
        );
        assert!(matches!(
            classify_ac_axiom(&assoc_left, &assoc_right),
            Some((symbol, AcAxiomKind::Associative)) if symbol == f
        ));
    }

    #[test]
    fn certifies_ac_superposition_replay() {
        let mut symbols = SymbolTable::new();
        let positive = symbols.intern("positive_part");
        let lub = symbols.intern("least_upper_bound");
        let glb = symbols.intern("greatest_lower_bound");
        let identity = symbols.intern("identity");
        let app = |symbol, args| Term::app(symbol, args);
        let constant = |symbol| Term::constant(symbol);
        let atom = |left, right| Formula::atom(Atom::eq(left, right));
        let x = Term::var(0);
        let y = Term::var(1);
        let z = Term::var(2);

        let source = atom(
            app(positive, vec![x.clone()]),
            app(lub, vec![x.clone(), constant(identity)]),
        );
        let target = atom(
            y.clone(),
            app(glb, vec![y.clone(), app(lub, vec![z.clone(), y.clone()])]),
        );
        let conclusion = atom(
            constant(identity),
            app(
                glb,
                vec![constant(identity), app(positive, vec![z.clone()])],
            ),
        );
        let lub_assoc = atom(
            app(lub, vec![app(lub, vec![x.clone(), y.clone()]), z.clone()]),
            app(lub, vec![x.clone(), app(lub, vec![y.clone(), z.clone()])]),
        );
        let lub_comm = atom(
            app(lub, vec![x.clone(), y.clone()]),
            app(lub, vec![y.clone(), x.clone()]),
        );
        let glb_comm = atom(
            app(glb, vec![x.clone(), y.clone()]),
            app(glb, vec![y.clone(), x.clone()]),
        );
        let glb_assoc = atom(
            app(glb, vec![app(glb, vec![x.clone(), y.clone()]), z.clone()]),
            app(glb, vec![x.clone(), app(glb, vec![y.clone(), z.clone()])]),
        );

        assert_eq!(
            verify_ac_superposition(
                &[source, target, lub_assoc, lub_comm, glb_comm, glb_assoc],
                &conclusion,
                VerificationLimits::default(),
                &std::collections::HashSet::new(),
                &std::collections::HashSet::new(),
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_grp167_ac_superposition_shape() {
        fn lower(input: &str, symbols: &mut SymbolTable) -> Formula {
            let problem = parse_tptp(input).expect("formula parses");
            lower_annotated(symbols, &problem.formulas[0], VerificationLimits::default())
                .expect("formula lowers")
        }

        let mut symbols = SymbolTable::new();
        let source = lower(
            "cnf(source, plain, positive_part(X) = least_upper_bound(X, identity)).",
            &mut symbols,
        );
        let target = lower(
            "cnf(target, plain, X = greatest_lower_bound(X, least_upper_bound(Y, X))).",
            &mut symbols,
        );
        let conclusion = lower(
            "cnf(conclusion, plain, identity = greatest_lower_bound(identity, positive_part(X))).",
            &mut symbols,
        );
        let lub_assoc = lower(
            "cnf(lub_assoc, axiom, least_upper_bound(least_upper_bound(X,Y),Z) = least_upper_bound(X,least_upper_bound(Y,Z))).",
            &mut symbols,
        );
        let lub_comm = lower(
            "cnf(lub_comm, axiom, least_upper_bound(X,Y) = least_upper_bound(Y,X)).",
            &mut symbols,
        );
        let glb_comm = lower(
            "cnf(glb_comm, axiom, greatest_lower_bound(X,Y) = greatest_lower_bound(Y,X)).",
            &mut symbols,
        );
        let glb_assoc = lower(
            "cnf(glb_assoc, axiom, greatest_lower_bound(greatest_lower_bound(X,Y),Z) = greatest_lower_bound(X,greatest_lower_bound(Y,Z))).",
            &mut symbols,
        );

        assert_eq!(
            verify_ac_superposition(
                &[source, target, lub_assoc, lub_comm, glb_comm, glb_assoc],
                &conclusion,
                VerificationLimits::default(),
                &std::collections::HashSet::new(),
                &std::collections::HashSet::new(),
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_ac_normalization_with_reversed_equality() {
        fn lower(input: &str, symbols: &mut SymbolTable) -> Formula {
            let problem = parse_tptp(input).expect("formula parses");
            lower_annotated(symbols, &problem.formulas[0], VerificationLimits::default())
                .expect("formula lowers")
        }

        let mut symbols = SymbolTable::new();
        let source = lower(
            "cnf(source, axiom, least_upper_bound(X, greatest_lower_bound(X,Y)) = X).",
            &mut symbols,
        );
        let conclusion = lower(
            "cnf(conclusion, plain, X = least_upper_bound(X, greatest_lower_bound(X,Y))).",
            &mut symbols,
        );
        let lub_assoc = lower(
            "cnf(lub_assoc, axiom, least_upper_bound(least_upper_bound(X,Y),Z) = least_upper_bound(X,least_upper_bound(Y,Z))).",
            &mut symbols,
        );
        let lub_comm = lower(
            "cnf(lub_comm, axiom, least_upper_bound(X,Y) = least_upper_bound(Y,X)).",
            &mut symbols,
        );
        let glb_comm = lower(
            "cnf(glb_comm, axiom, greatest_lower_bound(X,Y) = greatest_lower_bound(Y,X)).",
            &mut symbols,
        );
        let glb_assoc = lower(
            "cnf(glb_assoc, axiom, greatest_lower_bound(greatest_lower_bound(X,Y),Z) = greatest_lower_bound(X,greatest_lower_bound(Y,Z))).",
            &mut symbols,
        );

        assert_eq!(
            verify_ac_normalization(
                &[source, lub_assoc, lub_comm, glb_comm, glb_assoc],
                &conclusion,
                VerificationLimits::default(),
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_ac_normalization_predicate_args() {
        fn lower(input: &str, symbols: &mut SymbolTable) -> Formula {
            let problem = parse_tptp(input).expect("formula parses");
            lower_annotated(symbols, &problem.formulas[0], VerificationLimits::default())
                .expect("formula lowers")
        }

        let mut symbols = SymbolTable::new();
        let source = lower("cnf(source, plain, ~def(vplus(X5, X3), X5)).", &mut symbols);
        let conclusion = lower(
            "cnf(conclusion, plain, ~def(vplus(X3, X5), X5)).",
            &mut symbols,
        );
        let plus_comm = lower(
            "cnf(plus_comm, axiom, vplus(X1, X0) = vplus(X0, X1)).",
            &mut symbols,
        );
        let plus_assoc = lower(
            "cnf(plus_assoc, axiom, vplus(vplus(X0, X1), X2) = vplus(X0, vplus(X1, X2))).",
            &mut symbols,
        );

        assert_eq!(
            verify_ac_normalization(
                &[source, plus_comm, plus_assoc],
                &conclusion,
                VerificationLimits::default(),
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_ac_normalization_with_nested_lub_glb_terms() {
        fn lower(input: &str, symbols: &mut SymbolTable) -> Formula {
            let problem = parse_tptp(input).expect("formula parses");
            lower_annotated(symbols, &problem.formulas[0], VerificationLimits::default())
                .expect("formula lowers")
        }

        let mut symbols = SymbolTable::new();
        let source = lower(
            "cnf(source, axiom, least_upper_bound(X, greatest_lower_bound(X,Y)) = X).",
            &mut symbols,
        );
        let conclusion = lower(
            "cnf(conclusion, plain, X = least_upper_bound(X, greatest_lower_bound(X,Y))).",
            &mut symbols,
        );
        let lub_assoc = lower(
            "cnf(lub_assoc, axiom, least_upper_bound(least_upper_bound(X,Y),Z) = least_upper_bound(X,least_upper_bound(Y,Z))).",
            &mut symbols,
        );
        let lub_comm = lower(
            "cnf(lub_comm, axiom, least_upper_bound(X,Y) = least_upper_bound(Y,X)).",
            &mut symbols,
        );
        let glb_comm = lower(
            "cnf(glb_comm, axiom, greatest_lower_bound(X,Y) = greatest_lower_bound(Y,X)).",
            &mut symbols,
        );
        let glb_assoc = lower(
            "cnf(glb_assoc, axiom, greatest_lower_bound(greatest_lower_bound(X,Y),Z) = greatest_lower_bound(X,greatest_lower_bound(Y,Z))).",
            &mut symbols,
        );

        assert_eq!(
            verify_ac_normalization(
                &[source, lub_assoc, lub_comm, glb_comm, glb_assoc],
                &conclusion,
                VerificationLimits::default(),
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_ac_normalization_with_nested_addition_reassociation() {
        fn lower(input: &str, symbols: &mut SymbolTable) -> Formula {
            let problem = parse_tptp(input).expect("formula parses");
            lower_annotated(symbols, &problem.formulas[0], VerificationLimits::default())
                .expect("formula lowers")
        }

        let mut symbols = SymbolTable::new();
        let source = lower(
            "cnf(source, axiom, addition(multiplication(addition(X9,multiplication(X9,X1)),X8),multiplication(X9,X11)) = multiplication(X9,addition(addition(X8,multiplication(X1,X8)),X11))).",
            &mut symbols,
        );
        let conclusion = lower(
            "cnf(conclusion, plain, addition(multiplication(X9,X11),multiplication(addition(X9,multiplication(X9,X1)),X8)) = multiplication(X9,addition(X8,addition(multiplication(X1,X8),X11)))).",
            &mut symbols,
        );
        let add_assoc = lower(
            "cnf(add_assoc, axiom, addition(addition(X,Y),Z) = addition(X,addition(Y,Z))).",
            &mut symbols,
        );
        let add_comm = lower(
            "cnf(add_comm, axiom, addition(X,Y) = addition(Y,X)).",
            &mut symbols,
        );

        assert_eq!(
            verify_ac_normalization(
                &[source, add_assoc, add_comm],
                &conclusion,
                VerificationLimits::default(),
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_ac_normalization_with_nested_meet_reassociation() {
        fn lower(input: &str, symbols: &mut SymbolTable) -> Formula {
            let problem = parse_tptp(input).expect("formula parses");
            lower_annotated(symbols, &problem.formulas[0], VerificationLimits::default())
                .expect("formula lowers")
        }

        let mut symbols = SymbolTable::new();
        let source = lower(
            "cnf(source, axiom, meet(X9,meet(X1,X9)) = meet(X9,meet(meet(X1,X9),join(X9,meet(X1,X9))))).",
            &mut symbols,
        );
        let conclusion = lower(
            "cnf(conclusion, plain, meet(X1,meet(X9,X9)) = meet(X1,meet(X9,meet(X9,join(X9,meet(X1,X9)))))).",
            &mut symbols,
        );
        let meet_assoc = lower(
            "cnf(meet_assoc, axiom, meet(meet(X,Y),Z) = meet(X,meet(Y,Z))).",
            &mut symbols,
        );
        let meet_comm = lower(
            "cnf(meet_comm, axiom, meet(X,Y) = meet(Y,X)).",
            &mut symbols,
        );

        assert_eq!(
            verify_ac_normalization(
                &[source, meet_assoc, meet_comm],
                &conclusion,
                VerificationLimits::default(),
            ),
            KernelVerdict::Certified
        );
    }

    fn flat_definition_problem() -> &'static str {
        "fof(src, axiom, q(a) | r(a)).\n\
         fof(nq, axiom, ~q(a)).\n\
         fof(nr, axiom, ~r(a))."
    }

    #[test]
    fn certifies_definition_remainder_across_quantifier_boundary() {
        // SYO606+1 c102 shape: the defined block `(g & ![X]h(X))` places
        // the quantifier inside while the definition body `(g & h(X))`
        // generalises across it. The kernel floats the vacuous placement
        // outward before matching.
        let problem = "fof(src, axiom, ((g & ![X] : h(X)) | ![Y] : (k & m(Y)))).\n\
                       fof(ng, axiom, ~g).\n\
                       fof(nk, axiom, ~k).";
        let proof = "fof(src, axiom, ((g & ![X] : h(X)) | ![Y] : (k & m(Y))), file('problem.p', src)).\n\
                     fof(ng, axiom, ~g, file('problem.p', ng)).\n\
                     fof(nk, axiom, ~k, file('problem.p', nk)).\n\
                     fof(d1, definition, ![X] : (d1(X) <=> (g & h(X))), introduced(definition, [new_symbols(definition, [d1])])).\n\
                     fof(d2, definition, ![Y] : (d2(Y) <=> (k & m(Y))), introduced(definition, [new_symbols(definition, [d2])])).\n\
                     cnf(main, plain, d1(X) | d2(Y), inference(cnf_transformation, [status(thm)], [src,d2,d1])).\n\
                     cnf(w1, plain, ~d1(X) | g, inference(cnf_transformation, [status(thm)], [src,d2,d1])).\n\
                     cnf(w2, plain, ~d2(Y) | k, inference(cnf_transformation, [status(thm)], [src,d2,d1])).\n\
                     cnf(r1, plain, ~d1(X), inference(resolution, [status(thm)], [w1,ng])).\n\
                     cnf(r2, plain, ~d2(Y), inference(resolution, [status(thm)], [w2,nk])).\n\
                     cnf(m1, plain, d2(Y), inference(resolution, [status(thm)], [main,r1])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m1,r2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_shared_definition_remainder() {
        // GEO125+1 c176 shape: two definitions cover the two disjuncts
        // of one source block; the remainder keeps one literal from the
        // source alongside both defined heads.
        let problem = "fof(src, axiom, ![X,Y] : (~b(X,Y) | ((p(X,Y) & q(X,Y)) | (r(X,Y) & s(X,Y))))).\n\
                       fof(fb, axiom, b(c,d)).\n\
                       fof(fp, axiom, ~p(c,d)).\n\
                       fof(fr, axiom, ~r(c,d)).";
        let proof = "fof(src, axiom, ![X,Y] : (~b(X,Y) | ((p(X,Y) & q(X,Y)) | (r(X,Y) & s(X,Y)))), file('problem.p', src)).\n\
                     fof(fb, axiom, b(c,d), file('problem.p', fb)).\n\
                     fof(fp, axiom, ~p(c,d), file('problem.p', fp)).\n\
                     fof(fr, axiom, ~r(c,d), file('problem.p', fr)).\n\
                     fof(d0, definition, ![X,Y] : (d0(X,Y) <=> (p(X,Y) & q(X,Y))), introduced(definition, [new_symbols(definition, [d0])])).\n\
                     fof(d1, definition, ![X,Y] : (d1(X,Y) <=> (r(X,Y) & s(X,Y))), introduced(definition, [new_symbols(definition, [d1])])).\n\
                     cnf(main, plain, ~b(X,Y) | d0(X,Y) | d1(X,Y), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\n\
                     cnf(w0, plain, ~d0(X,Y) | p(X,Y), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\n\
                     cnf(w1, plain, ~d1(X,Y) | r(X,Y), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\n\
                     cnf(m1, plain, d0(c,d) | d1(c,d), inference(resolution, [status(thm)], [main,fb])).\n\
                     cnf(m2, plain, d1(c,d) | p(c,d), inference(resolution, [status(thm)], [m1,w0])).\n\
                     cnf(m3, plain, d1(c,d), inference(resolution, [status(thm)], [m2,fp])).\n\
                     cnf(m4, plain, r(c,d), inference(resolution, [status(thm)], [m3,w1])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m4,fr])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_symmetric_definition_blocks() {
        // GEO125+1 c176 shape: two definitions cover symmetric blocks;
        // each block must be claimed by its own (identity-matching)
        // definition, not cross-matched by permutation.
        let problem = "fof(src, axiom, ![X0]: (![X1]: (![X2]: (![X3]: (((~(between_o(X0, X1, X2, X3)) | ((ordered_by(X0, X3, X2) & ordered_by(X0, X2, X1)) | (ordered_by(X0, X2, X3) & ordered_by(X0, X1, X2)))) & (between_o(X0, X1, X2, X3) | ((~(ordered_by(X0, X3, X2)) | ~(ordered_by(X0, X2, X1))) & (~(ordered_by(X0, X2, X3)) | ~(ordered_by(X0, X1, X2))))))))))).\n\
                       fof(fb, axiom, between_o(a,b,c,d)).\n\
                       fof(f1, axiom, ~ordered_by(a,d,c)).\n\
                       fof(f2, axiom, ~ordered_by(a,c,d)).";
        let proof = "fof(src, axiom, ![X0]: (![X1]: (![X2]: (![X3]: (((~(between_o(X0, X1, X2, X3)) | ((ordered_by(X0, X3, X2) & ordered_by(X0, X2, X1)) | (ordered_by(X0, X2, X3) & ordered_by(X0, X1, X2)))) & (between_o(X0, X1, X2, X3) | ((~(ordered_by(X0, X3, X2)) | ~(ordered_by(X0, X2, X1))) & (~(ordered_by(X0, X2, X3)) | ~(ordered_by(X0, X1, X2)))))))))), file('problem.p', src)).\n\
                     fof(fb, axiom, between_o(a,b,c,d), file('problem.p', fb)).\n\
                     fof(f1, axiom, ~ordered_by(a,d,c), file('problem.p', f1)).\n\
                     fof(f2, axiom, ~ordered_by(a,c,d), file('problem.p', f2)).\n\
                     fof(d0, definition, ![X0]: (![X1]: (![X2]: (![X3]: ((def_between_o_defn_0(X0, X1, X2, X3) <=> (ordered_by(X0, X3, X2) & ordered_by(X0, X2, X1))))))), introduced(definition, [new_symbols(definition, [def_between_o_defn_0])])).\n\
                     fof(d1, definition, ![X0]: (![X1]: (![X2]: (![X3]: ((def_between_o_defn_1(X0, X1, X2, X3) <=> (ordered_by(X0, X2, X3) & ordered_by(X0, X1, X2))))))), introduced(definition, [new_symbols(definition, [def_between_o_defn_1])])).\n\
                     cnf(main, plain, ~between_o(X0, X1, X2, X3) | def_between_o_defn_0(X0, X1, X2, X3) | def_between_o_defn_1(X0, X1, X2, X3), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\n\
                     cnf(w0, plain, ~def_between_o_defn_0(X0,X1,X2,X3) | ordered_by(X0,X3,X2), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\n\
                     cnf(w1, plain, ~def_between_o_defn_1(X0,X1,X2,X3) | ordered_by(X0,X2,X3), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\n\
                     cnf(m1, plain, def_between_o_defn_0(a,b,c,d) | def_between_o_defn_1(a,b,c,d), inference(resolution, [status(thm)], [main,fb])).\n\
                     cnf(m2, plain, def_between_o_defn_1(a,b,c,d) | ordered_by(a,d,c), inference(resolution, [status(thm)], [m1,w0])).\n\
                     cnf(m3, plain, def_between_o_defn_1(a,b,c,d), inference(resolution, [status(thm)], [m2,f1])).\n\
                     cnf(m4, plain, ordered_by(a,c,d), inference(resolution, [status(thm)], [m3,w1])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m4,f2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    fn flat_definition_proof() -> &'static str {
        "fof(src, axiom, q(a) | r(a), file('problem.p', src)).\
         fof(nq, axiom, ~q(a), file('problem.p', nq)).\
         fof(nr, axiom, ~r(a), file('problem.p', nr)).\
         fof(d0, definition, ![X] : (d0(X) <=> q(X)),\
             introduced(definition, [new_symbols(definition, [d0])])).\
         fof(d1, definition, ![X] : (d1(X) <=> (d0(X) | r(X))),\
             introduced(definition, [new_symbols(definition, [d1])])).\
         cnf(main, plain, d1(a),\
             inference(cnf_transformation, [status(thm)], [src,d1,d0])).\
         cnf(d0_forward, plain, ~d0(X) | q(X),\
             inference(cnf_transformation, [status(thm)], [src,d1,d0])).\
         cnf(d1_forward, plain, ~d1(X) | d0(X) | r(X),\
             inference(cnf_transformation, [status(thm)], [src,d1,d0])).\
         cnf(nd0, plain, ~d0(a),\
             inference(resolution, [status(thm)], [d0_forward,nq])).\
         cnf(nd1_or_d0, plain, ~d1(a) | d0(a),\
             inference(resolution, [status(thm)], [d1_forward,nr])).\
         cnf(nd1, plain, ~d1(a),\
             inference(resolution, [status(thm)], [nd1_or_d0,nd0])).\
         cnf(bot, plain, $false,\
         inference(resolution, [status(thm)], [main,nd1]))."
    }

    fn explicit_avatar_sat_trace() -> (String, String) {
        let manifest = vec![vec![1, 2], vec![-1], vec![-2]];
        let original_ids = vec![1, 2, 4];
        let trace = b"o 1  1 2 0\no 2  -1 0\na 3  2 0  l 2 1 0\no 4  -2 0\na 5  0  l 3 4 0\nd 4  -2 0\nf 2  -1 0\nf 3  2 0\nf 1  1 2 0\nf 5  0\n";
        let digest = mrs_core::clause::avatar_sat_trace_digest(
            "frat-lrat",
            2,
            &original_ids,
            &[0, 1, 2],
            &manifest,
            trace,
        );
        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let annotation = format!(
            "avatar_sat_refutation([split], [branch_p, branch_q], sat_trace('frat-lrat', 2, '{}', [1, 2, 4], [0, 1, 2], [[1, 2], [-1], [-2]], '{}'))",
            hex(&digest),
            hex(trace),
        );
        (annotation, hex(&digest))
    }

    fn explicit_avatar_sat_trace_lrat() -> (String, String) {
        let manifest = vec![vec![1, 2], vec![-1], vec![-2]];
        let original_ids = vec![1, 2, 3];
        let trace = b"4 2 0 1 2 0\n5 d 2 0\n6 0 4 3 0\n";
        let digest = mrs_core::clause::avatar_sat_trace_digest(
            "lrat",
            2,
            &original_ids,
            &[0, 1, 2],
            &manifest,
            trace,
        );
        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let annotation = format!(
            "avatar_sat_refutation([split], [branch_p, branch_q], sat_trace('lrat', 2, '{}', [1, 2, 3], [0, 1, 2], [[1, 2], [-1], [-2]], '{}'))",
            hex(&digest),
            hex(trace),
        );
        (annotation, hex(&digest))
    }

    #[test]
    fn certifies_direct_resolution() {
        let input = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\n\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\n\
                     fof(s, plain, $false, inference(resolution, [status(thm)], [a,b])).";
        assert_eq!(check(input, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_bounded_formula_equivalence() {
        let problem = "fof(a, axiom, (p & q & p)).\nfof(n, axiom, ~p).";
        let proof = "fof(a, axiom, (p & q & p), file('problem.p', a)).\
                     fof(s, plain, (q & p), inference(formula_equivalence, [status(thm)], [a])).\
                     cnf(sc, plain, p, inference(cnf_transformation, [status(thm)], [s])).\
                     fof(n, axiom, ~p, file('problem.p', n)).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [sc,n])).";
        let verdict = check(problem, proof);
        eprintln!("quantified definition CNF verdict: {verdict}");
        assert_eq!(verdict, KernelVerdict::Certified);
    }

    #[test]
    fn certifies_quantified_cnf_after_explicit_skolemization() {
        let problem = "fof(src, axiom, ?[X] : p(X)).\n\
                       fof(n, axiom, ![X] : ~p(X)).";
        let proof = "fof(src, axiom, ?[X] : p(X), file('problem.p', src)).\
                     fof(n, axiom, ![X] : ~p(X), file('problem.p', n)).\
                     fof(sk, plain, p(sk0), inference(skolemisation, [status(esa)], [src])).\
                     cnf(c, plain, p(sk0), inference(cnf_transformation, [status(thm)], [src,sk])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [c,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_scoped_existential_quantified_cnf_clauses() {
        let problem = "fof(src, axiom, ![X] : ?[Y] : (p(X,Y) & q(Y))).\n\
                       fof(np, axiom, ![X,Y] : ~p(X,Y)).\n\
                       fof(nq, axiom, ![Y] : ~q(Y)).";
        let proof = "fof(src, axiom, ![X] : ?[Y] : (p(X,Y) & q(Y)), file('problem.p', src)).\
                     fof(np, axiom, ![X,Y] : ~p(X,Y), file('problem.p', np)).\
                     fof(nq, axiom, ![Y] : ~q(Y), file('problem.p', nq)).\
                     fof(sk, plain, ![X] : (p(X,sk0(X)) & q(sk0(X))), inference(skolemisation, [status(esa)], [src])).\
                     cnf(cp, plain, p(X,sk0(X)), inference(cnf_transformation, [status(thm)], [src,sk])).\
                     cnf(cq, plain, q(sk0(X)), inference(cnf_transformation, [status(thm)], [src,sk])).\
                     cnf(fp, plain, $false, inference(resolution, [status(thm)], [cp,np])).\
                     cnf(fq, plain, $false, inference(resolution, [status(thm)], [cq,nq])).\
                     fof(pair, plain, ($false & $false), inference(conjunction, [status(thm)], [fp,fq])).\
                     fof(bot, plain, $false, inference(split_conjunct, [status(thm)], [pair])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_quantified_cnf_with_a_fresh_definition_parent() {
        let problem = "fof(src, axiom, ?[X] : (p(X) | (q(X) & r(X)))).\n\
                       fof(np, axiom, ![X] : ~p(X)).\n\
                       fof(nq, axiom, ![X] : ~q(X)).";
        let proof = "fof(src, axiom, ?[X] : (p(X) | (q(X) & r(X))), file('problem.p', src)).\
                     fof(np, axiom, ![X] : ~p(X), file('problem.p', np)).\
                     fof(nq, axiom, ![X] : ~q(X), file('problem.p', nq)).\
                     fof(sk, plain, p(sk0) | (q(sk0) & r(sk0)), inference(skolemisation, [status(esa)], [src])).\
                     fof(d, definition, ![X] : (d(X) <=> (q(X) & r(X))), introduced(definition, [new_symbols(definition, [d])])).\
                     cnf(main, plain, p(sk0) | d(sk0), inference(cnf_transformation, [status(thm)], [src,sk,d])).\
                     cnf(dq, plain, ~d(X) | q(X), inference(cnf_transformation, [status(thm)], [src,sk,d])).\
                     cnf(mid, plain, d(sk0), inference(resolution, [status(thm)], [main,np])).\
                     cnf(nd, plain, ~d(X), inference(resolution, [status(thm)], [dq,nq])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [mid,nd])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_cnf_with_quantifiers_under_disjunction() {
        let problem = "fof(src, axiom, (![X] : p(X)) | (![Y] : q(Y))).\n\
                       fof(np, axiom, ![X] : ~p(X)).\n\
                       fof(nq, axiom, ![Y] : ~q(Y)).";
        let proof = "fof(src, axiom, (![X] : p(X)) | (![Y] : q(Y)), file('problem.p', src)).\
                     fof(np, axiom, ![X] : ~p(X), file('problem.p', np)).\
                     fof(nq, axiom, ![Y] : ~q(Y), file('problem.p', nq)).\
                     cnf(c, plain, p(X) | q(Y), inference(cnf_transformation, [status(thm)], [src])).\
                     cnf(mid, plain, q(Y), inference(resolution, [status(thm)], [c,np])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [mid,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn quantified_cnf_requires_a_cited_skolemization_parent() {
        let problem = "fof(src, axiom, ?[X] : p(X)).\n\
                       fof(n, axiom, ![X] : ~p(X)).";
        let proof = "fof(src, axiom, ?[X] : p(X), file('problem.p', src)).\
                     fof(n, axiom, ![X] : ~p(X), file('problem.p', n)).\
                     cnf(c, plain, p(sk0), inference(cnf_transformation, [status(thm)], [src])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [c,n])).";
        let verdict = check(problem, proof);
        eprintln!("{verdict}");
        assert!(matches!(verdict, KernelVerdict::Inconclusive(_)));
    }

    #[test]
    fn quantified_cnf_rejects_unrelated_skolemization_parent() {
        let problem = "fof(src, axiom, ?[X] : p(X)).\n\
                       fof(other, axiom, ?[X] : q(X)).\n\
                       fof(n, axiom, ![X] : ~p(X)).";
        let proof = "fof(src, axiom, ?[X] : p(X), file('problem.p', src)).\
                     fof(other, axiom, ?[X] : q(X), file('problem.p', other)).\
                     fof(n, axiom, ![X] : ~p(X), file('problem.p', n)).\
                     fof(sk, plain, q(sk0), inference(skolemisation, [status(esa)], [other])).\
                     cnf(c, plain, p(sk0), inference(cnf_transformation, [status(thm)], [src,sk])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [c,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn quantified_cnf_normalization_limit_is_inconclusive() {
        let problem = parse_tptp(
            "fof(src, axiom, (![X] : p(X)) | (![Y] : q(Y))).\n\
             fof(np, axiom, ![X] : ~p(X)).\n\
             fof(nq, axiom, ![Y] : ~q(Y)).",
        )
        .expect("problem parses");
        let proof = parse_tptp(
            "fof(src, axiom, (![X] : p(X)) | (![Y] : q(Y)), file('problem.p', src)).\
             fof(np, axiom, ![X] : ~p(X), file('problem.p', np)).\
             fof(nq, axiom, ![Y] : ~q(Y), file('problem.p', nq)).\
             cnf(c, plain, p(X) | q(Y), inference(cnf_transformation, [status(thm)], [src])).\
             cnf(mid, plain, q(Y), inference(resolution, [status(thm)], [c,np])).\
             cnf(bot, plain, $false, inference(resolution, [status(thm)], [mid,nq])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_forged_formula_equivalence() {
        let problem = "fof(a, axiom, (p & q)).";
        let proof = "fof(a, axiom, (p & q), file('problem.p', a)).\
                     fof(s, plain, p, inference(formula_equivalence, [status(thm)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_inconsistent_symbol_arity() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, p(a, b)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, p(a, b), file('problem.p', b)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [a])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_proof_symbol_arity_change() {
        let problem = "fof(a, axiom, p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(s, plain, p(a, b), inference(formula_equivalence, [status(thm)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn formula_equivalence_limit_is_inconclusive() {
        let problem = parse_tptp("fof(a, axiom, (p & q)).").expect("problem parses");
        let proof = parse_tptp(
            "fof(a, axiom, (p & q), file('problem.p', a)).\
             fof(s, plain, (q & p), inference(formula_equivalence, [status(thm)], [a])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_formula_nodes: 2,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_subsumption_resolution_with_matching() {
        let input = "cnf(target, axiom, ~p(a) | q(a) | r(a)).\n\
                     cnf(active, axiom, p(X) | q(X)).\n\
                     cnf(nq, axiom, ~q(a)).\n\
                     cnf(nr, axiom, ~r(a)).";
        let proof = "cnf(target, axiom, ~p(a) | q(a) | r(a), file('problem.p', target)).\n\
                     cnf(active, axiom, p(X) | q(X), file('problem.p', active)).\n\
                     cnf(cut, plain, q(a) | r(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nq, axiom, ~q(a), file('problem.p', nq)).\n\
                     cnf(r, plain, r(a), inference(resolution, [status(thm)], [cut,nq])).\n\
                     cnf(nr, axiom, ~r(a), file('problem.p', nr)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [r,nr])).";
        assert_eq!(check(input, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_subsumption_resolution_condensed_conclusion() {
        // SYN353+1 shape: the target carries a duplicate literal while the
        // conclusion is condensed (`C | L | L` vs `C | L`).
        let input = "cnf(target, axiom, ~p(a) | q(a) | r(a) | q(a)).\n\
                     cnf(active, axiom, p(X) | q(X)).\n\
                     cnf(nq, axiom, ~q(a)).\n\
                     cnf(nr, axiom, ~r(a)).";
        let proof = "cnf(target, axiom, ~p(a) | q(a) | r(a) | q(a), file('problem.p', target)).\n\
                     cnf(active, axiom, p(X) | q(X), file('problem.p', active)).\n\
                     cnf(cut, plain, q(a) | r(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nq, axiom, ~q(a), file('problem.p', nq)).\n\
                     cnf(r, plain, r(a), inference(resolution, [status(thm)], [cut,nq])).\n\
                     cnf(nr, axiom, ~r(a), file('problem.p', nr)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [r,nr])).";
        assert_eq!(check(input, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_subsumption_resolution_restating_parent() {
        // COM130+1 c45780 shape: chained simplifications delete the
        // intermediate steps, leaving a conclusion identical to the
        // active parent (and not reachable by single-literal removal).
        // A premise entails itself, so the step is sound regardless of
        // rule naming.
        let input = "cnf(target, axiom, p(a) | q(a)).\n\
                     cnf(active, axiom, q(a) | r(a)).\n\
                     cnf(nq, axiom, ~q(a)).\n\
                     cnf(nr, axiom, ~r(a)).";
        let proof = "cnf(target, axiom, p(a) | q(a), file('problem.p', target)).\n\
                     cnf(active, axiom, q(a) | r(a), file('problem.p', active)).\n\
                     cnf(cut, plain, q(a) | r(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nq, axiom, ~q(a), file('problem.p', nq)).\n\
                     cnf(r, plain, r(a), inference(resolution, [status(thm)], [cut,nq])).\n\
                     cnf(nr, axiom, ~r(a), file('problem.p', nr)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [r,nr])).";
        assert_eq!(check(input, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_subsumption_resolution_conclusion() {
        let input = "cnf(target, axiom, ~p(a) | q(a) | r(a)).\n\
                     cnf(active, axiom, p(X) | q(X)).\n\
                     cnf(nq, axiom, ~q(a)).\n\
                     cnf(ns, axiom, ~s(a)).";
        let proof = "cnf(target, axiom, ~p(a) | q(a) | r(a), file('problem.p', target)).\n\
                     cnf(active, axiom, p(X) | q(X), file('problem.p', active)).\n\
                     cnf(forged, plain, q(a) | s(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nq, axiom, ~q(a), file('problem.p', nq)).\n\
                     cnf(s, plain, s(a), inference(resolution, [status(thm)], [forged,nq])).\n\
                     cnf(ns, axiom, ~s(a), file('problem.p', ns)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [s,ns])).";
        assert!(matches!(check(input, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_subsumption_resolution_reusing_a_target_literal() {
        // Set-style matching: both active literals may witness the same
        // flipped target literal once they coincide under σ. Sound because
        // σ(active) set-inclusion still yields `active ∧ target ⊨ target\{L}`
        // (`p(X)|p(Y)` entails `p(a)`, so `~p(a)|r(a)` reduces to `r(a)`).
        let input = "cnf(target, axiom, ~p(a) | r(a)).\n\
                     cnf(active, axiom, p(X) | p(Y)).\n\
                     cnf(nr, axiom, ~r(a)).";
        let proof = "cnf(target, axiom, ~p(a) | r(a), file('problem.p', target)).\n\
                     cnf(active, axiom, p(X) | p(Y), file('problem.p', active)).\n\
                     cnf(cut, plain, r(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nr, axiom, ~r(a), file('problem.p', nr)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [cut,nr])).";
        assert_eq!(check(input, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_subsumption_resolution_shared_flipped_literal() {
        // MGT005+1 c1709 shape: active `~g(X,Y)|~g(Y,X)` becomes two copies
        // of `~g(a,a)` under σ, but the flipped target holds only one.
        let input = "cnf(target, axiom, g(a,a) | r(a)).\n\
                     cnf(active, axiom, ~g(X,Y) | ~g(Y,X)).\n\
                     cnf(nr, axiom, ~r(a)).\n\
                     cnf(ng, axiom, ~g(a,a)).";
        let proof = "cnf(target, axiom, g(a,a) | r(a), file('problem.p', target)).\n\
                     cnf(active, axiom, ~g(X,Y) | ~g(Y,X), file('problem.p', active)).\n\
                     cnf(cut, plain, r(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nr, axiom, ~r(a), file('problem.p', nr)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [cut,nr])).";
        assert_eq!(check(input, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_resolution_conclusion() {
        let input = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\n\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\n\
                     fof(s, plain, q(a), inference(resolution, [status(thm)], [a,b])).";
        assert!(matches!(check(input, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_anonymous_leaf_provenance() {
        let input = "fof(a, axiom, p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', unknown)).\n\
                     fof(s, plain, $false, inference(consequence, [status(thm)], [a])).";
        assert!(matches!(
            check(input, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_unrelated_dangling_node() {
        let input = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\n\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\n\
                     fof(s, plain, $false, inference(resolution, [status(thm)], [a,b])).\n\
                     fof(d, axiom, q(a), file('problem.p', a)).";
        assert!(matches!(check(input, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_mrs_style_preprocessing_chain() {
        let problem = "fof(ax, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(goal, conjecture, q(a)).";
        let proof = "fof(ax, axiom, ![X] : (p(X) => q(X)), file('problem.p', ax)).\n\
                     fof(nnf, plain, ![X] : (~p(X) | q(X)), inference(fof_nnf_transformation, [status(thm)], [ax])).\n\
                     fof(skol, plain, ![X] : (~p(X) | q(X)), inference(skolemisation, [status(esa)], [nnf])).\n\
                     cnf(clause, plain, ~p(X) | q(X), inference(cnf_transformation, [status(thm)], [skol])).\n\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\n\
                     fof(goal, conjecture, q(a), file('problem.p', goal)).\n\
                     fof(neg, negated_conjecture, ~q(a), inference(negated_conjecture, [status(cth)], [goal])).\n\
                     cnf(derived, plain, q(a), inference(resolution, [status(thm)], [clause, fact])).\n\
                     cnf(bot, plain, $false, inference(subsumption_resolution, [status(thm)], [derived, neg])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_definition_renaming_before_nnf() {
        let problem = "fof(goal, conjecture, ~((p => q) <=> r)).\n\
                       fof(posr, axiom, r).\n\
                       fof(negr, axiom, ~r).";
        let proof = "fof(goal, conjecture, ~((p => q) <=> r), file('problem.p', goal)).\
                     fof(posr, axiom, r, file('problem.p', posr)).\
                     fof(negr, axiom, ~r, file('problem.p', negr)).\
                     fof(d, definition, (d0 <=> (p => q)), introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(r, plain, ~(d0 <=> r), inference(definition_renaming, [status(thm)], [goal,d])).\
                     fof(n, plain, ((d0 | r) & (~d0 | ~r)), inference(fof_nnf_transformation, [status(thm)], [r])).\
                     fof(sk, plain, ((d0 | r) & (~d0 | ~r)), inference(skolemisation, [status(esa)], [n])).\
                     cnf(p, plain, d0 | r, inference(cnf_transformation, [status(thm)], [sk])).\
                     cnf(np, plain, ~d0 | ~r, inference(cnf_transformation, [status(thm)], [sk])).\
                     cnf(d0, plain, d0, inference(resolution, [status(thm)], [p,negr])).\
                     cnf(nd0, plain, ~d0, inference(resolution, [status(thm)], [np,posr])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [d0,nd0])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_krs187_definition_renaming() {
        let problem = "fof(goal, conjecture, ~(((?[X2]: (model(X2, a)) <=> ?[X3]: (model(X3, b))) <=> r))).\n\
                       fof(posr, axiom, r).\n\
                       fof(negr, axiom, ~r).";
        let proof = "fof(goal, conjecture, ~(((?[X2]: (model(X2, a)) <=> ?[X3]: (model(X3, b))) <=> r)), file('problem.p', goal)).\n\
                     fof(posr, axiom, r, file('problem.p', posr)).\n\
                     fof(negr, axiom, ~r, file('problem.p', negr)).\n\
                     fof(c95, definition, ![X0]: ((def_esa_iff_0(X0) <=> ?[X2]: (model(X2, X0)))), introduced(definition, [new_symbols(definition, [def_esa_iff_0])])).\n\
                     fof(c96, definition, ![X1]: ((def_esa_iff_1(X1) <=> ?[X3]: (model(X3, X1)))), introduced(definition, [new_symbols(definition, [def_esa_iff_1])])).\n\
                     fof(c97, definition, ![X0]: (![X1]: ((def_esa_iff_2(X0, X1) <=> (def_esa_iff_0(X0) <=> def_esa_iff_1(X1))))), introduced(definition, [new_symbols(definition, [def_esa_iff_2])])).\n\
                     fof(c98, plain, ~(def_esa_iff_2(a, b) <=> r), inference(definition_renaming, [status(thm)], [goal, c95, c96, c97])).\n\
                     fof(n, plain, ((def_esa_iff_2(a, b) | r) & (~def_esa_iff_2(a, b) | ~r)), inference(fof_nnf_transformation, [status(thm)], [c98])).\n\
                     fof(sk, plain, ((def_esa_iff_2(a, b) | r) & (~def_esa_iff_2(a, b) | ~r)), inference(skolemisation, [status(esa)], [n])).\n\
                     cnf(p, plain, def_esa_iff_2(a, b) | r, inference(cnf_transformation, [status(thm)], [sk])).\n\
                     cnf(np, plain, ~def_esa_iff_2(a, b) | ~r, inference(cnf_transformation, [status(thm)], [sk])).\n\
                     cnf(d2, plain, def_esa_iff_2(a, b), inference(resolution, [status(thm)], [p, negr])).\n\
                     cnf(nd2, plain, ~def_esa_iff_2(a, b), inference(resolution, [status(thm)], [np, posr])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [d2, nd2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_definition_renaming_noop_conclusion() {
        let problem = "fof(goal, conjecture, p).";
        let proof = "fof(goal, conjecture, p, file('problem.p', goal)).\
                     fof(d, definition, (d0 <=> q), introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(noop, plain, p, inference(definition_renaming, [status(thm)], [goal,d])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [noop])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_definition_renaming_inconsistent_repeated_head_arguments() {
        let problem = "fof(goal, conjecture, p(a)).";
        let proof = "fof(goal, conjecture, p(a), file('problem.p', goal)).\
                     fof(d, definition, ![X] : (d0(X,X) <=> q(X)), introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(bad, plain, d0(a,b), inference(definition_renaming, [status(thm)], [goal,d])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [bad])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn definition_renaming_cycle_is_inconclusive() {
        let problem = "fof(goal, conjecture, p).";
        let proof = "fof(goal, conjecture, p, file('problem.p', goal)).\
                     fof(d0, definition, (d0 <=> d1), introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(d1, definition, (d1 <=> d0), introduced(definition, [new_symbols(definition, [d1])])).\
                     fof(result, plain, d0, inference(definition_renaming, [status(thm)], [goal,d0,d1])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [result])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn definition_renaming_respects_equivalence_matching_limit() {
        let problem =
            parse_tptp("fof(goal, conjecture, ~((p => q) <=> r)).").expect("problem parses");
        let proof = parse_tptp(
            "fof(goal, conjecture, ~((p => q) <=> r), file('problem.p', goal)).\
             fof(d, definition, (d0 <=> (p => q)), introduced(definition, [new_symbols(definition, [d0])])).\
             fof(r, plain, ~(d0 <=> r), inference(definition_renaming, [status(thm)], [goal,d])).\
             fof(n, plain, ((d0 | r) & (~d0 | ~r)), inference(fof_nnf_transformation, [status(thm)], [r])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [n])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_rewrite_steps: 0,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_fresh_definition_and_its_cnf_clause() {
        let problem = "fof(p, axiom, p(a)).\nfof(np, axiom, ~p(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\n\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\n\
                     fof(d, definition, ![X] : (d0(X) <=> p(X)), introduced(definition, [new_symbols(definition, [d0])])).\n\
                     cnf(d_clause, plain, ~d0(X) | p(X), inference(cnf_transformation, [status(thm)], [d])).\n\
                     cnf(d_other, plain, d0(X) | ~p(X), inference(cnf_transformation, [status(thm)], [d])).\n\
                     cnf(d_atom, plain, d0(a), inference(resolution, [status(thm)], [d_other,p])).\n\
                     cnf(p_again, plain, p(a), inference(resolution, [status(thm)], [d_clause,d_atom])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [p_again,np])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_nested_definitional_cnf_with_transitive_parents() {
        let problem = "fof(src, axiom, p(a) | ((~q(a) | (r(a) & s(a))) & (q(a) | t(a)))).\n\
                       fof(np, axiom, ~p(a)).\n\
                       fof(nq, axiom, ~q(a)).\
                       fof(nt, axiom, ~t(a)).";
        let proof = "fof(src, axiom, p(a) | ((~q(a) | (r(a) & s(a))) & (q(a) | t(a))), file('problem.p', src)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(nt, axiom, ~t(a), file('problem.p', nt)).\
                     fof(d0, definition, ![X] : (d0(X) <=> (r(X) & s(X))), introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(d1, definition, ![X] : (d1(X) <=> ((~q(X) | d0(X)) & (q(X) | t(X)))), introduced(definition, [new_symbols(definition, [d1])])).\
                     cnf(main, plain, p(a) | d1(a), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\
                     cnf(d1_body, plain, ~d1(X) | q(X) | t(X), inference(cnf_transformation, [status(thm)], [src,d1,d0])).\
                     cnf(d1_from_source, plain, d1(a), inference(resolution, [status(thm)], [main,np])).\
                     cnf(nd1_or_t, plain, ~d1(a) | t(a), inference(resolution, [status(thm)], [d1_body,nq])).\
                     cnf(nd1, plain, ~d1(a), inference(resolution, [status(thm)], [nd1_or_t,nt])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [d1_from_source,nd1])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_definitional_cnf_clause() {
        let problem = "fof(src, axiom, p(a) | (q(a) & r(a))).";
        let proof = "fof(src, axiom, p(a) | (q(a) & r(a)), file('problem.p', src)).\
                     fof(d, definition, ![X] : (d0(X) <=> (q(X) & r(X))), introduced(definition, [new_symbols(definition, [d0])])).\
                     cnf(forged, plain, p(a) | d0(b), inference(cnf_transformation, [status(thm)], [src,d])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [forged])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_definitional_cnf_without_transitive_definition_parent() {
        let problem = "fof(src, axiom, p(a) | ((~q(a) | (r(a) & s(a))) & (q(a) | t(a)))).";
        let proof = "fof(src, axiom, p(a) | ((~q(a) | (r(a) & s(a))) & (q(a) | t(a))), file('problem.p', src)).\
                     fof(d0, definition, ![X] : (d0(X) <=> (r(X) & s(X))), introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(d1, definition, ![X] : (d1(X) <=> ((~q(X) | d0(X)) & (q(X) | t(X)))), introduced(definition, [new_symbols(definition, [d1])])).\
                     cnf(main, plain, p(a) | d1(a), inference(cnf_transformation, [status(thm)], [src,d1])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [main])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_) | KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn certifies_flat_clause_definition_dependencies() {
        assert_eq!(
            check(flat_definition_problem(), flat_definition_proof()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_cnf_clause_with_condensed_duplicates() {
        // GEO452+1 c6456 shape: CNF expansion of `(~d | p | q | p)`
        // yields a duplicated literal the prover condenses away.
        let problem = "fof(src, axiom, ((~d | (p | q) | p) & (d | (~p & ~q) & ~p))).\n\
                       fof(nd, axiom, d).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let proof = "fof(src, axiom, ((~d | (p | q) | p) & (d | (~p & ~q) & ~p)), file('problem.p', src)).\n\
                     fof(nd, axiom, d, file('problem.p', nd)).\n\
                     fof(np, axiom, ~p, file('problem.p', np)).\n\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\n\
                     cnf(c, plain, ~d | p | q, inference(cnf_transformation, [status(thm)], [src])).\n\
                     cnf(m1, plain, p | q, inference(resolution, [status(thm)], [c,nd])).\n\
                     cnf(m2, plain, q, inference(resolution, [status(thm)], [m1,np])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m2,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_flat_definition_cnf_with_missing_dependency() {
        let proof = flat_definition_proof().replacen(",d1,d0]", ",d1]", 1);
        assert!(matches!(
            check(flat_definition_problem(), &proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_flat_definition_cnf_with_wrong_instantiation() {
        let proof =
            flat_definition_proof().replace("cnf(main, plain, d1(a)", "cnf(main, plain, d1(b)");
        assert!(matches!(
            check(flat_definition_problem(), &proof),
            KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn rejects_cyclic_flat_definition_dependencies() {
        let problem = "fof(src, axiom, q(a) | r(a)).";
        let proof = "fof(src, axiom, q(a) | r(a), file('problem.p', src)).\
                     fof(d0, definition, ![X] : (d0(X) <=> (d1(X) | q(X))),\
                         introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(d1, definition, ![X] : (d1(X) <=> (d0(X) | r(X))),\
                         introduced(definition, [new_symbols(definition, [d1])])).\
                     cnf(main, plain, d0(a),\
                         inference(cnf_transformation, [status(thm)], [src,d0,d1])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [main])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn flat_definition_cnf_literal_limit_is_inconclusive() {
        let problem = parse_tptp(flat_definition_problem()).expect("problem parses");
        let proof = parse_tptp(flat_definition_proof()).expect("proof parses");
        let limits = VerificationLimits {
            max_clause_literals: 2,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_e_style_skolemize_metadata() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : r(X, Y)).\n\
                       fof(n, axiom, ![X,Y] : ~r(X, Y)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : r(X, Y), file('problem.p', a)).\
                     fof(sk, plain, ![X] : r(X, sK0(X)),\
                         inference(skolemize,\
                                   [status(esa), new_symbols(skolem, [sK0]),\
                                    skolemize(Y, sK0(X))], [a])).\
                     fof(n, axiom, ![X,Y] : ~r(X, Y), file('problem.p', n)).\
                     fof(bot, plain, $false,\
                         inference(resolution, [status(thm)], [sk, n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_e_style_negated_skolemize_metadata() {
        let problem = "fof(a, axiom, ~![X] : p(X)).\n\
                       fof(n, axiom, ![X] : p(X)).";
        let proof = "fof(a, axiom, ~![X] : p(X), file('problem.p', a)).\
                     fof(sk, plain, ~p(sK0),\
                         inference(skolemize,\
                                   [status(esa), new_symbols(skolem, [sK0]),\
                                    skolemize(X, sK0)], [a])).\
                     fof(n, axiom, ![X] : p(X), file('problem.p', n)).\
                     fof(bot, plain, $false,\
                         inference(resolution, [status(thm)], [sk, n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_e_style_skolemize_metadata_mismatch() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : r(X, Y)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : r(X, Y), file('problem.p', a)).\
                     fof(sk, plain, ![X] : r(X, sK0(X)),\
                         inference(skolemize,\
                                   [status(esa), new_symbols(skolem, [sK0]),\
                                    skolemize(Y, sK0)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [sk])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn e_style_skolemize_without_metadata_is_inconclusive() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : r(X, Y)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : r(X, Y), file('problem.p', a)).\
                     fof(sk, plain, ![X] : r(X, sK0(X)),\
                         inference(skolemize, [status(esa)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [sk])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_ground_instantiate() {
        let problem = "fof(a, axiom, ![X] : p(X)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, ![X] : p(X), file('problem.p', a)).\
                     fof(i, plain, p(a), inference(instantiate, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [i,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_nested_instantiate() {
        let problem = "fof(a, axiom, ![X] : ![Y] : p(X, Y)).\n\
                       fof(n, axiom, ~p(a, b)).";
        let proof = "fof(a, axiom, ![X] : ![Y] : p(X, Y), file('problem.p', a)).\
                     fof(i, plain, p(a, b), inference(instantiate, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a, b), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [i,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_permuted_clausal_instantiate() {
        let problem = "cnf(a, axiom, ~p(X) | q(X)).\ncnf(n1, axiom, p(c)).\ncnf(n2, axiom, ~q(c)).";
        let proof = "cnf(a, axiom, ~p(X) | q(X), file('problem.p', a)).\
                     cnf(i, plain, q(c) | ~p(c), inference(instantiate, [status(thm)], [a])).\
                     cnf(n1, axiom, p(c), file('problem.p', n1)).\
                     cnf(n2, axiom, ~q(c), file('problem.p', n2)).\
                     cnf(r1, plain, ~p(c), inference(resolution, [status(thm)], [i, n2])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [r1, n1])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_resolution_with_duplicate_condensation() {
        let problem = "cnf(c1, axiom, ~r | q).\ncnf(c2, axiom, q | p | r).\ncnf(c3, axiom, ~q).\ncnf(c4, axiom, ~p).";
        let proof = "cnf(c1, axiom, ~r | q, file('problem.p', c1)).\
                     cnf(c2, axiom, q | p | r, file('problem.p', c2)).\
                     cnf(c3, axiom, ~q, file('problem.p', c3)).\
                     cnf(c4, axiom, ~p, file('problem.p', c4)).\
                     cnf(c5, plain, q | p, inference(resolution, [status(thm)], [c1, c2])).\
                     cnf(c6, plain, p, inference(resolution, [status(thm)], [c5, c3])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [c6, c4])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    fn lower_test_formula_pair(parent_text: &str, conclusion_text: &str) -> (Formula, Formula) {
        let parent_problem = parse_tptp(parent_text).expect("parent parses");
        let conclusion_problem = parse_tptp(conclusion_text).expect("conclusion parses");
        let mut symbols = SymbolTable::new();
        let parent = lower_annotated(
            &mut symbols,
            &parent_problem.formulas[0],
            VerificationLimits::default(),
        )
        .expect("parent lowers");
        let conclusion = lower_annotated(
            &mut symbols,
            &conclusion_problem.formulas[0],
            VerificationLimits::default(),
        )
        .expect("conclusion lowers");
        (parent, conclusion)
    }

    fn lower_test_formulas(inputs: &[&str]) -> Vec<Formula> {
        let mut symbols = SymbolTable::new();
        inputs
            .iter()
            .map(|input| {
                let problem = parse_tptp(input).expect("formula parses");
                lower_annotated(
                    &mut symbols,
                    &problem.formulas[0],
                    VerificationLimits::default(),
                )
                .expect("formula lowers")
            })
            .collect()
    }

    #[test]
    fn certifies_existential_generation_from_ground_witness() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a)).", "fof(a, plain, ?[X] : p(X)).");
        assert_eq!(
            verify_existential_generation(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_existential_generation_under_universal_scope() {
        let (parent, conclusion) = lower_test_formula_pair(
            "fof(a, axiom, ![Y] : p(Y, a)).",
            "fof(a, plain, ![Y] : ?[X] : p(Y, X)).",
        );
        assert_eq!(
            verify_existential_generation(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn rejects_existential_generation_with_changed_matrix() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a)).", "fof(a, plain, ?[X] : q(X)).");
        assert!(matches!(
            verify_existential_generation(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn rejects_existential_generation_without_existential() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a)).", "fof(a, plain, p(a)).");
        assert!(matches!(
            verify_existential_generation(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn existential_generation_matching_limit_is_inconclusive() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a)).", "fof(a, plain, ?[X] : p(X)).");
        let limits = VerificationLimits {
            max_formula_nodes: 2,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_existential_generation(&[parent], &conclusion, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_conjunction_of_parents() {
        let formulas = lower_test_formulas(&[
            "fof(a, axiom, p(a)).",
            "fof(a, axiom, q(a)).",
            "fof(a, plain, p(a) & q(a)).",
        ]);
        assert_eq!(
            verify_conjunction(&formulas[..2], &formulas[2], VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_reordered_conjunction_of_parents() {
        let formulas = lower_test_formulas(&[
            "fof(a, axiom, p(a)).",
            "fof(a, axiom, q(a)).",
            "fof(a, plain, q(a) & p(a)).",
        ]);
        assert_eq!(
            verify_conjunction(&formulas[..2], &formulas[2], VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn rejects_conjunction_with_missing_parent_part() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, q(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, q(a), file('problem.p', b)).\
                     fof(c, plain, p(a), inference(conjunction, [status(thm)], [a,b])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [c])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn conjunction_matching_limit_is_inconclusive() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, q(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, q(a), file('problem.p', b)).\
                     fof(c, plain, (p(a) & q(a)),\
                         inference(conjunction, [status(thm)], [a,b])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [c])).";
        let problem = parse_tptp(problem).expect("problem parses");
        let proof = parse_tptp(proof).expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_split_conjunct() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a) & q(a)).", "fof(a, plain, p(a)).");
        assert_eq!(
            verify_split_conjunct(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn certifies_split_conjunct_under_universal_prefix() {
        let (parent, conclusion) = lower_test_formula_pair(
            "fof(a, axiom, ![X] : (p(X) & q(X))).",
            "fof(a, plain, ![X] : q(X)).",
        );
        assert_eq!(
            verify_split_conjunct(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn rejects_split_conjunct_with_non_conjunct() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a) & q(a)).", "fof(a, plain, r(a)).");
        assert!(matches!(
            verify_split_conjunct(&[parent], &conclusion, VerificationLimits::default()),
            KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn split_conjunct_matching_limit_is_inconclusive() {
        let (parent, conclusion) =
            lower_test_formula_pair("fof(a, axiom, p(a) & q(a)).", "fof(a, plain, q(a)).");
        let limits = VerificationLimits {
            max_equivalence_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_split_conjunct(&[parent], &conclusion, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_copy_alias() {
        let problem = "fof(a, axiom, p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(c, plain, p(a), inference(copy, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [c,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_double_negation_alias() {
        let problem = "fof(a, axiom, ~~p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, ~~p(a), file('problem.p', a)).\
                     fof(d, plain, p(a),\
                         inference(double_negation, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [d,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_copy_with_changed_formula() {
        let problem = "fof(a, axiom, p(a)).\nfof(f, axiom, $false).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(c, plain, q(a), inference(copy, [status(thm)], [a])).\
                     fof(f, axiom, $false, file('problem.p', f)).\
                     fof(bot, plain, $false, inference(conjunction, [status(thm)], [c,f])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_double_negation_with_non_equivalent_formula() {
        let problem = "fof(a, axiom, p(a)).\nfof(f, axiom, $false).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(d, plain, ~~q(a), inference(double_negation, [status(thm)], [a])).\
                     fof(f, axiom, $false, file('problem.p', f)).\
                     fof(bot, plain, $false, inference(conjunction, [status(thm)], [d,f])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_excluded_middle() {
        let problem = "fof(a, axiom, p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(e, plain, (p(a) | ~p(a)),\
                         inference(excluded_middle, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(mid, plain, ~p(a), inference(resolution, [status(thm)], [e,n])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,a])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_modus_ponens_with_universal_instantiation() {
        let problem = "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(s, plain, q(a), inference(modus_ponens, [status(thm)], [rule,fact])).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_modus_ponens_with_reversed_parents() {
        let problem = "fof(rule, axiom, (p(a) => q(a))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(rule, axiom, (p(a) => q(a)), file('problem.p', rule)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(s, plain, q(a), inference(modus_ponens, [status(thm)], [fact,rule])).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_modus_ponens_with_forged_conclusion() {
        let problem = "fof(rule, axiom, (p(a) => q(a))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(rule, axiom, (p(a) => q(a)), file('problem.p', rule)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(s, plain, r(a), inference(modus_ponens, [status(thm)], [rule,fact])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nr])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_modus_ponens_without_an_antecedent_parent() {
        let problem = "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
                     fof(s, plain, q(a), inference(modus_ponens, [status(thm)], [rule])).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nq])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_bounded_horn_forward_chain() {
        let problem = "fof(r1, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(r2, axiom, ![X] : (q(X) => r(X))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(r1, axiom, ![X] : (p(X) => q(X)), file('problem.p', r1)).\
                     fof(r2, axiom, ![X] : (q(X) => r(X)), file('problem.p', r2)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(h, plain, r(a), inference(horn, [status(thm)], [r2,fact,r1])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [h,nr])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_horn_with_forged_conclusion() {
        let problem = "fof(r1, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(ns, axiom, ~s(a)).";
        let proof = "fof(r1, axiom, ![X] : (p(X) => q(X)), file('problem.p', r1)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(h, plain, s(a), inference(horn, [status(thm)], [fact,r1])).\
                     fof(ns, axiom, ~s(a), file('problem.p', ns)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [h,ns])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn horn_does_not_split_conjunction_antecedents() {
        let problem = "fof(rule, axiom, ((p(a) & q(a)) => r(a))).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(q, axiom, q(a)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(rule, axiom, ((p(a) & q(a)) => r(a)), file('problem.p', rule)).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(q, axiom, q(a), file('problem.p', q)).\
                     fof(h, plain, r(a), inference(horn, [status(thm)], [rule,p,q])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [h,nr])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn horn_matching_limit_is_inconclusive() {
        let problem = parse_tptp(
            "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
             fof(fact, axiom, p(a)).\n\
             fof(nq, axiom, ~q(a)).",
        )
        .expect("problem parses");
        let proof = parse_tptp(
            "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
             fof(fact, axiom, p(a), file('problem.p', fact)).\
             fof(h, plain, q(a), inference(horn, [status(thm)], [fact,rule])).\
             fof(nq, axiom, ~q(a), file('problem.p', nq)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [h,nq])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 0,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_consequence_as_resolution_alias() {
        let problem = "fof(p, axiom, p(a)).\nfof(np, axiom, ~p(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [p,np])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_consequence_with_forged_non_resolution_step() {
        let problem = "fof(p, axiom, p(a)).\nfof(q, axiom, q(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(q, axiom, q(a), file('problem.p', q)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [p,q])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn consequence_requires_two_parents() {
        let problem = "fof(p, axiom, p(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [p])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_consequence_from_resolving_pair() {
        // Three parents where one pair resolves: extra cited parents are
        // harmless by monotonicity.
        let problem = "fof(p, axiom, p(a)).\nfof(np, axiom, ~p(a)).\nfof(q, axiom, q(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(q, axiom, q(a), file('problem.p', q)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [p,np,q])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_consequence_ex_falso_single_parent() {
        let problem = "fof(p, axiom, p(a)).\nfof(np, axiom, ~p(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(f, plain, $false, inference(resolution, [status(thm)], [p,np])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [f])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_consequence_when_no_pair_resolves() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, p(b)).\nfof(c, axiom, q(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, p(b), file('problem.p', b)).\
                     fof(c, axiom, q(a), file('problem.p', c)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [a,b,c])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn consequence_pair_search_is_step_bounded() {
        // Six pairs but a budget of two: exhaustion must fail open
        // (Inconclusive) instead of running the full quadratic scan.
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, p(b)).\nfof(c, axiom, q(a)).\nfof(d, axiom, q(b)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, p(b), file('problem.p', b)).\
                     fof(c, axiom, q(a), file('problem.p', c)).\
                     fof(d, axiom, q(b), file('problem.p', d)).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [a,b,c,d])).";
        let problem = parse_tptp(problem).expect("problem parses");
        let proof = parse_tptp(proof).expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 2,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_assume_and_rewrite_identity_aliases() {
        let problem = "fof(p, axiom, p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(a, plain, p(a), inference(assume, [status(thm)], [p])).\
                     fof(r, plain, p(a), inference(rewrite, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [r,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_assume_with_changed_formula() {
        let problem = "fof(p, axiom, p(a)).\nfof(n, axiom, ~q(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(a, plain, q(a), inference(assume, [status(thm)], [p])).\
                     fof(n, axiom, ~q(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [a,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_ex_falso_from_two_contradiction_parents() {
        let problem = "fof(p, axiom, p(a)).\n\
                       fof(np, axiom, ~p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(ex, plain, q(a), inference(ex_falso, [status(thm)], [p,np])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [ex,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_ex_falso_from_a_false_parent() {
        let problem = "fof(p, axiom, p(a)).\n\
                       fof(np, axiom, ~p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(false_parent, plain, $false, inference(resolution, [status(thm)], [p,np])).\
                     fof(ex, plain, q(a), inference(ex_falso, [status(thm)], [false_parent])).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [ex,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_ex_falso_without_a_contradiction() {
        let problem = "fof(p, axiom, p(a)).\n\
                       fof(q, axiom, q(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(q, axiom, q(a), file('problem.p', q)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(ex, plain, q(a), inference(ex_falso, [status(thm)], [p,q])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [ex,nq])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_ex_falso_with_wrong_parent_count() {
        let problem = "fof(p, axiom, p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(ex, plain, q(a), inference(ex_falso, [status(thm)], [p,n,p])).\
                     fof(nq, axiom, ~q(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [ex,nq])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_bounded_disjunctive_weakening() {
        let problem = "fof(src, axiom, p(a)).\n\
                       fof(nq, axiom, ~q(a)).\n\
                       fof(nr, axiom, ~r(a)).\n\
                       fof(np, axiom, ~p(a)).";
        let proof = "fof(src, axiom, p(a), file('problem.p', src)).\
                     fof(w, plain, (r(a) | p(a) | q(a)), inference(weaken, [status(thm)], [src])).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(mid, plain, (r(a) | p(a)), inference(resolution, [status(thm)], [w,nq])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(last, plain, p(a), inference(resolution, [status(thm)], [mid,nr])).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [last,np])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_weakening_that_changes_a_parent_disjunct() {
        let problem = "fof(src, axiom, p(a) | q(a)).\nfof(nr, axiom, ~r(a)).";
        let proof = "fof(src, axiom, p(a) | q(a), file('problem.p', src)).\
                     fof(w, plain, p(a) | r(a), inference(weaken, [status(thm)], [src])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [w,nr])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_weakening_with_non_disjunctive_conclusion() {
        let problem = "fof(src, axiom, p(a)).\nfof(np, axiom, ~p(a)).";
        let proof = "fof(src, axiom, p(a), file('problem.p', src)).\
                     fof(w, plain, (p(a) & q(a)), inference(weaken, [status(thm)], [src])).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [w,np])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn weaken_matching_limit_is_inconclusive() {
        let problem = parse_tptp("fof(src, axiom, p(a) | q(a)).\nfof(nr, axiom, ~r(a)).")
            .expect("problem parses");
        let proof = parse_tptp(
            "fof(src, axiom, p(a) | q(a), file('problem.p', src)).\
             fof(w, plain, (p(a) | q(a) | r(a)), inference(weaken, [status(thm)], [src])).\
             fof(nr, axiom, ~r(a), file('problem.p', nr)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [w,nr])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_reflexivity_from_a_dependency_parent() {
        let problem = "fof(src, axiom, p(a)).\nfof(neg, axiom, ~p(a)).";
        let proof = "fof(src, axiom, p(a), file('problem.p', src)).\
                     fof(eq, plain, f(a) = f(a), inference(reflexivity, [status(thm)], [src])).\
                     fof(pair, plain, (f(a) = f(a) & p(a)), inference(conjunction, [status(thm)], [eq,src])).\
                     fof(selected, plain, p(a), inference(split_conjunct, [status(thm)], [pair])).\
                     fof(neg, axiom, ~p(a), file('problem.p', neg)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [selected,neg])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_reflexivity_with_non_reflexive_equality() {
        let problem = "fof(src, axiom, p(a)).";
        let proof = "fof(src, axiom, p(a), file('problem.p', src)).\
                     fof(eq, plain, a = b, inference(reflexivity, [status(thm)], [src])).\
                     fof(pair, plain, (a = b & p(a)), inference(conjunction, [status(thm)], [eq,src])).\
                     fof(selected, plain, p(a), inference(split_conjunct, [status(thm)], [pair])).\
                     fof(neg, axiom, ~p(a), file('problem.p', src)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [selected,neg])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_reflexivity_without_a_parent() {
        let problem = "fof(src, axiom, p(a)).";
        let proof = "fof(src, axiom, p(a), file('problem.p', src)).\
                     fof(eq, plain, a = a, inference(reflexivity, [status(thm)], [src,src])).\
                     fof(neg, axiom, ~p(a), file('problem.p', src)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [src,neg])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_ground_equality_transitivity() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(bc, axiom, b = c).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(a)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(bc, axiom, b = c, file('problem.p', bc)).\
                     fof(ac, plain, a = c, inference(transitivity, [status(thm)], [ab,bc])).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(pair, plain, (a = c & p(a)), inference(conjunction, [status(thm)], [ac,p])).\
                     fof(selected, plain, p(a), inference(split_conjunct, [status(thm)], [pair])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [selected,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_transitivity_with_reversed_orientation() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(cb, axiom, c = b).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(a)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(cb, axiom, c = b, file('problem.p', cb)).\
                     fof(ac, plain, a = c, inference(transitivity, [status(thm)], [ab,cb])).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(pair, plain, (a = c & p(a)), inference(conjunction, [status(thm)], [ac,p])).\
                     fof(selected, plain, p(a), inference(split_conjunct, [status(thm)], [pair])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [selected,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_transitivity_with_wrong_middle_term() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(cd, axiom, c = d).\n\
                       fof(n, axiom, ~p(a)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(cd, axiom, c = d, file('problem.p', cd)).\
                     fof(ad, plain, a = d, inference(transitivity, [status(thm)], [ab,cd])).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [p,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn transitivity_with_variables_is_inconclusive() {
        let problem =
            parse_tptp("fof(ab, axiom, a = b).\nfof(bc, axiom, b = c).").expect("problem parses");
        let proof = parse_tptp(
            "fof(ab, axiom, X = b, file('problem.p', ab)).\
             fof(bc, axiom, b = c, file('problem.p', bc)).\
             fof(ac, plain, X = c, inference(transitivity, [status(thm)], [ab,bc])).\
             fof(n, axiom, ~p(a), file('problem.p', n)).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [n,n])).",
        )
        .expect("proof parses");
        assert!(matches!(
            verify_strict(&problem, &proof, VerificationLimits::default()),
            KernelVerdict::Inconclusive(_) | KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn certifies_equality_normalization_predicate_rewrite() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(b)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(b), file('problem.p', n)).\
                     fof(na, plain, ~p(a), inference(equality_normalization, [status(thm)], [n,ab])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [na,p])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_equality_normalization_multi_parent_chain() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(bc, axiom, b = c).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(c)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(bc, axiom, b = c, file('problem.p', bc)).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(c), file('problem.p', n)).\
                     fof(na, plain, ~p(a), inference(equality_normalization, [status(thm)], [n,ab,bc])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [na,p])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_equality_normalization_any_representative() {
        // The search-side union-find is rank-based while the kernel is
        // left-biased: both agree on connectivity but may elect different
        // class representatives. A conclusion using the `b` representative
        // must certify just like the `a` form above.
        let problem = "fof(bc, axiom, b = c).\n\
                       fof(ab, axiom, a = b).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(c)).";
        let proof = "fof(bc, axiom, b = c, file('problem.p', bc)).\
                     fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(c), file('problem.p', n)).\
                     fof(nb, plain, ~p(b), inference(equality_normalization, [status(thm)], [n,bc])).\
                     fof(pb, plain, p(b), inference(equality_normalization, [status(thm)], [p,ab])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [nb,pb])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_equality_normalization_target_last() {
        // Parent order must not matter: the target is the parent that is
        // not a positive ground unit equality.
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(b)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(b), file('problem.p', n)).\
                     fof(na, plain, ~p(a), inference(equality_normalization, [status(thm)], [ab,n])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [na,p])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_equality_normalization_to_empty_clause() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(neq, axiom, ~(a = b)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(neq, axiom, ~(a = b), file('problem.p', neq)).\
                     fof(bot, plain, $false, inference(equality_normalization, [status(thm)], [neq,ab])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_equality_normalization_forged_conclusion() {
        let problem = "fof(ab, axiom, a = b).\n\
                       fof(p, axiom, p(a)).\n\
                       fof(n, axiom, ~p(b)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(n, axiom, ~p(b), file('problem.p', n)).\
                     fof(bad, plain, ~r(a), inference(equality_normalization, [status(thm)], [n,ab])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [p,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_equality_normalization_non_unit_equality_parent() {
        let problem = "fof(dis, axiom, (a = b | c = d)).\n\
                       fof(n, axiom, ~p(a)).";
        let proof = "fof(dis, axiom, (a = b | c = d), file('problem.p', dis)).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bad, plain, ~p(b), inference(equality_normalization, [status(thm)], [n,dis])).\
                     fof(p, axiom, p(a), file('problem.p', p)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [p,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_equality_normalization_non_equality_parent() {
        let problem = "fof(q, axiom, q(a)).\n\
                       fof(n, axiom, ~p(b)).";
        let proof = "fof(q, axiom, q(a), file('problem.p', q)).\
                     fof(n, axiom, ~p(b), file('problem.p', n)).\
                     fof(bad, plain, ~p(b), inference(equality_normalization, [status(thm)], [n,q])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [n,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_equality_normalization_positive_reflexive_target() {
        let problem = "fof(ab, axiom, a = b).";
        let proof = "fof(ab, axiom, a = b, file('problem.p', ab)).\
                     fof(bad, plain, a = b, inference(equality_normalization, [status(thm)], [ab,ab])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [bad,bad])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_equality_normalization_missing_equality_parents() {
        let problem = "fof(n, axiom, ~p(b)).";
        let proof = "fof(n, axiom, ~p(b), file('problem.p', n)).\
                     fof(bad, plain, ~p(b), inference(equality_normalization, [status(thm)], [n])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [n,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn equality_normalization_non_ground_parent_is_inconclusive() {
        let problem =
            parse_tptp("fof(ab, axiom, X = b).\nfof(n, axiom, ~p(X)).").expect("problem parses");
        let proof = parse_tptp(
            "fof(ab, axiom, X = b, file('problem.p', ab)).\
             fof(n, axiom, ~p(b), file('problem.p', n)).\
             fof(bad, plain, ~p(b), inference(equality_normalization, [status(thm)], [n,ab])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [n,n])).",
        )
        .expect("proof parses");
        assert!(matches!(
            verify_strict(&problem, &proof, VerificationLimits::default()),
            KernelVerdict::Inconclusive(_) | KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn certifies_commute_and_instantiate_mp_aliases() {
        let problem = "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(s, plain, q(a), inference(instantiate_mp, [status(thm)], [rule,fact])).\
                     fof(em, plain, (q(a) | ~q(a)), inference(excluded_middle, [status(thm)], [s])).\
                     fof(commuted, plain, (~q(a) | q(a)), inference(commute, [status(thm)], [em])).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(mid, plain, ~q(a), inference(resolution, [status(thm)], [commuted,nq])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,s])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_instantiate_mp_with_forged_conclusion() {
        let problem = "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(fact, axiom, p(a)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
                     fof(fact, axiom, p(a), file('problem.p', fact)).\
                     fof(s, plain, r(a), inference(instantiate_mp, [status(thm)], [rule,fact])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nr])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_contrapositive_and_disjunctive_syllogism() {
        let problem = "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(nq, axiom, ~q(a)).\n\
                       fof(disj, axiom, (r(a) | q(a))).\n\
                       fof(nq2, axiom, ~q(a)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(np, plain, ~p(a), inference(contrapositive, [status(thm)], [rule,nq])).\
                     fof(disj, axiom, (r(a) | q(a)), file('problem.p', disj)).\
                     fof(nq2, axiom, ~q(a), file('problem.p', nq)).\
                     fof(r, plain, r(a), inference(disjunctive_syllogism, [status(thm)], [disj,nq2])).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(mid, plain, $false, inference(resolution, [status(thm)], [r,nr])).\
                     fof(ex, plain, p(a), inference(ex_falso, [status(thm)], [mid])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [ex,np])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_contrapositive_with_forged_conclusion() {
        let problem = "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
                       fof(nq, axiom, ~q(a)).\n\
                       fof(nr, axiom, ~r(a)).";
        let proof = "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(nr, axiom, ~r(a), file('problem.p', nr)).\
                     fof(np, plain, ~p(a), inference(contrapositive, [status(thm)], [rule,nr])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [np,nq])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_disjunctive_syllogism_with_wrong_remaining_disjunct() {
        let problem = "fof(disj, axiom, (p(a) | q(a))).\n\
                       fof(nq, axiom, ~q(a)).\n\
                       fof(ns, axiom, ~s(a)).";
        let proof = "fof(disj, axiom, (p(a) | q(a)), file('problem.p', disj)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     fof(s, plain, s(a), inference(disjunctive_syllogism, [status(thm)], [disj,nq])).\
                     fof(ns, axiom, ~s(a), file('problem.p', ns)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,ns])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_paramodulation_in_both_parent_orders() {
        let problem = "fof(eq, axiom, f(a) = b).\n\
                       fof(target, axiom, p(f(a))).\n\
                       fof(neg, axiom, ~p(b)).";
        let proof = "fof(eq, axiom, f(a) = b, file('problem.p', eq)).\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\
                     fof(first, plain, p(b), inference(paramodulation, [status(thm)], [eq,target])).\
                     fof(second, plain, p(b), inference(paramodulation, [status(thm)], [target,eq])).\
                     fof(neg, axiom, ~p(b), file('problem.p', neg)).\
                     fof(bot1, plain, $false, inference(resolution, [status(thm)], [first,neg])).\
                     fof(pair, plain, (p(b) & $false), inference(conjunction, [status(thm)], [second,bot1])).\
                     fof(bot, plain, $false, inference(split_conjunct, [status(thm)], [pair])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_paramodulation_conclusion() {
        let problem = "fof(eq, axiom, f(a) = b).\nfof(target, axiom, p(f(a))).";
        let proof = "fof(eq, axiom, f(a) = b, file('problem.p', eq)).\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\
                     fof(s, plain, q(b), inference(paramodulation, [status(thm)], [eq,target])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s,s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn modus_ponens_matching_limit_is_inconclusive() {
        let problem = parse_tptp(
            "fof(rule, axiom, ![X] : (p(X) => q(X))).\n\
             fof(fact, axiom, p(a)).\n\
             fof(nq, axiom, ~q(a)).",
        )
        .expect("problem parses");
        let proof = parse_tptp(
            "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
             fof(fact, axiom, p(a), file('problem.p', fact)).\
             fof(s, plain, q(a), inference(modus_ponens, [status(thm)], [rule,fact])).\
             fof(nq, axiom, ~q(a), file('problem.p', nq)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nq])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_excluded_middle_with_changed_formula() {
        let problem = "fof(a, axiom, p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(e, plain, (q(a) | ~q(a)),\
                         inference(excluded_middle, [status(thm)], [a])).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [a,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn matches_leaf_by_provenance_after_name_mismatch() {
        let problem =
            "fof(alias, axiom, p(a)).\nfof(source_name, axiom, q(a)).\nfof(n, axiom, ~q(a)).";
        let proof = "fof(alias, axiom, q(a), file('problem.p', source_name)).\
                     fof(n, axiom, ~q(a), file('problem.p', n)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [alias,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn excluded_middle_matching_limit_is_inconclusive() {
        let problem = parse_tptp("fof(a, axiom, (p(a) & q(a) & r(a))).").expect("problem parses");
        let proof = parse_tptp(
            "fof(a, axiom, (p(a) & q(a) & r(a)), file('problem.p', a)).\
             fof(e, plain, ((p(a) & q(a) & r(a)) | ~(p(a) & q(a) & r(a))),\
                 inference(excluded_middle, [status(thm)], [a])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [e])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_formula_nodes: 2,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_instantiate_with_inconsistent_substitution() {
        let problem = "fof(a, axiom, ![X] : p(X, X)).";
        let proof = "fof(a, axiom, ![X] : p(X, X), file('problem.p', a)).\
                     fof(i, plain, p(a, b), inference(instantiate, [status(thm)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [i])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_instantiate_with_changed_matrix() {
        let problem = "fof(a, axiom, ![X] : p(X)).";
        let proof = "fof(a, axiom, ![X] : p(X), file('problem.p', a)).\
                     fof(i, plain, q(a), inference(instantiate, [status(thm)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [i])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn instantiate_matching_limit_is_inconclusive() {
        let problem = parse_tptp("fof(a, axiom, ![X] : p(X)).").expect("problem parses");
        let proof = parse_tptp(
            "fof(a, axiom, ![X] : p(X), file('problem.p', a)).\
             fof(i, plain, p(a), inference(instantiate, [status(thm)], [a])).\
             fof(bot, plain, $false, inference(limit_wrapper, [status(thm)], [i])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_equivalence_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_non_fresh_definition_symbol() {
        let problem = "fof(d, axiom, p(a)).";
        let proof = "fof(d, definition, ![X] : (p(X) <=> q(X)), introduced(definition, [new_symbols(definition, [p])])).\n\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [d])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_recursive_definition() {
        let problem = "fof(a, axiom, p(a)).";
        let proof = "fof(d, definition, ![X] : (d0(X) <=> (p(X) | d0(X))), introduced(definition, [new_symbols(definition, [d0])])).\n\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [d])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_forged_cnf_clause() {
        let problem = "fof(a, axiom, p(a) | q(a)).";
        let proof = "fof(a, axiom, p(a) | q(a), file('problem.p', a)).\n\
                     cnf(s, plain, p(b), inference(cnf_transformation, [status(thm)], [a])).\n\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_constant_skolemization() {
        let problem = "fof(a, axiom, ?[X] : p(X)).\nfof(n, axiom, ![X] : ~p(X)).";
        let proof = "fof(a, axiom, ?[X] : p(X), file('problem.p', a)).\n\
                     fof(s, plain, p(sk0), inference(skolemisation, [status(esa)], [a])).\n\
                     fof(n, axiom, ![X] : ~p(X), file('problem.p', n)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_scoped_skolemization() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : p(X, Y)).\nfof(n, axiom, ![X] : ~p(a, X)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : p(X, Y), file('problem.p', a)).\n\
                     fof(s, plain, ![X] : p(X, sk0(X)), inference(skolemisation, [status(esa)], [a])).\n\
                     fof(n, axiom, ![X] : ~p(a, X), file('problem.p', n)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_skolemization_that_reorders_preserved_existential_scope() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : ?[Z] : p(X, Y, Z)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : ?[Z] : p(X, Y, Z), file('problem.p', a)).\
                     fof(s, plain, ?[Y] : ![X] : p(X, Y, sk0(X)), inference(skolemisation, [status(esa)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s,s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_nested_multi_existential_skolemization() {
        let problem = "fof(a, axiom, ?[X] : (![Y] : ?[Z] : (p(X, Z) | q(Y, Z)))).\n\
                       fof(np, axiom, ![X,Y] : ~p(X, Y)).\n\
                       fof(nq, axiom, ![X,Y] : ~q(X, Y)).";
        let proof = "fof(a, axiom, ?[X] : (![Y] : ?[Z] : (p(X, Z) | q(Y, Z))), file('problem.p', a)).\n\
                     fof(s, plain, ![Y] : (p(sk0, sk1(Y)) | q(Y, sk1(Y))), inference(skolemisation, [status(esa)], [a])).\n\
                     fof(np, axiom, ![X,Y] : ~p(X, Y), file('problem.p', np)).\n\
                     fof(mid, plain, q(Y, sk1(Y)), inference(resolution, [status(thm)], [s,np])).\n\
                     fof(nq, axiom, ![X,Y] : ~q(X, Y), file('problem.p', nq)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_regrouped_universal_skolemization() {
        let problem = "fof(a, axiom, ![X] : (? [Y] : (![Z] : (? [W] : (p(X, Y) | q(Z, W)))))).\n\
                       fof(np, axiom, ![X,Y] : ~p(X, Y)).\n\
                       fof(nq, axiom, ![X,Y] : ~q(X, Y)).";
        let proof = "fof(a, axiom, ![X] : (? [Y] : (![Z] : (? [W] : (p(X, Y) | q(Z, W))))), file('problem.p', a)).\
                     fof(s, plain, ![X,Z] : (p(X, sk0(X)) | q(Z, sk1(X,Z))), inference(skolemisation, [status(esa)], [a])).\
                     fof(np, axiom, ![X,Y] : ~p(X, Y), file('problem.p', np)).\
                     fof(mid, plain, q(Z, sk1(X,Z)), inference(resolution, [status(thm)], [s,np])).\
                     fof(nq, axiom, ![X,Y] : ~q(X, Y), file('problem.p', nq)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_associative_reordered_skolemization_matrix() {
        let problem = "fof(a, axiom, ?[X] : (p(X) | (q(X) | r(X)))).\n\
                       fof(np, axiom, ~p(X)).\n\
                       fof(nq, axiom, ~q(X)).\
                       fof(nr, axiom, ~r(X)).";
        let proof = "fof(a, axiom, ?[X] : (p(X) | (q(X) | r(X))), file('problem.p', a)).\
                     fof(s, plain, (q(sk0) | r(sk0)) | p(sk0), inference(skolemisation, [status(esa)], [a])).\
                     fof(np, axiom, ~p(X), file('problem.p', np)).\
                     fof(mid, plain, q(sk0) | r(sk0), inference(resolution, [status(thm)], [s,np])).\
                     fof(nq, axiom, ~q(X), file('problem.p', nq)).\
                     fof(last, plain, r(sk0), inference(resolution, [status(thm)], [mid,nq])).\
                     fof(nr, axiom, ~r(X), file('problem.p', nr)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [last,nr])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_skolemization_with_extra_witness_argument() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : p(X, Y)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : p(X, Y), file('problem.p', a)).\
                     fof(s, plain, ![X] : p(X, sk0(X, X)), inference(skolemisation, [status(esa)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_nested_skolemization_that_drops_matrix_content() {
        let problem = "fof(a, axiom, ?[X] : (![Y] : ?[Z] : (p(X, Z) | q(Y, Z)))).";
        let proof = "fof(a, axiom, ?[X] : (![Y] : ?[Z] : (p(X, Z) | q(Y, Z))), file('problem.p', a)).\
                     fof(s, plain, ![Y] : p(sk0, sk1(Y)), inference(skolemisation, [status(esa)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn skolemization_matching_limit_is_inconclusive() {
        let problem = parse_tptp("fof(a, axiom, ![X] : ?[Y] : p(X, Y)).").expect("problem parses");
        let proof = parse_tptp(
            "fof(a, axiom, ![X] : ?[Y] : p(X, Y), file('problem.p', a)).\
             fof(s, plain, ![X] : p(X, sk0(X)), inference(skolemisation, [status(esa)], [a])).\
             fof(bot, plain, $false, inference(limit_wrapper, [status(thm)], [s])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_skolem_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_skolem_that_drops_universal_scope() {
        let problem = "fof(a, axiom, ![X] : ?[Y] : p(X, Y)).\nfof(n, axiom, ![X] : ~p(a, X)).";
        let proof = "fof(a, axiom, ![X] : ?[Y] : p(X, Y), file('problem.p', a)).\n\
                     fof(s, plain, ![X] : p(X, sk0), inference(skolemisation, [status(esa)], [a])).\n\
                     fof(n, axiom, ![X] : ~p(a, X), file('problem.p', n)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_skolem_witness_reuse_across_existentials() {
        let problem = "fof(a, axiom, ?[X] : ?[Y] : p(X, Y)).\n\
                       fof(n, axiom, ![X] : ![Y] : ~(p(X, Y))).";
        let proof = "fof(a, axiom, ?[X] : ?[Y] : p(X, Y), file('problem.p', a)).\n\
                     fof(s, plain, p(sk0, sk0), inference(skolemisation, [status(esa)], [a])).\n\
                     fof(n, axiom, ![X] : ![Y] : ~(p(X, Y)), file('problem.p', n)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_negated_universal_skolemization() {
        let problem = "fof(a, axiom, ![X] : r(X)).\n\
                       fof(c, conjecture, ![X] : r(X)).";
        let proof = "fof(a, axiom, ![X] : r(X), file('problem.p', a)).\
                     fof(c, conjecture, ![X] : r(X), file('problem.p', c)).\
                     fof(neg, negated_conjecture, ~![X] : r(X),\
                         inference(negated_conjecture, [status(cth)], [c])).\
                     fof(sk, plain, ~r(sk0),\
                         inference(skolemisation, [status(esa)], [neg])).\
                     fof(bot, plain, $false,\
                         inference(resolution, [status(thm)], [a, sk])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_scoped_negated_universal_skolemization() {
        let problem = "fof(a, axiom, ![U] : ~(![X] : p(U, X))).\n\
                       fof(n, axiom, ![U,X] : p(U, X)).";
        let proof = "fof(a, axiom, ![U] : ~(![X] : p(U, X)), file('problem.p', a)).\
                     fof(n, axiom, ![U,X] : p(U, X), file('problem.p', n)).\
                     fof(sk, plain, ![U] : ~p(U, sk0(U)),\
                         inference(skolemisation, [status(esa)], [a])).\
                     fof(bot, plain, $false,\
                         inference(resolution, [status(thm)], [n, sk])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_negated_skolemization_with_dropped_scope() {
        let problem = "fof(a, axiom, ![U] : ~(![X] : p(U, X))).";
        let proof = "fof(a, axiom, ![U] : ~(![X] : p(U, X)), file('problem.p', a)).\
                     fof(sk, plain, ![U] : ~p(U, sk0),\
                         inference(skolemisation, [status(esa)], [a])).\
                     fof(bot, plain, $false,\
                         inference(consequence, [status(thm)], [sk])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_skolemization_of_universal_effective_quantifier() {
        let problem = "fof(a, axiom, ~?[X] : p(X)).";
        let proof = "fof(a, axiom, ~?[X] : p(X), file('problem.p', a)).\
                     fof(sk, plain, ~p(sk0),\
                         inference(skolemisation, [status(esa)], [a])).\
                     fof(bot, plain, $false,\
                         inference(consequence, [status(thm)], [sk])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn negated_skolemization_matching_limit_is_inconclusive() {
        let problem = parse_tptp("fof(a, axiom, ~![X] : p(X)).").expect("problem parses");
        let proof = parse_tptp(
            "fof(a, axiom, ~![X] : p(X), file('problem.p', a)).\
             fof(sk, plain, ~p(sk0), inference(skolemisation, [status(esa)], [a])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [sk])).",
        )
        .expect("proof parses");
        let limits = VerificationLimits {
            max_skolem_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_vampire_dependent_multi_parent_skolemization() {
        let problem = "fof(source, axiom, ?[X] : (p(X) | ?[Y] : r(X, Y))).\n\
                       fof(np, axiom, ![X] : ~p(X)).\n\
                       fof(nr, axiom, ![X,Y] : ~r(X, Y)).";
        let proof = "fof(source, axiom, ?[X] : (p(X) | ?[Y] : r(X, Y)), file('problem.p', source)).\
                     fof(ax_outer, plain,\
                         ((? [X] : (p(X) | ? [Y] : r(X, Y)))\
                          => (p(sk0) | ? [Y] : r(sk0, Y))),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(ax_inner, plain,\
                         ((? [Y] : r(sk0, Y)) => r(sk0, sk1)),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(step, plain, (p(sk0) | r(sk0, sk1)),\
                         inference(skolemisation,\
                                   [status(esa), new_symbols(skolem, [sk0, sk1])],\
                                   [source, ax_inner, ax_outer])).\
                     fof(np, axiom, ![X] : ~p(X), file('problem.p', np)).\
                     fof(nr, axiom, ![X,Y] : ~r(X, Y), file('problem.p', nr)).\
                      fof(mid, plain, r(sk0, sk1), inference(resolution, [status(thm)], [step, np])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid, nr])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_multi_existential_skolem_axiom() {
        let problem = "fof(source, axiom, ?[X] : ?[Y] : (p(X) | q(Y))).\n\
                       fof(np, axiom, ![X] : ~p(X)).\n\
                       fof(nq, axiom, ![X] : ~q(X)).";
        let proof = "fof(source, axiom, ?[X] : ?[Y] : (p(X) | q(Y)), file('problem.p', source)).\
                     fof(ax, plain,\
                         ((? [X] : ? [Y] : (p(X) | q(Y))) => (p(sk0) | q(sk1))),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(step, plain, (p(sk0) | q(sk1)),\
                         inference(skolemisation,\
                                   [status(esa), new_symbols(skolem, [sk0, sk1])],\
                                   [source, ax])).\
                     fof(np, axiom, ![X] : ~p(X), file('problem.p', np)).\
                     fof(nq, axiom, ![X] : ~q(X), file('problem.p', nq)).\
                     fof(mid, plain, q(sk1), inference(resolution, [status(thm)], [step, np])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid, nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_multi_parent_skolemization_conclusion() {
        let problem = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y))).";
        let proof = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y)), file('problem.p', source)).\
                     fof(ax_outer, plain,\
                         ((? [X] : (p(X) & ? [Y] : r(X, Y)))\
                          => (p(sk0) & ? [Y] : r(sk0, Y))),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(ax_inner, plain,\
                         ((? [Y] : r(sk0, Y)) => r(sk0, sk1)),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(step, plain, (p(sk0) & q(sk1)),\
                         inference(skolemisation,\
                                   [status(esa), new_symbols(skolem, [sk0, sk1])],\
                                   [source, ax_outer, ax_inner])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_multi_parent_skolemization_with_omitted_axiom() {
        let problem = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y))).";
        let proof = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y)), file('problem.p', source)).\
                     fof(ax_outer, plain,\
                         ((? [X] : (p(X) & ? [Y] : r(X, Y)))\
                          => (p(sk0) & ? [Y] : r(sk0, Y))),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(step, plain, (p(sk0) & r(sk0, sk1)),\
                         inference(skolemisation,\
                                   [status(esa), new_symbols(skolem, [sk0, sk1])],\
                                   [source, ax_outer])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_multi_parent_skolemization_with_wrong_declaration() {
        let problem = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y))).";
        let proof = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y)), file('problem.p', source)).\
                     fof(ax_outer, plain,\
                         ((? [X] : (p(X) & ? [Y] : r(X, Y)))\
                          => (p(sk0) & ? [Y] : r(sk0, Y))),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(ax_inner, plain,\
                         ((? [Y] : r(sk0, Y)) => r(sk0, sk1)),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(step, plain, (p(sk0) & r(sk0, sk1)),\
                         inference(skolemisation,\
                                   [status(esa), new_symbols(skolem, [sk0])],\
                                   [source, ax_outer, ax_inner])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn multi_parent_skolemization_matching_limit_is_inconclusive() {
        let problem = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y))).\n\
                       fof(np, axiom, ![X] : ~p(X)).\n\
                       fof(nr, axiom, ![X,Y] : ~r(X, Y)).";
        let proof = "fof(source, axiom, ?[X] : (p(X) & ?[Y] : r(X, Y)), file('problem.p', source)).\
                     fof(ax_outer, plain,\
                         ((? [X] : (p(X) & ? [Y] : r(X, Y)))\
                          => (p(sk0) & ? [Y] : r(sk0, Y))),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(ax_inner, plain,\
                         ((? [Y] : r(sk0, Y)) => r(sk0, sk1)),\
                         introduced(definition, [], [skolem_symbol_introduction])).\
                     fof(step, plain, (p(sk0) & r(sk0, sk1)),\
                         inference(skolemisation,\
                                   [status(esa), new_symbols(skolem, [sk0, sk1])],\
                                   [source, ax_inner, ax_outer])).\
                     fof(np, axiom, ![X] : ~p(X), file('problem.p', np)).\
                     fof(nr, axiom, ![X,Y] : ~r(X, Y), file('problem.p', nr)).\
                     fof(mid, plain, r(sk0, sk1), inference(resolution, [status(thm)], [step, np])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid, nr])).";
        let problem = parse_tptp(problem).expect("problem parses");
        let proof = parse_tptp(proof).expect("proof parses");
        let limits = VerificationLimits {
            max_skolem_steps: 1,
            ..VerificationLimits::default()
        };
        assert!(matches!(
            verify_strict(&problem, &proof, limits),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_leaf_source_path_mismatch() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\n\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\n\
                     fof(s, plain, $false, inference(resolution, [status(thm)], [a,b])).";
        let problem = parse_tptp(problem).expect("problem parses");
        let proof = parse_tptp(proof).expect("proof parses");
        assert!(matches!(
            verify_strict_with_source(
                &problem,
                &proof,
                Some("different.p"),
                VerificationLimits::default()
            ),
            KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn ignores_axiom_leaves_outside_the_root_derivation() {
        // GEO127+1 shape: the exporter emits every input axiom as a leaf,
        // including ones the refutation never uses. Unreachable nodes
        // cannot affect validity, so the kernel verifies the reachable
        // subgraph instead of rejecting the proof.
        let problem = "fof(a, axiom, p(a)).\n\
                       fof(unused, axiom, q(b)).\n\
                       fof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\n\
                     fof(unused, axiom, q(b), file('problem.p', unused)).\n\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [a,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn matches_leaf_against_duplicate_problem_names() {
        // LCL518+1 shape: two includes define `dup` differently. A leaf
        // is substantiated when ANY same-named problem formula matches.
        let problem = "fof(dup, axiom, p(a)).\n\
                       fof(dup, axiom, q(b)).\n\
                       fof(n, axiom, ~q(b)).";
        let proof = "fof(dup, axiom, q(b), file('problem.p', dup)).\n\
                     fof(n, axiom, ~q(b), file('problem.p', n)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [dup,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_factoring() {
        let problem = "fof(a, axiom, p(X) | p(a) | q(X)).\n\
                       fof(n1, axiom, ~p(a)).\n\
                       fof(n2, axiom, ~q(a)).";
        let proof = "fof(a, axiom, p(X) | p(a) | q(X), file('problem.p', a)).\n\
                     fof(n1, axiom, ~p(a), file('problem.p', n1)).\n\
                     fof(n2, axiom, ~q(a), file('problem.p', n2)).\n\
                     fof(s, plain, p(a) | q(a), inference(factoring, [status(thm)], [a])).\n\
                     fof(mid, plain, q(a), inference(resolution, [status(thm)], [s,n1])).\n\
                      fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,n2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_factoring_condensed_conclusion() {
        // MGT079+1 c257→c9135 shape: merge two negative `is_v` lits, which
        // leaves a duplicate the prover condenses before export.
        let problem = "fof(a, axiom, ~is_v(X) | ~is_v(Y) | ~is_v(X) | q(X)).\n\
                       fof(n1, axiom, is_v(a)).\n\
                       fof(n2, axiom, ~q(a)).";
        let proof = "fof(a, axiom, ~is_v(X) | ~is_v(Y) | ~is_v(X) | q(X), file('problem.p', a)).\n\
                     fof(n1, axiom, is_v(a), file('problem.p', n1)).\n\
                     fof(n2, axiom, ~q(a), file('problem.p', n2)).\n\
                     fof(s, plain, ~is_v(X) | q(X), inference(factoring, [status(thm)], [a])).\n\
                     fof(mid, plain, q(a), inference(resolution, [status(thm)], [s,n1])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,n2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_multi_literal_factoring() {
        // SYN353+1 shape: three literals collapse at once via X1 = X2 = X3.
        let problem = "fof(a, axiom, ~p(X1,X2,X3) | ~p(X2,X3,X1) | ~p(X3,X1,X2) | ~q(X1,X2,X3)).\n\
                       fof(n1, axiom, p(a,a,a)).\n\
                       fof(n2, axiom, q(a,a,a)).";
        let proof = "fof(a, axiom, ~p(X1,X2,X3) | ~p(X2,X3,X1) | ~p(X3,X1,X2) | ~q(X1,X2,X3), file('problem.p', a)).\n\
                     fof(n1, axiom, p(a,a,a), file('problem.p', n1)).\n\
                     fof(n2, axiom, q(a,a,a), file('problem.p', n2)).\n\
                     fof(s, plain, ~p(X3,X3,X3) | ~q(X3,X3,X3), inference(factoring, [status(thm)], [a])).\n\
                     fof(mid, plain, ~q(a,a,a), inference(resolution, [status(thm)], [s,n1])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [mid,n2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_condensation() {
        let problem = "fof(a, axiom, p(X) | q(X) | p(Y)).\n\
                       fof(n, axiom, ~p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(a, axiom, p(X) | q(X) | p(Y), file('problem.p', a)).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     cnf(c, plain, p(X) | q(X), inference(condensation, [status(thm)], [a])).\
                     cnf(mid, plain, q(a), inference(resolution, [status(thm)], [c,n])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [mid,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_condensation_equality_flipped_orientation() {
        // CSR015+1 c10797→c10798 shape: unify `push = X0` with `X0 = push`.
        // The direct unifier binds X0↦push and misses the goal; the flipped
        // unifier is the identity and yields exactly the exported clause.
        let problem = "fof(a, axiom, r(X) | push = X | pl = X | X = push).\n\
                       fof(n, axiom, ~r(push)).\n\
                       fof(nq, axiom, pl != push).\n\
                       fof(np, axiom, ~(push = push)).";
        let proof = "fof(a, axiom, r(X) | push = X | pl = X | X = push, file('problem.p', a)).\
                     fof(n, axiom, ~r(push), file('problem.p', n)).\
                     fof(nq, axiom, pl != push, file('problem.p', nq)).\
                     fof(np, axiom, ~(push = push), file('problem.p', np)).\
                     cnf(c, plain, r(X) | pl = X | X = push, inference(condensation, [status(thm)], [a])).\
                     cnf(m1, plain, pl = push | push = push, inference(resolution, [status(thm)], [c,n])).\
                     cnf(m2, plain, push = push, inference(resolution, [status(thm)], [m1,nq])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m2,np])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_condensation_retaining_duplicates() {
        // CSR115+6 c41390→c648782 shape: the factor substitutes one variable
        // into another, so the exported conclusion still holds two syntactic
        // copies of a literal the parent had only once under different vars.
        let problem = "fof(a, axiom, p(X) | p(Y) | p(X) | q(X)).\n\
                       fof(n, axiom, ~p(a)).\n\
                       fof(nq, axiom, ~q(a)).";
        let proof = "fof(a, axiom, p(X) | p(Y) | p(X) | q(X), file('problem.p', a)).\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\
                     fof(nq, axiom, ~q(a), file('problem.p', nq)).\
                     cnf(c, plain, p(Y) | p(Y) | q(Y), inference(condensation, [status(thm)], [a])).\
                     cnf(mid1, plain, p(a) | q(a), inference(resolution, [status(thm)], [c,n])).\
                     cnf(mid2, plain, q(a), inference(resolution, [status(thm)], [mid1,n])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [mid2,nq])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_condensation_conclusion() {
        let problem = "fof(a, axiom, p(X) | p(a) | q(X)).";
        let proof = "fof(a, axiom, p(X) | p(a) | q(X), file('problem.p', a)).\
                     cnf(c, plain, p(a) | r(a), inference(condensation, [status(thm)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [c])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_equality_resolution() {
        let problem = "fof(a, axiom, ~(X = X) | p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, ~(X = X) | p(a), file('problem.p', a)).\n\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\n\
                     fof(s, plain, p(a), inference(equality_resolution, [status(thm)], [a])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_equality_resolution_condensed_conclusion() {
        // CSR117+1 c15639 shape: resolving `~(X = Y)` instantiates the rest
        // into three copies of the same literal; the exported conclusion is
        // the condensed single copy.
        let problem = "fof(a, axiom, ~(X = Y) | p(X) | p(Y) | p(X)).\n\
                       fof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, ~(X = Y) | p(X) | p(Y) | p(X), file('problem.p', a)).\n\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\n\
                     fof(s, plain, p(X), inference(equality_resolution, [status(thm)], [a])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_equality_factoring() {
        let problem = "fof(a, axiom, f(X) = a | f(X) = a).\n\
                       fof(target, axiom, p(f(b))).\n\
                       fof(np, axiom, ~p(a)).";
        let proof = "fof(a, axiom, f(X) = a | f(X) = a, file('problem.p', a)).\
                     fof(target, axiom, p(f(b)), file('problem.p', target)).\
                     fof(np, axiom, ~p(a), file('problem.p', np)).\
                     fof(f, plain, f(X) = a | ~(a = a), inference(equality_factoring, [status(thm)], [a])).\
                     fof(mid, plain, ~(a = a) | p(a), inference(superposition, [status(thm)], [f,target])).\
                     fof(last, plain, p(a), inference(equality_resolution, [status(thm)], [mid])).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [last,np])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_equality_factoring_conclusion() {
        let problem = "fof(a, axiom, f(X) = a | f(Y) = b | p(X, Y)).";
        let proof = "fof(a, axiom, f(X) = a | f(Y) = b | p(X, Y), file('problem.p', a)).\
                     fof(f, plain, f(a) = a | ~(a = c) | p(a, a), inference(equality_factoring, [status(thm)], [a])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [f])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_forged_factoring_conclusion() {
        let problem = "fof(a, axiom, p(X) | p(a) | q(X)).";
        let proof = "fof(a, axiom, p(X) | p(a) | q(X), file('problem.p', a)).\n\
                     fof(s, plain, r(a), inference(factoring, [status(thm)], [a])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,a])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_demodulation() {
        let problem = "fof(rule, axiom, f(a) = b).\n\
                       fof(target, axiom, p(f(a))).\n\
                       fof(neg, axiom, ~p(b)).";
        let proof = "fof(rule, axiom, f(a) = b, file('problem.p', rule)).\n\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     fof(s, plain, p(b), inference(demodulation, [status(thm)], [target,rule])).\n\
                     fof(n, axiom, ~p(b), file('problem.p', neg)).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_avatar_conditional_demodulation() {
        let problem = "cnf(target, axiom, p(f(a)) | ~spl0_1).\n\
                       cnf(rule, axiom, f(a) = b | ~spl0_1).\n\
                       cnf(neg, axiom, ~p(b) | ~spl0_1).\n\
                       cnf(spl, axiom, spl0_1).";
        let proof = "cnf(target, axiom, p(f(a)) | ~spl0_1, file('problem.p', target)).\n\
                     cnf(rule, axiom, f(a) = b | ~spl0_1, file('problem.p', rule)).\
                     cnf(neg, axiom, ~p(b) | ~spl0_1, file('problem.p', neg)).\
                     cnf(spl, axiom, spl0_1, file('problem.p', spl)).\
                     cnf(s, plain, p(b) | ~spl0_1, inference(demodulation, [status(thm)], [target, rule])).\
                     cnf(c, plain, ~spl0_1, inference(resolution, [status(thm)], [s, neg])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [c, spl])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_conditional_demodulation_with_unsatisfied_condition() {
        let problem = "cnf(target, axiom, p(f(a))).\n\
                       cnf(rule, axiom, f(a) = b | ~spl0_1).";
        let proof = "cnf(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     cnf(rule, axiom, f(a) = b | ~spl0_1, file('problem.p', rule)).\
                     cnf(s, plain, p(b), inference(demodulation, [status(thm)], [target, rule])).\
                     cnf(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_conditional_demodulation_propagating_ground_condition() {
        let problem = "cnf(target, axiom, p(f(a))).\n\
                       cnf(rule, axiom, f(a) = b | ~spl0_1).\n\
                       cnf(neg, axiom, ~p(b) | ~spl0_1).\n\
                       cnf(spl, axiom, spl0_1).";
        let proof = "cnf(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     cnf(rule, axiom, f(a) = b | ~spl0_1, file('problem.p', rule)).\
                     cnf(neg, axiom, ~p(b) | ~spl0_1, file('problem.p', neg)).\
                     cnf(spl, axiom, spl0_1, file('problem.p', spl)).\
                     cnf(s, plain, p(b) | ~spl0_1, inference(demodulation, [status(thm)], [target, rule])).\
                     cnf(c, plain, ~spl0_1, inference(resolution, [status(thm)], [s, neg])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [c, spl])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn demodulation_replay_failure_is_inconclusive() {
        let problem = "fof(rule, axiom, f(a) = b).\n\
                       fof(target, axiom, p(f(a))).";
        let proof = "fof(rule, axiom, f(a) = b, file('problem.p', rule)).\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\
                     fof(s, plain, q(a), inference(demodulation, [status(thm)], [target,rule])).\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn rejects_demodulation_without_unit_equality_parent() {
        let problem = "fof(rule, axiom, f(a) = b).\nfof(target, axiom, p(f(a))).";
        let proof = "fof(rule, axiom, f(a) = b, file('problem.p', rule)).\n\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     fof(s, plain, p(b), inference(demodulation, [status(thm)], [target])).\n\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn introduced_definition_symbol_is_reserved_for_later_skolemization() {
        let problem = "fof(source, axiom, ?[X] : p(X)).\n\
                       fof(neg, axiom, ![X] : ~p(X)).";
        let proof = "fof(source, axiom, ?[X] : p(X), file('problem.p', source)).\
                     fof(neg, axiom, ![X] : ~p(X), file('problem.p', neg)).\
                     cnf(def, definition, d0 = a, introduced(definition, [new_symbols(definition, [d0])])).\
                     fof(sk, plain, p(d0), inference(skolemisation, [status(esa)], [source])).\
                     cnf(p_a, plain, p(a), inference(superposition, [status(thm)], [def, sk])).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [p_a, neg])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_superposition_into_predicate() {
        let problem = "fof(eq, axiom, f(a) = b).\n\
                       fof(target, axiom, p(f(a))).\n\
                       fof(neg, axiom, ~p(b)).";
        let proof = "fof(eq, axiom, f(a) = b, file('problem.p', eq)).\n\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     fof(neg, axiom, ~p(b), file('problem.p', neg)).\n\
                     fof(s, plain, p(b), inference(superposition, [status(thm)], [eq,target])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,neg])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_superposition_with_condensed_conclusion() {
        // GEO127+1 c11104 shape: both parents share an AVATAR assumption,
        // so the raw rewrite has a duplicate literal the prover condenses.
        let problem = "cnf(eq, axiom, f(a) = b | ~s1 | ~s2).\n\
                       cnf(target, axiom, p(f(a)) | ~s1).\n\
                       cnf(neg, axiom, ~p(b)).\n\
                       cnf(a1, axiom, s1).\n\
                       cnf(a2, axiom, s2).";
        let proof = "cnf(eq, axiom, f(a) = b | ~s1 | ~s2, file('problem.p', eq)).\n\
                     cnf(target, axiom, p(f(a)) | ~s1, file('problem.p', target)).\n\
                     cnf(neg, axiom, ~p(b), file('problem.p', neg)).\n\
                     cnf(a1, axiom, s1, file('problem.p', a1)).\n\
                     cnf(a2, axiom, s2, file('problem.p', a2)).\n\
                     cnf(s, plain, p(b) | ~s1 | ~s2, inference(superposition, [status(thm)], [eq,target])).\n\
                     cnf(mid, plain, ~s1 | ~s2, inference(resolution, [status(thm)], [s,neg])).\n\
                     cnf(mid2, plain, ~s2, inference(resolution, [status(thm)], [mid,a1])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [mid2,a2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_forged_superposition_conclusion() {
        let problem = "fof(eq, axiom, f(a) = b).\n\
                       fof(target, axiom, p(f(a))).";
        let proof = "fof(eq, axiom, f(a) = b, file('problem.p', eq)).\n\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     fof(s, plain, q(b), inference(superposition, [status(thm)], [eq,target])).\n\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_superposition_with_background_commutativity() {
        // SWV486+1 shape: the step needs commutativity of `plus`, but is
        // exported as plain `superposition` without AC citations. The
        // kernel retries against problem-theory AC axioms, which are
        // premises of the problem itself.
        let problem = "fof(comm, axiom, ![X,Y] : (plus(X,Y) = plus(Y,X))).\n\
                       fof(eq, axiom, plus(X1,one) = plus(X1,sk(X1))).\n\
                       fof(target, axiom, p(plus(Y,sk0)) | q(Y)).\n\
                       fof(n1, axiom, ~p(plus(sk0,sk(sk0)))).\n\
                       fof(n2, axiom, ~q(one)).";
        let proof = "fof(comm, axiom, ![X,Y] : (plus(X,Y) = plus(Y,X)), file('problem.p', comm)).\n\
                     fof(eq, axiom, plus(X1,one) = plus(X1,sk(X1)), file('problem.p', eq)).\n\
                     fof(target, axiom, p(plus(Y,sk0)) | q(Y), file('problem.p', target)).\n\
                     fof(n1, axiom, ~p(plus(sk0,sk(sk0))), file('problem.p', n1)).\n\
                     fof(n2, axiom, ~q(one), file('problem.p', n2)).\n\
                     cnf(s, plain, p(plus(sk0,sk(sk0))) | q(one), inference(superposition, [status(thm)], [eq,target])).\n\
                     cnf(m1, plain, q(one), inference(resolution, [status(thm)], [s,n1])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m1,n2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_superposition_with_negated_conjecture_commutativity() {
        // SWX217+1 shape: commutativity of `x` is the *negated conjecture*
        // itself (`? [X,Y] : x(X,Y) != x(Y,X)`), not a cited axiom. Negating
        // a conjecture is a premise of every refutation of it, so the kernel
        // loads the flipped equality as background AC and replays the plain
        // superposition that used `unify_comm`.
        let problem = "fof(goal, conjecture, ? [X,Y] : x(X,Y) != x(Y,X)).\n\
                       fof(eq, axiom, x(suc(zero), a) = b).\n\
                       fof(target, axiom, p(x(a, suc(zero))) | q(a)).\n\
                       fof(n1, axiom, ~p(b)).\n\
                       fof(n2, axiom, ~q(a)).";
        let proof = "fof(goal, conjecture, ? [X,Y] : x(X,Y) != x(Y,X), file('problem.p', goal)).\n\
                     fof(eq, axiom, x(suc(zero), a) = b, file('problem.p', eq)).\n\
                     fof(target, axiom, p(x(a, suc(zero))) | q(a), file('problem.p', target)).\n\
                     fof(n1, axiom, ~p(b), file('problem.p', n1)).\n\
                     fof(n2, axiom, ~q(a), file('problem.p', n2)).\n\
                     cnf(s, plain, p(b) | q(a), inference(superposition, [status(thm)], [eq,target])).\n\
                     cnf(m1, plain, q(a), inference(resolution, [status(thm)], [s,n1])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m1,n2])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_superposition_with_folded_demodulation() {
        // SET637+1 c21169 shape: the single rewrite leaves a redex
        // (`inter(es,es)`) that the prover demodulates with the cited
        // equation itself before emitting the conclusion.
        let problem = "fof(eq, axiom, inter(X,es) = es).\n\
                       fof(target, axiom, d(X8) | m(sk(X8), inter(X8,inter(X8,X8)))).\n\
                       fof(nd, axiom, ~d(es)).\n\
                       fof(nm, axiom, ~m(sk(es),es)).";
        let proof = "fof(eq, axiom, inter(X,es) = es, file('problem.p', eq)).\n\
                     fof(target, axiom, d(X8) | m(sk(X8), inter(X8,inter(X8,X8))), file('problem.p', target)).\n\
                     fof(nd, axiom, ~d(es), file('problem.p', nd)).\n\
                     fof(nm, axiom, ~m(sk(es),es), file('problem.p', nm)).\n\
                     cnf(s, plain, d(es) | m(sk(es),es), inference(superposition, [status(thm)], [eq,target])).\n\
                     cnf(m1, plain, m(sk(es),es), inference(resolution, [status(thm)], [s,nd])).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [m1,nm])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn certifies_complete_case_split() {
        let problem = "fof(top, axiom, p | q).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let proof = "fof(top, axiom, p | q, file('problem.p', top)).\n\
                     fof(np, axiom, ~p, file('problem.p', np)).\n\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\n\
                     fof(b0, plain, p, inference(split_component, [status(esa)], [top])).\n\
                     fof(b1, plain, q, inference(split_component, [status(esa)], [top])).\n\
                     fof(f0, plain, $false, inference(resolution, [status(thm)], [b0,np])).\n\
                     fof(f1, plain, $false, inference(resolution, [status(thm)], [b1,nq])).\n\
                     fof(bot, plain, $false, inference(avatar_sat_refutation, [status(thm)], [top,f0,f1])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_case_split_with_missing_branch() {
        let problem = "fof(top, axiom, p | q).\nfof(np, axiom, ~p).";
        let proof = "fof(top, axiom, p | q, file('problem.p', top)).\n\
                     fof(np, axiom, ~p, file('problem.p', np)).\n\
                     fof(b0, plain, p, inference(split_component, [status(esa)], [top])).\n\
                     fof(f0, plain, $false, inference(resolution, [status(thm)], [b0,np])).\n\
                     fof(bot, plain, $false, inference(avatar_sat_refutation, [status(thm)], [top,f0])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn rejects_avatar_variable_that_overflows_cadical_literal() {
        let problem = "fof(top, axiom, p | q).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let proof = "fof(top, axiom, p | q, file('problem.p', top)).\n\
                     fof(np, axiom, ~p, file('problem.p', np)).\n\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\n\
                     fof(split, plain, spl0_2147483648 | spl0_2,\
                         inference(avatar_split_clause,\
                           [status(esa),\
                            avatar_split([branch(0, spl0_2147483648, [0]),\
                                         branch(1, spl0_2, [1])], [])],\
                           [top])).\n\
                     fof(comp_p, plain, p | ~spl0_2147483648,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 0, spl0_2147483648)],\
                           [split])).\n\
                     fof(comp_q, plain, q | ~spl0_2,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 1, spl0_2)],\
                           [split])).\n\
                     fof(empty_p, plain, ~spl0_2147483648,\
                         inference(resolution, [status(thm)], [comp_p, np])).\n\
                     fof(empty_q, plain, ~spl0_2,\
                         inference(resolution, [status(thm)], [comp_q, nq])).\n\
                     fof(branch_p, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_2147483648])], [empty_p])).\n\
                     fof(branch_q, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_2])], [empty_q])).\n\
                     fof(bot, plain, $false,\
                         inference(avatar_sat_refutation, [status(thm)],\
                           [split, branch_p, branch_q])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_explicit_avatar_certificate() {
        let problem = "fof(top, axiom, p | q).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let (sat_trace, _) = explicit_avatar_sat_trace();
        let proof = format!(
            "fof(top, axiom, p | q, file('problem.p', top)).\
                     fof(np, axiom, ~p, file('problem.p', np)).\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\
                     fof(split, plain, spl0_1 | spl0_2,\
                         inference(avatar_split_clause,\
                           [status(esa),\
                            avatar_split([branch(0, spl0_1, [0]),\
                                         branch(1, spl0_2, [1])], [])],\
                           [top])).\
                     fof(comp_p, plain, p | ~spl0_1,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 0, spl0_1)],\
                           [split])).\
                     fof(comp_q, plain, q | ~spl0_2,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 1, spl0_2)],\
                           [split])).\
                     fof(empty_p, plain, ~spl0_1,\
                         inference(resolution, [status(thm)], [comp_p, np])).\
                     fof(empty_q, plain, ~spl0_2,\
                         inference(resolution, [status(thm)], [comp_q, nq])).\
                     fof(branch_p, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_1])], [empty_p])).\
                     fof(branch_q, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_2])], [empty_q])).\
                     fof(bot, plain, $false,\
                         inference(avatar_sat_refutation,\
                            [status(thm),\
                             {sat_trace}],\
                            [split, branch_p, branch_q])).",
        );
        assert_eq!(check(problem, &proof), KernelVerdict::Certified);

        let (sat_trace, digest) = explicit_avatar_sat_trace();
        let corrupted = proof.replace(&digest, &"0".repeat(digest.len()));
        assert!(matches!(
            check(problem, &corrupted),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
        assert!(sat_trace.contains("sat_trace"));
    }

    #[test]
    fn certifies_avatar_component_with_condensed_duplicates() {
        // COM133+1 shape: the split parent carries a duplicated literal,
        // so branch 0 covers two identical indices while the derived
        // component clause is condensed to one.
        let problem = "fof(top, axiom, p | p | q).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let (sat_trace, _) = explicit_avatar_sat_trace();
        let proof = format!(
            "fof(top, axiom, p | p | q, file('problem.p', top)).\
                     fof(np, axiom, ~p, file('problem.p', np)).\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\
                     fof(split, plain, spl0_1 | spl0_2,\
                         inference(avatar_split_clause,\
                           [status(esa),\
                            avatar_split([branch(0, spl0_1, [0, 1]),\
                                         branch(1, spl0_2, [2])], [])],\
                           [top])).\
                     fof(comp_p, plain, p | ~spl0_1,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 0, spl0_1)],\
                           [split])).\
                     fof(comp_q, plain, q | ~spl0_2,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 1, spl0_2)],\
                           [split])).\
                     fof(empty_p, plain, ~spl0_1,\
                         inference(resolution, [status(thm)], [comp_p, np])).\
                     fof(empty_q, plain, ~spl0_2,\
                         inference(resolution, [status(thm)], [comp_q, nq])).\
                     fof(branch_p, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_1])], [empty_p])).\
                     fof(branch_q, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_2])], [empty_q])).\
                     fof(bot, plain, $false,\
                         inference(avatar_sat_refutation,\
                            [status(thm),\
                             {sat_trace}],\
                            [split, branch_p, branch_q])).",
        );
        assert_eq!(check(problem, &proof), KernelVerdict::Certified);
    }

    fn sat_backed_annotation(
        manifest: &[Vec<i32>],
        original_ids: &[i64],
        trace: &[u8],
    ) -> (String, String) {
        let digest = mrs_core::clause::avatar_sat_trace_digest(
            "frat-lrat",
            1,
            original_ids,
            &[0, 1],
            manifest,
            trace,
        );
        let hex = |bytes: &[u8]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let manifest_text = manifest
            .iter()
            .map(|clause| {
                format!(
                    "[{}]",
                    clause
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let annotation = format!(
            "sat_backed_refutation([ax1, ax2], sat_trace('frat-lrat', 1, '{}', [1, 2], [0, 1], [{manifest_text}], '{}'))",
            hex(&digest),
            hex(trace),
        );
        (annotation, hex(&digest))
    }

    fn sat_backed_proof(annotation: &str) -> (String, String) {
        let problem = "cnf(ax1, axiom, p(a)).\ncnf(ax2, axiom, ~p(a)).";
        let proof = format!(
            "cnf(ax1, axiom, p(a), file('problem.p', ax1)).\
             cnf(ax2, axiom, ~p(a), file('problem.p', ax2)).\
             cnf(bot, plain, $false, inference(sat_backed_refutation, [status(thm), {annotation}], [ax1, ax2])).",
        );
        (problem.to_string(), proof)
    }

    #[test]
    fn certifies_sat_backed_refutation() {
        // Real (if tiny) FRAT proof of [1],[-1]: two originals, one empty
        // RUP addition, finalization.
        let trace = b"o 1 1 0\no 2 -1 0\na 3 0 l 1 2 0\nf 3 0\n";
        let manifest = vec![vec![1], vec![-1]];
        let (annotation, _) = sat_backed_annotation(&manifest, &[1, 2], trace);
        let (problem, proof) = sat_backed_proof(&annotation);
        assert_eq!(check(&problem, &proof), KernelVerdict::Certified);
    }

    #[test]
    fn rejects_sat_backed_refutation_mutations() {
        let trace = b"o 1 1 0\no 2 -1 0\na 3 0 l 1 2 0\nf 3 0\n";
        let manifest = vec![vec![1], vec![-1]];
        let (annotation, digest) = sat_backed_annotation(&manifest, &[1, 2], trace);
        let (problem, proof) = sat_backed_proof(&annotation);
        assert_eq!(check(&problem, &proof), KernelVerdict::Certified);

        // Corrupted digest.
        let corrupted = proof.replace(&digest, &"0".repeat(digest.len()));
        assert!(matches!(
            check(&problem, &corrupted),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
        // Dropped parent (ax2 gone from parents and inputs alike): the
        // manifest no longer matches the cited set.
        let dropped = proof.replace(", ax2]", "]").replace("[ax1, ax2]", "[ax1]");
        assert!(matches!(
            check(&problem, &dropped),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
        // Flipped trace literal breaks the manifest binding (the digest
        // is recomputed over the flipped bytes, so only the replay
        // content check can catch it).
        let flipped_trace = b"o 1 1 0\no 2 1 0\na 3 0 l 1 2 0\nf 3 0\n";
        let (bad_annotation, _) = sat_backed_annotation(&manifest, &[1, 2], flipped_trace);
        let (_, bad_proof) = sat_backed_proof(&bad_annotation);
        assert!(matches!(
            check(&problem, &bad_proof),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
        // Missing trace payload is inconclusive, never certified.
        let bare = "cnf(ax1, axiom, p(a), file('problem.p', ax1)).\
                    cnf(ax2, axiom, ~p(a), file('problem.p', ax2)).\
                    cnf(bot, plain, $false, inference(sat_backed_refutation, [status(thm)], [ax1, ax2])).";
        assert!(matches!(
            check(&problem, bare),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn certifies_explicit_avatar_certificate_lrat() {
        let problem = "fof(top, axiom, p | q).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let (sat_trace, _) = explicit_avatar_sat_trace_lrat();
        let proof = format!(
            "fof(top, axiom, p | q, file('problem.p', top)).\
                     fof(np, axiom, ~p, file('problem.p', np)).\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\
                     fof(split, plain, spl0_1 | spl0_2,\
                         inference(avatar_split_clause,\
                           [status(esa),\
                            avatar_split([branch(0, spl0_1, [0]),\
                                         branch(1, spl0_2, [1])], [])],\
                           [top])).\
                     fof(comp_p, plain, p | ~spl0_1,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 0, spl0_1)],\
                           [split])).\
                     fof(comp_q, plain, q | ~spl0_2,\
                         inference(avatar_component_clause,\
                           [status(esa), avatar_component(split, 1, spl0_2)],\
                           [split])).\
                     fof(empty_p, plain, ~spl0_1,\
                         inference(resolution, [status(thm)], [comp_p, np])).\
                     fof(empty_q, plain, ~spl0_2,\
                         inference(resolution, [status(thm)], [comp_q, nq])).\
                     fof(branch_p, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_1])], [empty_p])).\
                     fof(branch_q, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_2])], [empty_q])).\
                     fof(bot, plain, $false,\
                         inference(avatar_sat_refutation,\
                            [status(thm),\
                             {sat_trace}],\
                            [split, branch_p, branch_q])).",
        );
        assert_eq!(check(problem, &proof), KernelVerdict::Certified);

        let (sat_trace, digest) = explicit_avatar_sat_trace_lrat();
        let corrupted = proof.replace(&digest, &"0".repeat(digest.len()));
        assert!(matches!(
            check(problem, &corrupted),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
        assert!(sat_trace.contains("sat_trace"));
    }

    #[test]
    fn rejects_avatar_trace_manifest_binding_mutation() {
        let problem = "fof(top, axiom, p | q).\nfof(np, axiom, ~p).\nfof(nq, axiom, ~q).";
        let (sat_trace, _) = explicit_avatar_sat_trace();
        let proof = format!(
            "fof(top, axiom, p | q, file('problem.p', top)).\
             fof(np, axiom, ~p, file('problem.p', np)).\
             fof(nq, axiom, ~q, file('problem.p', nq)).\
             fof(split, plain, spl0_1 | spl0_2, inference(avatar_split_clause, [status(esa), avatar_split([branch(0, spl0_1, [0]), branch(1, spl0_2, [1])], [])], [top])).\
             fof(comp_p, plain, p | ~spl0_1, inference(avatar_component_clause, [status(esa), avatar_component(split, 0, spl0_1)], [split])).\
             fof(comp_q, plain, q | ~spl0_2, inference(avatar_component_clause, [status(esa), avatar_component(split, 1, spl0_2)], [split])).\
             fof(empty_p, plain, ~spl0_1, inference(resolution, [status(thm)], [comp_p, np])).\
             fof(empty_q, plain, ~spl0_2, inference(resolution, [status(thm)], [comp_q, nq])).\
             fof(branch_p, plain, $false, inference(avatar_branch_refutation, [status(esa), avatar_context([spl0_1])], [empty_p])).\
             fof(branch_q, plain, $false, inference(avatar_branch_refutation, [status(esa), avatar_context([spl0_2])], [empty_q])).\
             fof(bot, plain, $false, inference(avatar_sat_refutation, [status(thm), {sat_trace}], [split, branch_p, branch_q]))."
        );
        let mutated = proof.replace("[0, 1, 2]", "[0, 0, 2]");
        assert!(matches!(
            check(problem, &mutated),
            KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn explicit_avatar_certificate_requires_metadata() {
        let problem = "fof(top, axiom, p | q).\nfof(np, axiom, ~p).\nfof(nq, axiom, ~q).";
        let proof = "fof(top, axiom, p | q, file('problem.p', top)).\
                     fof(np, axiom, ~p, file('problem.p', np)).\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\
                     fof(split, plain, spl0_1 | spl0_2,\
                         inference(avatar_split_clause, [status(esa)], [top])).\
                     fof(bot, plain, $false,\
                         inference(avatar_sat_refutation,\
                           [status(thm)], [split])).";
        assert!(matches!(
            check(problem, proof),
            KernelVerdict::Inconclusive(_) | KernelVerdict::Rejected(_)
        ));
    }

    #[test]
    fn rejects_explicit_avatar_certificate_with_duplicate_branch_context() {
        let problem = "fof(top, axiom, p | q).\nfof(np, axiom, ~p).\nfof(nq, axiom, ~q).";
        let proof = "fof(top, axiom, p | q, file('problem.p', top)).\
                     fof(np, axiom, ~p, file('problem.p', np)).\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\
                     fof(split, plain, spl0_1 | spl0_2,\
                         inference(avatar_split_clause,\
                           [status(esa),\
                            avatar_split([branch(0, spl0_1, [0]),\
                                         branch(1, spl0_2, [1])], [])],\
                           [top])).\
                     fof(empty_p, plain, ~spl0_1,\
                         inference(resolution, [status(thm)], [split,np])).\
                     fof(branch_p, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_1])], [empty_p])).\
                     fof(branch_p2, plain, $false,\
                         inference(avatar_branch_refutation,\
                           [status(esa), avatar_context([spl0_1])], [empty_p])).\
                     fof(bot, plain, $false,\
                         inference(avatar_sat_refutation,\
                           [status(thm)], [split, branch_p, branch_p2])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }

    #[test]
    fn certifies_ac_resolution_with_commutativity_axiom() {
        let problem = "cnf(c1, axiom, p(f(a, b))).\n\
                       cnf(c2, axiom, ~p(f(b, a))).\n\
                       cnf(c_comm, axiom, f(X, Y) = f(Y, X)).";
        let proof = "cnf(c1, axiom, p(f(a, b)), file('problem.p', c1)).\n\
                     cnf(c2, axiom, ~p(f(b, a)), file('problem.p', c2)).\n\
                     cnf(c_comm, axiom, f(X, Y) = f(Y, X), file('problem.p', c_comm)).\n\
                     cnf(c_bot, plain, $false, inference(ac_resolution, [status(thm)], [c1, c2, c_comm])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
    }
}

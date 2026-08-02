//! Deterministic, dependency-light proof kernel for a strict subset of TSTP.
//!
//! This crate deliberately does not depend on `mrs-search`, external ATPs, or
//! the competition verifier. Unsupported proof rules return `Inconclusive`;
//! they are never accepted by guessing or by an inference-rule name.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mrs_core::{Atom, Formula, Substitution, SymbolTable, Term, VarId};
use mrs_tptp::ast::common::{AtomicWord, GeneralTerm};
use mrs_tptp::proover::ParentRef;
use mrs_tptp::{AnnotatedFormula, BinaryConnective, CNFFormula, CNFLiteral, CNFStatement};
use mrs_tptp::{FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, FormulaRole, Quantifier};

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
    pub verdict: KernelVerdict,
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
}

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
    let verdict = verify_strict_with_source(problem, proof, expected_source, limits);
    VerificationTelemetry {
        elapsed: start.elapsed(),
        problem_nodes,
        proof_nodes: proof_nodes.len(),
        proof_fof_nodes,
        proof_cnf_nodes,
        proof_clause_literals,
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
    let mut branch_contexts: HashMap<usize, BranchContext> = HashMap::new();
    let mut defined_symbols = HashSet::new();
    let mut skolem_axioms: HashMap<usize, OwnedSkolemAxiom> = HashMap::new();

    let mut problem_names = HashSet::with_capacity(problem.formulas.len());
    for formula in &problem.formulas {
        if formula.role() == FormulaRole::Type {
            continue;
        }
        if !problem_names.insert(formula.name()) {
            return KernelVerdict::Rejected(format!(
                "problem contains duplicate formula name `{}`",
                formula.name()
            ));
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
            "variable_rename" | "rectify" => verify_alpha_identity(&parents, conclusion),
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
            | "simplification" => verify_formula_equivalence(&parents, conclusion, limits),
            "instantiate" => verify_instantiation(&parents, conclusion, limits),
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
            "subsumption_resolution" => verify_subsumption_resolution(&parents, conclusion, limits),
            "factoring" => verify_factoring(&parents, conclusion, limits),
            "equality_resolution" => verify_equality_resolution(&parents, conclusion, limits),
            "equality_factoring" => verify_equality_factoring(&parents, conclusion, limits),
            "condensation" => verify_condensation(&parents, conclusion, limits),
            "demodulation" => verify_demodulation(&parents, conclusion, limits),
            "superposition" => verify_superposition(&parents, conclusion, limits),
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
            "avatar_sat_refutation" => verify_case_split(
                node,
                &dag,
                &parent_indices,
                &proof_formulas,
                &branch_contexts,
                limits,
            ),
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
        if !matches!(outcome, KernelVerdict::Certified) {
            return outcome;
        }
        if rule != "split_component" && rule != "avatar_sat_refutation" {
            let mut context: Option<BranchContext> = None;
            for parent_idx in &parent_indices {
                let Some(parent_context) = branch_contexts.get(parent_idx) else {
                    continue;
                };
                if let Some(existing) = &context {
                    if existing != parent_context {
                        return KernelVerdict::Rejected(format!(
                            "node `{}` combines incompatible case-split branches",
                            node.name
                        ));
                    }
                } else {
                    context = Some(parent_context.clone());
                }
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

fn expected_status(rule: &str) -> Option<&'static str> {
    match rule {
        "negated_conjecture" | "assume_negation" => Some("cth"),
        "skolemisation" | "skolemize" => Some("esa"),
        "fof_nnf"
        | "fof_nnf_transformation"
        | "nnf_transformation"
        | "variable_rename"
        | "rectify"
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
        | "instantiate"
        | "existential_gen"
        | "conjunction"
        | "split_conjunct"
        | "cnf_transformation"
        | "resolution"
        | "subsumption_resolution"
        | "factoring"
        | "equality_resolution"
        | "equality_factoring"
        | "condensation"
        | "demodulation"
        | "split_component"
        | "avatar_sat_refutation"
        | "superposition" => Some("thm"),
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
    let Some(fof) = node.formula.as_fof() else {
        return KernelVerdict::Inconclusive("definition is not FOF".into());
    };
    let FOFStatement::Logical(formula) = &fof.formula else {
        return KernelVerdict::Inconclusive("definition sequent is unsupported".into());
    };
    let body = strip_forall_fof(formula);
    let FOFFormula::Binary {
        left,
        connective: BinaryConnective::Iff,
        right,
    } = body
    else {
        return KernelVerdict::Rejected(format!(
            "definition `{}` is not a biconditional",
            node.name
        ));
    };
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
        return KernelVerdict::Rejected(format!("definition `{}` is recursive", node.name));
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
    let source = &parents[0];
    if contains_exists(source) {
        return KernelVerdict::Inconclusive(
            "strict CNF transformation requires an existential-free parent".into(),
        );
    }
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive("CNF conclusion is not a supported clause".into());
    };

    let mut definitions = Vec::with_capacity(parents.len().saturating_sub(1));
    for (parent, parent_idx) in parents.iter().skip(1).zip(&parent_indices[1..]) {
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
    let mut expanded = Vec::new();
    let normalized = to_nnf(&named_source);
    if !cnf_expand(&normalized, &mut expanded, limits) {
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
        .any(|clause| clause_alpha_equiv(clause, &goal))
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
        for definition in definitions {
            let (next, replaced) = replace_one_definition(&current, definition);
            current = next;
            changed |= replaced;
        }
        if !changed {
            return Some(current);
        }
    }
}

fn replace_one_definition(source: &Formula, definition: &CoreDefinition) -> (Formula, bool) {
    let (transformed, replaced) = match source {
        Formula::Atom(_) | Formula::True | Formula::False => (source.clone(), false),
        Formula::Neg(inner) => {
            let (inner, replaced) = replace_one_definition(inner, definition);
            (Formula::neg(inner), replaced)
        }
        Formula::And(parts) => {
            let mut replaced = false;
            let parts = parts
                .iter()
                .map(|part| {
                    let (part, part_replaced) = replace_one_definition(part, definition);
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
                    let (part, part_replaced) = replace_one_definition(part, definition);
                    replaced |= part_replaced;
                    part
                })
                .collect();
            (Formula::Or(parts), replaced)
        }
        Formula::Implies(left, right) => {
            let (left, left_replaced) = replace_one_definition(left, definition);
            let (right, right_replaced) = replace_one_definition(right, definition);
            (
                Formula::implies(left, right),
                left_replaced || right_replaced,
            )
        }
        Formula::Iff(left, right) => {
            let (left, left_replaced) = replace_one_definition(left, definition);
            let (right, right_replaced) = replace_one_definition(right, definition);
            (Formula::iff(left, right), left_replaced || right_replaced)
        }
        Formula::Forall(var, body) => {
            let (body, replaced) = replace_one_definition(body, definition);
            (Formula::forall(*var, body), replaced)
        }
        Formula::Exists(var, body) => {
            let (body, replaced) = replace_one_definition(body, definition);
            (Formula::exists(*var, body), replaced)
        }
    };

    let mut mapping = HashMap::new();
    if match_core_formula(&definition.rhs, &transformed, &mut mapping)
        && let Some(head) = apply_core_definition_head(&definition.head, &mapping)
    {
        (Formula::atom(head), true)
    } else {
        (transformed, replaced)
    }
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
    let root = match roots.as_slice() {
        [root] => *root,
        [] => {
            return Err(KernelVerdict::Rejected(
                "proof has no unparented `$false` root".into(),
            ));
        }
        _ => {
            return Err(KernelVerdict::Rejected(format!(
                "proof has {} unparented `$false` roots",
                roots.len()
            )));
        }
    };

    let mut reachable = HashSet::new();
    let mut stack = vec![root];
    while let Some(idx) = stack.pop() {
        if !reachable.insert(idx) {
            continue;
        }
        for parent in &nodes[idx].parents {
            stack.push(*by_name.get(parent.name).expect("validated parent"));
        }
    }
    if reachable.len() != nodes.len() {
        return Err(KernelVerdict::Rejected(
            "proof contains nodes outside the root derivation".into(),
        ));
    }

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
    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &degree)| (degree == 0).then_some(idx))
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(idx) = ready.pop() {
        order.push(idx);
        for &child in &children[idx] {
            indegree[child] -= 1;
            if indegree[child] == 0 {
                ready.push(child);
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
    let expected = problem
        .formulas
        .iter()
        .find(|formula| formula.name() == annotation.1)
        .ok_or_else(|| {
            KernelVerdict::Rejected(format!(
                "leaf `{}` references missing problem formula `{}`",
                node.name, annotation.1
            ))
        })?;
    if !roles_compatible(node.role, expected.role()) {
        return Err(KernelVerdict::Rejected(format!(
            "leaf `{}` role `{}` is incompatible with problem role `{}`",
            node.name,
            node.role.as_str(),
            expected.role().as_str()
        )));
    }
    let proof_formula = lower_annotated(symbols, node.formula, limits)?;
    let expected_formula = lower_annotated(symbols, expected, limits)?;
    if alpha_equiv(&proof_formula, &expected_formula) {
        Ok(())
    } else {
        // Keep the argument in the error so callers can diagnose a forged
        // provenance leaf without exposing the full formula in SZS output.
        Err(KernelVerdict::Rejected(format!(
            "leaf `{}` does not match problem formula `{}`",
            node.name, annotation.1
        )))
    }
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
    if parents.len() != 1 {
        return KernelVerdict::Rejected("identity rule must have one parent".into());
    }
    if alpha_equiv(&parents[0], conclusion) {
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
    let mut core_substitution = Substitution::new();
    for (var, term) in substitution {
        core_substitution.bind(var, term);
    }
    if alpha_equiv(&core_substitution.apply_formula(parent_body), target_body) {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("instantiate conclusion is not a parent instance".into())
    }
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
    symbol == &annotation.symbol && arguments == &annotation.arguments
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
struct SkolemMatch {
    fresh_symbols: HashSet<String>,
    used_symbols: HashSet<String>,
    universal_map: HashMap<String, String>,
    existential_terms: HashMap<String, String>,
    witness_owners: HashMap<String, String>,
    active_existentials: HashMap<String, Vec<String>>,
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
    if step_prefix
        .iter()
        .any(|(quantifier, _)| !is_effective_universal(*quantifier, polarity))
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

    let mut step_universal_idx = 0;
    let mut local_universals = Vec::new();
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
                state
                    .active_existentials
                    .insert(parent_var.clone(), state.active_universals.clone());
                local_existentials.push(parent_var.clone());
            }
        }
    }

    let matched = match_skolem_matrix(parent_matrix, step_matrix, state, polarity);
    for parent_var in local_existentials.into_iter().rev() {
        state.active_existentials.remove(&parent_var);
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
                match_skolem_multiset(&parent_parts, &step_parts, state, polarity)
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
                let expected: HashSet<&str> = scope.iter().map(String::as_str).collect();
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
                if arguments.len() != expected.len()
                    || actual.len() != actual_set.len()
                    || actual_set != expected
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
    split_parent: usize,
    branch_index: usize,
    literal: Literal,
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
    if left
        .iter()
        .any(|literal| matches!(literal.atom, Atom::Eq(..)))
        || right
            .iter()
            .any(|literal| matches!(literal.atom, Atom::Eq(..)))
    {
        return KernelVerdict::Inconclusive(
            "equality resolution is not yet implemented by the strict kernel".into(),
        );
    }

    let shift = max_var_clause(&left).saturating_add(1);
    shift_clause(&mut right, shift);
    for (left_idx, left_literal) in left.iter().enumerate() {
        for (right_idx, right_literal) in right.iter().enumerate() {
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
            if !left_args
                .iter()
                .zip(right_args)
                .all(|(left, right)| unify_terms(left, right, &mut substitution))
            {
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
        }
    }
    KernelVerdict::Rejected("resolution conclusion is not a parent resolvent".into())
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
    let Some(target) = clause_from_formula(&parents[0], limits) else {
        return KernelVerdict::Inconclusive(
            "subsumption_resolution target is not a supported clause".into(),
        );
    };
    let Some(active) = clause_from_formula(&parents[1], limits) else {
        return KernelVerdict::Inconclusive(
            "subsumption_resolution active parent is not a supported clause".into(),
        );
    };
    let Some(goal) = clause_from_formula(conclusion, limits) else {
        return KernelVerdict::Inconclusive(
            "subsumption_resolution conclusion is not a supported clause".into(),
        );
    };
    if active.is_empty() || active.len() > target.len() {
        return KernelVerdict::Rejected(
            "subsumption_resolution active parent cannot subsume the target".into(),
        );
    }

    let mut matching_steps = 0;
    for removed_idx in 0..target.len() {
        let mut modified_target = target.clone();
        modified_target[removed_idx].positive = !modified_target[removed_idx].positive;
        match clause_subsumes(
            &active,
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
        if clause_alpha_equiv(&expected, &goal) {
            return KernelVerdict::Certified;
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
    if pattern.len() > target.len() {
        return Ok(false);
    }

    // Keep target variables rigid while allowing only the standardized-apart
    // active-clause variables to receive matching substitutions.
    let shift = max_var_clause(target).saturating_add(1);
    let mut pattern = pattern.to_vec();
    shift_clause(&mut pattern, shift);
    let mut used = vec![false; target.len()];
    match_subsumption_literals(
        &pattern,
        target,
        &HashMap::new(),
        shift,
        &mut used,
        steps,
        step_limit,
    )
}

fn match_subsumption_literals(
    remaining: &[Literal],
    target: &[Literal],
    substitution: &HashMap<VarId, Term>,
    min_bindable: VarId,
    used: &mut [bool],
    steps: &mut usize,
    step_limit: usize,
) -> Result<bool, ()> {
    let Some((literal, rest)) = remaining.split_first() else {
        return Ok(true);
    };
    if *steps >= step_limit {
        return Err(());
    }
    *steps += 1;

    for (target_idx, target_literal) in target.iter().enumerate() {
        if used[target_idx] {
            continue;
        }
        if literal.positive != target_literal.positive {
            continue;
        }
        if let Some(next) = match_subsumption_atom(
            &literal.atom,
            &target_literal.atom,
            substitution,
            min_bindable,
        ) {
            used[target_idx] = true;
            let matched = match_subsumption_literals(
                rest,
                target,
                &next,
                min_bindable,
                used,
                steps,
                step_limit,
            );
            used[target_idx] = false;
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
            if clause_alpha_equiv(&expected, &goal) {
                return KernelVerdict::Certified;
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
        if clause_alpha_equiv(&expected, &goal) {
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

    for removed in 0..parent.len() {
        for matched in 0..parent.len() {
            if removed == matched || parent[removed].positive != parent[matched].positive {
                continue;
            }
            let Some(substitution) =
                unify_atoms_for_condensation(&parent[removed].atom, &parent[matched].atom)
            else {
                continue;
            };
            let mut expected = Vec::with_capacity(parent.len() - 1);
            for (index, literal) in parent.iter().enumerate() {
                if index == removed {
                    continue;
                }
                let substituted = apply_substitution_literal(literal, &substitution);
                if !expected.contains(&substituted) {
                    expected.push(substituted);
                }
            }
            if expected.len() >= parent.len() {
                continue;
            }
            let mut matching_steps = 0;
            let subsumes_parent = match clause_subsumes(
                &expected,
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
            if subsumes_parent && clause_alpha_equiv(&expected, &goal) {
                return KernelVerdict::Certified;
            }
        }
    }
    KernelVerdict::Rejected("condensation conclusion is not a valid condensed clause".into())
}

fn unify_atoms_for_condensation(left: &Atom, right: &Atom) -> Option<HashMap<VarId, Term>> {
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
                Some(substitution)
            } else {
                None
            }
        }
        (Atom::Eq(left_left, left_right), Atom::Eq(right_left, right_right)) => {
            let mut substitution = HashMap::new();
            if unify_terms(left_left, right_left, &mut substitution)
                && unify_terms(left_right, right_right, &mut substitution)
            {
                return Some(substitution);
            }
            let mut substitution = HashMap::new();
            if unify_terms(left_left, right_right, &mut substitution)
                && unify_terms(left_right, right_left, &mut substitution)
            {
                return Some(substitution);
            }
            None
        }
        _ => None,
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
    let mut rules = Vec::new();
    for parent in &parents[1..] {
        let Some(clause) = clause_from_formula(parent, limits) else {
            return KernelVerdict::Inconclusive(
                "demodulation equality parent is not a supported clause".into(),
            );
        };
        if clause.len() != 1 || !clause[0].positive {
            return KernelVerdict::Rejected(
                "demodulation parents must be positive unit equalities".into(),
            );
        }
        let Atom::Eq(left, right) = &clause[0].atom else {
            return KernelVerdict::Rejected(
                "demodulation parents must be positive unit equalities".into(),
            );
        };
        if left == right {
            continue;
        }
        let left_vars = term_var_set(left);
        let right_vars = term_var_set(right);
        if right_vars.is_subset(&left_vars) {
            rules.push((left.clone(), right.clone()));
        }
        if left_vars.is_subset(&right_vars) {
            rules.push((right.clone(), left.clone()));
        }
    }
    if rules.is_empty() {
        return KernelVerdict::Rejected("demodulation has no non-trivial rewrite rule".into());
    }

    let mut current = target;
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
    loop {
        if clause_alpha_equiv(&current, &goal) {
            return KernelVerdict::Certified;
        }
        if steps >= limits.max_rewrite_steps {
            return KernelVerdict::Inconclusive(
                "demodulation exceeded strict rewrite-step limit".into(),
            );
        }
        let mut changed = false;
        for literal in &mut current {
            if rewrite_atom(&mut literal.atom, &rules, &mut steps, limits) {
                changed = true;
                break;
            }
        }
        if !changed {
            return KernelVerdict::Rejected(
                "demodulation conclusion is not reachable from cited rewrites".into(),
            );
        }
    }
}

fn verify_superposition(
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
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
                }
            }
        }
    }
    KernelVerdict::Rejected("superposition conclusion is not a valid rewrite".into())
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
        split_parent: split_parent
            .ok_or_else(|| KernelVerdict::Rejected("split_component has no parent index".into()))?,
        branch_index,
        literal,
    })
}

fn verify_case_split(
    node: &Node<'_>,
    dag: &Dag<'_>,
    parent_indices: &[usize],
    formulas: &HashMap<usize, Formula>,
    branch_contexts: &HashMap<usize, BranchContext>,
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
        if context.split_parent != split_parent {
            return KernelVerdict::Rejected(format!(
                "avatar_sat_refutation branch `{}` cites a different split parent",
                dag.nodes[*branch_root].name
            ));
        }
        if !seen.insert(context.branch_index) {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation contains a duplicate branch".into(),
            );
        }
        let matches_top = top_clause
            .get(context.branch_index)
            .is_some_and(|top_literal| {
                clause_alpha_equiv(
                    std::slice::from_ref(top_literal),
                    std::slice::from_ref(&context.literal),
                )
            });
        if !matches_top {
            return KernelVerdict::Rejected(
                "avatar_sat_refutation branch literal does not match split parent".into(),
            );
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
    let _ = limits;
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
            term_alpha_equiv(left_l, right_l, mapping, reverse)
                && term_alpha_equiv(left_r, right_r, mapping, reverse)
        }
        _ => false,
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

    fn flat_definition_problem() -> &'static str {
        "fof(src, axiom, q(a) | r(a)).\n\
         fof(nq, axiom, ~q(a)).\n\
         fof(nr, axiom, ~r(a))."
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
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
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
            max_equivalence_steps: 1,
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
    fn rejects_subsumption_resolution_reusing_a_target_literal() {
        let input = "cnf(target, axiom, ~p(a) | r(a)).\n\
                     cnf(active, axiom, p(X) | p(Y)).\n\
                     cnf(nr, axiom, ~r(a)).";
        let proof = "cnf(target, axiom, ~p(a) | r(a), file('problem.p', target)).\n\
                     cnf(active, axiom, p(X) | p(Y), file('problem.p', active)).\n\
                     cnf(forged, plain, r(a), inference(subsumption_resolution, [status(thm)], [target,active])).\n\
                     cnf(nr, axiom, ~r(a), file('problem.p', nr)).\n\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [forged,nr])).";
        assert!(matches!(check(input, proof), KernelVerdict::Rejected(_)));
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
            max_equivalence_steps: 1,
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
             fof(bot, plain, $false, inference(consequence, [status(thm)], [i])).",
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
             fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).",
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
    fn rejects_demodulation_without_unit_equality_parent() {
        let problem = "fof(rule, axiom, f(a) = b).\nfof(target, axiom, p(f(a))).";
        let proof = "fof(rule, axiom, f(a) = b, file('problem.p', rule)).\n\
                     fof(target, axiom, p(f(a)), file('problem.p', target)).\n\
                     fof(s, plain, p(b), inference(demodulation, [status(thm)], [target])).\n\
                     fof(bot, plain, $false, inference(consequence, [status(thm)], [s])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
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
    fn certifies_complete_case_split() {
        let problem = "fof(top, axiom, p | q).\n\
                       fof(np, axiom, ~p).\n\
                       fof(nq, axiom, ~q).";
        let proof = "fof(top, axiom, p | q, file('problem.p', top)).\n\
                     fof(np, axiom, ~p, file('problem.p', np)).\n\
                     fof(nq, axiom, ~q, file('problem.p', nq)).\n\
                     fof(b0, plain, p, inference(split_component, [status(thm)], [top])).\n\
                     fof(b1, plain, q, inference(split_component, [status(thm)], [top])).\n\
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
                     fof(b0, plain, p, inference(split_component, [status(thm)], [top])).\n\
                     fof(f0, plain, $false, inference(resolution, [status(thm)], [b0,np])).\n\
                     fof(bot, plain, $false, inference(avatar_sat_refutation, [status(thm)], [top,f0])).";
        assert!(matches!(check(problem, proof), KernelVerdict::Rejected(_)));
    }
}

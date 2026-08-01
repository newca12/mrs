//! Deterministic, dependency-light proof kernel for a strict subset of TSTP.
//!
//! This crate deliberately does not depend on `mrs-search`, external ATPs, or
//! the competition verifier. Unsupported proof rules return `Inconclusive`;
//! they are never accepted by guessing or by an inference-rule name.

use std::collections::{HashMap, HashSet};

use mrs_core::{Atom, Formula, SymbolTable, Term, VarId};
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
}

impl Default for VerificationLimits {
    fn default() -> Self {
        Self {
            max_nodes: 100_000,
            max_formula_nodes: 100_000,
            max_clause_literals: 10_000,
            max_term_depth: 256,
            max_rewrite_steps: 64,
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
    let mut known_symbols: HashSet<String> = HashSet::new();
    for formula in &problem.formulas {
        collect_function_symbols(formula, &mut known_function_symbols);
        collect_all_symbols(formula, &mut known_symbols);
    }

    for &idx in &dag.topo {
        let node = &dag.nodes[idx];
        let conclusion = proof_formulas.get(&idx).expect("lowered proof formula");

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
                let outcome = verify_definition(node, annotations, &known_symbols);
                if !matches!(outcome, KernelVerdict::Certified) {
                    return outcome;
                }
                collect_function_symbols(node.formula, &mut known_function_symbols);
                collect_all_symbols(node.formula, &mut known_symbols);
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
            "skolemisation" => verify_skolemisation(
                node,
                &dag,
                &parent_indices,
                &parents,
                conclusion,
                &known_function_symbols,
            ),
            "cnf_transformation" => verify_cnf_transformation(&parents, conclusion, limits),
            "resolution" | "subsumption_resolution" => {
                verify_resolution(&parents, conclusion, limits)
            }
            "factoring" => verify_factoring(&parents, conclusion, limits),
            "equality_resolution" => verify_equality_resolution(&parents, conclusion, limits),
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
        collect_all_symbols(node.formula, &mut known_symbols);
    }

    KernelVerdict::Certified
}

fn expected_status(rule: &str) -> Option<&'static str> {
    match rule {
        "negated_conjecture" | "assume_negation" => Some("cth"),
        "skolemisation" => Some("esa"),
        "fof_nnf"
        | "fof_nnf_transformation"
        | "nnf_transformation"
        | "variable_rename"
        | "rectify"
        | "cnf_transformation"
        | "resolution"
        | "subsumption_resolution"
        | "factoring"
        | "equality_resolution"
        | "demodulation"
        | "split_component"
        | "avatar_sat_refutation"
        | "superposition" => Some("thm"),
        _ => None,
    }
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
    if known_symbols.contains(declared) {
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
    parents: &[Formula],
    conclusion: &Formula,
    limits: VerificationLimits,
) -> KernelVerdict {
    if parents.is_empty() || parents.len() > 2 {
        return KernelVerdict::Inconclusive(
            "strict CNF transformation requires one formula parent and at most one definition parent".into(),
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
    let mut expanded = Vec::new();
    let normalized = to_nnf(source);
    if !cnf_expand(&normalized, &mut expanded, limits) {
        return KernelVerdict::Inconclusive("CNF expansion exceeded strict limits".into());
    }
    if expanded
        .iter()
        .any(|clause| clause_alpha_equiv(clause, &goal))
    {
        KernelVerdict::Certified
    } else {
        KernelVerdict::Rejected("CNF conclusion is not a clause of the parent CNF".into())
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
    known_function_symbols: &HashSet<String>,
) -> KernelVerdict {
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
        return verify_existential_free_identity(parents, conclusion);
    }
    if !contains_exists(&parents[0]) {
        return KernelVerdict::Inconclusive(
            "Skolemization introduced symbols without an existential parent".into(),
        );
    }
    let mut state = SkolemMatch::new(fresh.clone());
    if !match_skolem_formula(parent_formula, step_formula, &mut state) {
        return KernelVerdict::Rejected(format!(
            "node `{}` is not an exact Skolemization of its parent",
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

struct SkolemMatch {
    fresh_symbols: HashSet<String>,
    used_symbols: HashSet<String>,
    universal_map: HashMap<String, String>,
    existential_terms: HashMap<String, String>,
    witness_owners: HashMap<String, String>,
    active_existentials: HashSet<String>,
    active_universals: Vec<String>,
}

impl SkolemMatch {
    fn new(fresh_symbols: HashSet<String>) -> Self {
        Self {
            fresh_symbols,
            used_symbols: HashSet::new(),
            universal_map: HashMap::new(),
            existential_terms: HashMap::new(),
            witness_owners: HashMap::new(),
            active_existentials: HashSet::new(),
            active_universals: Vec::new(),
        }
    }
}

fn match_skolem_formula(
    parent: &FOFFormula<'_>,
    step: &FOFFormula<'_>,
    state: &mut SkolemMatch,
) -> bool {
    match (parent, step) {
        (FOFFormula::Parens(parent), _) => match_skolem_formula(parent, step, state),
        (_, FOFFormula::Parens(step)) => match_skolem_formula(parent, step, state),
        (
            FOFFormula::Quantified {
                quantifier: parent_q,
                variables: parent_vars,
                formula: parent_body,
            },
            FOFFormula::Quantified {
                quantifier: step_q,
                variables: step_vars,
                formula: step_body,
            },
        ) if parent_q == step_q && parent_vars.len() == step_vars.len() => {
            let mut inserted = Vec::new();
            for (parent_var, step_var) in parent_vars.iter().zip(step_vars) {
                if parent_q == &Quantifier::Exists {
                    state.active_existentials.insert((*parent_var).to_string());
                    inserted.push((*parent_var, None));
                } else {
                    state
                        .universal_map
                        .insert((*parent_var).to_string(), (*step_var).to_string());
                    state.active_universals.push((*step_var).to_string());
                    inserted.push((*parent_var, Some(*step_var)));
                }
            }
            let ok = if parent_q == &Quantifier::Exists {
                match_skolem_formula(parent_body, step, state)
            } else {
                match_skolem_formula(parent_body, step_body, state)
            };
            for (parent_var, step_var) in inserted.into_iter().rev() {
                if parent_q == &Quantifier::Exists {
                    state.active_existentials.remove(parent_var);
                } else {
                    state.universal_map.remove(parent_var);
                    if step_var.is_some() {
                        state.active_universals.pop();
                    }
                }
            }
            ok
        }
        (
            FOFFormula::Quantified {
                quantifier: Quantifier::Exists,
                variables,
                formula,
            },
            _,
        ) => {
            for variable in variables {
                state.active_existentials.insert((*variable).to_string());
            }
            let ok = match_skolem_formula(formula, step, state);
            for variable in variables {
                state.active_existentials.remove(*variable);
            }
            ok
        }
        (FOFFormula::Atomic(parent), FOFFormula::Atomic(step)) => {
            match_skolem_atom(parent, step, state)
        }
        (FOFFormula::Negation(parent), FOFFormula::Negation(step)) => {
            match_skolem_formula(parent, step, state)
        }
        (
            FOFFormula::Binary {
                left: parent_left,
                connective: parent_conn,
                right: parent_right,
            },
            FOFFormula::Binary {
                left: step_left,
                connective: step_conn,
                right: step_right,
            },
        ) => {
            parent_conn == step_conn
                && match_skolem_formula(parent_left, step_left, state)
                && match_skolem_formula(parent_right, step_right, state)
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
            if let Some(previous) = state.existential_terms.get(*parent_var) {
                return previous == &format!("{step:?}");
            }
            if let Some(mapped) = state.universal_map.get(*parent_var) {
                return matches!(step, FOFTerm::Variable(step_var) if *step_var == mapped);
            }
            if !state.active_existentials.contains(*parent_var) {
                return matches!(step, FOFTerm::Variable(step_var) if *step_var == *parent_var);
            }
            if let FOFTerm::Function(symbol, args) = step {
                let symbol = symbol.as_str();
                let witness_args = args
                    .iter()
                    .map(|arg| match arg {
                        FOFTerm::Variable(variable) => Some(*variable),
                        _ => None,
                    })
                    .collect::<Option<HashSet<_>>>();
                let expected_args: HashSet<&str> =
                    state.active_universals.iter().map(String::as_str).collect();
                if state.fresh_symbols.contains(symbol)
                    && witness_args.as_ref().is_some_and(|args| {
                        args.len() == expected_args.len()
                            && args.iter().all(|arg| expected_args.contains(arg))
                    })
                {
                    if let Some(owner) = state.witness_owners.get(symbol)
                        && owner != parent_var
                    {
                        return false;
                    }
                    state
                        .witness_owners
                        .insert(symbol.to_string(), (*parent_var).to_string());
                    state.used_symbols.insert(symbol.to_string());
                    state
                        .existential_terms
                        .insert((*parent_var).to_string(), format!("{step:?}"));
                    return true;
                }
                let _ = args;
            }
            false
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

    #[test]
    fn certifies_direct_resolution() {
        let input = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\n\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\n\
                     fof(s, plain, $false, inference(resolution, [status(thm)], [a,b])).";
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
    fn certifies_equality_resolution() {
        let problem = "fof(a, axiom, ~(X = X) | p(a)).\nfof(n, axiom, ~p(a)).";
        let proof = "fof(a, axiom, ~(X = X) | p(a), file('problem.p', a)).\n\
                     fof(n, axiom, ~p(a), file('problem.p', n)).\n\
                     fof(s, plain, p(a), inference(equality_resolution, [status(thm)], [a])).\n\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [s,n])).";
        assert_eq!(check(problem, proof), KernelVerdict::Certified);
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

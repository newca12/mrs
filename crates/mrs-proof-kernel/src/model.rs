//! Independent, deterministic finite model certificate validation for First-Order Logic / EPR.
//!
//! Validates:
//! - Finite domain and non-emptiness ($|D| \ge 1$);
//! - Every constant interpretation;
//! - Every function interpretation, if present;
//! - Every predicate interpretation;
//! - Equality semantics (strict identity);
//! - All original formulas and resolved includes;
//! - Correct conjecture polarity;
//! - Complete ground clause coverage;
//! - Deterministic model digest (SHA-256).
//!
//! The certificate *data* format lives in `mrs-core::model` so a producer can
//! emit one without depending on this crate; the checking methods below are
//! this kernel's own and are reached through [`ModelEvaluation`].

use mrs_tptp::ast::common::{BinaryConnective, DefinedWord};
use mrs_tptp::{
    AnnotatedFormula, CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement, FOFAtomicFormula,
    FOFFormula, FOFStatement, FOFTerm, FormulaRole, Quantifier, TPTPProblem,
};
use std::collections::{BTreeMap, BTreeSet};

pub use mrs_core::model::{EqualitySemantics, FunctionTable, ModelCertificate, PredicateTable};

/// Result of validating a model certificate against a TPTP problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelVerdict {
    /// Model certificate is valid and satisfies all formulas with correct polarity.
    Certified {
        domain_size: usize,
        digest: String,
        formulas_evaluated: usize,
        ground_clauses_evaluated: usize,
    },
    /// The model is invalid, incomplete, or fails to satisfy the problem.
    Rejected(String),
    /// Inconclusive due to unsupported construct or resource limit.
    Inconclusive(String),
}

impl std::fmt::Display for ModelVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Certified {
                domain_size,
                digest,
                formulas_evaluated,
                ground_clauses_evaluated,
            } => write!(
                f,
                "Certified (domain={domain_size}, formulas={formulas_evaluated}, ground_clauses={ground_clauses_evaluated}, digest={digest})"
            ),
            Self::Rejected(reason) => write!(f, "Rejected: {reason}"),
            Self::Inconclusive(reason) => write!(f, "Inconclusive: {reason}"),
        }
    }
}

/// Deterministic first-order model checking.
///
/// Implemented for [`ModelCertificate`] (whose data format lives in
/// `mrs-core::model`) so a producer can emit a certificate without depending
/// on this crate, while the checking methods stay the kernel's own.
pub trait ModelEvaluation {
    /// Validates this model certificate against a problem file on disk.
    fn validate_file(
        &self,
        problem_path: &std::path::Path,
        expected_status: Option<&str>,
    ) -> ModelVerdict;

    /// Row-major table index for the argument tuple `args`.
    fn table_index(&self, args: &[usize]) -> Result<usize, String>;

    /// Evaluates a TPTP term under `env`.
    fn eval_term(&self, term: &FOFTerm<'_>, env: &BTreeMap<String, usize>)
    -> Result<usize, String>;

    /// Evaluates a FOF atom.
    fn eval_fof_atomic(
        &self,
        atom: &FOFAtomicFormula<'_>,
        env: &BTreeMap<String, usize>,
    ) -> Result<bool, String>;

    /// Evaluates a CNF atom.
    fn eval_cnf_atomic(
        &self,
        atom: &CNFAtomicFormula<'_>,
        env: &BTreeMap<String, usize>,
    ) -> Result<bool, String>;

    /// Evaluates a FOF formula.
    fn eval_fof(
        &self,
        formula: &FOFFormula<'_>,
        env: &mut BTreeMap<String, usize>,
    ) -> Result<bool, String>;

    /// Evaluates a CNF clause, returning `(satisfied, ground_clause_count)`.
    fn eval_cnf_clause(&self, clause: &CNFFormula<'_>) -> Result<(bool, usize), String>;

    /// Validates this model certificate against a parsed problem.
    fn validate(&self, problem: &TPTPProblem<'_>, expected_status: Option<&str>) -> ModelVerdict;
}

impl ModelEvaluation for ModelCertificate {
    /// Validates this model certificate against a problem file on disk.
    fn validate_file(
        &self,
        problem_path: &std::path::Path,
        expected_status: Option<&str>,
    ) -> ModelVerdict {
        let problem_text = match std::fs::read_to_string(problem_path) {
            Ok(t) => t,
            Err(e) => return ModelVerdict::Inconclusive(format!("read problem file: {e}")),
        };
        let problem = match mrs_tptp::parse_tptp(&problem_text) {
            Ok(p) => p,
            Err(e) => return ModelVerdict::Inconclusive(format!("parse problem file: {e}")),
        };
        self.validate(&problem, expected_status)
    }

    /// Helper to compute index into a flat row-major table for inputs `[d_0, ..., d_{k-1}]`.
    fn table_index(&self, args: &[usize]) -> Result<usize, String> {
        let mut idx: usize = 0;
        for &arg in args {
            if arg >= self.domain_size {
                return Err(format!(
                    "domain element {arg} out of range (domain size {})",
                    self.domain_size
                ));
            }
            idx = idx
                .checked_mul(self.domain_size)
                .and_then(|value| value.checked_add(arg))
                .ok_or_else(|| "model table index overflow".to_string())?;
        }
        Ok(idx)
    }

    /// Evaluates a term under a variable assignment environment.
    fn eval_term(
        &self,
        term: &FOFTerm<'_>,
        env: &BTreeMap<String, usize>,
    ) -> Result<usize, String> {
        match term {
            FOFTerm::Variable(v) => env
                .get(*v)
                .copied()
                .ok_or_else(|| format!("unbound variable `{v}`")),
            FOFTerm::Function(functor, args) => {
                let name = functor.as_str();
                if args.is_empty() {
                    // Constant
                    self.constants
                        .get(name)
                        .copied()
                        .ok_or_else(|| format!("missing interpretation for constant `{name}`"))
                } else {
                    let ft = self
                        .functions
                        .get(name)
                        .ok_or_else(|| format!("missing interpretation for function `{name}`"))?;
                    if ft.arity != args.len() {
                        return Err(format!(
                            "function `{name}` arity mismatch: expected {}, got {}",
                            ft.arity,
                            args.len()
                        ));
                    }
                    let mut arg_vals = Vec::with_capacity(ft.arity);
                    for arg in args {
                        arg_vals.push(self.eval_term(arg, env)?);
                    }
                    let idx = self.table_index(&arg_vals)?;
                    ft.table
                        .get(idx)
                        .copied()
                        .ok_or_else(|| format!("function table index out of bounds for `{name}`"))
                }
            }
            FOFTerm::DistinctObject(s) => self
                .constants
                .get(*s)
                .copied()
                .ok_or_else(|| format!("missing interpretation for distinct object `{s}`")),
            FOFTerm::DefinedFunction(d, _) => Err(format!(
                "unsupported defined function construct `{:?}`",
                d.0
            )),
            _ => Err("unsupported term shape in model evaluation".to_string()),
        }
    }

    /// Evaluates an atomic FOF formula under a variable assignment environment.
    fn eval_fof_atomic(
        &self,
        atomic: &FOFAtomicFormula<'_>,
        env: &BTreeMap<String, usize>,
    ) -> Result<bool, String> {
        match atomic {
            FOFAtomicFormula::Plain(pred, args) => {
                let name = pred.as_str();
                let pt = self
                    .predicates
                    .get(name)
                    .ok_or_else(|| format!("missing interpretation for predicate `{name}`"))?;
                if pt.arity != args.len() {
                    return Err(format!(
                        "predicate `{name}` arity mismatch: expected {}, got {}",
                        pt.arity,
                        args.len()
                    ));
                }
                let mut arg_vals = Vec::with_capacity(pt.arity);
                for arg in args {
                    arg_vals.push(self.eval_term(arg, env)?);
                }
                let idx = self.table_index(&arg_vals)?;
                pt.table
                    .get(idx)
                    .copied()
                    .ok_or_else(|| format!("predicate table index out of bounds for `{name}`"))
            }
            FOFAtomicFormula::Defined(DefinedWord("equal"), args) if args.len() == 2 => {
                let left_val = self.eval_term(&args[0], env)?;
                let right_val = self.eval_term(&args[1], env)?;
                Ok(match self.equality {
                    EqualitySemantics::StrictIdentity => left_val == right_val,
                })
            }
            FOFAtomicFormula::True => Ok(true),
            FOFAtomicFormula::False => Ok(false),
            _ => Err("unsupported atomic formula in model evaluation".into()),
        }
    }

    /// Evaluates a CNF atomic formula under a variable assignment environment.
    fn eval_cnf_atomic(
        &self,
        atomic: &CNFAtomicFormula<'_>,
        env: &BTreeMap<String, usize>,
    ) -> Result<bool, String> {
        match atomic {
            CNFAtomicFormula::Plain(pred, args) => {
                let name = pred.as_str();
                let pt = self
                    .predicates
                    .get(name)
                    .ok_or_else(|| format!("missing interpretation for predicate `{name}`"))?;
                if pt.arity != args.len() {
                    return Err(format!(
                        "predicate `{name}` arity mismatch: expected {}, got {}",
                        pt.arity,
                        args.len()
                    ));
                }
                let mut arg_vals = Vec::with_capacity(pt.arity);
                for arg in args {
                    arg_vals.push(self.eval_term(arg, env)?);
                }
                let idx = self.table_index(&arg_vals)?;
                pt.table
                    .get(idx)
                    .copied()
                    .ok_or_else(|| format!("predicate table index out of bounds for `{name}`"))
            }
            CNFAtomicFormula::True => Ok(true),
            CNFAtomicFormula::False => Ok(false),
            _ => Err("unsupported CNF atomic formula in model evaluation".into()),
        }
    }

    /// Evaluates a first-order formula under a variable assignment environment.
    fn eval_fof(
        &self,
        formula: &FOFFormula<'_>,
        env: &mut BTreeMap<String, usize>,
    ) -> Result<bool, String> {
        match formula {
            FOFFormula::Atomic(atom) => self.eval_fof_atomic(atom, env),
            FOFFormula::Negation(inner) => {
                let val = self.eval_fof(inner, env)?;
                Ok(!val)
            }
            FOFFormula::Parens(inner) => self.eval_fof(inner, env),
            FOFFormula::Equality(left, right) => {
                let left_val = self.eval_term(left, env)?;
                let right_val = self.eval_term(right, env)?;
                Ok(match self.equality {
                    EqualitySemantics::StrictIdentity => left_val == right_val,
                })
            }
            FOFFormula::Inequality(left, right) => {
                let left_val = self.eval_term(left, env)?;
                let right_val = self.eval_term(right, env)?;
                Ok(match self.equality {
                    EqualitySemantics::StrictIdentity => left_val != right_val,
                })
            }
            FOFFormula::Binary {
                left,
                connective,
                right,
            } => match connective {
                BinaryConnective::And => {
                    let left_val = self.eval_fof(left, env)?;
                    if !left_val {
                        return Ok(false);
                    }
                    self.eval_fof(right, env)
                }
                BinaryConnective::Or => {
                    let left_val = self.eval_fof(left, env)?;
                    if left_val {
                        return Ok(true);
                    }
                    self.eval_fof(right, env)
                }
                BinaryConnective::Iff => {
                    let left_val = self.eval_fof(left, env)?;
                    let right_val = self.eval_fof(right, env)?;
                    Ok(left_val == right_val)
                }
                BinaryConnective::Impl => {
                    let left_val = self.eval_fof(left, env)?;
                    if !left_val {
                        return Ok(true);
                    }
                    self.eval_fof(right, env)
                }
                BinaryConnective::RevImpl => {
                    let right_val = self.eval_fof(right, env)?;
                    if !right_val {
                        return Ok(true);
                    }
                    self.eval_fof(left, env)
                }
                BinaryConnective::Xor => {
                    let left_val = self.eval_fof(left, env)?;
                    let right_val = self.eval_fof(right, env)?;
                    Ok(left_val != right_val)
                }
                BinaryConnective::Nand => {
                    let left_val = self.eval_fof(left, env)?;
                    let right_val = self.eval_fof(right, env)?;
                    Ok(!(left_val && right_val))
                }
                BinaryConnective::Nor => {
                    let left_val = self.eval_fof(left, env)?;
                    let right_val = self.eval_fof(right, env)?;
                    Ok(!(left_val || right_val))
                }
            },
            FOFFormula::Quantified {
                quantifier,
                variables,
                formula,
            } => {
                let vars: Vec<String> = variables.iter().map(|v| v.to_string()).collect();
                model_eval_quantified(self, &vars, 0, *quantifier, formula, env)
            }
        }
    }

    /// Evaluates a CNF clause under all valuations of its free variables.
    fn eval_cnf_clause(&self, clause: &CNFFormula<'_>) -> Result<(bool, usize), String> {
        let mut vars = BTreeSet::new();
        let lits = clause.literals();
        for lit in &lits {
            collect_cnf_literal_vars(lit, &mut vars);
        }
        let var_list: Vec<String> = vars.into_iter().collect();
        let mut env = BTreeMap::new();
        let mut evaluations = 0;
        let satisfied =
            model_eval_cnf_all_valuations(self, &var_list, 0, &lits, &mut env, &mut evaluations)?;
        Ok((satisfied, evaluations))
    }

    /// Fully validates this model certificate against a TPTP problem.
    fn validate(&self, problem: &TPTPProblem<'_>, expected_status: Option<&str>) -> ModelVerdict {
        // 1. Finite domain non-emptiness
        if self.domain_size == 0 {
            return ModelVerdict::Rejected("domain size must be >= 1".into());
        }

        // 2. Equality semantics check
        if self.equality != EqualitySemantics::StrictIdentity {
            return ModelVerdict::Rejected(
                "only strict identity equality semantics is supported".into(),
            );
        }

        // 3. Digest check
        let computed = self.compute_digest();
        if self.digest != computed {
            return ModelVerdict::Rejected(format!(
                "model certificate digest mismatch: expected {}, computed {}",
                self.digest, computed
            ));
        }

        // 4. Function & predicate table sizes and ranges
        for (name, func) in &self.functions {
            let expected_len = match model_table_len(self, func.arity, "function", name) {
                Ok(length) => length,
                Err(verdict) => return verdict,
            };
            if func.table.len() != expected_len {
                return ModelVerdict::Rejected(format!(
                    "function `{name}` table length {} != expected {}",
                    func.table.len(),
                    expected_len
                ));
            }
            for (idx, &val) in func.table.iter().enumerate() {
                if val >= self.domain_size {
                    return ModelVerdict::Rejected(format!(
                        "function `{name}` entry at index {idx} has value {val} >= domain_size {}",
                        self.domain_size
                    ));
                }
            }
        }

        for (name, pred) in &self.predicates {
            let expected_len = match model_table_len(self, pred.arity, "predicate", name) {
                Ok(length) => length,
                Err(verdict) => return verdict,
            };
            if pred.table.len() != expected_len {
                return ModelVerdict::Rejected(format!(
                    "predicate `{name}` table length {} != expected {}",
                    pred.table.len(),
                    expected_len
                ));
            }
        }

        for (name, &val) in &self.constants {
            if val >= self.domain_size {
                return ModelVerdict::Rejected(format!(
                    "constant `{name}` has value {val} >= domain_size {}",
                    self.domain_size
                ));
            }
        }

        // A certificate is complete only when it supplies interpretations for
        // every symbol occurring in the supported input. Extra entries are
        // harmless, but missing entries must not be discovered accidentally
        // halfway through formula evaluation.
        let mut required_constants = BTreeSet::new();
        let mut required_functions = BTreeMap::<String, usize>::new();
        let mut required_predicates = BTreeMap::<String, usize>::new();
        for input in &problem.formulas {
            match input {
                AnnotatedFormula::FOF(formula) => {
                    let FOFStatement::Logical(statement) = &formula.formula else {
                        return ModelVerdict::Inconclusive(
                            "FOF sequents unsupported in model evaluation".into(),
                        );
                    };
                    collect_fof_signatures(
                        statement,
                        &mut required_constants,
                        &mut required_functions,
                        &mut required_predicates,
                    );
                }
                AnnotatedFormula::CNF(formula) => {
                    let CNFStatement::Logical(statement) = &formula.formula;
                    collect_cnf_signatures(
                        statement,
                        &mut required_constants,
                        &mut required_functions,
                        &mut required_predicates,
                    );
                }
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model validation currently supports FOF and CNF dialects".into(),
                    );
                }
            }
        }
        for name in required_constants {
            if !self.constants.contains_key(&name) {
                return ModelVerdict::Rejected(format!(
                    "missing interpretation for constant `{name}`"
                ));
            }
        }
        for (name, arity) in required_functions {
            if name == "$true" || name == "$false" {
                continue;
            }
            let Some(function) = self.functions.get(&name) else {
                return ModelVerdict::Rejected(format!(
                    "missing interpretation for function `{name}`"
                ));
            };
            if function.arity != arity {
                return ModelVerdict::Rejected(format!(
                    "function `{name}` arity mismatch: expected {arity}, got {}",
                    function.arity
                ));
            }
        }
        for (name, arity) in required_predicates {
            let Some(predicate) = self.predicates.get(&name) else {
                return ModelVerdict::Rejected(format!(
                    "missing interpretation for predicate `{name}`"
                ));
            };
            if predicate.arity != arity {
                return ModelVerdict::Rejected(format!(
                    "predicate `{name}` arity mismatch: expected {arity}, got {}",
                    predicate.arity
                ));
            }
        }

        // 5. Evaluate all formulas in problem
        let mut formulas_evaluated = 0;
        let mut ground_clauses_evaluated = 0;
        let mut has_conjecture = false;

        for input in &problem.formulas {
            match input {
                AnnotatedFormula::FOF(fof_annotated) => {
                    formulas_evaluated += 1;
                    let FOFStatement::Logical(ref formula) = fof_annotated.formula else {
                        return ModelVerdict::Inconclusive(
                            "FOF sequents unsupported in model evaluation".into(),
                        );
                    };

                    let mut env = BTreeMap::new();
                    let val = match self.eval_fof(formula, &mut env) {
                        Ok(v) => v,
                        Err(err) => {
                            return ModelVerdict::Rejected(format!(
                                "evaluation failed for FOF statement `{}`: {err}",
                                fof_annotated.name.as_str()
                            ));
                        }
                    };

                    match fof_annotated.role {
                        FormulaRole::Conjecture => {
                            has_conjecture = true;
                            // For CounterSatisfiable, the model must FALSIFY the conjecture!
                            if val {
                                return ModelVerdict::Rejected(format!(
                                    "model satisfies conjecture `{}`; not a counter-model (conjecture polarity violation)",
                                    fof_annotated.name.as_str()
                                ));
                            }
                        }
                        FormulaRole::NegatedConjecture => {
                            has_conjecture = true;
                            // For Satisfiable with negated conjecture, model must satisfy negated conjecture
                            if !val {
                                return ModelVerdict::Rejected(format!(
                                    "model violates negated conjecture `{}`",
                                    fof_annotated.name.as_str()
                                ));
                            }
                        }
                        FormulaRole::Axiom
                        | FormulaRole::Hypothesis
                        | FormulaRole::Definition
                        | FormulaRole::Lemma
                        | FormulaRole::Plain
                            if !val =>
                        {
                            return ModelVerdict::Rejected(format!(
                                "model violates axiom `{}`",
                                fof_annotated.name.as_str()
                            ));
                        }
                        _ => {}
                    }
                }
                AnnotatedFormula::CNF(cnf_annotated) => {
                    formulas_evaluated += 1;
                    let CNFStatement::Logical(ref clause) = cnf_annotated.formula;
                    let (satisfied, count) = match self.eval_cnf_clause(clause) {
                        Ok(res) => res,
                        Err(err) => {
                            return ModelVerdict::Rejected(format!(
                                "evaluation failed for CNF statement `{}`: {err}",
                                cnf_annotated.name.as_str()
                            ));
                        }
                    };
                    ground_clauses_evaluated += count;

                    match cnf_annotated.role {
                        FormulaRole::Conjecture => {
                            has_conjecture = true;
                            if satisfied {
                                return ModelVerdict::Rejected(format!(
                                    "model satisfies conjecture clause `{}`; not a counter-model",
                                    cnf_annotated.name.as_str()
                                ));
                            }
                        }
                        FormulaRole::NegatedConjecture => {
                            has_conjecture = true;
                            if !satisfied {
                                return ModelVerdict::Rejected(format!(
                                    "model violates negated conjecture clause `{}`",
                                    cnf_annotated.name.as_str()
                                ));
                            }
                        }
                        FormulaRole::Axiom
                        | FormulaRole::Hypothesis
                        | FormulaRole::Definition
                        | FormulaRole::Lemma
                        | FormulaRole::Plain
                            if !satisfied =>
                        {
                            return ModelVerdict::Rejected(format!(
                                "model violates axiom clause `{}`",
                                cnf_annotated.name.as_str()
                            ));
                        }
                        _ => {}
                    }
                }
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model validation currently supports FOF and CNF dialects".into(),
                    );
                }
            }
        }

        // 6. Polarity vs expected status
        if let Some(status) = expected_status
            && status == "CounterSatisfiable"
            && !has_conjecture
        {
            return ModelVerdict::Rejected(
                "expected CounterSatisfiable, but problem has no conjecture to falsify".into(),
            );
        }

        ModelVerdict::Certified {
            domain_size: self.domain_size,
            digest: self.digest.clone(),
            formulas_evaluated,
            ground_clauses_evaluated,
        }
    }
}

fn collect_cnf_literal_vars(lit: &CNFLiteral<'_>, vars: &mut BTreeSet<String>) {
    match lit {
        CNFLiteral::Positive(atom) | CNFLiteral::Negative(atom) => {
            if let CNFAtomicFormula::Plain(_, args) = atom {
                for arg in args {
                    collect_term_vars(arg, vars);
                }
            }
        }
        CNFLiteral::Equality(left, right) | CNFLiteral::Inequality(left, right) => {
            collect_term_vars(left, vars);
            collect_term_vars(right, vars);
        }
    }
}

fn collect_fof_signatures(
    formula: &FOFFormula<'_>,
    constants: &mut BTreeSet<String>,
    functions: &mut BTreeMap<String, usize>,
    predicates: &mut BTreeMap<String, usize>,
) {
    match formula {
        FOFFormula::Atomic(atom) => match atom {
            FOFAtomicFormula::Plain(name, args) => {
                predicates.insert(name.as_str().to_string(), args.len());
                for arg in args {
                    collect_fof_term_signatures(arg, constants, functions);
                }
            }
            FOFAtomicFormula::Defined(_, args) | FOFAtomicFormula::System(_, args) => {
                for arg in args {
                    collect_fof_term_signatures(arg, constants, functions);
                }
            }
            FOFAtomicFormula::True | FOFAtomicFormula::False => {}
        },
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
            collect_fof_signatures(inner, constants, functions, predicates)
        }
        FOFFormula::Quantified { formula, .. } => {
            collect_fof_signatures(formula, constants, functions, predicates)
        }
        FOFFormula::Binary { left, right, .. } => {
            collect_fof_signatures(left, constants, functions, predicates);
            collect_fof_signatures(right, constants, functions, predicates);
        }
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
            collect_fof_term_signatures(left, constants, functions);
            collect_fof_term_signatures(right, constants, functions);
        }
    }
}

fn collect_fof_term_signatures(
    term: &FOFTerm<'_>,
    constants: &mut BTreeSet<String>,
    functions: &mut BTreeMap<String, usize>,
) {
    match term {
        FOFTerm::Function(name, args) => {
            let name = name.as_str().to_string();
            if args.is_empty() {
                constants.insert(name);
            } else {
                functions.insert(name, args.len());
                for arg in args {
                    collect_fof_term_signatures(arg, constants, functions);
                }
            }
        }
        FOFTerm::DistinctObject(name) => {
            constants.insert(name.to_string());
        }
        FOFTerm::DefinedFunction(_, args) | FOFTerm::SystemFunction(_, args) => {
            for arg in args {
                collect_fof_term_signatures(arg, constants, functions);
            }
        }
        FOFTerm::Variable(_) | FOFTerm::Number(_) => {}
    }
}

fn collect_cnf_signatures(
    formula: &CNFFormula<'_>,
    constants: &mut BTreeSet<String>,
    functions: &mut BTreeMap<String, usize>,
    predicates: &mut BTreeMap<String, usize>,
) {
    for literal in formula.literals() {
        match literal {
            CNFLiteral::Positive(CNFAtomicFormula::Plain(name, args))
            | CNFLiteral::Negative(CNFAtomicFormula::Plain(name, args)) => {
                predicates.insert(name.as_str().to_string(), args.len());
                for arg in args {
                    collect_fof_term_signatures(arg, constants, functions);
                }
            }
            CNFLiteral::Positive(CNFAtomicFormula::Defined(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::Defined(_, args))
            | CNFLiteral::Positive(CNFAtomicFormula::System(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::System(_, args)) => {
                for arg in args {
                    collect_fof_term_signatures(arg, constants, functions);
                }
            }
            CNFLiteral::Positive(CNFAtomicFormula::True)
            | CNFLiteral::Positive(CNFAtomicFormula::False)
            | CNFLiteral::Negative(CNFAtomicFormula::True)
            | CNFLiteral::Negative(CNFAtomicFormula::False) => {}
            CNFLiteral::Equality(left, right) | CNFLiteral::Inequality(left, right) => {
                collect_fof_term_signatures(left, constants, functions);
                collect_fof_term_signatures(right, constants, functions);
            }
        }
    }
}

fn collect_term_vars(term: &FOFTerm<'_>, vars: &mut BTreeSet<String>) {
    match term {
        FOFTerm::Variable(v) => {
            vars.insert(v.to_string());
        }
        FOFTerm::Function(_, args) => {
            for arg in args {
                collect_term_vars(arg, vars);
            }
        }
        _ => {}
    }
}

fn model_eval_cnf_all_valuations(
    cert: &ModelCertificate,
    vars: &[String],
    idx: usize,
    literals: &[&CNFLiteral<'_>],
    env: &mut BTreeMap<String, usize>,
    count: &mut usize,
) -> Result<bool, String> {
    if idx == vars.len() {
        *count += 1;
        // Check if any literal is satisfied under env
        for lit in literals {
            let sat = match lit {
                CNFLiteral::Positive(atom) => cert.eval_cnf_atomic(atom, env)?,
                CNFLiteral::Negative(atom) => !cert.eval_cnf_atomic(atom, env)?,
                CNFLiteral::Equality(left, right) => {
                    let l_val = cert.eval_term(left, env)?;
                    let r_val = cert.eval_term(right, env)?;
                    l_val == r_val
                }
                CNFLiteral::Inequality(left, right) => {
                    let l_val = cert.eval_term(left, env)?;
                    let r_val = cert.eval_term(right, env)?;
                    l_val != r_val
                }
            };
            if sat {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    let var = &vars[idx];
    for d in 0..cert.domain_size {
        env.insert(var.clone(), d);
        let ok = model_eval_cnf_all_valuations(cert, vars, idx + 1, literals, env, count)?;
        env.remove(var);
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

fn model_eval_quantified(
    cert: &ModelCertificate,
    vars: &[String],
    idx: usize,
    quantifier: Quantifier,
    formula: &FOFFormula<'_>,
    env: &mut BTreeMap<String, usize>,
) -> Result<bool, String> {
    if idx == vars.len() {
        return cert.eval_fof(formula, env);
    }

    let var = &vars[idx];
    match quantifier {
        Quantifier::Forall => {
            for d in 0..cert.domain_size {
                env.insert(var.clone(), d);
                let res = model_eval_quantified(cert, vars, idx + 1, quantifier, formula, env)?;
                env.remove(var);
                if !res {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Quantifier::Exists => {
            for d in 0..cert.domain_size {
                env.insert(var.clone(), d);
                let res = model_eval_quantified(cert, vars, idx + 1, quantifier, formula, env)?;
                env.remove(var);
                if res {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn model_table_len(
    cert: &ModelCertificate,
    arity: usize,
    kind: &str,
    name: &str,
) -> Result<usize, ModelVerdict> {
    (0..arity).try_fold(1usize, |length, _| {
        length.checked_mul(cert.domain_size).ok_or_else(|| {
            ModelVerdict::Inconclusive(format!(
                "{kind} `{name}` table size overflows the host usize"
            ))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    #[test]
    fn validates_pure_relational_satisfiable_model() {
        let problem_text = "\
fof(ax1, axiom, p(a)).
fof(ax2, axiom, ~p(b)).
";
        let problem = parse_tptp(problem_text).unwrap();

        let mut constants = BTreeMap::new();
        constants.insert("a".to_string(), 0);
        constants.insert("b".to_string(), 1);

        let mut predicates = BTreeMap::new();
        // Domain {0, 1}, p(0) = true, p(1) = false
        predicates.insert(
            "p".to_string(),
            PredicateTable {
                arity: 1,
                table: vec![true, false],
            },
        );

        let mut cert = ModelCertificate {
            domain_size: 2,
            constants,
            functions: BTreeMap::new(),
            predicates,
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();

        let verdict = cert.validate(&problem, Some("Satisfiable"));
        assert!(
            matches!(
                verdict,
                ModelVerdict::Certified {
                    domain_size: 2,
                    formulas_evaluated: 2,
                    ..
                }
            ),
            "expected certified, got {verdict:?}"
        );
    }

    #[test]
    fn validates_counter_model_with_conjecture() {
        // Axiom: p(a). Conjecture: p(b).
        // Model with D={0,1}, a=0, b=1, p(0)=true, p(1)=false.
        // Falsifies conjecture p(b)!
        let problem_text = "\
fof(ax, axiom, p(a)).
fof(conj, conjecture, p(b)).
";
        let problem = parse_tptp(problem_text).unwrap();

        let mut constants = BTreeMap::new();
        constants.insert("a".to_string(), 0);
        constants.insert("b".to_string(), 1);

        let mut predicates = BTreeMap::new();
        predicates.insert(
            "p".to_string(),
            PredicateTable {
                arity: 1,
                table: vec![true, false],
            },
        );

        let mut cert = ModelCertificate {
            domain_size: 2,
            constants,
            functions: BTreeMap::new(),
            predicates,
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();

        let verdict = cert.validate(&problem, Some("CounterSatisfiable"));
        assert!(matches!(verdict, ModelVerdict::Certified { .. }));
    }

    #[test]
    fn rejects_conjecture_satisfaction_for_counter_model() {
        // Axiom: p(a). Conjecture: p(a).
        // Model satisfies p(a), so it does NOT falsify the conjecture!
        let problem_text = "\
fof(ax, axiom, p(a)).
fof(conj, conjecture, p(a)).
";
        let problem = parse_tptp(problem_text).unwrap();

        let mut constants = BTreeMap::new();
        constants.insert("a".to_string(), 0);

        let mut predicates = BTreeMap::new();
        predicates.insert(
            "p".to_string(),
            PredicateTable {
                arity: 1,
                table: vec![true],
            },
        );

        let mut cert = ModelCertificate {
            domain_size: 1,
            constants,
            functions: BTreeMap::new(),
            predicates,
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();

        let verdict = cert.validate(&problem, Some("CounterSatisfiable"));
        assert!(
            matches!(verdict, ModelVerdict::Rejected(ref r) if r.contains("not a counter-model")),
            "expected rejected counter-model, got {verdict:?}"
        );
    }

    #[test]
    fn rejects_tampered_model_digest() {
        let problem_text = "fof(ax, axiom, p(a)).";
        let problem = parse_tptp(problem_text).unwrap();

        let mut constants = BTreeMap::new();
        constants.insert("a".to_string(), 0);
        let mut predicates = BTreeMap::new();
        predicates.insert(
            "p".to_string(),
            PredicateTable {
                arity: 1,
                table: vec![true],
            },
        );

        let cert = ModelCertificate {
            domain_size: 1,
            constants,
            functions: BTreeMap::new(),
            predicates,
            equality: EqualitySemantics::StrictIdentity,
            digest: "deadbeefbadcafe00000".to_string(),
        };

        let verdict = cert.validate(&problem, None);
        assert!(matches!(verdict, ModelVerdict::Rejected(ref r) if r.contains("digest mismatch")));
    }

    #[test]
    fn rejects_overflowing_model_table_shape_inconclusively() {
        let problem = parse_tptp("fof(ax, axiom, p(f(a))).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: usize::MAX,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: [(
                "f".to_string(),
                FunctionTable {
                    arity: 2,
                    table: Vec::new(),
                },
            )]
            .into_iter()
            .collect(),
            predicates: [(
                "p".to_string(),
                PredicateTable {
                    arity: 1,
                    table: vec![true],
                },
            )]
            .into_iter()
            .collect(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();
        assert!(matches!(
            cert.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Inconclusive(reason) if reason.contains("overflows")
        ));
    }

    #[test]
    fn validates_quantified_cnf_clauses() {
        // CNF: p(X) | q(X)
        let problem_text = "cnf(c1, axiom, p(X) | q(X)).";
        let problem = parse_tptp(problem_text).unwrap();

        let mut predicates = BTreeMap::new();
        // Domain {0, 1}: p(0)=true, p(1)=false, q(0)=false, q(1)=true
        predicates.insert(
            "p".to_string(),
            PredicateTable {
                arity: 1,
                table: vec![true, false],
            },
        );
        predicates.insert(
            "q".to_string(),
            PredicateTable {
                arity: 1,
                table: vec![false, true],
            },
        );

        let mut cert = ModelCertificate {
            domain_size: 2,
            constants: BTreeMap::new(),
            functions: BTreeMap::new(),
            predicates,
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();

        let verdict = cert.validate(&problem, None);
        assert!(matches!(
            verdict,
            ModelVerdict::Certified {
                ground_clauses_evaluated: 2,
                ..
            }
        ));
    }
}

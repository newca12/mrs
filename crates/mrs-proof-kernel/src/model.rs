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

/// Bound the total dense interpretation tables processed or constructed by a
/// model certificate. This covers function (usize) and predicate (bool)
/// entries across all symbols, rather than allowing a single table or many
/// small tables to consume unbounded memory.
pub use mrs_core::model::MAX_MODEL_TABLE_ENTRIES;
const MAX_MODEL_EVALUATION_WORK: u64 = 10_000_000;
const MAX_MODEL_EVALUATION_DEPTH: usize = 256;
const MAX_MODEL_METADATA_BYTES: usize = 16 * 1024 * 1024;
const MAX_MODEL_PROBLEM_BYTES: usize = 16 * 1024 * 1024;

fn model_formula_role(role: FormulaRole) -> bool {
    role.is_premise()
        || role.is_goal()
        || role == FormulaRole::Plain
        || role == FormulaRole::NegatedConjecture
}
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

    #[doc(hidden)]
    fn eval_cnf_clause_with_limit(
        &self,
        clause: &CNFFormula<'_>,
        remaining: usize,
    ) -> Result<(bool, usize), String>;

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
        let file_bytes = match std::fs::metadata(problem_path) {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                return ModelVerdict::Inconclusive(format!("stat problem file: {error}"));
            }
        };
        if file_bytes > MAX_MODEL_PROBLEM_BYTES as u64 {
            return ModelVerdict::Inconclusive(
                "problem file exceeds strict model-validation byte limit".into(),
            );
        }
        let problem_text = match std::fs::File::open(problem_path).and_then(|file| {
            use std::io::Read as _;
            let mut bounded = file.take((MAX_MODEL_PROBLEM_BYTES + 1) as u64);
            let mut text = String::new();
            bounded.read_to_string(&mut text).map(|_| text)
        }) {
            Ok(text) if text.len() <= MAX_MODEL_PROBLEM_BYTES => text,
            Ok(_) => {
                return ModelVerdict::Inconclusive(
                    "problem file exceeds strict model-validation byte limit".into(),
                );
            }
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
                .get(&format!("\"{s}\""))
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
                model_eval_quantified(
                    self,
                    &vars,
                    0,
                    *quantifier,
                    formula,
                    env,
                    MAX_MODEL_EVALUATION_WORK,
                    0,
                )
            }
        }
    }

    /// Evaluates a CNF clause under all valuations of its free variables.
    fn eval_cnf_clause(&self, clause: &CNFFormula<'_>) -> Result<(bool, usize), String> {
        self.eval_cnf_clause_with_limit(clause, MAX_MODEL_EVALUATION_WORK as usize)
    }

    #[doc(hidden)]
    fn eval_cnf_clause_with_limit(
        &self,
        clause: &CNFFormula<'_>,
        remaining: usize,
    ) -> Result<(bool, usize), String> {
        let mut vars = BTreeSet::new();
        let lits = clause.literals();
        for lit in &lits {
            collect_cnf_literal_vars(lit, &mut vars);
        }
        let var_list: Vec<String> = vars.into_iter().collect();
        let mut env = BTreeMap::new();
        let mut evaluations = 0;
        let mut remaining = u64::try_from(remaining).unwrap_or(u64::MAX);
        if var_list.len() > MAX_MODEL_EVALUATION_DEPTH {
            return Err("model evaluation work/depth limit exceeded".into());
        }
        let satisfied = model_eval_cnf_all_valuations(
            self,
            &var_list,
            0,
            &lits,
            &mut env,
            &mut evaluations,
            &mut remaining,
            0,
        )?;
        Ok((satisfied, evaluations))
    }

    /// Fully validates this model certificate against a TPTP problem.
    fn validate(&self, problem: &TPTPProblem<'_>, expected_status: Option<&str>) -> ModelVerdict {
        if problem
            .formulas
            .iter()
            .map(|formula| formula.name().len())
            .try_fold(0usize, usize::checked_add)
            .is_none_or(|bytes| bytes > MAX_MODEL_METADATA_BYTES)
        {
            return ModelVerdict::Inconclusive(
                "problem formula metadata exceeds strict byte limit".into(),
            );
        }
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

        if self.constants.len() > MAX_MODEL_TABLE_ENTRIES
            || self.functions.len() > MAX_MODEL_TABLE_ENTRIES
            || self.predicates.len() > MAX_MODEL_TABLE_ENTRIES
        {
            return ModelVerdict::Inconclusive("model symbol count exceeds strict limit".into());
        }
        let metadata_bytes = self
            .constants
            .keys()
            .chain(self.functions.keys())
            .chain(self.predicates.keys())
            .try_fold(self.digest.len(), |bytes, name| {
                bytes.checked_add(name.len())
            });
        if metadata_bytes.is_none_or(|bytes| bytes > MAX_MODEL_METADATA_BYTES) {
            return ModelVerdict::Inconclusive(
                "model certificate metadata exceeds strict byte limit".into(),
            );
        }
        if let Some(input) = problem
            .formulas
            .iter()
            .find(|input| input.role() == FormulaRole::Unknown)
        {
            return ModelVerdict::Inconclusive(format!(
                "model validation does not support unknown formula role in `{}`",
                input.name()
            ));
        }

        // 3. Function & predicate table sizes and ranges. Check aggregate
        // size before digesting/scanning entries: the digest implementation
        // formats table contents, so this also bounds its temporary memory.
        let mut total_table_entries = 0usize;
        for (name, func) in &self.functions {
            let expected_len = match model_table_len(self, func.arity, "function", name) {
                Ok(length) => length,
                Err(verdict) => return verdict,
            };
            total_table_entries = match total_table_entries.checked_add(expected_len) {
                Some(total) if total <= MAX_MODEL_TABLE_ENTRIES => total,
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model tables exceed strict entry limit".into(),
                    );
                }
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
            total_table_entries = match total_table_entries.checked_add(expected_len) {
                Some(total) if total <= MAX_MODEL_TABLE_ENTRIES => total,
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model tables exceed strict entry limit".into(),
                    );
                }
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
        // every symbol occurring in premise/goal formulas. Extra entries are
        // harmless, but missing entries must not be discovered accidentally
        // halfway through formula evaluation. Type/logic/interpretation
        // metadata records are not logical axioms constraining the model.
        let mut required_constants = BTreeSet::new();
        let mut required_functions = BTreeMap::<String, usize>::new();
        let mut required_predicates = BTreeMap::<String, usize>::new();
        let mut required_distinct_objects = BTreeSet::new();
        let mut signature_conflict = false;
        for (name, function) in &self.functions {
            if function.arity == 0
                && (function.table.len() != 1 || function.table[0] >= self.domain_size)
            {
                return ModelVerdict::Rejected(format!(
                    "constant function `{name}` has an invalid interpretation"
                ));
            }
        }
        for input in &problem.formulas {
            if !model_formula_role(input.role()) {
                continue;
            }
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
                        &mut required_distinct_objects,
                        &mut signature_conflict,
                    );
                }
                AnnotatedFormula::CNF(formula) => {
                    let CNFStatement::Logical(statement) = &formula.formula;
                    collect_cnf_signatures(
                        statement,
                        &mut required_constants,
                        &mut required_functions,
                        &mut required_predicates,
                        &mut required_distinct_objects,
                        &mut signature_conflict,
                    );
                }
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model validation currently supports FOF and CNF dialects".into(),
                    );
                }
            }
        }
        if signature_conflict {
            return ModelVerdict::Rejected(
                "problem contains inconsistent symbol arities or kinds".into(),
            );
        }
        if required_predicates
            .keys()
            .any(|name| required_constants.contains(name) || required_functions.contains_key(name))
        {
            return ModelVerdict::Rejected(
                "problem reuses a symbol as both a term and predicate".into(),
            );
        }
        if required_predicates.keys().any(|name| {
            required_constants.contains(name)
                || required_functions.contains_key(name)
                || required_distinct_objects.contains(name)
        }) {
            return ModelVerdict::Rejected(
                "problem reuses a symbol as both a term and predicate".into(),
            );
        }
        for name in &required_distinct_objects {
            if required_functions.contains_key(name) {
                return ModelVerdict::Rejected(format!(
                    "distinct object `{name}` conflicts with function symbol"
                ));
            }
            required_constants.insert(name.clone());
        }
        for name in required_constants {
            if !self.constants.contains_key(&name) {
                return ModelVerdict::Rejected(format!(
                    "missing interpretation for constant `{name}`"
                ));
            }
        }
        for (name, arity) in required_functions {
            if arity == 0 || name == "$true" || name == "$false" {
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
        let mut distinct_values = BTreeSet::new();
        for name in required_distinct_objects {
            let Some(value) = self.constants.get(&name) else {
                return ModelVerdict::Rejected(format!(
                    "missing interpretation for distinct object {name}"
                ));
            };
            if !distinct_values.insert(*value) {
                return ModelVerdict::Rejected(
                    "distinct objects must have pairwise distinct interpretations".into(),
                );
            }
        }

        let mut work = 0u64;
        for input in &problem.formulas {
            if !model_formula_role(input.role()) {
                continue;
            }
            let item_work = match input {
                AnnotatedFormula::FOF(formula) => {
                    let FOFStatement::Logical(statement) = &formula.formula else {
                        return ModelVerdict::Inconclusive(
                            "FOF sequents unsupported in model evaluation".into(),
                        );
                    };
                    let mut free_vars = BTreeSet::new();
                    collect_fof_free_vars(statement, &BTreeSet::new(), &mut free_vars);
                    let free_assignment_count = (0..free_vars.len()).try_fold(1u64, |count, _| {
                        count.checked_mul(u64::try_from(self.domain_size).ok()?)
                    });
                    let item_work = fof_model_work(statement, self.domain_size, 0)
                        .and_then(|item_work| item_work.checked_mul(free_assignment_count?));
                    match item_work {
                        Some(work) => work,
                        None => {
                            return ModelVerdict::Inconclusive(
                                "model evaluation exceeds strict work/depth limit".into(),
                            );
                        }
                    }
                }
                AnnotatedFormula::CNF(formula) => {
                    let CNFStatement::Logical(statement) = &formula.formula;
                    match cnf_model_work(statement, self.domain_size) {
                        Some(work) => work,
                        None => {
                            return ModelVerdict::Inconclusive(
                                "model evaluation exceeds strict work limit".into(),
                            );
                        }
                    }
                }
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model validation currently supports FOF and CNF premise/goal formulas"
                            .into(),
                    );
                }
            };
            if item_work == u64::MAX || item_work > MAX_MODEL_EVALUATION_WORK {
                return ModelVerdict::Inconclusive(
                    "model evaluation exceeds strict work limit".into(),
                );
            }
            work = match work.checked_add(item_work) {
                Some(total) if total <= MAX_MODEL_EVALUATION_WORK => total,
                _ => {
                    return ModelVerdict::Inconclusive(
                        "model evaluation exceeds strict work limit".into(),
                    );
                }
            };
        }

        // Table validation above bounds the digest's work and temporary
        // formatting memory.
        let computed = self.compute_digest();
        if self.digest != computed {
            return ModelVerdict::Rejected(format!(
                "model certificate digest mismatch: expected {}, computed {}",
                self.digest, computed
            ));
        }

        // 5. Evaluate all formulas in problem
        let mut formulas_evaluated = 0;
        let mut ground_clauses_evaluated = 0;
        // Whether the *status* depends on falsifying something, i.e. whether
        // the input carries a `conjecture`-role formula. A `negated_conjecture`
        // formula is a premise of the refutation (the prover's clause set
        // already has the conjecture negated), so a model of it is a
        // `Satisfiable` answer, not a counter-model. The per-formula polarity
        // checks below enforce both cases separately; this flag only drives
        // the status cross-check in step 6.
        for input in &problem.formulas {
            if !model_formula_role(input.role()) {
                continue;
            }
            match input {
                AnnotatedFormula::FOF(fof_annotated) => {
                    formulas_evaluated += 1;
                    let FOFStatement::Logical(ref formula) = fof_annotated.formula else {
                        return ModelVerdict::Inconclusive(
                            "FOF sequents unsupported in model evaluation".into(),
                        );
                    };

                    let mut env = BTreeMap::new();
                    let mut free_vars = BTreeSet::new();
                    collect_fof_free_vars(formula, &BTreeSet::new(), &mut free_vars);
                    let free_vars: Vec<String> = free_vars.into_iter().collect();
                    let val = match model_eval_quantified(
                        self,
                        &free_vars,
                        0,
                        Quantifier::Forall,
                        formula,
                        &mut env,
                        MAX_MODEL_EVALUATION_WORK,
                        0,
                    ) {
                        Ok(v) => v,
                        Err(err) => {
                            if err.contains("work/depth limit") {
                                return ModelVerdict::Inconclusive(format!(
                                    "model evaluation limited for FOF statement `{}`: {err}",
                                    fof_annotated.name.as_str()
                                ));
                            }
                            return if err.contains("unsupported") {
                                ModelVerdict::Inconclusive(format!(
                                    "unsupported evaluation for FOF statement `{}`: {err}",
                                    fof_annotated.name.as_str()
                                ))
                            } else {
                                ModelVerdict::Rejected(format!(
                                    "evaluation failed for FOF statement `{}`: {err}",
                                    fof_annotated.name.as_str()
                                ))
                            };
                        }
                    };

                    match fof_annotated.role {
                        FormulaRole::Conjecture => {
                            // For CounterSatisfiable, the model must FALSIFY the conjecture!
                            if val {
                                return ModelVerdict::Rejected(format!(
                                    "model satisfies conjecture `{}`; not a counter-model (conjecture polarity violation)",
                                    fof_annotated.name.as_str()
                                ));
                            }
                        }
                        FormulaRole::NegatedConjecture => {
                            // A direct problem stated with negated-conjecture
                            // formulas requires the model to satisfy them.
                            if !val {
                                return ModelVerdict::Rejected(format!(
                                    "model violates negated conjecture `{}`",
                                    fof_annotated.name.as_str()
                                ));
                            }
                        }
                        role if (role.is_premise() || role == FormulaRole::Plain) && !val => {
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
                    let remaining = MAX_MODEL_EVALUATION_WORK
                        .saturating_sub(ground_clauses_evaluated as u64)
                        .min(usize::MAX as u64) as usize;
                    let (satisfied, count) =
                        match self.eval_cnf_clause_with_limit(clause, remaining) {
                            Ok(res) => res,
                            Err(err) => {
                                if err.contains("work limit") {
                                    return ModelVerdict::Inconclusive(format!(
                                        "model evaluation limited for CNF statement `{}`: {err}",
                                        cnf_annotated.name.as_str()
                                    ));
                                }
                                return if err.contains("unsupported") {
                                    ModelVerdict::Inconclusive(format!(
                                        "unsupported evaluation for CNF statement `{}`: {err}",
                                        cnf_annotated.name.as_str()
                                    ))
                                } else {
                                    ModelVerdict::Rejected(format!(
                                        "evaluation failed for CNF statement `{}`: {err}",
                                        cnf_annotated.name.as_str()
                                    ))
                                };
                            }
                        };
                    ground_clauses_evaluated += count;

                    match cnf_annotated.role {
                        FormulaRole::Conjecture => {
                            if satisfied {
                                return ModelVerdict::Rejected(format!(
                                    "model satisfies conjecture clause `{}`; not a counter-model",
                                    cnf_annotated.name.as_str()
                                ));
                            }
                        }
                        FormulaRole::NegatedConjecture => {
                            if !satisfied {
                                return ModelVerdict::Rejected(format!(
                                    "model violates negated conjecture clause `{}`",
                                    cnf_annotated.name.as_str()
                                ));
                            }
                        }
                        role if (role.is_premise() || role == FormulaRole::Plain) && !satisfied => {
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
        //
        // `Satisfiable` is a model of the input as given, so it is only
        // coherent when there is no `conjecture`-role formula to falsify.
        // `CounterSatisfiable` is a model of the axioms plus the negated
        // conjecture, so it requires one. A problem supplied directly as
        // `negated_conjecture` clauses has no conjecture role and answers
        // `Satisfiable`; the CASC EPR divisions are full of that shape.
        if let Some(status) = expected_status {
            let has_explicit_conjecture = problem
                .formulas
                .iter()
                .any(|input| input.role() == FormulaRole::Conjecture);
            match status {
                "Satisfiable" if has_explicit_conjecture => {
                    return ModelVerdict::Rejected(
                        "expected Satisfiable, but the problem contains a conjecture".into(),
                    );
                }
                "CounterSatisfiable" if !has_explicit_conjecture => {
                    return ModelVerdict::Rejected(
                        "expected CounterSatisfiable, but problem has no conjecture to falsify"
                            .into(),
                    );
                }
                "Satisfiable" | "CounterSatisfiable" => {}
                other => {
                    return ModelVerdict::Inconclusive(format!(
                        "model certificates cannot validate expected status `{other}`"
                    ));
                }
            }
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
            let args: Option<&[_]> = match atom {
                CNFAtomicFormula::Plain(_, args)
                | CNFAtomicFormula::Defined(_, args)
                | CNFAtomicFormula::System(_, args) => Some(args.as_slice()),
                CNFAtomicFormula::True | CNFAtomicFormula::False => None,
            };
            if let Some(args) = args {
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
    distinct_objects: &mut BTreeSet<String>,
    signature_conflict: &mut bool,
) {
    match formula {
        FOFFormula::Atomic(atom) => match atom {
            FOFAtomicFormula::Plain(name, args) => {
                insert_model_signature(predicates, name.as_str(), args.len(), signature_conflict);
                for arg in args {
                    collect_fof_term_signatures(
                        arg,
                        constants,
                        functions,
                        distinct_objects,
                        signature_conflict,
                    );
                }
            }
            FOFAtomicFormula::Defined(_, args) | FOFAtomicFormula::System(_, args) => {
                for arg in args {
                    collect_fof_term_signatures(
                        arg,
                        constants,
                        functions,
                        distinct_objects,
                        signature_conflict,
                    );
                }
            }
            FOFAtomicFormula::True | FOFAtomicFormula::False => {}
        },
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => collect_fof_signatures(
            inner,
            constants,
            functions,
            predicates,
            distinct_objects,
            signature_conflict,
        ),
        FOFFormula::Quantified { formula, .. } => collect_fof_signatures(
            formula,
            constants,
            functions,
            predicates,
            distinct_objects,
            signature_conflict,
        ),
        FOFFormula::Binary { left, right, .. } => {
            collect_fof_signatures(
                left,
                constants,
                functions,
                predicates,
                distinct_objects,
                signature_conflict,
            );
            collect_fof_signatures(
                right,
                constants,
                functions,
                predicates,
                distinct_objects,
                signature_conflict,
            );
        }
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
            collect_fof_term_signatures(
                left,
                constants,
                functions,
                distinct_objects,
                signature_conflict,
            );
            collect_fof_term_signatures(
                right,
                constants,
                functions,
                distinct_objects,
                signature_conflict,
            );
        }
    }
}

fn insert_model_signature(
    signatures: &mut BTreeMap<String, usize>,
    name: &str,
    arity: usize,
    conflict: &mut bool,
) {
    if let Some(previous) = signatures.insert(name.to_string(), arity)
        && previous != arity
    {
        *conflict = true;
    }
}

fn collect_fof_term_signatures(
    term: &FOFTerm<'_>,
    constants: &mut BTreeSet<String>,
    functions: &mut BTreeMap<String, usize>,
    distinct_objects: &mut BTreeSet<String>,
    signature_conflict: &mut bool,
) {
    match term {
        FOFTerm::Function(name, args) => {
            let name = name.as_str().to_string();
            if args.is_empty() {
                if functions.contains_key(&name) || distinct_objects.contains(&name) {
                    *signature_conflict = true;
                }
                constants.insert(name);
            } else {
                if let Some(previous) = functions.insert(name.clone(), args.len())
                    && previous != args.len()
                {
                    *signature_conflict = true;
                }
                if constants.contains(&name) || distinct_objects.contains(&name) {
                    *signature_conflict = true;
                }
                for arg in args {
                    collect_fof_term_signatures(
                        arg,
                        constants,
                        functions,
                        distinct_objects,
                        signature_conflict,
                    );
                }
            }
        }
        FOFTerm::DistinctObject(name) => {
            let name = format!("\"{name}\"");
            if functions.contains_key(&name) || constants.contains(&name) {
                *signature_conflict = true;
            }
            constants.insert(name.clone());
            distinct_objects.insert(name);
        }
        FOFTerm::DefinedFunction(_, args) | FOFTerm::SystemFunction(_, args) => {
            for arg in args {
                collect_fof_term_signatures(
                    arg,
                    constants,
                    functions,
                    distinct_objects,
                    signature_conflict,
                );
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
    distinct_objects: &mut BTreeSet<String>,
    signature_conflict: &mut bool,
) {
    for literal in formula.literals() {
        match literal {
            CNFLiteral::Positive(CNFAtomicFormula::Plain(name, args))
            | CNFLiteral::Negative(CNFAtomicFormula::Plain(name, args)) => {
                insert_model_signature(predicates, name.as_str(), args.len(), signature_conflict);
                for arg in args {
                    collect_fof_term_signatures(
                        arg,
                        constants,
                        functions,
                        distinct_objects,
                        signature_conflict,
                    );
                }
            }
            CNFLiteral::Positive(CNFAtomicFormula::Defined(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::Defined(_, args))
            | CNFLiteral::Positive(CNFAtomicFormula::System(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::System(_, args)) => {
                for arg in args {
                    collect_fof_term_signatures(
                        arg,
                        constants,
                        functions,
                        distinct_objects,
                        signature_conflict,
                    );
                }
            }
            CNFLiteral::Positive(CNFAtomicFormula::True)
            | CNFLiteral::Positive(CNFAtomicFormula::False)
            | CNFLiteral::Negative(CNFAtomicFormula::True)
            | CNFLiteral::Negative(CNFAtomicFormula::False) => {}
            CNFLiteral::Equality(left, right) | CNFLiteral::Inequality(left, right) => {
                collect_fof_term_signatures(
                    left,
                    constants,
                    functions,
                    distinct_objects,
                    signature_conflict,
                );
                collect_fof_term_signatures(
                    right,
                    constants,
                    functions,
                    distinct_objects,
                    signature_conflict,
                );
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

/// Collect free FOF variables, respecting shadowing by nested binders. TPTP
/// allows free variables in FOF input and they are universally closed at the
/// model-checking boundary just like free variables in a CNF clause.
fn collect_fof_free_vars(
    formula: &FOFFormula<'_>,
    bound: &BTreeSet<String>,
    free: &mut BTreeSet<String>,
) {
    fn visit_term(value: &FOFTerm<'_>, bound: &BTreeSet<String>, free: &mut BTreeSet<String>) {
        match value {
            FOFTerm::Variable(name) => {
                if !bound.contains(*name) {
                    free.insert((*name).to_string());
                }
            }
            FOFTerm::Function(_, args)
            | FOFTerm::DefinedFunction(_, args)
            | FOFTerm::SystemFunction(_, args) => {
                for arg in args {
                    visit_term(arg, bound, free);
                }
            }
            _ => {}
        }
    }
    match formula {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(_, args))
        | FOFFormula::Atomic(FOFAtomicFormula::Defined(_, args))
        | FOFFormula::Atomic(FOFAtomicFormula::System(_, args)) => {
            for arg in args {
                visit_term(arg, bound, free);
            }
        }
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
            collect_fof_free_vars(inner, bound, free)
        }
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
            visit_term(left, bound, free);
            visit_term(right, bound, free);
        }
        FOFFormula::Binary { left, right, .. } => {
            collect_fof_free_vars(left, bound, free);
            collect_fof_free_vars(right, bound, free);
        }
        FOFFormula::Quantified {
            variables, formula, ..
        } => {
            let mut nested = bound.clone();
            nested.extend(variables.iter().map(|name| (*name).to_string()));
            collect_fof_free_vars(formula, &nested, free);
        }
        FOFFormula::Atomic(FOFAtomicFormula::True | FOFAtomicFormula::False) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn model_eval_cnf_all_valuations(
    cert: &ModelCertificate,
    vars: &[String],
    idx: usize,
    literals: &[&CNFLiteral<'_>],
    env: &mut BTreeMap<String, usize>,
    count: &mut usize,
    remaining: &mut u64,
    depth: usize,
) -> Result<bool, String> {
    if idx > MAX_MODEL_EVALUATION_DEPTH || depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    if idx == vars.len() {
        if *remaining == 0 {
            return Err("model evaluation work limit exceeded".into());
        }
        *remaining -= 1;
        *count += 1;
        // Check if any literal is satisfied under env
        for lit in literals {
            let sat = match lit {
                CNFLiteral::Positive(atom) => {
                    eval_cnf_atomic_bounded(cert, atom, env, remaining, depth + 1)?
                }
                CNFLiteral::Negative(atom) => {
                    !eval_cnf_atomic_bounded(cert, atom, env, remaining, depth + 1)?
                }
                CNFLiteral::Equality(left, right) => {
                    let l_val = eval_term_bounded(cert, left, env, remaining, depth + 1)?;
                    let r_val = eval_term_bounded(cert, right, env, remaining, depth + 1)?;
                    l_val == r_val
                }
                CNFLiteral::Inequality(left, right) => {
                    let l_val = eval_term_bounded(cert, left, env, remaining, depth + 1)?;
                    let r_val = eval_term_bounded(cert, right, env, remaining, depth + 1)?;
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
        let previous = env.insert(var.clone(), d);
        let result = model_eval_cnf_all_valuations(
            cert,
            vars,
            idx + 1,
            literals,
            env,
            count,
            remaining,
            depth + 1,
        );
        restore_binding(env, var, previous);
        let ok = result?;
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn model_eval_quantified(
    cert: &ModelCertificate,
    vars: &[String],
    idx: usize,
    quantifier: Quantifier,
    formula: &FOFFormula<'_>,
    env: &mut BTreeMap<String, usize>,
    remaining: u64,
    depth: usize,
) -> Result<bool, String> {
    if vars.len() > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    let mut work = remaining;
    model_eval_quantified_inner(cert, vars, idx, quantifier, formula, env, &mut work, depth)
}

#[allow(clippy::too_many_arguments)]
fn model_eval_quantified_inner(
    cert: &ModelCertificate,
    vars: &[String],
    idx: usize,
    quantifier: Quantifier,
    formula: &FOFFormula<'_>,
    env: &mut BTreeMap<String, usize>,
    remaining: &mut u64,
    depth: usize,
) -> Result<bool, String> {
    if depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    if idx == vars.len() {
        if *remaining == 0 {
            return Err("model evaluation work/depth limit exceeded".into());
        }
        *remaining -= 1;
        return eval_fof_bounded(cert, formula, env, remaining, depth);
    }

    let var = &vars[idx];
    match quantifier {
        Quantifier::Forall => {
            for d in 0..cert.domain_size {
                let previous = env.insert(var.clone(), d);
                let result = model_eval_quantified_inner(
                    cert,
                    vars,
                    idx + 1,
                    quantifier,
                    formula,
                    env,
                    remaining,
                    depth + 1,
                );
                restore_binding(env, var, previous);
                if !result? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Quantifier::Exists => {
            for d in 0..cert.domain_size {
                let previous = env.insert(var.clone(), d);
                let result = model_eval_quantified_inner(
                    cert,
                    vars,
                    idx + 1,
                    quantifier,
                    formula,
                    env,
                    remaining,
                    depth + 1,
                );
                restore_binding(env, var, previous);
                if result? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_fof_bounded(
    cert: &ModelCertificate,
    formula: &FOFFormula<'_>,
    env: &mut BTreeMap<String, usize>,
    remaining: &mut u64,
    depth: usize,
) -> Result<bool, String> {
    if *remaining == 0 || depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    *remaining -= 1;
    let child = |formula: &FOFFormula<'_>,
                 env: &mut BTreeMap<String, usize>,
                 remaining: &mut u64|
     -> Result<bool, String> {
        if remaining.saturating_sub(1) == 0 || depth.saturating_add(1) > MAX_MODEL_EVALUATION_DEPTH
        {
            return Err("model evaluation work/depth limit exceeded".into());
        }
        eval_fof_bounded(cert, formula, env, remaining, depth + 1)
    };
    match formula {
        FOFFormula::Atomic(atom) => eval_fof_atomic_bounded(cert, atom, env, remaining, depth + 1),
        FOFFormula::Negation(inner) => Ok(!child(inner, env, remaining)?),
        FOFFormula::Parens(inner) => child(inner, env, remaining),
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => {
            preflight_term_depth(left, depth + 1)?;
            preflight_term_depth(right, depth + 1)?;
            let left = eval_term_bounded(cert, left, env, remaining, depth + 1)?;
            let right = eval_term_bounded(cert, right, env, remaining, depth + 1)?;
            let equal = left == right;
            Ok(if matches!(formula, FOFFormula::Equality(_, _)) {
                equal
            } else {
                !equal
            })
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => match connective {
            BinaryConnective::And => {
                if !child(left, env, remaining)? {
                    Ok(false)
                } else {
                    child(right, env, remaining)
                }
            }
            BinaryConnective::Or => {
                if child(left, env, remaining)? {
                    Ok(true)
                } else {
                    child(right, env, remaining)
                }
            }
            BinaryConnective::Impl => {
                if !child(left, env, remaining)? {
                    Ok(true)
                } else {
                    child(right, env, remaining)
                }
            }
            BinaryConnective::RevImpl => {
                if !child(right, env, remaining)? {
                    Ok(true)
                } else {
                    child(left, env, remaining)
                }
            }
            BinaryConnective::Iff => {
                let left = child(left, env, remaining)?;
                let right = child(right, env, remaining)?;
                Ok(left == right)
            }
            BinaryConnective::Xor => {
                let left = child(left, env, remaining)?;
                let right = child(right, env, remaining)?;
                Ok(left != right)
            }
            BinaryConnective::Nand => {
                let left = child(left, env, remaining)?;
                let right = child(right, env, remaining)?;
                Ok(!(left && right))
            }
            BinaryConnective::Nor => {
                let left = child(left, env, remaining)?;
                let right = child(right, env, remaining)?;
                Ok(!(left || right))
            }
        },
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            if variables.len() > MAX_MODEL_EVALUATION_DEPTH.saturating_sub(depth) {
                return Err("model evaluation work/depth limit exceeded".into());
            }
            eval_quantified_bounded(
                cert,
                variables,
                0,
                *quantifier,
                formula,
                env,
                remaining,
                depth + 1,
            )
        }
    }
}

fn preflight_term_depth(term: &FOFTerm<'_>, depth: usize) -> Result<(), String> {
    if depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    match term {
        FOFTerm::Function(_, args)
        | FOFTerm::DefinedFunction(_, args)
        | FOFTerm::SystemFunction(_, args) => {
            for arg in args {
                preflight_term_depth(arg, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn eval_fof_atomic_bounded(
    cert: &ModelCertificate,
    atom: &FOFAtomicFormula<'_>,
    env: &BTreeMap<String, usize>,
    remaining: &mut u64,
    depth: usize,
) -> Result<bool, String> {
    if *remaining == 0 || depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    *remaining -= 1;
    match atom {
        FOFAtomicFormula::Plain(name, args) => {
            let predicate = cert.predicates.get(name.as_str()).ok_or_else(|| {
                format!("missing interpretation for predicate `{}`", name.as_str())
            })?;
            if predicate.arity != args.len() {
                return Err(format!(
                    "predicate `{}` arity mismatch: expected {}, got {}",
                    name.as_str(),
                    predicate.arity,
                    args.len()
                ));
            }
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(eval_term_bounded(cert, arg, env, remaining, depth + 1)?);
            }
            let index = cert.table_index(&values)?;
            predicate.table.get(index).copied().ok_or_else(|| {
                format!(
                    "predicate table index out of bounds for `{}`",
                    name.as_str()
                )
            })
        }
        FOFAtomicFormula::Defined(DefinedWord("equal"), args) if args.len() == 2 => {
            let left = eval_term_bounded(cert, &args[0], env, remaining, depth + 1)?;
            let right = eval_term_bounded(cert, &args[1], env, remaining, depth + 1)?;
            Ok(left == right)
        }
        FOFAtomicFormula::True => Ok(true),
        FOFAtomicFormula::False => Ok(false),
        _ => Err("unsupported atomic formula in model evaluation".into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_quantified_bounded(
    cert: &ModelCertificate,
    vars: &[impl AsRef<str>],
    idx: usize,
    quantifier: Quantifier,
    formula: &FOFFormula<'_>,
    env: &mut BTreeMap<String, usize>,
    remaining: &mut u64,
    depth: usize,
) -> Result<bool, String> {
    if vars.len() > MAX_MODEL_EVALUATION_DEPTH.saturating_sub(depth) {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    if depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    if idx == vars.len() {
        return eval_fof_bounded(cert, formula, env, remaining, depth);
    }
    if idx >= MAX_MODEL_EVALUATION_DEPTH.saturating_sub(depth) {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    let name = vars[idx].as_ref();
    match quantifier {
        Quantifier::Forall => {
            for value in 0..cert.domain_size {
                let previous = env.insert(name.to_string(), value);
                let result = eval_quantified_bounded(
                    cert,
                    vars,
                    idx + 1,
                    quantifier,
                    formula,
                    env,
                    remaining,
                    depth + 1,
                );
                restore_binding(env, name, previous);
                if !result? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Quantifier::Exists => {
            for value in 0..cert.domain_size {
                let previous = env.insert(name.to_string(), value);
                let result = eval_quantified_bounded(
                    cert,
                    vars,
                    idx + 1,
                    quantifier,
                    formula,
                    env,
                    remaining,
                    depth + 1,
                );
                restore_binding(env, name, previous);
                if result? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn eval_term_bounded(
    cert: &ModelCertificate,
    term: &FOFTerm<'_>,
    env: &BTreeMap<String, usize>,
    remaining: &mut u64,
    depth: usize,
) -> Result<usize, String> {
    if *remaining == 0 || depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    *remaining -= 1;
    match term {
        FOFTerm::Function(_, _)
        | FOFTerm::DefinedFunction(_, _)
        | FOFTerm::SystemFunction(_, _) => {
            preflight_term_depth(term, depth)?;
        }
        _ => {}
    }
    match term {
        FOFTerm::Function(name, args) if !args.is_empty() => {
            let function = cert.functions.get(name.as_str()).ok_or_else(|| {
                format!("missing interpretation for function `{}`", name.as_str())
            })?;
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(eval_term_bounded(cert, arg, env, remaining, depth + 1)?);
            }
            let index = cert.table_index(&values)?;
            function.table.get(index).copied().ok_or_else(|| {
                format!("function table index out of bounds for `{}`", name.as_str())
            })
        }
        _ => cert.eval_term(term, env),
    }
}

fn eval_cnf_atomic_bounded(
    cert: &ModelCertificate,
    atom: &CNFAtomicFormula<'_>,
    env: &BTreeMap<String, usize>,
    remaining: &mut u64,
    depth: usize,
) -> Result<bool, String> {
    if *remaining == 0 || depth > MAX_MODEL_EVALUATION_DEPTH {
        return Err("model evaluation work/depth limit exceeded".into());
    }
    *remaining -= 1;
    match atom {
        CNFAtomicFormula::Plain(_, args)
        | CNFAtomicFormula::Defined(_, args)
        | CNFAtomicFormula::System(_, args) => {
            for arg in args {
                preflight_term_depth(arg, depth + 1)?;
            }
            let CNFAtomicFormula::Plain(name, args) = atom else {
                return Err("unsupported CNF atomic formula in model evaluation".into());
            };
            let predicate = cert.predicates.get(name.as_str()).ok_or_else(|| {
                format!("missing interpretation for predicate `{}`", name.as_str())
            })?;
            if predicate.arity != args.len() {
                return Err(format!(
                    "predicate `{}` arity mismatch: expected {}, got {}",
                    name.as_str(),
                    predicate.arity,
                    args.len()
                ));
            }
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(eval_term_bounded(cert, arg, env, remaining, depth + 1)?);
            }
            let index = cert.table_index(&values)?;
            predicate.table.get(index).copied().ok_or_else(|| {
                format!(
                    "predicate table index out of bounds for `{}`",
                    name.as_str()
                )
            })
        }
        CNFAtomicFormula::True => Ok(true),
        CNFAtomicFormula::False => Ok(false),
    }
}

fn restore_binding(env: &mut BTreeMap<String, usize>, name: &str, previous: Option<usize>) {
    if let Some(value) = previous {
        env.insert(name.to_string(), value);
    } else {
        env.remove(name);
    }
}

fn model_table_len(
    cert: &ModelCertificate,
    arity: usize,
    kind: &str,
    name: &str,
) -> Result<usize, ModelVerdict> {
    if arity == 0 {
        return Ok(1);
    }
    let exponent = u32::try_from(arity).map_err(|_| {
        ModelVerdict::Inconclusive(format!("{kind} `{name}` arity exceeds strict limit"))
    })?;
    let length = cert.domain_size.checked_pow(exponent).ok_or_else(|| {
        ModelVerdict::Inconclusive(format!(
            "{kind} `{name}` table size overflows the host usize"
        ))
    })?;
    if length > mrs_core::model::MAX_MODEL_TABLE_ENTRIES {
        Err(ModelVerdict::Inconclusive(format!(
            "{kind} `{name}` table exceeds strict entry limit"
        )))
    } else {
        Ok(length)
    }
}

/// Conservative work estimate for validating a finite interpretation.
/// Counts recursive formula/term visits under all quantified assignments.
fn fof_model_work(formula: &FOFFormula<'_>, domain: usize, depth: usize) -> Option<u64> {
    if depth > MAX_MODEL_EVALUATION_DEPTH {
        return None;
    }
    let recur = |child: &FOFFormula<'_>| fof_model_work(child, domain, depth + 1);
    let combine = |children: Vec<u64>| children.into_iter().try_fold(1u64, u64::checked_add);
    match formula {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(_, args))
        | FOFFormula::Atomic(FOFAtomicFormula::Defined(_, args))
        | FOFFormula::Atomic(FOFAtomicFormula::System(_, args)) => {
            if args
                .iter()
                .any(|term| term_eval_work(term, depth + 1).is_none())
            {
                None
            } else {
                args.iter().try_fold(1u64, |work, term| {
                    work.checked_add(term_eval_work(term, depth + 1)?)
                })
            }
        }
        FOFFormula::Atomic(_) => Some(1),
        FOFFormula::Parens(inner) | FOFFormula::Negation(inner) => recur(inner)?.checked_add(1),
        FOFFormula::Equality(left, right) | FOFFormula::Inequality(left, right) => Some(
            1u64.checked_add(term_eval_work(left, depth + 1)?)?
                .checked_add(term_eval_work(right, depth + 1)?)?,
        ),
        FOFFormula::Binary { left, right, .. } => combine(vec![recur(left)?, recur(right)?]),
        FOFFormula::Quantified {
            variables, formula, ..
        } => {
            if variables.len() > MAX_MODEL_EVALUATION_DEPTH {
                return None;
            }
            let valuations = (0..variables.len())
                .try_fold(1u64, |n, _| n.checked_mul(u64::try_from(domain).ok()?))?;
            recur(formula)?.checked_mul(valuations)?.checked_add(1)
        }
    }
}

fn term_eval_work(term: &FOFTerm<'_>, depth: usize) -> Option<u64> {
    if depth > MAX_MODEL_EVALUATION_DEPTH {
        return None;
    }
    match term {
        FOFTerm::Function(_, args)
        | FOFTerm::DefinedFunction(_, args)
        | FOFTerm::SystemFunction(_, args) => args.iter().try_fold(1u64, |work, arg| {
            work.checked_add(term_eval_work(arg, depth + 1)?)
        }),
        _ => Some(1),
    }
}

fn cnf_model_work(formula: &CNFFormula<'_>, domain: usize) -> Option<u64> {
    let mut vars = BTreeSet::new();
    let literals = formula.literals();
    for literal in &literals {
        collect_cnf_literal_vars(literal, &mut vars);
    }
    if vars.len() > MAX_MODEL_EVALUATION_DEPTH {
        return None;
    }
    let valuations =
        (0..vars.len()).try_fold(1u64, |n, _| n.checked_mul(u64::try_from(domain).ok()?))?;
    let literal_work = literals.iter().try_fold(1u64, |work, literal| {
        let term_work = match literal {
            CNFLiteral::Positive(CNFAtomicFormula::Plain(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::Plain(_, args))
            | CNFLiteral::Positive(CNFAtomicFormula::Defined(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::Defined(_, args))
            | CNFLiteral::Positive(CNFAtomicFormula::System(_, args))
            | CNFLiteral::Negative(CNFAtomicFormula::System(_, args)) => args
                .iter()
                .try_fold(1u64, |sum, term| sum.checked_add(term_eval_work(term, 0)?))?,
            CNFLiteral::Equality(left, right) | CNFLiteral::Inequality(left, right) => 1u64
                .checked_add(term_eval_work(left, 0)?)?
                .checked_add(term_eval_work(right, 0)?)?,
            _ => 1,
        };
        work.checked_add(term_work)
    })?;
    valuations.checked_mul(literal_work)
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
    fn rejects_duplicate_interpretations_for_distinct_objects() {
        let problem = parse_tptp("fof(a, axiom, \"red\" != \"blue\").").unwrap();
        let mut certificate = ModelCertificate {
            domain_size: 1,
            constants: [("\"red\"".to_string(), 0), ("\"blue\"".to_string(), 0)]
                .into_iter()
                .collect(),
            functions: BTreeMap::new(),
            predicates: BTreeMap::new(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        certificate.digest = certificate.compute_digest();
        assert!(matches!(
            certificate.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Rejected(reason) if reason.contains("distinct objects")
        ));
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
    fn rejects_satisfiable_expectation_for_problem_with_conjecture() {
        let problem = parse_tptp("fof(conj, conjecture, p(a)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
            predicates: [(
                "p".to_string(),
                PredicateTable {
                    arity: 1,
                    table: vec![false],
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
            ModelVerdict::Rejected(reason) if reason.contains("contains a conjecture")
        ));
    }

    #[test]
    fn rejects_expected_unsatisfiable_status_for_model_certificate() {
        let problem = parse_tptp("fof(ax, axiom, p(a)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
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
            cert.validate(&problem, Some("Unsatisfiable")),
            ModelVerdict::Inconclusive(reason) if reason.contains("cannot validate expected status")
        ));
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

    #[test]
    fn rejects_model_table_entry_limit_before_digest_or_scan() {
        let problem = parse_tptp("fof(ax, axiom, p(a)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: MAX_MODEL_TABLE_ENTRIES + 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
            predicates: [(
                "p".to_string(),
                PredicateTable {
                    arity: 1,
                    table: vec![],
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
            ModelVerdict::Inconclusive(reason) if reason.contains("entry limit")
        ));
    }

    #[test]
    fn unknown_formula_roles_are_not_silently_ignored() {
        let problem = parse_tptp("fof(ax, unknown, p(a)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
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
            ModelVerdict::Inconclusive(reason) if reason.contains("unknown formula role")
        ));
    }

    #[test]
    fn rejects_inconsistent_symbol_arities_before_model_evaluation() {
        let problem = parse_tptp("fof(a, axiom, p(a)). fof(b, axiom, p(a,a)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
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
            ModelVerdict::Rejected(reason) if reason.contains("inconsistent symbol arities")
        ));
    }

    #[test]
    fn rejects_symbol_reused_as_constant_and_predicate() {
        let problem = parse_tptp("fof(a, axiom, p). fof(b, axiom, q(p)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 1,
            constants: [("p".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
            predicates: [
                (
                    "p".to_string(),
                    PredicateTable {
                        arity: 0,
                        table: vec![true],
                    },
                ),
                (
                    "q".to_string(),
                    PredicateTable {
                        arity: 1,
                        table: vec![true],
                    },
                ),
            ]
            .into_iter()
            .collect(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();
        assert!(matches!(
            cert.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Rejected(reason) if reason.contains("both a term and predicate")
        ));
    }

    #[test]
    fn rejects_excessive_quantifier_model_evaluation_before_enumeration() {
        let problem = parse_tptp("fof(ax, axiom, ![X1,X2,X3,X4,X5] : p(X1)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 100,
            constants: BTreeMap::new(),
            functions: BTreeMap::new(),
            predicates: [(
                "p".to_string(),
                PredicateTable {
                    arity: 1,
                    table: vec![false; 100],
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
            ModelVerdict::Inconclusive(reason) if reason.contains("work limit")
        ));
    }

    #[test]
    fn free_fof_variables_are_universally_closed() {
        let problem = parse_tptp("fof(ax, axiom, p(X)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 2,
            constants: BTreeMap::new(),
            functions: BTreeMap::new(),
            predicates: [(
                "p".to_string(),
                PredicateTable {
                    arity: 1,
                    table: vec![true, false],
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
            ModelVerdict::Rejected(reason) if reason.contains("violates axiom")
        ));
    }

    #[test]
    fn nested_quantifier_restores_shadowed_free_variable_assignment() {
        let problem = parse_tptp("fof(ax, axiom, (![X] : q(X)) & p(X)).").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 2,
            constants: BTreeMap::new(),
            functions: BTreeMap::new(),
            predicates: [
                (
                    "p".to_string(),
                    PredicateTable {
                        arity: 1,
                        table: vec![true, true],
                    },
                ),
                (
                    "q".to_string(),
                    PredicateTable {
                        arity: 1,
                        table: vec![true, true],
                    },
                ),
            ]
            .into_iter()
            .collect(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();
        assert!(matches!(
            cert.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Certified { .. }
        ));
    }

    #[test]
    fn accepts_large_arity_when_dense_table_is_still_bounded() {
        // The domain-one table has just one cell despite its high arity. The
        // cap is on aggregate table entries, not syntactic arity by itself.
        let arguments = (0..100)
            .map(|i| format!("X{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let problem_text = format!("fof(ax, axiom, ![{}] : p({})).", arguments, arguments);
        let problem = parse_tptp(&problem_text).unwrap();
        let mut predicates = BTreeMap::new();
        predicates.insert(
            "p".to_string(),
            PredicateTable {
                arity: 100,
                table: vec![true],
            },
        );
        let mut certificate = ModelCertificate {
            domain_size: 1,
            constants: BTreeMap::new(),
            functions: BTreeMap::new(),
            predicates,
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        certificate.digest = certificate.compute_digest();
        assert!(matches!(
            certificate.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Certified { .. }
        ));
    }

    #[test]
    fn distinct_objects_must_have_distinct_interpretations() {
        let problem = parse_tptp("fof(ax, axiom, \"red\" != \"blue\").").unwrap();
        let mut cert = ModelCertificate {
            domain_size: 2,
            constants: [("\"red\"".to_string(), 0), ("\"blue\"".to_string(), 0)]
                .into_iter()
                .collect(),
            functions: BTreeMap::new(),
            predicates: BTreeMap::new(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        cert.digest = cert.compute_digest();
        assert!(matches!(
            cert.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Rejected(reason) if reason.contains("distinct objects")
        ));
    }

    #[test]
    fn negated_conjecture_is_only_a_premise_for_model_validation() {
        let problem = parse_tptp("fof(negated, negated_conjecture, ~p(a)).").unwrap();
        let mut certificate = ModelCertificate {
            domain_size: 1,
            constants: [("a".to_string(), 0)].into_iter().collect(),
            functions: BTreeMap::new(),
            predicates: [(
                "p".to_string(),
                PredicateTable {
                    arity: 1,
                    table: vec![false],
                },
            )]
            .into_iter()
            .collect(),
            equality: EqualitySemantics::StrictIdentity,
            digest: String::new(),
        };
        certificate.digest = certificate.compute_digest();
        assert!(matches!(
            certificate.validate(&problem, Some("Satisfiable")),
            ModelVerdict::Certified { .. }
        ));
        assert!(matches!(
            certificate.validate(&problem, Some("CounterSatisfiable")),
            ModelVerdict::Rejected(_)
        ));
        assert!(matches!(
            certificate.validate(&problem, None),
            ModelVerdict::Certified { .. }
        ));
    }
}

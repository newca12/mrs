//! Deep problem profiling and feature extraction for TPTP problems.
//!
//! Extracts comprehensive multi-dimensional syntactic, structural, algebraic,
//! and conjectural characteristics from a problem's clauses and symbol table.
//! Provides both human-readable diagnostic display and compact serializable
//! representations for SQLite storage in `mrs-codex` and dynamic strategy routing.

use std::fmt;

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use serde::{Deserialize, Serialize};

use crate::clause::{Clause, ClauseSource};
use crate::formula::Atom;
use crate::symbol::{SymbolId, SymbolTable};
use crate::term::{Term, VarId};

/// Structural archetype classification of a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProblemArchetype {
    /// Pure Unit Equality: 100% unit equality clauses.
    PureUnitEquality,
    /// Effectively Propositional: No function symbols of arity >= 1.
    EssentiallyPropositional,
    /// Propositional Skeleton (FVO): All predicate arguments are variables.
    PropositionalSkeleton,
    /// Pure Horn logic: 100% Horn clauses with high unit density.
    PureHorn,
    /// Large Theory: > 150 axioms (requires SInE filtering).
    LargeTheory,
    /// Deep Equational: Mixed equality with high term nesting depth (>= 7).
    DeepEquational,
    /// General First-Order with Equality (FEQ).
    GeneralFirstOrderEquality,
    /// General First-Order without Equality (FNE).
    GeneralFirstOrderNonEquational,
    /// Empty or non-logical clause set.
    Empty,
}

impl ProblemArchetype {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PureUnitEquality => "PureUnitEquality",
            Self::EssentiallyPropositional => "EssentiallyPropositional",
            Self::PropositionalSkeleton => "PropositionalSkeleton",
            Self::PureHorn => "PureHorn",
            Self::LargeTheory => "LargeTheory",
            Self::DeepEquational => "DeepEquational",
            Self::GeneralFirstOrderEquality => "GeneralFirstOrderEquality",
            Self::GeneralFirstOrderNonEquational => "GeneralFirstOrderNonEquational",
            Self::Empty => "Empty",
        }
    }
}

impl fmt::Display for ProblemArchetype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Optional input metadata from TPTP headers or file system.
#[derive(Debug, Clone, Default)]
pub struct InputMetadata {
    pub dialect: Option<String>,
    pub raw_formulas_count: usize,
    pub includes_count: usize,
    pub file_size_bytes: Option<u64>,
    pub header_status: Option<String>,
    pub header_rating: Option<f32>,
    /// Number of supported input formulas with premise roles.
    pub input_axioms_count: Option<usize>,
    /// Number of supported input formulas with goal roles.
    pub input_conjectures_count: Option<usize>,
}

/// Comprehensive deep profile of a TPTP problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemProfile {
    // ── Problem Identity & Provenance ─────────────────────────────────────────
    pub problem_name: String,
    pub domain: String,
    pub dialect: String,
    pub header_status: Option<String>,
    pub header_rating: Option<f32>,
    /// Whether all input clauses were available during extraction.
    pub profile_complete: bool,

    // ── Scale & Counts ────────────────────────────────────────────────────────
    pub num_clauses: usize,
    pub num_literals: usize,
    pub num_axioms: usize,
    pub num_conjectures: usize,
    pub num_variables: usize,
    pub avg_vars_per_clause: f32,
    pub max_vars_per_clause: usize,

    // ── Clausal Morphology & Ratios [0.0 ..= 1.0] ─────────────────────────────
    pub unit_ratio: f32,
    pub horn_ratio: f32,
    pub definite_ratio: f32,
    pub goal_clause_ratio: f32,
    pub ground_ratio: f32,
    pub equality_literal_ratio: f32,
    pub non_linear_var_ratio: f32,
    pub max_clause_len: usize,
    pub avg_clause_len: f32,
    pub max_pos_literals: usize,

    // ── Term & Depth Structure ────────────────────────────────────────────────
    pub max_term_depth: usize,
    pub avg_term_depth: f32,
    pub max_term_size: usize,
    pub avg_term_size: f32,
    pub num_predicates: usize,
    pub num_functions: usize,
    pub num_constants: usize,
    pub max_fun_arity: usize,
    pub max_pred_arity: usize,
    pub skolem_symbols_count: usize,

    // ── Algebraic & Structural Symmetries ─────────────────────────────────────
    pub is_ueq: bool,
    pub is_peq: bool,
    pub is_fne: bool,
    pub is_feq: bool,
    pub is_epr: bool,
    pub is_fvo: bool,
    pub is_large_theory: bool,
    pub has_ac_symbols: bool,
    pub ac_symbols: Vec<String>,
    pub has_identity_axiom: bool,
    pub has_inverse_axiom: bool,
    pub has_idempotence: bool,

    // ── Conjectural Topology ──────────────────────────────────────────────────
    pub has_conjecture: bool,
    pub conjecture_clauses: usize,
    pub conjecture_literals: usize,
    pub conjecture_max_depth: usize,
    pub conjecture_symbol_overlap: f32,
    pub unique_conjecture_symbols: Vec<String>,

    // ── Classification & Routing Recommendations ─────────────────────────────
    pub archetype: ProblemArchetype,
    pub casc_division: String,
    pub recommended_schedule: String,
    pub recommended_engine: String,
    pub recommended_avatar: bool,
    pub recommended_sine: bool,
}

impl ProblemProfile {
    /// Extract a comprehensive `ProblemProfile` from the given clauses and symbol table.
    pub fn extract(
        problem_name: &str,
        meta: Option<&InputMetadata>,
        clauses: &[Clause],
        symbols: &SymbolTable,
    ) -> Self {
        let domain = extract_domain(problem_name);
        let dialect = meta
            .and_then(|m| m.dialect.clone())
            .unwrap_or_else(|| "CNF".to_string());
        let header_status = meta.and_then(|m| m.header_status.clone());
        let header_rating = meta.and_then(|m| m.header_rating);

        let num_clauses = clauses.len();
        if num_clauses == 0 {
            return Self::empty(problem_name, domain, dialect, header_status, header_rating);
        }

        let mut num_literals = 0;
        let mut unit_clauses = 0;
        let mut horn_clauses = 0;
        let mut definite_clauses = 0;
        let mut goal_clauses = 0;
        let mut ground_clauses = 0;
        let mut non_linear_clauses = 0;
        let mut max_clause_len = 0;
        let mut max_pos_literals = 0;

        let mut eq_literals = 0;
        let mut total_variables: HashSet<VarId> = HashSet::default();
        let mut max_vars_per_clause = 0;
        let mut sum_vars_per_clause = 0;

        let input_axioms_count = meta.and_then(|m| m.input_axioms_count);
        let input_conjectures_count = meta.and_then(|m| m.input_conjectures_count);
        let mut num_axioms = 0;
        let mut num_conjectures = 0;
        let mut conjecture_clauses = 0;
        let mut conjecture_literals = 0;
        let mut conjecture_max_depth = 0;

        let mut max_term_depth = 0;
        let mut sum_term_depth = 0;
        let mut max_term_size = 0;
        let mut sum_term_size = 0;
        let mut total_term_occurrences = 0;

        let mut is_fvo = true;

        let mut pred_arities: HashMap<SymbolId, HashSet<usize>> = HashMap::default();
        let mut func_arities: HashMap<SymbolId, HashSet<usize>> = HashMap::default();

        let mut axiom_symbols: HashSet<SymbolId> = HashSet::default();
        let mut conj_symbols: HashSet<SymbolId> = HashSet::default();

        // Algebraic symmetry trackers
        let mut commutative_symbols: HashSet<SymbolId> = HashSet::default();
        let mut associative_symbols: HashSet<SymbolId> = HashSet::default();
        let mut has_identity_axiom = false;
        let mut has_inverse_axiom = false;
        let mut has_idempotence = false;

        for c in clauses {
            let clen = c.literals.len();
            num_literals += clen;
            max_clause_len = max_clause_len.max(clen);

            let is_conj = c.distance == 0
                || matches!(
                    &c.source,
                    ClauseSource::Input { role, .. }
                        if role == "conjecture" || role == "negated_conjecture"
                )
                || matches!(
                    &c.source,
                    ClauseSource::Inference { rule, .. }
                        if *rule == "negated_conjecture"
                );
            if is_conj {
                if input_conjectures_count.is_none() {
                    num_conjectures += 1;
                }
                conjecture_clauses += 1;
                conjecture_literals += clen;
            } else if input_axioms_count.is_none() {
                num_axioms += 1;
            }

            if clen == 1 {
                unit_clauses += 1;
            }

            let mut pos_count = 0;
            let mut clause_vars: HashSet<VarId> = HashSet::default();
            let mut clause_is_non_linear = false;

            // Algebraic checks on unit equality clauses
            if clen == 1
                && c.literals[0].positive
                && let Atom::Eq(l, r) = &c.literals[0].atom
            {
                check_algebraic_identities(
                    l,
                    r,
                    &mut commutative_symbols,
                    &mut associative_symbols,
                    &mut has_identity_axiom,
                    &mut has_inverse_axiom,
                    &mut has_idempotence,
                );
            }

            for lit in &c.literals {
                if lit.positive {
                    pos_count += 1;
                }

                // Variable linearity check within literal
                let mut lit_var_counts: HashMap<VarId, usize> = HashMap::default();

                match &lit.atom {
                    Atom::Eq(l, r) => {
                        is_fvo = false;
                        eq_literals += 1;

                        count_term_vars(l, &mut lit_var_counts);
                        count_term_vars(r, &mut lit_var_counts);

                        let (dl, sl) = measure_term(
                            l,
                            &mut func_arities,
                            if is_conj {
                                &mut conj_symbols
                            } else {
                                &mut axiom_symbols
                            },
                        );
                        let (dr, sr) = measure_term(
                            r,
                            &mut func_arities,
                            if is_conj {
                                &mut conj_symbols
                            } else {
                                &mut axiom_symbols
                            },
                        );

                        let max_d = dl.max(dr);
                        max_term_depth = max_term_depth.max(max_d);
                        if is_conj {
                            conjecture_max_depth = conjecture_max_depth.max(max_d);
                        }
                        sum_term_depth += dl + dr;
                        max_term_size = max_term_size.max(sl).max(sr);
                        sum_term_size += sl + sr;
                        total_term_occurrences += 2;
                    }
                    Atom::Pred(p, args) => {
                        pred_arities.entry(*p).or_default().insert(args.len());
                        if is_conj {
                            conj_symbols.insert(*p);
                        } else {
                            axiom_symbols.insert(*p);
                        }

                        for arg in args {
                            if !matches!(arg, Term::Var(_)) {
                                is_fvo = false;
                            }
                            count_term_vars(arg, &mut lit_var_counts);
                            let (d, s) = measure_term(
                                arg,
                                &mut func_arities,
                                if is_conj {
                                    &mut conj_symbols
                                } else {
                                    &mut axiom_symbols
                                },
                            );
                            max_term_depth = max_term_depth.max(d);
                            if is_conj {
                                conjecture_max_depth = conjecture_max_depth.max(d);
                            }
                            sum_term_depth += d;
                            max_term_size = max_term_size.max(s);
                            sum_term_size += s;
                            total_term_occurrences += 1;
                        }
                    }
                }

                if lit_var_counts.values().any(|&cnt| cnt > 1) {
                    clause_is_non_linear = true;
                }

                for v in lit_var_counts.keys() {
                    clause_vars.insert(*v);
                    total_variables.insert(*v);
                }
            }

            max_pos_literals = max_pos_literals.max(pos_count);
            if pos_count <= 1 {
                horn_clauses += 1;
            }
            if pos_count == 1 {
                definite_clauses += 1;
            }
            if pos_count == 0 {
                goal_clauses += 1;
            }
            if clause_vars.is_empty() {
                ground_clauses += 1;
            }
            if clause_is_non_linear {
                non_linear_clauses += 1;
            }

            let num_vars = clause_vars.len();
            max_vars_per_clause = max_vars_per_clause.max(num_vars);
            sum_vars_per_clause += num_vars;
        }

        if let Some(count) = input_axioms_count {
            num_axioms = count;
        }
        if let Some(count) = input_conjectures_count {
            num_conjectures = count;
        }

        let num_predicates = pred_arities.len();
        let mut num_constants = 0;
        let mut max_fun_arity = 0;
        let mut skolem_symbols_count = 0;

        for (f_id, arities) in &func_arities {
            if let Some(name) = get_symbol_name(*f_id, symbols)
                && is_skolem(&name)
            {
                skolem_symbols_count += 1;
            }
            if arities.contains(&0) {
                num_constants += 1;
            }
            for &a in arities {
                max_fun_arity = max_fun_arity.max(a);
            }
        }

        let max_pred_arity = pred_arities
            .values()
            .flat_map(|arities| arities.iter().copied())
            .max()
            .unwrap_or(0);

        let num_functions = func_arities.len();

        let unit_ratio = unit_clauses as f32 / num_clauses as f32;
        let horn_ratio = horn_clauses as f32 / num_clauses as f32;
        let definite_ratio = definite_clauses as f32 / num_clauses as f32;
        let goal_clause_ratio = goal_clauses as f32 / num_clauses as f32;
        let ground_ratio = ground_clauses as f32 / num_clauses as f32;
        let equality_literal_ratio = if num_literals > 0 {
            eq_literals as f32 / num_literals as f32
        } else {
            0.0
        };
        let non_linear_var_ratio = non_linear_clauses as f32 / num_clauses as f32;
        let avg_clause_len = num_literals as f32 / num_clauses as f32;
        let avg_vars_per_clause = sum_vars_per_clause as f32 / num_clauses as f32;

        let avg_term_depth = if total_term_occurrences > 0 {
            sum_term_depth as f32 / total_term_occurrences as f32
        } else {
            0.0
        };
        let avg_term_size = if total_term_occurrences > 0 {
            sum_term_size as f32 / total_term_occurrences as f32
        } else {
            0.0
        };

        // AC symbol detection: must satisfy both commutativity and associativity
        let mut ac_symbols = Vec::new();
        for &sym in &commutative_symbols {
            if associative_symbols.contains(&sym)
                && let Some(name) = get_symbol_name(sym, symbols)
            {
                ac_symbols.push(name);
            }
        }
        ac_symbols.sort();
        let has_ac_symbols = !ac_symbols.is_empty();

        let is_ueq = num_clauses > 0
            && clauses
                .iter()
                .all(|c| c.literals.len() == 1 && matches!(c.literals[0].atom, Atom::Eq(_, _)));
        let is_peq = num_clauses > 0
            && clauses
                .iter()
                .all(|c| c.literals.iter().all(|l| matches!(l.atom, Atom::Eq(_, _))));
        let is_fne = eq_literals == 0;
        let is_feq = !is_ueq && !is_fne;
        let is_epr = max_fun_arity == 0;
        let is_large_theory = num_axioms > 150;

        // Conjectural overlap
        let (conjecture_symbol_overlap, unique_conjecture_symbols) = if conj_symbols.is_empty() {
            (1.0, Vec::new())
        } else {
            let mut unique = Vec::new();
            let mut shared_count = 0;
            for &s in &conj_symbols {
                if axiom_symbols.contains(&s) {
                    shared_count += 1;
                } else if let Some(name) = get_symbol_name(s, symbols) {
                    unique.push(name);
                }
            }
            unique.sort();
            (shared_count as f32 / conj_symbols.len() as f32, unique)
        };

        // Determine Archetype & Recommendations
        let (
            archetype,
            casc_division,
            recommended_schedule,
            recommended_engine,
            recommended_avatar,
            recommended_sine,
        ) = classify_problem(
            is_ueq,
            is_epr,
            is_fvo,
            is_fne,
            is_feq,
            is_large_theory,
            horn_ratio,
            unit_ratio,
            max_term_depth,
            max_pos_literals,
        );

        Self {
            problem_name: problem_name.to_string(),
            domain,
            dialect,
            header_status,
            header_rating,
            profile_complete: true,
            num_clauses,
            num_literals,
            num_axioms,
            num_conjectures,
            num_variables: total_variables.len(),
            avg_vars_per_clause,
            max_vars_per_clause,
            unit_ratio,
            horn_ratio,
            definite_ratio,
            goal_clause_ratio,
            ground_ratio,
            equality_literal_ratio,
            non_linear_var_ratio,
            max_clause_len,
            avg_clause_len,
            max_pos_literals,
            max_term_depth,
            avg_term_depth,
            max_term_size,
            avg_term_size,
            num_predicates,
            num_functions,
            num_constants,
            max_fun_arity,
            max_pred_arity,
            skolem_symbols_count,
            is_ueq,
            is_peq,
            is_fne,
            is_feq,
            is_epr,
            is_fvo,
            is_large_theory,
            has_ac_symbols,
            ac_symbols,
            has_identity_axiom,
            has_inverse_axiom,
            has_idempotence,
            has_conjecture: num_conjectures > 0,
            conjecture_clauses,
            conjecture_literals,
            conjecture_max_depth,
            conjecture_symbol_overlap,
            unique_conjecture_symbols,
            archetype,
            casc_division,
            recommended_schedule,
            recommended_engine,
            recommended_avatar,
            recommended_sine,
        }
    }

    pub fn empty(
        problem_name: &str,
        domain: String,
        dialect: String,
        header_status: Option<String>,
        header_rating: Option<f32>,
    ) -> Self {
        Self {
            problem_name: problem_name.to_string(),
            domain,
            dialect,
            header_status,
            header_rating,
            profile_complete: true,
            num_clauses: 0,
            num_literals: 0,
            num_axioms: 0,
            num_conjectures: 0,
            num_variables: 0,
            avg_vars_per_clause: 0.0,
            max_vars_per_clause: 0,
            unit_ratio: 0.0,
            horn_ratio: 0.0,
            definite_ratio: 0.0,
            goal_clause_ratio: 0.0,
            ground_ratio: 0.0,
            equality_literal_ratio: 0.0,
            non_linear_var_ratio: 0.0,
            max_clause_len: 0,
            avg_clause_len: 0.0,
            max_pos_literals: 0,
            max_term_depth: 0,
            avg_term_depth: 0.0,
            max_term_size: 0,
            avg_term_size: 0.0,
            num_predicates: 0,
            num_functions: 0,
            num_constants: 0,
            max_fun_arity: 0,
            max_pred_arity: 0,
            skolem_symbols_count: 0,
            is_ueq: false,
            is_peq: false,
            is_fne: false,
            is_feq: false,
            is_epr: false,
            is_fvo: false,
            is_large_theory: false,
            has_ac_symbols: false,
            ac_symbols: Vec::new(),
            has_identity_axiom: false,
            has_inverse_axiom: false,
            has_idempotence: false,
            has_conjecture: false,
            conjecture_clauses: 0,
            conjecture_literals: 0,
            conjecture_max_depth: 0,
            conjecture_symbol_overlap: 1.0,
            unique_conjecture_symbols: Vec::new(),
            archetype: ProblemArchetype::Empty,
            casc_division: "Unknown".to_string(),
            recommended_schedule: "casc".to_string(),
            recommended_engine: "None".to_string(),
            recommended_avatar: false,
            recommended_sine: false,
        }
    }

    /// Create a profile that contains identity metadata but not complete
    /// clause statistics. Partial profiles must not be used for routing.
    pub fn incomplete(
        problem_name: &str,
        domain: String,
        dialect: String,
        header_status: Option<String>,
        header_rating: Option<f32>,
    ) -> Self {
        let mut profile = Self::empty(problem_name, domain, dialect, header_status, header_rating);
        profile.profile_complete = false;
        profile
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_problem(
    is_ueq: bool,
    is_epr: bool,
    is_fvo: bool,
    is_fne: bool,
    is_feq: bool,
    is_large_theory: bool,
    horn_ratio: f32,
    unit_ratio: f32,
    max_term_depth: usize,
    max_pos_literals: usize,
) -> (ProblemArchetype, String, String, String, bool, bool) {
    if is_ueq {
        (
            ProblemArchetype::PureUnitEquality,
            "UEQ".to_string(),
            "casc_ueq".to_string(),
            "TweeCompletion".to_string(),
            false,
            false,
        )
    } else if is_epr {
        (
            ProblemArchetype::EssentiallyPropositional,
            "EPR".to_string(),
            "casc_epr".to_string(),
            "InstGen".to_string(),
            false,
            false,
        )
    } else if is_fvo {
        let div = if is_fne { "FNE" } else { "FEQ" };
        let sched = if is_fne { "casc_fne" } else { "casc_feq" };
        (
            ProblemArchetype::PropositionalSkeleton,
            div.to_string(),
            sched.to_string(),
            "FVORefutation".to_string(),
            false,
            false,
        )
    } else if is_large_theory {
        let div = if is_fne { "FNE" } else { "FEQ" };
        let sched = if is_fne { "casc_fne" } else { "casc_feq" };
        (
            ProblemArchetype::LargeTheory,
            div.to_string(),
            sched.to_string(),
            "SuperpositionPortfolio".to_string(),
            max_pos_literals > 1,
            true, // SInE enabled
        )
    } else if horn_ratio >= 1.0 && unit_ratio >= 0.3 {
        let div = if is_fne { "FNE" } else { "FEQ" };
        let sched = if is_fne { "casc_fne" } else { "casc_feq" };
        (
            ProblemArchetype::PureHorn,
            div.to_string(),
            sched.to_string(),
            "HyperResolution".to_string(),
            false, // AVATAR not needed on pure Horn
            false,
        )
    } else if is_feq && max_term_depth >= 7 {
        (
            ProblemArchetype::DeepEquational,
            "FEQ".to_string(),
            "casc_feq".to_string(),
            "LPOSuperposition".to_string(),
            true,
            false,
        )
    } else if is_fne {
        (
            ProblemArchetype::GeneralFirstOrderNonEquational,
            "FNE".to_string(),
            "casc_fne".to_string(),
            "SuperpositionPortfolio".to_string(),
            max_pos_literals > 1,
            false,
        )
    } else {
        (
            ProblemArchetype::GeneralFirstOrderEquality,
            "FEQ".to_string(),
            "casc_feq".to_string(),
            "SuperpositionPortfolio".to_string(),
            true,
            false,
        )
    }
}

fn extract_domain(name: &str) -> String {
    let clean = name.strip_prefix("Problems/").unwrap_or(name);
    let filename = std::path::Path::new(clean)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(clean);

    let prefix: String = filename
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if prefix.is_empty() {
        "SYN".to_string()
    } else {
        prefix.to_uppercase()
    }
}

fn count_term_vars(term: &Term, counts: &mut HashMap<VarId, usize>) {
    let mut stack = vec![term];
    while let Some(t) = stack.pop() {
        match t {
            Term::Var(v) => {
                *counts.entry(*v).or_default() += 1;
            }
            Term::App(_, args) => {
                stack.extend(args.iter());
            }
        }
    }
}

fn measure_term(
    term: &Term,
    func_arities: &mut HashMap<SymbolId, HashSet<usize>>,
    symbols_collected: &mut HashSet<SymbolId>,
) -> (usize, usize) {
    match term {
        Term::Var(_) => (1, 1),
        Term::App(f_id, args) => {
            func_arities.entry(*f_id).or_default().insert(args.len());
            symbols_collected.insert(*f_id);
            let mut max_depth = 0;
            let mut total_size = 1;
            for arg in args {
                let (d, s) = measure_term(arg, func_arities, symbols_collected);
                max_depth = max_depth.max(d);
                total_size += s;
            }
            (max_depth + 1, total_size)
        }
    }
}

fn get_symbol_name(id: SymbolId, symbols: &SymbolTable) -> Option<String> {
    if (id.index() as usize) < symbols.len() {
        Some(symbols.resolve(id).to_string())
    } else {
        None
    }
}

fn is_skolem(name: &str) -> bool {
    name.starts_with("sk_") || name.contains("sK")
}

fn check_algebraic_identities(
    l: &Term,
    r: &Term,
    commutative_symbols: &mut HashSet<SymbolId>,
    associative_symbols: &mut HashSet<SymbolId>,
    has_identity: &mut bool,
    has_inverse: &mut bool,
    has_idempotence: &mut bool,
) {
    // 1. Commutativity: f(X, Y) = f(Y, X)
    if let (Term::App(f1, args1), Term::App(f2, args2)) = (l, r)
        && f1 == f2
        && args1.len() == 2
        && args2.len() == 2
        && let (Term::Var(x1), Term::Var(y1), Term::Var(x2), Term::Var(y2)) =
            (&args1[0], &args1[1], &args2[0], &args2[1])
        && x1 == y2
        && y1 == x2
        && x1 != y1
    {
        commutative_symbols.insert(*f1);
    }

    // 2. Associativity: f(f(X, Y), Z) = f(X, f(Y, Z))
    if let Some(f) = check_associativity(l, r) {
        associative_symbols.insert(f);
    }

    // 3. Identity / Neutral element: f(X, e) = X or f(e, X) = X
    if check_identity(l, r) || check_identity(r, l) {
        *has_identity = true;
    }

    // 4. Inverse: f(X, i(X)) = e or f(i(X), X) = e
    if check_inverse(l, r) || check_inverse(r, l) {
        *has_inverse = true;
    }

    // 5. Idempotence: f(X, X) = X
    if let (Term::App(_f, args), Term::Var(v2)) = (l, r)
        && args.len() == 2
        && let (Term::Var(v1a), Term::Var(v1b)) = (&args[0], &args[1])
        && v1a == v1b
        && v1a == v2
    {
        *has_idempotence = true;
    }
}

fn is_assoc_form(
    f: SymbolId,
    left_inner: &Term,
    left_outer: &Term,
    right_outer: &Term,
    right_inner: &Term,
) -> bool {
    if let (Term::App(fi1, in_args), Term::Var(z)) = (left_inner, left_outer)
        && *fi1 == f
        && in_args.len() == 2
        && let (Term::Var(x), Term::Var(y)) = (&in_args[0], &in_args[1])
        && let (Term::Var(rx), Term::App(fi2, r_args)) = (right_outer, right_inner)
        && *fi2 == f
        && r_args.len() == 2
        && let (Term::Var(ry), Term::Var(rz)) = (&r_args[0], &r_args[1])
        && x == rx
        && y == ry
        && z == rz
        && x != y
        && y != z
        && x != z
    {
        true
    } else {
        false
    }
}

fn check_associativity(l: &Term, r: &Term) -> Option<SymbolId> {
    if let (Term::App(f1, args1), Term::App(f2, args2)) = (l, r)
        && f1 == f2
        && args1.len() == 2
        && args2.len() == 2
    {
        // Case A: f(f(X, Y), Z) = f(X, f(Y, Z))
        if is_assoc_form(*f1, &args1[0], &args1[1], &args2[0], &args2[1]) {
            return Some(*f1);
        }
        // Case B: f(X, f(Y, Z)) = f(f(X, Y), Z)
        if is_assoc_form(*f1, &args2[0], &args2[1], &args1[0], &args1[1]) {
            return Some(*f1);
        }
    }
    None
}

fn check_identity(l: &Term, r: &Term) -> bool {
    if let (Term::App(_f, args), Term::Var(v)) = (l, r)
        && args.len() == 2
    {
        // f(X, e) = X where e is constant (arity 0)
        if let (Term::Var(vx), Term::App(_, const_args)) = (&args[0], &args[1])
            && vx == v
            && const_args.is_empty()
        {
            return true;
        }
        // f(e, X) = X
        if let (Term::App(_, const_args), Term::Var(vx)) = (&args[0], &args[1])
            && vx == v
            && const_args.is_empty()
        {
            return true;
        }
    }
    false
}

fn check_inverse(l: &Term, r: &Term) -> bool {
    if let (Term::App(_f, args), Term::App(_, const_args)) = (l, r)
        && args.len() == 2
        && const_args.is_empty()
    {
        // f(X, i(X)) = e
        if let (Term::Var(vx), Term::App(_, inv_args)) = (&args[0], &args[1])
            && inv_args.len() == 1
            && matches!(&inv_args[0], Term::Var(vy) if vy == vx)
        {
            return true;
        }
        // f(i(X), X) = e
        if let (Term::App(_, inv_args), Term::Var(vx)) = (&args[0], &args[1])
            && inv_args.len() == 1
            && matches!(&inv_args[0], Term::Var(vy) if vy == vx)
        {
            return true;
        }
    }
    false
}

impl fmt::Display for ProblemProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "================================================================================"
        )?;
        writeln!(
            f,
            "                       mrs DEEP PROBLEM PROFILE: {}",
            self.problem_name
        )?;
        writeln!(
            f,
            "================================================================================"
        )?;

        writeln!(f, " [ CLASSIFICATION & ROUTING ]")?;
        writeln!(f, "  Archetype:           {}", self.archetype)?;
        writeln!(f, "  CASC Division:       {}", self.casc_division)?;
        writeln!(f, "  Recommended Sched:   {}", self.recommended_schedule)?;
        writeln!(f, "  Recommended Engine:  {}", self.recommended_engine)?;
        writeln!(
            f,
            "  AVATAR Splitting:    {}",
            if self.recommended_avatar {
                "ENABLED"
            } else {
                "DISABLED"
            }
        )?;
        writeln!(
            f,
            "  SInE Axiom Gating:   {}",
            if self.recommended_sine {
                "ENABLED (>150 axioms)"
            } else {
                "DISABLED"
            }
        )?;
        writeln!(f)?;

        writeln!(f, " [ PROBLEM METADATA ]")?;
        writeln!(f, "  Domain:              {}", self.domain)?;
        writeln!(f, "  Dialect:             {}", self.dialect)?;
        if let Some(status) = &self.header_status {
            writeln!(f, "  Header Status:       {}", status)?;
        }
        if let Some(rating) = self.header_rating {
            writeln!(f, "  Header Rating:       {:.2}", rating)?;
        }
        writeln!(f)?;

        writeln!(f, " [ ALGEBRAIC & THEORY SIGNATURES ]")?;
        writeln!(
            f,
            "  Pure Unit Equality:  {}",
            if self.is_ueq { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  Pure Equational:     {}",
            if self.is_peq { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  Zero Equality (FNE): {}",
            if self.is_fne { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  Mixed Equality (FEQ):{}",
            if self.is_feq { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  AC Symbols Detected: {}",
            if self.has_ac_symbols {
                self.ac_symbols.join(", ")
            } else {
                "None".to_string()
            }
        )?;
        writeln!(
            f,
            "  Identity Axiom:      {}",
            if self.has_identity_axiom { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  Inverse Axiom:       {}",
            if self.has_inverse_axiom { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  Idempotence Axiom:   {}",
            if self.has_idempotence { "Yes" } else { "No" }
        )?;
        writeln!(f)?;

        writeln!(f, " [ CLAUSAL MORPHOLOGY & COMPLEXITY ]")?;
        writeln!(f, "  Total Clauses:       {}", self.num_clauses)?;
        writeln!(f, "  Total Literals:      {}", self.num_literals)?;
        writeln!(
            f,
            "  Unit Clauses:        {} ({:.1}%)",
            (self.unit_ratio * self.num_clauses as f32).round() as usize,
            self.unit_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Horn Clauses:        {} ({:.1}%)",
            (self.horn_ratio * self.num_clauses as f32).round() as usize,
            self.horn_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Definite Clauses:    {} ({:.1}%)",
            (self.definite_ratio * self.num_clauses as f32).round() as usize,
            self.definite_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Goal Clauses:        {} ({:.1}%)",
            (self.goal_clause_ratio * self.num_clauses as f32).round() as usize,
            self.goal_clause_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Ground Clauses:      {} ({:.1}%)",
            (self.ground_ratio * self.num_clauses as f32).round() as usize,
            self.ground_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Equality Literals:   {:.1}%",
            self.equality_literal_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Non-Linear Clauses:  {:.1}%",
            self.non_linear_var_ratio * 100.0
        )?;
        writeln!(
            f,
            "  Max / Avg Clause Len:{} / {:.2} literals",
            self.max_clause_len, self.avg_clause_len
        )?;
        writeln!(
            f,
            "  Max Pos Literals:    {} (AVATAR branching factor)",
            self.max_pos_literals
        )?;
        writeln!(f)?;

        writeln!(f, " [ TERM & VARIABLE STRUCTURE ]")?;
        writeln!(f, "  Unique Variables:    {}", self.num_variables)?;
        writeln!(
            f,
            "  Max/Avg Vars/Clause: {} / {:.2}",
            self.max_vars_per_clause, self.avg_vars_per_clause
        )?;
        writeln!(
            f,
            "  Max / Avg Term Depth:{} / {:.2}",
            self.max_term_depth, self.avg_term_depth
        )?;
        writeln!(
            f,
            "  Max / Avg Term Size: {} / {:.2}",
            self.max_term_size, self.avg_term_size
        )?;
        writeln!(
            f,
            "  Predicates / Max Ar: {} / {}",
            self.num_predicates, self.max_pred_arity
        )?;
        writeln!(
            f,
            "  Functions / Max Ar:  {} / {}",
            self.num_functions, self.max_fun_arity
        )?;
        writeln!(f, "  Constants Count:     {}", self.num_constants)?;
        writeln!(f, "  Skolem Symbols:      {}", self.skolem_symbols_count)?;
        writeln!(
            f,
            "  EPR (No Functors):   {}",
            if self.is_epr { "Yes" } else { "No" }
        )?;
        writeln!(
            f,
            "  FVO (Vars-Only Pred):{}",
            if self.is_fvo { "Yes" } else { "No" }
        )?;
        writeln!(f)?;

        writeln!(f, " [ CONJECTURAL TOPOLOGY ]")?;
        writeln!(
            f,
            "  Has Goal/Conjecture: {}",
            if self.has_conjecture {
                "Yes"
            } else {
                "No (Consistency check)"
            }
        )?;
        if self.has_conjecture {
            writeln!(f, "  Conjecture Clauses:  {}", self.conjecture_clauses)?;
            writeln!(f, "  Conjecture Literals: {}", self.conjecture_literals)?;
            writeln!(f, "  Conjecture Max Depth:{}", self.conjecture_max_depth)?;
            writeln!(
                f,
                "  Goal-Axiom Overlap:  {:.1}%",
                self.conjecture_symbol_overlap * 100.0
            )?;
            if !self.unique_conjecture_symbols.is_empty() {
                writeln!(
                    f,
                    "  Unique Goal Symbols: {}",
                    self.unique_conjecture_symbols.join(", ")
                )?;
            }
        }
        writeln!(
            f,
            "================================================================================"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clause::{ClauseId, Literal};
    use smallvec::smallvec;

    #[test]
    fn test_empty_problem() {
        let symbols = SymbolTable::new();
        let clauses = vec![];
        let profile = ProblemProfile::extract("EMPTY.p", None, &clauses, &symbols);
        assert_eq!(profile.num_clauses, 0);
        assert_eq!(profile.archetype, ProblemArchetype::Empty);
        assert_eq!(profile.domain, "EMPTY");
    }

    #[test]
    fn test_pure_unit_equality_ac_group() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let e = symbols.intern("e");
        let inv = symbols.intern("inv");

        let x: VarId = 0;
        let y: VarId = 1;
        let z: VarId = 2;

        // c1: f(X, Y) = f(Y, X) [Commutativity]
        let c1 = Clause::new(
            ClauseId(1),
            smallvec![Literal {
                positive: true,
                atom: Atom::Eq(
                    Term::App(f, vec![Term::Var(x), Term::Var(y)]),
                    Term::App(f, vec![Term::Var(y), Term::Var(x)]),
                ),
            }],
            ClauseSource::Input {
                name: "comm".into(),
                role: "axiom".into(),
            },
        );

        // c2: f(f(X, Y), Z) = f(X, f(Y, Z)) [Associativity]
        let c2 = Clause::new(
            ClauseId(2),
            smallvec![Literal {
                positive: true,
                atom: Atom::Eq(
                    Term::App(
                        f,
                        vec![Term::App(f, vec![Term::Var(x), Term::Var(y)]), Term::Var(z)],
                    ),
                    Term::App(
                        f,
                        vec![Term::Var(x), Term::App(f, vec![Term::Var(y), Term::Var(z)])],
                    ),
                ),
            }],
            ClauseSource::Input {
                name: "assoc".into(),
                role: "axiom".into(),
            },
        );

        // c3: f(X, e) = X [Identity]
        let c3 = Clause::new(
            ClauseId(3),
            smallvec![Literal {
                positive: true,
                atom: Atom::Eq(
                    Term::App(f, vec![Term::Var(x), Term::App(e, vec![])]),
                    Term::Var(x),
                ),
            }],
            ClauseSource::Input {
                name: "ident".into(),
                role: "axiom".into(),
            },
        );

        // c4: f(X, inv(X)) = e [Inverse]
        let c4 = Clause::new(
            ClauseId(4),
            smallvec![Literal {
                positive: true,
                atom: Atom::Eq(
                    Term::App(f, vec![Term::Var(x), Term::App(inv, vec![Term::Var(x)])]),
                    Term::App(e, vec![]),
                ),
            }],
            ClauseSource::Input {
                name: "inverse".into(),
                role: "axiom".into(),
            },
        );

        // c5: f(X, X) = X [Idempotence]
        let c5 = Clause::new(
            ClauseId(5),
            smallvec![Literal {
                positive: true,
                atom: Atom::Eq(Term::App(f, vec![Term::Var(x), Term::Var(x)]), Term::Var(x),),
            }],
            ClauseSource::Input {
                name: "idemp".into(),
                role: "axiom".into(),
            },
        );

        let clauses = vec![c1, c2, c3, c4, c5];
        let profile = ProblemProfile::extract("GRP001-1.p", None, &clauses, &symbols);

        assert_eq!(profile.domain, "GRP");
        assert!(profile.is_ueq);
        assert!(profile.is_peq);
        assert!(!profile.is_fne);
        assert!(profile.has_ac_symbols);
        assert_eq!(profile.ac_symbols, vec!["f"]);
        assert!(profile.has_identity_axiom);
        assert!(profile.has_inverse_axiom);
        assert!(profile.has_idempotence);
        assert_eq!(profile.archetype, ProblemArchetype::PureUnitEquality);
        assert_eq!(profile.casc_division, "UEQ");
        assert_eq!(profile.recommended_engine, "TweeCompletion");
        assert!(!profile.recommended_avatar);
    }

    #[test]
    fn test_epr_and_fvo() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");

        let x: VarId = 0;

        // c1: p(X) | ~q(X)
        let c1 = Clause::new(
            ClauseId(1),
            smallvec![
                Literal {
                    positive: true,
                    atom: Atom::Pred(p, vec![Term::Var(x)]),
                },
                Literal {
                    positive: false,
                    atom: Atom::Pred(q, vec![Term::Var(x)]),
                },
            ],
            ClauseSource::Input {
                name: "c1".into(),
                role: "axiom".into(),
            },
        );

        // c2: q(a)
        let c2 = Clause::new(
            ClauseId(2),
            smallvec![Literal {
                positive: true,
                atom: Atom::Pred(q, vec![Term::App(a, vec![])]),
            }],
            ClauseSource::Input {
                name: "c2".into(),
                role: "axiom".into(),
            },
        );

        let clauses = vec![c1, c2];
        let profile = ProblemProfile::extract("SYN001.p", None, &clauses, &symbols);

        assert!(profile.is_epr);
        assert!(!profile.is_fvo); // c2 has constant 'a', not a variable
        assert_eq!(
            profile.archetype,
            ProblemArchetype::EssentiallyPropositional
        );
        assert_eq!(profile.casc_division, "EPR");
        assert_eq!(profile.recommended_engine, "InstGen");
    }

    #[test]
    fn equality_is_not_fvo() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let p = symbols.intern("p");
        let x: VarId = 0;

        let clause = Clause::new(
            ClauseId(1),
            smallvec![Literal {
                positive: true,
                atom: Atom::Eq(
                    Term::App(f, vec![Term::Var(x)]),
                    Term::App(f, vec![Term::Var(x)]),
                ),
            }],
            ClauseSource::Input {
                name: "eq".into(),
                role: "axiom".into(),
            },
        );

        let profile = ProblemProfile::extract("FOO001.p", None, &[clause], &symbols);
        assert!(!profile.is_fvo);
        assert_ne!(profile.archetype, ProblemArchetype::PropositionalSkeleton);

        let predicate = Clause::new(
            ClauseId(2),
            smallvec![Literal {
                positive: true,
                atom: Atom::Pred(p, vec![Term::Var(x)]),
            }],
            ClauseSource::Input {
                name: "p".into(),
                role: "axiom".into(),
            },
        );
        let profile = ProblemProfile::extract("FOO001.p", None, &[predicate], &symbols);
        assert!(profile.is_fvo);
    }

    #[test]
    fn non_premise_roles_are_not_counted_as_axioms() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let clause = Clause::new(
            ClauseId(1),
            smallvec![Literal {
                positive: true,
                atom: Atom::Pred(p, vec![]),
            }],
            ClauseSource::Input {
                name: "type".into(),
                role: "type".into(),
            },
        );
        let metadata = InputMetadata {
            input_axioms_count: Some(0),
            input_conjectures_count: Some(0),
            ..Default::default()
        };
        let profile = ProblemProfile::extract("FOO001.p", Some(&metadata), &[clause], &symbols);
        assert_eq!(profile.num_axioms, 0);
        assert_eq!(profile.num_conjectures, 0);
    }

    #[test]
    fn formula_counts_are_not_clause_counts() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let x: VarId = 0;
        let clauses = (0..3)
            .map(|id| {
                Clause::new(
                    ClauseId(id),
                    smallvec![Literal {
                        positive: true,
                        atom: Atom::Pred(p, vec![Term::Var(x)]),
                    }],
                    ClauseSource::Input {
                        name: format!("c{id}"),
                        role: "axiom".into(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let metadata = InputMetadata {
            input_axioms_count: Some(1),
            input_conjectures_count: Some(0),
            ..Default::default()
        };

        let profile = ProblemProfile::extract("FOO001.p", Some(&metadata), &clauses, &symbols);
        assert_eq!(profile.num_clauses, 3);
        assert_eq!(profile.num_axioms, 1);
        assert!(!profile.is_large_theory);
    }

    #[test]
    fn test_horn_and_conjecture_overlap() {
        let mut symbols = SymbolTable::new();
        let edge = symbols.intern("edge");
        let path = symbols.intern("path");
        let start = symbols.intern("start");
        let goal = symbols.intern("goal");

        let x: VarId = 0;
        let y: VarId = 1;
        let z: VarId = 2;

        // c1: path(X, Y) <- edge(X, Y)  [Horn definite]
        let c1 = Clause::new(
            ClauseId(1),
            smallvec![
                Literal {
                    positive: true,
                    atom: Atom::Pred(path, vec![Term::Var(x), Term::Var(y)]),
                },
                Literal {
                    positive: false,
                    atom: Atom::Pred(edge, vec![Term::Var(x), Term::Var(y)]),
                },
            ],
            ClauseSource::Input {
                name: "base".into(),
                role: "axiom".into(),
            },
        );

        // c2: path(X, Z) <- path(X, Y), edge(Y, Z) [Horn definite]
        let c2 = Clause::new(
            ClauseId(2),
            smallvec![
                Literal {
                    positive: true,
                    atom: Atom::Pred(path, vec![Term::Var(x), Term::Var(z)]),
                },
                Literal {
                    positive: false,
                    atom: Atom::Pred(path, vec![Term::Var(x), Term::Var(y)]),
                },
                Literal {
                    positive: false,
                    atom: Atom::Pred(edge, vec![Term::Var(y), Term::Var(z)]),
                },
            ],
            ClauseSource::Input {
                name: "trans".into(),
                role: "axiom".into(),
            },
        );

        // c3: ~path(start, goal) [Horn goal clause, conjecture]
        let c3 = Clause::new(
            ClauseId(3),
            smallvec![Literal {
                positive: false,
                atom: Atom::Pred(
                    path,
                    vec![Term::App(start, vec![]), Term::App(goal, vec![])],
                ),
            }],
            ClauseSource::Input {
                name: "query".into(),
                role: "negated_conjecture".into(),
            },
        );

        let clauses = vec![c1, c2, c3];
        let profile = ProblemProfile::extract("CSR026+3.p", None, &clauses, &symbols);

        assert_eq!(profile.domain, "CSR");
        assert_eq!(profile.horn_ratio, 1.0);
        assert!(profile.has_conjecture);
        assert!(profile.conjecture_symbol_overlap < 1.0);
        assert_eq!(profile.unique_conjecture_symbols, vec!["goal", "start"]);
    }

    #[test]
    fn test_profile_json_roundtrip() {
        let symbols = SymbolTable::new();
        let profile = ProblemProfile::extract("GRP001-1.p", None, &[], &symbols);
        let json = serde_json::to_string(&profile).expect("serialize");
        let deserialized: ProblemProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.problem_name, "GRP001-1.p");
        assert_eq!(deserialized.archetype, ProblemArchetype::Empty);
    }
}

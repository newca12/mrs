//! Lowering: converts the TPTP parser's AST into `mrs-core` types.
//!
//! The parser ([`mrs_tptp`]) produces a zero-copy AST with `&str` references.
//! This module converts those into the owned, interned `mrs-core` types
//! (`Term`, `Formula`, `Atom`, `Literal`, `Clause`) that the prover works with.
//!
//! Variable names (uppercase words in TPTP) are mapped to unique [`VarId`]s.
//! Function and predicate names are interned into the [`SymbolTable`].

use std::collections::HashMap;

use mrs_core::{
    Atom, Clause, ClauseSource, Formula, Literal, SymbolId, SymbolTable, Term, VarId,
    clause::ClauseIdGen,
};
use mrs_tptp::{
    AnnotatedFormula, BinaryConnective, CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement,
    FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, FormulaRole, Quantifier, TCFAtomicFormula,
    TCFClause, TCFFormula, TCFLiteral, TCFStatement, TFFAtomicFormula, TFFFormula, TFFStatement,
    TFFTerm, TPTPProblem,
};

/// Result of lowering a TPTP problem into core types.
pub struct LoweredProblem {
    /// The symbol table with all interned names.
    pub symbols: SymbolTable,
    /// Axioms: formulas with axiom-like roles (need clausification).
    pub axioms: Vec<LoweredFormula>,
    /// Conjectures: formulas with the conjecture role (need clausification).
    pub conjectures: Vec<LoweredFormula>,
    /// Pre-clausified clauses from CNF input (skip clausification).
    pub cnf_clauses: Vec<Clause>,
    /// Clause ID generator, continues from IDs assigned to CNF clauses.
    pub id_gen: ClauseIdGen,
}

/// A formula with its metadata from the TPTP input.
pub struct LoweredFormula {
    /// Original name from the TPTP file.
    pub name: String,
    /// The TPTP role string.
    pub role: String,
    /// The lowered formula.
    pub formula: Formula,
}

/// Lowering context: tracks variable name → VarId mapping and symbol interning.
struct LowerCtx<'a> {
    symbols: &'a mut SymbolTable,
    /// Maps variable names to VarId within the current scope.
    var_map: HashMap<String, VarId>,
    /// Counter for generating fresh variable IDs.
    next_var: VarId,
}

impl<'a> LowerCtx<'a> {
    fn new(symbols: &'a mut SymbolTable) -> Self {
        Self {
            symbols,
            var_map: HashMap::new(),
            next_var: 0,
        }
    }

    /// Resets variable mapping for a new formula.
    fn reset_vars(&mut self) {
        self.var_map.clear();
        self.next_var = 0;
    }

    /// Gets or creates a VarId for the given variable name.
    fn var_id(&mut self, name: &str) -> VarId {
        if let Some(&id) = self.var_map.get(name) {
            return id;
        }
        let id = self.next_var;
        self.next_var += 1;
        self.var_map.insert(name.to_string(), id);
        id
    }

    /// Interns a symbol name.
    fn intern(&mut self, name: &str) -> SymbolId {
        self.symbols.intern(name)
    }
}

/// Lowers an entire TPTP problem into core types.
///
/// This processes all FOF and CNF formulas, converting them to `mrs-core` types.
/// TFF, TCF, and THF formulas are currently skipped with a warning.
pub fn lower_problem(problem: &TPTPProblem<'_>) -> LoweredProblem {
    let mut symbols = SymbolTable::new();
    let mut axioms = Vec::new();
    let mut conjectures = Vec::new();
    let mut cnf_clauses = Vec::new();
    let mut id_gen = ClauseIdGen::new();

    for formula in &problem.formulas {
        let mut ctx = LowerCtx::new(&mut symbols);
        ctx.reset_vars();

        match formula {
            AnnotatedFormula::FOF(fof) => {
                let name = fof.name.as_str().to_string();
                let role = fof.role;
                let result = match &fof.formula {
                    FOFStatement::Logical(f) => Some((name, role, lower_fof_formula(&mut ctx, f))),
                    FOFStatement::Sequent(_, _) => {
                        // Sequents are rare; skip for now
                        None
                    }
                };
                if let Some((name, role, formula)) = result {
                    let lowered = LoweredFormula {
                        name,
                        role: role.as_str().to_string(),
                        formula,
                    };
                    if role == FormulaRole::Conjecture {
                        conjectures.push(lowered);
                    } else {
                        axioms.push(lowered);
                    }
                }
            }
            AnnotatedFormula::CNF(cnf) => {
                // Lower CNF directly to clauses, bypassing clausification
                if let Some(clause) = lower_cnf_to_clause(&mut symbols, &mut id_gen, cnf) {
                    cnf_clauses.push(clause);
                }
            }
            AnnotatedFormula::TFF(tff) => {
                let name = tff.name.as_str().to_string();
                let role = tff.role;
                match &tff.formula {
                    TFFStatement::Logical(f) => {
                        if let Some(formula) = lower_tff_formula(&mut ctx, f) {
                            let lowered = LoweredFormula {
                                name,
                                role: role.as_str().to_string(),
                                formula,
                            };
                            if role == FormulaRole::Conjecture {
                                conjectures.push(lowered);
                            } else {
                                axioms.push(lowered);
                            }
                        } else {
                            eprintln!(
                                "% Warning: skipped TFF formula '{}' (unsupported constructs)",
                                name
                            );
                        }
                    }
                    // type declarations, sequents, and NXF logic specs are not logical content
                    TFFStatement::Typing(_)
                    | TFFStatement::Sequent(_, _)
                    | TFFStatement::Logic(_) => {}
                }
            }
            AnnotatedFormula::TCF(tcf) => {
                if let Some(clause) = lower_tcf_to_clause(&mut symbols, &mut id_gen, tcf) {
                    cnf_clauses.push(clause);
                }
            }
            // THF (higher-order) and TPI (process instructions) are not supported
            _ => {}
        }
    }

    LoweredProblem {
        symbols,
        axioms,
        conjectures,
        cnf_clauses,
        id_gen,
    }
}

/// Lowers additional formulas into an existing `LoweredProblem`.
///
/// Shares the same symbol table and clause ID generator, so all formulas
/// (from the main problem and included files) use consistent interning.
/// Optionally filters by a set of formula names (for include selection).
pub fn lower_into(
    lowered: &mut LoweredProblem,
    problem: &TPTPProblem<'_>,
    selection: Option<&[&str]>,
) {
    for formula in &problem.formulas {
        // Filter by selection if provided
        if let Some(sel) = selection {
            let name = match formula {
                AnnotatedFormula::FOF(fof) => fof.name.as_str(),
                AnnotatedFormula::CNF(cnf) => cnf.name.as_str(),
                AnnotatedFormula::TFF(tff) => tff.name.as_str(),
                AnnotatedFormula::TCF(tcf) => tcf.name.as_str(),
                _ => continue,
            };
            if !sel.contains(&name) {
                continue;
            }
        }

        let mut ctx = LowerCtx::new(&mut lowered.symbols);
        ctx.reset_vars();

        match formula {
            AnnotatedFormula::FOF(fof) => {
                let name = fof.name.as_str().to_string();
                let role = fof.role;
                let result = match &fof.formula {
                    FOFStatement::Logical(f) => Some((name, role, lower_fof_formula(&mut ctx, f))),
                    FOFStatement::Sequent(_, _) => None,
                };
                if let Some((name, role, formula)) = result {
                    let lf = LoweredFormula {
                        name,
                        role: role.as_str().to_string(),
                        formula,
                    };
                    if role == FormulaRole::Conjecture {
                        lowered.conjectures.push(lf);
                    } else {
                        lowered.axioms.push(lf);
                    }
                }
            }
            AnnotatedFormula::CNF(cnf) => {
                if let Some(clause) =
                    lower_cnf_to_clause(&mut lowered.symbols, &mut lowered.id_gen, cnf)
                {
                    lowered.cnf_clauses.push(clause);
                }
            }
            AnnotatedFormula::TFF(tff) => {
                let name = tff.name.as_str().to_string();
                let role = tff.role;
                match &tff.formula {
                    TFFStatement::Logical(f) => {
                        if let Some(formula) = lower_tff_formula(&mut ctx, f) {
                            let lf = LoweredFormula {
                                name,
                                role: role.as_str().to_string(),
                                formula,
                            };
                            if role == FormulaRole::Conjecture {
                                lowered.conjectures.push(lf);
                            } else {
                                lowered.axioms.push(lf);
                            }
                        } else {
                            eprintln!(
                                "% Warning: skipped TFF formula '{}' (unsupported constructs)",
                                name
                            );
                        }
                    }
                    TFFStatement::Typing(_)
                    | TFFStatement::Sequent(_, _)
                    | TFFStatement::Logic(_) => {}
                }
            }
            AnnotatedFormula::TCF(tcf) => {
                if let Some(clause) =
                    lower_tcf_to_clause(&mut lowered.symbols, &mut lowered.id_gen, tcf)
                {
                    lowered.cnf_clauses.push(clause);
                }
            }
            _ => {}
        }
    }
}

/// Lowers a FOF formula to a core `Formula`.
fn lower_fof_formula(ctx: &mut LowerCtx<'_>, f: &FOFFormula<'_>) -> Formula {
    match f {
        FOFFormula::Atomic(a) => lower_fof_atomic(ctx, a),
        FOFFormula::Negation(inner) => Formula::neg(lower_fof_formula(ctx, inner)),
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            // Save old bindings and create fresh VarIds for the quantified variables.
            // This handles variable shadowing: if an inner quantifier reuses a name
            // like X that was already bound by an outer quantifier, we create a new
            // VarId so the two X's don't collide.
            let mut saved: Vec<(String, Option<VarId>)> = Vec::new();
            let mut var_ids: Vec<VarId> = Vec::new();

            for v in variables {
                let name = (*v).to_string();
                let old = ctx.var_map.get(&name).copied();
                saved.push((name.clone(), old));

                // Always create a fresh VarId for each quantified variable
                let id = ctx.next_var;
                ctx.next_var += 1;
                ctx.var_map.insert(name, id);
                var_ids.push(id);
            }

            let body = lower_fof_formula(ctx, formula);

            // Restore old bindings
            for (name, old) in saved {
                match old {
                    Some(id) => {
                        ctx.var_map.insert(name, id);
                    }
                    None => {
                        ctx.var_map.remove(&name);
                    }
                }
            }

            // Nest quantifiers: ![X, Y]: F becomes ![X]: ![Y]: F
            let mut result = body;
            for &vid in var_ids.iter().rev() {
                result = match quantifier {
                    Quantifier::Forall => Formula::forall(vid, result),
                    Quantifier::Exists => Formula::exists(vid, result),
                };
            }
            result
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => {
            let l = lower_fof_formula(ctx, left);
            let r = lower_fof_formula(ctx, right);
            match connective {
                BinaryConnective::And => Formula::and(vec![l, r]),
                BinaryConnective::Or => Formula::or(vec![l, r]),
                BinaryConnective::Impl => Formula::implies(l, r),
                BinaryConnective::RevImpl => Formula::implies(r, l),
                BinaryConnective::Iff => Formula::iff(l, r),
                BinaryConnective::Xor => Formula::neg(Formula::iff(l, r)),
                BinaryConnective::Nor => Formula::neg(Formula::or(vec![l, r])),
                BinaryConnective::Nand => Formula::neg(Formula::and(vec![l, r])),
            }
        }
        FOFFormula::Equality(l, r) => {
            let lt = lower_fof_term(ctx, l);
            let rt = lower_fof_term(ctx, r);
            Formula::atom(Atom::eq(lt, rt))
        }
        FOFFormula::Inequality(l, r) => {
            let lt = lower_fof_term(ctx, l);
            let rt = lower_fof_term(ctx, r);
            Formula::neg(Formula::atom(Atom::eq(lt, rt)))
        }
        FOFFormula::Parens(inner) => lower_fof_formula(ctx, inner),
    }
}

/// Lowers a FOF atomic formula.
fn lower_fof_atomic(ctx: &mut LowerCtx<'_>, a: &FOFAtomicFormula<'_>) -> Formula {
    match a {
        FOFAtomicFormula::Plain(word, args) => {
            let sym = ctx.intern(word.as_str());
            let terms: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, terms))
        }
        FOFAtomicFormula::Defined(word, args) => {
            let name = format!("${}", word.0);
            let sym = ctx.intern(&name);
            let terms: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, terms))
        }
        FOFAtomicFormula::System(word, args) => {
            let name = format!("$${}", word.0);
            let sym = ctx.intern(&name);
            let terms: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, terms))
        }
        FOFAtomicFormula::True => Formula::True,
        FOFAtomicFormula::False => Formula::False,
    }
}

/// Lowers a FOF term.
fn lower_fof_term(ctx: &mut LowerCtx<'_>, t: &FOFTerm<'_>) -> Term {
    match t {
        FOFTerm::Variable(name) => {
            let vid = ctx.var_id(name);
            Term::var(vid)
        }
        FOFTerm::Function(word, args) => {
            let sym = ctx.intern(word.as_str());
            let terms: Vec<Term> = args.iter().map(|a| lower_fof_term(ctx, a)).collect();
            Term::app(sym, terms)
        }
        FOFTerm::DefinedFunction(word, args) => {
            let name = format!("${}", word.0);
            let sym = ctx.intern(&name);
            let terms: Vec<Term> = args.iter().map(|a| lower_fof_term(ctx, a)).collect();
            Term::app(sym, terms)
        }
        FOFTerm::SystemFunction(word, args) => {
            let name = format!("$${}", word.0);
            let sym = ctx.intern(&name);
            let terms: Vec<Term> = args.iter().map(|a| lower_fof_term(ctx, a)).collect();
            Term::app(sym, terms)
        }
        FOFTerm::Number(n) => {
            // Represent numbers as constants with their string representation
            let name = format!("{n}");
            let sym = ctx.intern(&name);
            Term::constant(sym)
        }
        FOFTerm::DistinctObject(s) => {
            let name = format!("\"{s}\"");
            let sym = ctx.intern(&name);
            Term::constant(sym)
        }
    }
}

/// Lowers a CNF formula directly into `mrs-core` clause representation.
/// This is useful when the input is already in CNF and doesn't need clausification.
pub fn lower_cnf_to_clause(
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    cnf: &mrs_tptp::CNFAnnotated<'_>,
) -> Option<Clause> {
    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();

    let name = cnf.name.as_str().to_string();
    let role = cnf.role.as_str().to_string();

    match &cnf.formula {
        CNFStatement::Logical(f) => {
            let literals = lower_cnf_to_literals(&mut ctx, f);
            Some(Clause::new(
                id_gen.next(),
                literals,
                ClauseSource::Input { name, role },
            ))
        }
    }
}

/// Lowers a CNF formula to a vector of literals.
fn lower_cnf_to_literals(ctx: &mut LowerCtx<'_>, f: &CNFFormula<'_>) -> Vec<Literal> {
    let raw_lits = f.literals();
    raw_lits.iter().map(|l| lower_cnf_literal(ctx, l)).collect()
}

/// Lowers a single CNF literal.
fn lower_cnf_literal(ctx: &mut LowerCtx<'_>, lit: &CNFLiteral<'_>) -> Literal {
    match lit {
        CNFLiteral::Positive(a) => Literal::pos(lower_cnf_atomic(ctx, a)),
        CNFLiteral::Negative(a) => Literal::neg(lower_cnf_atomic(ctx, a)),
        CNFLiteral::Equality(l, r) => {
            let lt = lower_fof_term(ctx, l);
            let rt = lower_fof_term(ctx, r);
            Literal::pos(Atom::eq(lt, rt))
        }
        CNFLiteral::Inequality(l, r) => {
            let lt = lower_fof_term(ctx, l);
            let rt = lower_fof_term(ctx, r);
            Literal::neg(Atom::eq(lt, rt))
        }
    }
}

/// Lowers a CNF atomic formula to an `Atom`.
fn lower_cnf_atomic(ctx: &mut LowerCtx<'_>, a: &CNFAtomicFormula<'_>) -> Atom {
    match a {
        CNFAtomicFormula::Plain(word, args) => {
            let sym = ctx.intern(word.as_str());
            let terms: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Atom::pred(sym, terms)
        }
        CNFAtomicFormula::Defined(word, args) => {
            let name = format!("${}", word.0);
            let sym = ctx.intern(&name);
            let terms: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Atom::pred(sym, terms)
        }
        CNFAtomicFormula::System(word, args) => {
            let name = format!("$${}", word.0);
            let sym = ctx.intern(&name);
            let terms: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Atom::pred(sym, terms)
        }
        // $true and $false as atoms: use special symbol names
        CNFAtomicFormula::True => {
            let sym = ctx.intern("$true");
            Atom::pred(sym, vec![])
        }
        CNFAtomicFormula::False => {
            let sym = ctx.intern("$false");
            Atom::pred(sym, vec![])
        }
    }
}

// ---------------------------------------------------------------------------
// TFF lowering (Typed First-order Form → untyped mrs-core types)
// ---------------------------------------------------------------------------
//
// TFF0 is structurally identical to FOF; type annotations on variables and
// symbols are ignored (the prover is untyped).  TXF / TF1 / NXF extensions
// (conditional terms, let-expressions, tuples, type quantifiers, non-classical
// operators) are not supported and cause the enclosing formula to be skipped
// with a warning.

/// Lowers a TFF formula to a core `Formula`.
///
/// Returns `None` if the formula uses TXF/TF1/NXF constructs that the
/// untyped prover cannot represent.
fn lower_tff_formula(ctx: &mut LowerCtx<'_>, f: &TFFFormula<'_>) -> Option<Formula> {
    match f {
        TFFFormula::Atomic(a) => lower_tff_atomic(ctx, a),
        TFFFormula::Negation(inner) => Some(Formula::neg(lower_tff_formula(ctx, inner)?)),
        TFFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            // Save old bindings for shadowing support (same logic as FOF).
            let mut saved: Vec<(String, Option<VarId>)> = Vec::new();
            let mut var_ids: Vec<VarId> = Vec::new();
            for v in variables {
                let name = v.name.to_string();
                let old = ctx.var_map.get(&name).copied();
                saved.push((name.clone(), old));
                let id = ctx.next_var;
                ctx.next_var += 1;
                ctx.var_map.insert(name, id);
                var_ids.push(id);
            }
            let body = lower_tff_formula(ctx, formula)?;
            for (name, old) in saved {
                match old {
                    Some(id) => {
                        ctx.var_map.insert(name, id);
                    }
                    None => {
                        ctx.var_map.remove(&name);
                    }
                }
            }
            let mut result = body;
            for &vid in var_ids.iter().rev() {
                result = match quantifier {
                    Quantifier::Forall => Formula::forall(vid, result),
                    Quantifier::Exists => Formula::exists(vid, result),
                };
            }
            Some(result)
        }
        // TF1 type quantifiers (!> / ?*) — not supported
        TFFFormula::TypeQuantified { .. } => None,
        TFFFormula::Binary {
            left,
            connective,
            right,
        } => {
            let l = lower_tff_formula(ctx, left)?;
            let r = lower_tff_formula(ctx, right)?;
            Some(match connective {
                BinaryConnective::And => Formula::and(vec![l, r]),
                BinaryConnective::Or => Formula::or(vec![l, r]),
                BinaryConnective::Impl => Formula::implies(l, r),
                BinaryConnective::RevImpl => Formula::implies(r, l),
                BinaryConnective::Iff => Formula::iff(l, r),
                BinaryConnective::Xor => Formula::neg(Formula::iff(l, r)),
                BinaryConnective::Nor => Formula::neg(Formula::or(vec![l, r])),
                BinaryConnective::Nand => Formula::neg(Formula::and(vec![l, r])),
            })
        }
        TFFFormula::Equality(l, r) => {
            let lt = lower_tff_term(ctx, l)?;
            let rt = lower_tff_term(ctx, r)?;
            Some(Formula::atom(Atom::eq(lt, rt)))
        }
        TFFFormula::Inequality(l, r) => {
            let lt = lower_tff_term(ctx, l)?;
            let rt = lower_tff_term(ctx, r)?;
            Some(Formula::neg(Formula::atom(Atom::eq(lt, rt))))
        }
        TFFFormula::Parens(inner) => lower_tff_formula(ctx, inner),
        // TXF / NXF extensions — not supported
        TFFFormula::Conditional { .. }
        | TFFFormula::Let { .. }
        | TFFFormula::NonClassical { .. } => None,
    }
}

/// Lowers a TFF atomic formula.
fn lower_tff_atomic(ctx: &mut LowerCtx<'_>, a: &TFFAtomicFormula<'_>) -> Option<Formula> {
    match a {
        TFFAtomicFormula::Plain(word, args) => {
            let sym = ctx.intern(word.as_str());
            let terms = args
                .iter()
                .map(|t| lower_tff_term(ctx, t))
                .collect::<Option<Vec<_>>>()?;
            Some(Formula::atom(Atom::pred(sym, terms)))
        }
        TFFAtomicFormula::Defined(word, args) => {
            let name = format!("${}", word.0);
            let sym = ctx.intern(&name);
            let terms = args
                .iter()
                .map(|t| lower_tff_term(ctx, t))
                .collect::<Option<Vec<_>>>()?;
            Some(Formula::atom(Atom::pred(sym, terms)))
        }
        TFFAtomicFormula::System(word, args) => {
            let name = format!("$${}", word.0);
            let sym = ctx.intern(&name);
            let terms = args
                .iter()
                .map(|t| lower_tff_term(ctx, t))
                .collect::<Option<Vec<_>>>()?;
            Some(Formula::atom(Atom::pred(sym, terms)))
        }
        TFFAtomicFormula::True => Some(Formula::True),
        TFFAtomicFormula::False => Some(Formula::False),
        // FOOL boolean variable used as a formula — not supported
        TFFAtomicFormula::Variable(_) => None,
    }
}

/// Lowers a TFF term.
///
/// Returns `None` for TXF/FOOL extensions (conditional terms, let-expressions,
/// tuples, formulas-as-terms) that the untyped prover cannot represent.
fn lower_tff_term(ctx: &mut LowerCtx<'_>, t: &TFFTerm<'_>) -> Option<Term> {
    match t {
        TFFTerm::Variable(name) => {
            let vid = ctx.var_id(name);
            Some(Term::var(vid))
        }
        TFFTerm::Function(word, args) => {
            let sym = ctx.intern(word.as_str());
            let terms = args
                .iter()
                .map(|a| lower_tff_term(ctx, a))
                .collect::<Option<Vec<_>>>()?;
            Some(Term::app(sym, terms))
        }
        TFFTerm::DefinedFunction(word, args) => {
            let name = format!("${}", word.0);
            let sym = ctx.intern(&name);
            let terms = args
                .iter()
                .map(|a| lower_tff_term(ctx, a))
                .collect::<Option<Vec<_>>>()?;
            Some(Term::app(sym, terms))
        }
        TFFTerm::SystemFunction(word, args) => {
            let name = format!("$${}", word.0);
            let sym = ctx.intern(&name);
            let terms = args
                .iter()
                .map(|a| lower_tff_term(ctx, a))
                .collect::<Option<Vec<_>>>()?;
            Some(Term::app(sym, terms))
        }
        TFFTerm::Number(n) => {
            let name = format!("{n}");
            let sym = ctx.intern(&name);
            Some(Term::constant(sym))
        }
        TFFTerm::DistinctObject(s) => {
            let name = format!("\"{s}\"");
            let sym = ctx.intern(&name);
            Some(Term::constant(sym))
        }
        TFFTerm::Parens(inner) => lower_tff_term(ctx, inner),
        // TXF / FOOL extensions — not supported
        TFFTerm::Conditional { .. }
        | TFFTerm::Let { .. }
        | TFFTerm::Tuple(_)
        | TFFTerm::FormulaAsTerm(_) => None,
    }
}

// ---------------------------------------------------------------------------
// TCF lowering (Typed Clause Form → untyped mrs-core clauses)
// ---------------------------------------------------------------------------
//
// TCF is typed CNF: each formula is a universally-quantified clause with typed
// variables and TFF-style terms.  Types are ignored; `lower_tff_term` is
// reused for terms.  Type declarations (TCFStatement::Typing) are skipped.

/// Lowers a TCF annotated formula directly into a `mrs-core` clause.
pub fn lower_tcf_to_clause(
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    tcf: &mrs_tptp::TCFAnnotated<'_>,
) -> Option<Clause> {
    let name = tcf.name.as_str().to_string();
    let role = tcf.role.as_str().to_string();
    match &tcf.formula {
        TCFStatement::Logical(f) => {
            let mut ctx = LowerCtx::new(symbols);
            ctx.reset_vars();
            let literals = lower_tcf_formula(&mut ctx, f)?;
            Some(Clause::new(
                id_gen.next(),
                literals,
                ClauseSource::Input { name, role },
            ))
        }
        // Type declarations are not logical content
        TCFStatement::Typing(_) => None,
    }
}

fn lower_tcf_formula(ctx: &mut LowerCtx<'_>, f: &TCFFormula<'_>) -> Option<Vec<Literal>> {
    match f {
        TCFFormula::Quantified { variables, clause } => {
            // Register quantified variables (types are ignored)
            for v in variables {
                ctx.var_id(v.name);
            }
            lower_tcf_clause(ctx, clause)
        }
        TCFFormula::Clause(clause) => lower_tcf_clause(ctx, clause),
    }
}

fn lower_tcf_clause(ctx: &mut LowerCtx<'_>, c: &TCFClause<'_>) -> Option<Vec<Literal>> {
    match c {
        TCFClause::Disjunction(lits) => lits
            .iter()
            .map(|l| lower_tcf_literal(ctx, l))
            .collect::<Option<Vec<_>>>(),
        TCFClause::Parens(inner) => lower_tcf_clause(ctx, inner),
    }
}

fn lower_tcf_literal(ctx: &mut LowerCtx<'_>, lit: &TCFLiteral<'_>) -> Option<Literal> {
    match lit {
        TCFLiteral::Positive(a) => Some(Literal::pos(lower_tcf_atomic(ctx, a)?)),
        TCFLiteral::Negative(a) => Some(Literal::neg(lower_tcf_atomic(ctx, a)?)),
        TCFLiteral::Equality(l, r) => {
            let lt = lower_tff_term(ctx, l)?;
            let rt = lower_tff_term(ctx, r)?;
            Some(Literal::pos(Atom::eq(lt, rt)))
        }
        TCFLiteral::Inequality(l, r) => {
            let lt = lower_tff_term(ctx, l)?;
            let rt = lower_tff_term(ctx, r)?;
            Some(Literal::neg(Atom::eq(lt, rt)))
        }
        TCFLiteral::Parens(inner) => lower_tcf_literal(ctx, inner),
    }
}

fn lower_tcf_atomic(ctx: &mut LowerCtx<'_>, a: &TCFAtomicFormula<'_>) -> Option<Atom> {
    match a {
        TCFAtomicFormula::Plain(word, args) => {
            let sym = ctx.intern(word.as_str());
            let terms = args
                .iter()
                .map(|t| lower_tff_term(ctx, t))
                .collect::<Option<Vec<_>>>()?;
            Some(Atom::pred(sym, terms))
        }
        TCFAtomicFormula::Defined(word, args) => {
            let name = format!("${}", word.0);
            let sym = ctx.intern(&name);
            let terms = args
                .iter()
                .map(|t| lower_tff_term(ctx, t))
                .collect::<Option<Vec<_>>>()?;
            Some(Atom::pred(sym, terms))
        }
        TCFAtomicFormula::System(word, args) => {
            let name = format!("$${}", word.0);
            let sym = ctx.intern(&name);
            let terms = args
                .iter()
                .map(|t| lower_tff_term(ctx, t))
                .collect::<Option<Vec<_>>>()?;
            Some(Atom::pred(sym, terms))
        }
        TCFAtomicFormula::True => {
            let sym = ctx.intern("$true");
            Some(Atom::pred(sym, vec![]))
        }
        TCFAtomicFormula::False => {
            let sym = ctx.intern("$false");
            Some(Atom::pred(sym, vec![]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;

    fn parse_and_lower(input: &str) -> LoweredProblem {
        let problem = mrs_tptp::parse_tptp(input).expect("parse failed");
        lower_problem(&problem)
    }

    #[test]
    fn lower_simple_axiom() {
        let input = "fof(ax1, axiom, p(a)).";
        let result = parse_and_lower(input);
        assert_eq!(result.axioms.len(), 1);
        assert_eq!(result.conjectures.len(), 0);

        let f = &result.axioms[0];
        assert_eq!(f.name, "ax1");
        assert_eq!(f.role, "axiom");
        let display = format!("{}", f.formula.display(&result.symbols));
        assert_eq!(display, "p(a)");
    }

    #[test]
    fn lower_quantified() {
        let input = "fof(ax1, axiom, ![X]: (p(X) => q(X))).";
        let result = parse_and_lower(input);
        let f = &result.axioms[0];
        let display = format!("{}", f.formula.display(&result.symbols));
        assert_eq!(display, "![X0]: ((p(X0) => q(X0)))");
    }

    #[test]
    fn lower_conjecture() {
        let input = r#"
            fof(ax1, axiom, ![X]: (human(X) => mortal(X))).
            fof(ax2, axiom, human(socrates)).
            fof(goal, conjecture, mortal(socrates)).
        "#;
        let result = parse_and_lower(input);
        assert_eq!(result.axioms.len(), 2);
        assert_eq!(result.conjectures.len(), 1);
        assert_eq!(result.conjectures[0].name, "goal");
    }

    #[test]
    fn lower_equality() {
        let input = "fof(eq1, axiom, a = b).";
        let result = parse_and_lower(input);
        let display = format!("{}", result.axioms[0].formula.display(&result.symbols));
        assert_eq!(display, "a = b");
    }

    #[test]
    fn lower_cnf() {
        let input = "cnf(c1, axiom, p(X) | ~q(X, a)).";
        let result = parse_and_lower(input);
        assert_eq!(result.cnf_clauses.len(), 1);
        // CNF is lowered directly to clauses
        let display = format!("{}", result.cnf_clauses[0].display(&result.symbols));
        assert_eq!(display, "p(X0) | ~q(X0, a)");
    }

    #[test]
    fn lower_cnf_to_clause_direct() {
        let input = "cnf(c1, axiom, p(X) | ~q(X, a)).";
        let problem = mrs_tptp::parse_tptp(input).expect("parse failed");
        let mut symbols = SymbolTable::new();
        let mut id_gen = ClauseIdGen::new();

        if let AnnotatedFormula::CNF(cnf) = &problem.formulas[0] {
            let clause = lower_cnf_to_clause(&mut symbols, &mut id_gen, cnf).unwrap();
            assert_eq!(clause.len(), 2);
            assert!(clause.literals[0].is_positive());
            assert!(clause.literals[1].is_negative());

            let display = format!("{}", clause.display(&symbols));
            assert_eq!(display, "p(X0) | ~q(X0, a)");
        } else {
            panic!("expected CNF formula");
        }
    }

    #[test]
    fn variable_shadowing() {
        // The two X's in this formula are in different quantifier scopes.
        // They must get different VarIds so they are independent after clausification.
        let input = "fof(ax1, axiom, (?[X]: p(X)) => (![X]: q(X))).";
        let result = parse_and_lower(input);
        let f = &result.axioms[0];
        let display = format!("{}", f.formula.display(&result.symbols));
        // X0 from the existential, X1 from the universal
        assert_eq!(display, "(?[X0]: (p(X0)) => ![X1]: (q(X1)))");
    }

    #[test]
    fn variable_shadowing_restored() {
        // After inner quantifier, outer variable name should be restored.
        // ∀X. (p(X) ∧ ∃X. q(X)) — the outer X should be used outside the ∃ scope.
        // But since there's no reference to X outside ∃ scope in the body part,
        // let's test with: ∀X. (p(X) ∧ (∃X. q(X)) ∧ r(X))
        // Inner ∃X should shadow, but r(X) should use outer X.
        let input = "fof(ax1, axiom, ![X]: (p(X) & (?[X]: q(X)) & r(X))).";
        let result = parse_and_lower(input);
        let f = &result.axioms[0];
        let display = format!("{}", f.formula.display(&result.symbols));
        // X0 = outer ∀X, X1 = inner ∃X
        // p(X0) & (∃X1. q(X1)) & r(X0) — r uses the outer X
        assert_eq!(display, "![X0]: (((p(X0) & ?[X1]: (q(X1))) & r(X0)))");
    }

    // -----------------------------------------------------------------------
    // TFF lowering tests
    // -----------------------------------------------------------------------

    #[test]
    fn lower_tff_simple_axiom() {
        let input = "tff(ax1, axiom, p(a)).";
        let result = parse_and_lower(input);
        assert_eq!(result.axioms.len(), 1);
        assert_eq!(result.conjectures.len(), 0);
        assert_eq!(result.axioms[0].name, "ax1");
        let display = format!("{}", result.axioms[0].formula.display(&result.symbols));
        assert_eq!(display, "p(a)");
    }

    #[test]
    fn lower_tff_conjecture() {
        let input = r#"
            tff(ax1, axiom, human(socrates)).
            tff(goal, conjecture, mortal(socrates)).
        "#;
        let result = parse_and_lower(input);
        assert_eq!(result.axioms.len(), 1);
        assert_eq!(result.conjectures.len(), 1);
        assert_eq!(result.conjectures[0].name, "goal");
    }

    #[test]
    fn lower_tff_typed_variables_ignored() {
        // Type annotations on variables must be stripped; logic is unchanged.
        let input = "tff(ax1, axiom, ![X: $i]: (p(X) => q(X))).";
        let result = parse_and_lower(input);
        assert_eq!(result.axioms.len(), 1);
        let display = format!("{}", result.axioms[0].formula.display(&result.symbols));
        assert_eq!(display, "![X0]: ((p(X0) => q(X0)))");
    }

    #[test]
    fn lower_tff_type_declaration_skipped() {
        // tff(_, type, …) is a type declaration — must not produce axioms/conjectures.
        let input = r#"
            tff(human_type, type, human: $i > $o).
            tff(ax1, axiom, human(socrates)).
        "#;
        let result = parse_and_lower(input);
        // Only the logical axiom should be lowered; the type decl is silently dropped.
        assert_eq!(result.axioms.len(), 1);
        assert_eq!(result.axioms[0].name, "ax1");
    }

    #[test]
    fn lower_tff_equality() {
        let input = "tff(eq1, axiom, a = b).";
        let result = parse_and_lower(input);
        let display = format!("{}", result.axioms[0].formula.display(&result.symbols));
        assert_eq!(display, "a = b");
    }

    // -----------------------------------------------------------------------
    // TCF lowering tests
    // -----------------------------------------------------------------------

    #[test]
    fn lower_tcf_clause() {
        let input = "tcf(c1, axiom, ![X: $i]: (p(X) | ~q(X, a))).";
        let result = parse_and_lower(input);
        assert_eq!(result.cnf_clauses.len(), 1);
        let display = format!("{}", result.cnf_clauses[0].display(&result.symbols));
        assert_eq!(display, "p(X0) | ~q(X0, a)");
    }

    #[test]
    fn lower_tcf_type_declaration_skipped() {
        let input = r#"
            tcf(p_type, type, p: $i > $o).
            tcf(c1, axiom, ![X: $i]: p(X)).
        "#;
        let result = parse_and_lower(input);
        // Type declaration must be dropped; only the clause is produced.
        assert_eq!(result.cnf_clauses.len(), 1);
    }

    #[test]
    fn lower_tcf_equality_literal() {
        let input = "tcf(c1, axiom, a = b).";
        let result = parse_and_lower(input);
        assert_eq!(result.cnf_clauses.len(), 1);
        let display = format!("{}", result.cnf_clauses[0].display(&result.symbols));
        assert_eq!(display, "a = b");
    }
}

//! Lowering from `mrs_tptp` FOF AST into [`mrs_core`] [`Formula`]s.
//!
//! This module is intentionally small: the proover only deals with the FOF
//! subset (no CNF, TFF, THF). All lowering shares a single [`SymbolTable`]
//! so that comparisons between formulas across the proof are meaningful.

use std::collections::HashMap;

use mrs_core::{Atom, Formula, SymbolId, SymbolTable, Term, VarId};
use mrs_tptp::{
    AnnotatedFormula, BinaryConnective, CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement, FOFAtomicFormula,
    FOFFormula, FOFStatement, FOFTerm, Quantifier,
};

/// Lowering context: shared symbol table plus a per-formula variable map.
pub struct LowerCtx<'a> {
    pub symbols: &'a mut SymbolTable,
    var_map: HashMap<String, VarId>,
    pub(crate) next_var: VarId,
}

impl<'a> LowerCtx<'a> {
    pub fn new(symbols: &'a mut SymbolTable) -> Self {
        Self {
            symbols,
            var_map: HashMap::new(),
            next_var: 0,
        }
    }

    /// Reset per-formula variable state. Call before lowering each formula
    /// from the proof, so VarIds restart from 0 for each formula.
    pub fn reset_vars(&mut self) {
        self.var_map.clear();
        self.next_var = 0;
    }

    fn fresh(&mut self) -> VarId {
        let id = self.next_var;
        self.next_var += 1;
        id
    }

    fn intern(&mut self, name: &str) -> SymbolId {
        self.symbols.intern(name)
    }
}

pub fn lower_annotated_formula(ctx: &mut LowerCtx<'_>, af: &AnnotatedFormula<'_>) -> Formula {
    match af {
        AnnotatedFormula::FOF(f) => lower_fof_statement(ctx, &f.formula),
        AnnotatedFormula::CNF(c) => lower_cnf_statement(ctx, &c.formula),
        _ => panic!("Unsupported formula type in lower_annotated_formula"),
    }
}

/// Lower a top-level FOF statement.
pub fn lower_fof_statement(ctx: &mut LowerCtx<'_>, s: &FOFStatement<'_>) -> Formula {
    match s {
        FOFStatement::Logical(f) => lower_fof_formula(ctx, f),
        FOFStatement::Sequent(lhs, rhs) => {
            // Treat `lhs --> rhs` as (∧ lhs) → (∨ rhs).
            let l = if lhs.is_empty() {
                Formula::True
            } else {
                Formula::and(lhs.iter().map(|f| lower_fof_formula(ctx, f)).collect())
            };
            let r = if rhs.is_empty() {
                Formula::False
            } else {
                Formula::or(rhs.iter().map(|f| lower_fof_formula(ctx, f)).collect())
            };
            Formula::implies(l, r)
        }
    }
}

pub fn lower_fof_formula(ctx: &mut LowerCtx<'_>, f: &FOFFormula<'_>) -> Formula {
    match f {
        FOFFormula::Atomic(a) => lower_fof_atomic(ctx, a),
        FOFFormula::Negation(inner) => Formula::neg(lower_fof_formula(ctx, inner)),
        FOFFormula::Parens(inner) => lower_fof_formula(ctx, inner),
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
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            // Save old bindings, allocate a fresh VarId per quantified var.
            let mut saved: Vec<(String, Option<VarId>)> = Vec::with_capacity(variables.len());
            let mut ids: Vec<VarId> = Vec::with_capacity(variables.len());
            for v in variables {
                let name = (*v).to_string();
                let old = ctx.var_map.get(&name).copied();
                saved.push((name.clone(), old));
                let id = ctx.fresh();
                ctx.var_map.insert(name, id);
                ids.push(id);
            }
            let body = lower_fof_formula(ctx, formula);
            for (name, old) in saved {
                match old {
                    Some(v) => {
                        ctx.var_map.insert(name, v);
                    }
                    None => {
                        ctx.var_map.remove(&name);
                    }
                }
            }
            let mut result = body;
            for &vid in ids.iter().rev() {
                result = match quantifier {
                    Quantifier::Forall => Formula::forall(vid, result),
                    Quantifier::Exists => Formula::exists(vid, result),
                };
            }
            result
        }
    }
}

fn lower_fof_atomic(ctx: &mut LowerCtx<'_>, a: &FOFAtomicFormula<'_>) -> Formula {
    match a {
        FOFAtomicFormula::Plain(w, args) => {
            let sym = ctx.intern(w.as_str());
            let ts: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, ts))
        }
        FOFAtomicFormula::Defined(w, args) => {
            let name = format!("${}", w.0);
            let sym = ctx.intern(&name);
            let ts: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, ts))
        }
        FOFAtomicFormula::System(w, args) => {
            let name = format!("$${}", w.0);
            let sym = ctx.intern(&name);
            let ts: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, ts))
        }
        FOFAtomicFormula::True => Formula::True,
        FOFAtomicFormula::False => Formula::False,
    }
}

pub fn lower_fof_term(ctx: &mut LowerCtx<'_>, t: &FOFTerm<'_>) -> Term {
    match t {
        FOFTerm::Variable(name) => {
            // If the variable is bound by some enclosing quantifier we've seen,
            // reuse its VarId; otherwise allocate a fresh one (free variable).
            let id = match ctx.var_map.get(*name).copied() {
                Some(id) => id,
                None => {
                    let id = ctx.fresh();
                    ctx.var_map.insert((*name).to_string(), id);
                    id
                }
            };
            Term::var(id)
        }
        FOFTerm::Function(w, args) => {
            let sym = ctx.intern(w.as_str());
            let ts: Vec<Term> = args.iter().map(|a| lower_fof_term(ctx, a)).collect();
            Term::app(sym, ts)
        }
        FOFTerm::DefinedFunction(w, args) => {
            let name = format!("${}", w.0);
            let sym = ctx.intern(&name);
            let ts: Vec<Term> = args.iter().map(|a| lower_fof_term(ctx, a)).collect();
            Term::app(sym, ts)
        }
        FOFTerm::SystemFunction(w, args) => {
            let name = format!("$${}", w.0);
            let sym = ctx.intern(&name);
            let ts: Vec<Term> = args.iter().map(|a| lower_fof_term(ctx, a)).collect();
            Term::app(sym, ts)
        }
        FOFTerm::Number(n) => {
            let name = n.as_str().to_string();
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

/// Lower a top-level CNF statement.
///
/// A CNF clause `lit1 | lit2 | … | litN` with free variables
/// `V1, …, Vk` is logically `∀V1…∀Vk. lit1 ∨ … ∨ litN`. We lower the
/// disjunction (the existing free-variable allocation in
/// `lower_fof_term` mints a fresh `VarId` for each unbound variable
/// encountered), then wrap the body with an outer `Forall` for every
/// `VarId` allocated during this call. Callers should
/// [`LowerCtx::reset_vars`] before invoking to ensure a clean slate.
pub fn lower_cnf_statement(ctx: &mut LowerCtx<'_>, s: &CNFStatement<'_>) -> Formula {
    match s {
        CNFStatement::Logical(f) => lower_cnf_formula(ctx, f),
    }
}

pub fn lower_cnf_formula(ctx: &mut LowerCtx<'_>, f: &CNFFormula<'_>) -> Formula {
    let before = ctx.next_var;
    let body = lower_cnf_formula_body(ctx, f);
    let after = ctx.next_var;
    let mut result = body;
    // Wrap in Forall for each freshly-allocated VarId (highest first
    // so the innermost binder is the lowest-numbered variable —
    // matches the convention used by FOF Quantified lowering above).
    for vid in (before..after).rev() {
        result = Formula::forall(vid, result);
    }
    result
}

fn lower_cnf_formula_body(ctx: &mut LowerCtx<'_>, f: &CNFFormula<'_>) -> Formula {
    match f {
        CNFFormula::Parens(inner) => lower_cnf_formula_body(ctx, inner),
        CNFFormula::Disjunction(lits) => {
            let parts: Vec<Formula> = lits.iter().map(|l| lower_cnf_literal(ctx, l)).collect();
            if parts.is_empty() {
                Formula::False
            } else if parts.len() == 1 {
                parts.into_iter().next().unwrap()
            } else {
                Formula::or(parts)
            }
        }
    }
}

fn lower_cnf_literal(ctx: &mut LowerCtx<'_>, l: &CNFLiteral<'_>) -> Formula {
    match l {
        CNFLiteral::Positive(a) => lower_cnf_atomic(ctx, a),
        CNFLiteral::Negative(a) => Formula::neg(lower_cnf_atomic(ctx, a)),
        CNFLiteral::Equality(lhs, rhs) => {
            let lt = lower_fof_term(ctx, lhs);
            let rt = lower_fof_term(ctx, rhs);
            Formula::atom(Atom::eq(lt, rt))
        }
        CNFLiteral::Inequality(lhs, rhs) => {
            let lt = lower_fof_term(ctx, lhs);
            let rt = lower_fof_term(ctx, rhs);
            Formula::neg(Formula::atom(Atom::eq(lt, rt)))
        }
    }
}

fn lower_cnf_atomic(ctx: &mut LowerCtx<'_>, a: &CNFAtomicFormula<'_>) -> Formula {
    match a {
        CNFAtomicFormula::Plain(w, args) => {
            let sym = ctx.intern(w.as_str());
            let ts: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, ts))
        }
        CNFAtomicFormula::Defined(w, args) => {
            let name = format!("${}", w.0);
            let sym = ctx.intern(&name);
            let ts: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, ts))
        }
        CNFAtomicFormula::System(w, args) => {
            let name = format!("$${}", w.0);
            let sym = ctx.intern(&name);
            let ts: Vec<Term> = args.iter().map(|t| lower_fof_term(ctx, t)).collect();
            Formula::atom(Atom::pred(sym, ts))
        }
        CNFAtomicFormula::True => Formula::True,
        CNFAtomicFormula::False => Formula::False,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    fn lower_first(input: &str, symbols: &mut SymbolTable) -> Formula {
        let problem = parse_tptp(input).expect("parse");
        let mut ctx = LowerCtx::new(symbols);
        match problem.formulas.into_iter().next().expect("af") {
            mrs_tptp::AnnotatedFormula::FOF(f) => lower_fof_statement(&mut ctx, &f.formula),
            mrs_tptp::AnnotatedFormula::CNF(f) => lower_cnf_statement(&mut ctx, &f.formula),
            _ => panic!("unexpected dialect"),
        }
    }

    #[test]
    fn cnf_ground_clause_matches_fof() {
        // Ground CNF and equivalent FOF must lower to α-equivalent
        // formulas (no implicit quantifiers since there are no
        // variables).
        let mut sym = SymbolTable::new();
        let cnf = lower_first("cnf(c1, axiom, p(a)).", &mut sym);
        let fof = lower_first("fof(c1, axiom, p(a)).", &mut sym);
        assert!(mrs_core::alpha::alpha_equiv(&cnf, &fof));
    }

    #[test]
    fn cnf_disjunction_matches_fof() {
        let mut sym = SymbolTable::new();
        let cnf = lower_first("cnf(c, axiom, p(a) | ~q(b)).", &mut sym);
        let fof = lower_first("fof(c, axiom, (p(a) | ~q(b))).", &mut sym);
        assert!(mrs_core::alpha::alpha_equiv(&cnf, &fof));
    }

    #[test]
    fn cnf_with_free_vars_becomes_forall() {
        // `cnf(c, axiom, p(X) | ~q(X))` should be α-equivalent to
        // `fof(c, axiom, ![X]: (p(X) | ~q(X)))`.
        let mut sym = SymbolTable::new();
        let cnf = lower_first("cnf(c, axiom, p(X) | ~q(X)).", &mut sym);
        let fof = lower_first("fof(c, axiom, ![X]: (p(X) | ~q(X))).", &mut sym);
        assert!(
            mrs_core::alpha::alpha_equiv(&cnf, &fof),
            "cnf {cnf:?}\nfof {fof:?}"
        );
    }

    #[test]
    fn cnf_equality_literal() {
        let mut sym = SymbolTable::new();
        let cnf = lower_first("cnf(c, axiom, X = a | X = b).", &mut sym);
        let fof = lower_first("fof(c, axiom, ![X]: (X = a | X = b)).", &mut sym);
        assert!(mrs_core::alpha::alpha_equiv(&cnf, &fof));
    }
}

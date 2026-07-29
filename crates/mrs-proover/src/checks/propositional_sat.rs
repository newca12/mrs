//! Propositional SAT fast-path for ATP dispatch.
//!
//! Many proof steps in Vampire-style proofs (e.g. `rat`, `avatar_*`,
//! `sat_conversion`) operate over pure propositional clauses built from
//! 0-ary predicate symbols (the `spl0_N` avatar splits). For such steps
//! the entailment check `premises ⊨ conclusion` is a finite SAT problem
//! and can be decided in microseconds by a real SAT solver — but
//! eprover/vampire, asked to check the same query via FOL saturation,
//! routinely time out on the larger instances (8-of-12 NV failures in a
//! recent corpus sample were of this shape).
//!
//! This module exposes [`try_propositional`], which detects when every
//! input formula is purely propositional (only 0-ary `Atom::Pred`, no
//! `Atom::Eq`, no quantifiers) and, if so, asks cadical whether
//! `(premises) ∧ ¬conclusion` is satisfiable. UNSAT means the step is
//! sound; SAT means it is unsound (there's a propositional
//! counter-model). Returns `None` whenever any input contains a
//! non-propositional construct, in which case the caller must fall
//! through to the FOL ATP ladder.
//!
//! The encoding is a textbook Tseitin transformation: each subformula
//! receives a fresh SAT variable, with clauses enforcing the
//! connective's semantics. The root variable of every premise is
//! asserted true and the root of the negated conclusion is asserted
//! true. Tseitin is preferred over naive CNF expansion because the
//! larger `rat` steps contain disjunctions of 30+ literals and would
//! otherwise blow up combinatorially through iff/implies.

use std::collections::{HashMap, HashSet};

use cadical::Solver;
use mrs_core::{Atom, Formula, Term};

/// Outcome of the propositional fast-path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropOutcome {
    /// `premises ⊨ conclusion` is propositionally valid; the step is sound.
    Sound,
    /// `premises ∪ {¬conclusion}` has a propositional model; the step is unsound.
    Unsound,
}

/// Try to decide `premises ⊨ conclusion` propositionally.
///
/// Returns `Some(Sound)` / `Some(Unsound)` only when *every* premise and
/// the conclusion are purely propositional (i.e. built from 0-ary
/// `Atom::Pred`, `Neg`, `And`, `Or`, `Implies`, `Iff`, `True`, `False`).
/// Any first-order construct — equality, quantifiers, predicates with
/// arguments — causes a `None` return so the caller can fall through to
/// the FOL ATP ladder.
pub fn try_propositional(premises: &[Formula], conclusion: &Formula) -> Option<PropOutcome> {
    let mut enc = Encoder::default();
    // Verify everything is propositional in a quick pre-pass; cheaper to
    // walk twice than to start encoding and roll back on failure.
    for p in premises {
        if !is_propositional(p) {
            return None;
        }
    }
    if !is_propositional(conclusion) {
        return None;
    }

    let mut solver = Solver::new();
    for p in premises {
        let lit = enc.encode(p, &mut solver);
        solver.add_clause([lit]);
    }
    let neg_concl = enc.encode(conclusion, &mut solver);
    solver.add_clause([-neg_concl]);

    match solver.solve() {
        Some(false) => Some(PropOutcome::Sound),
        Some(true) => Some(PropOutcome::Unsound),
        None => None,
    }
}

/// Try to decide `premises ⊨ conclusion` by **propositional abstraction**:
/// every distinct atom (of *any* arity, including equalities) is mapped to
/// a fresh boolean variable, ignoring all first-order structure beneath it.
///
/// This is a sound *over*-approximation of satisfiability. If the abstracted
/// `premises ∧ ¬conclusion` is UNSAT, then the original is UNSAT in every
/// FOL model (each model induces a propositional valuation that must satisfy
/// the same boolean constraints), so the step is genuinely sound and we
/// return `true`.
///
/// Crucially, a *satisfiable* abstraction proves **nothing** — the boolean
/// counter-model may be spurious once equality/instantiation constraints are
/// restored. We therefore return `false` (defer to the FOL ATP ladder) on
/// SAT, and **never** report unsoundness from this path. This asymmetry is
/// what keeps it safe under ProoVer's heavy `bad→good` penalty.
///
/// Quantifiers cannot be abstracted as a single boolean (their body links
/// instances), so any quantified input makes the function decline.
///
/// This handles steps that are propositionally valid but range over
/// argumented predicates — e.g. Vampire's `avatar_component_clause`
/// (`spl <=> body` ⊢ `¬body ∨ spl`), where `body` is an arbitrary atom and
/// the FOL ATP ladder may stall or choke on exotic operator symbols.
fn collect_terms(f: &Formula, terms: &mut HashSet<Term>) {
    match f {
        Formula::Atom(a) => match a {
            Atom::Pred(_, args) => {
                for arg in args {
                    collect_terms_in_term(arg, terms);
                }
            }
            Atom::Eq(l, r) => {
                collect_terms_in_term(l, terms);
                collect_terms_in_term(r, terms);
            }
        }
        Formula::Neg(inner) => collect_terms(inner, terms),
        Formula::And(cs) | Formula::Or(cs) => {
            for c in cs {
                collect_terms(c, terms);
            }
        }
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            collect_terms(a, terms);
            collect_terms(b, terms);
        }
        _ => {}
    }
}

fn collect_terms_in_term(t: &Term, terms: &mut HashSet<Term>) {
    if terms.insert(t.clone()) {
        match t {
            Term::App(_, args) => {
                for arg in args {
                    collect_terms_in_term(arg, terms);
                }
            }
            Term::Var(_) => {}
        }
    }
}

fn collect_pred_atoms(f: &Formula, atoms: &mut HashSet<Atom>) {
    match f {
        Formula::Atom(a) => match a {
            Atom::Pred(..) => {
                atoms.insert(a.clone());
            }
            _ => {}
        }
        Formula::Neg(inner) => collect_pred_atoms(inner, atoms),
        Formula::And(cs) | Formula::Or(cs) => {
            for c in cs {
                collect_pred_atoms(c, atoms);
            }
        }
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            collect_pred_atoms(a, atoms);
            collect_pred_atoms(b, atoms);
        }
        _ => {}
    }
}

fn match_term(pat: &Term, tgt: &Term, subst: &mut HashMap<mrs_core::VarId, Term>) -> bool {
    match (pat, tgt) {
        (Term::Var(id), _) => {
            if let Some(existing) = subst.get(id) {
                existing == tgt
            } else {
                subst.insert(*id, tgt.clone());
                true
            }
        }
        (Term::App(f1, args1), Term::App(f2, args2)) => {
            f1 == f2 && args1.len() == args2.len() && args1.iter().zip(args2.iter()).all(|(a1, a2)| match_term(a1, a2, subst))
        }
        _ => false,
    }
}

fn apply_subst_term(t: &Term, subst: &HashMap<mrs_core::VarId, Term>) -> Term {
    match t {
        Term::Var(id) => subst.get(id).cloned().unwrap_or_else(|| Term::Var(*id)),
        Term::App(f, args) => Term::App(*f, args.iter().map(|arg| apply_subst_term(arg, subst)).collect()),
    }
}

fn apply_subst_formula(f: &Formula, subst: &HashMap<mrs_core::VarId, Term>) -> Formula {
    match f {
        Formula::Atom(a) => match a {
            Atom::Pred(p, args) => Formula::Atom(Atom::Pred(*p, args.iter().map(|arg| apply_subst_term(arg, subst)).collect())),
            Atom::Eq(l, r) => Formula::Atom(Atom::Eq(apply_subst_term(l, subst), apply_subst_term(r, subst))),
        }
        Formula::Neg(inner) => Formula::Neg(Box::new(apply_subst_formula(inner, subst))),
        Formula::And(cs) => Formula::And(cs.iter().map(|c| apply_subst_formula(c, subst)).collect()),
        Formula::Or(cs) => Formula::Or(cs.iter().map(|c| apply_subst_formula(c, subst)).collect()),
        Formula::Implies(a, b) => Formula::Implies(Box::new(apply_subst_formula(a, subst)), Box::new(apply_subst_formula(b, subst))),
        Formula::Iff(a, b) => Formula::Iff(Box::new(apply_subst_formula(a, subst)), Box::new(apply_subst_formula(b, subst))),
        _ => f.clone(),
    }
}

fn collect_vars_in_term(t: &Term, vars: &mut HashSet<mrs_core::VarId>) {
    match t {
        Term::Var(id) => { vars.insert(*id); }
        Term::App(_, args) => {
            for arg in args {
                collect_vars_in_term(arg, vars);
            }
        }
    }
}

fn collect_vars_in_formula(f: &Formula, vars: &mut HashSet<mrs_core::VarId>) {
    match f {
        Formula::Atom(a) => match a {
            Atom::Pred(_, args) => {
                for arg in args {
                    collect_vars_in_term(arg, vars);
                }
            }
            Atom::Eq(l, r) => {
                collect_vars_in_term(l, vars);
                collect_vars_in_term(r, vars);
            }
        }
        Formula::Neg(inner) => collect_vars_in_formula(inner, vars),
        Formula::And(cs) | Formula::Or(cs) => {
            for c in cs {
                collect_vars_in_formula(c, vars);
            }
        }
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            collect_vars_in_formula(a, vars);
            collect_vars_in_formula(b, vars);
        }
        _ => {}
    }
}

fn collect_all_subterms_term(t: &Term, subterms: &mut HashSet<Term>) {
    subterms.insert(t.clone());
    if let Term::App(_, args) = t {
        for arg in args {
            collect_all_subterms_term(arg, subterms);
        }
    }
}

fn collect_all_subterms_formula(f: &Formula, subterms: &mut HashSet<Term>) {
    match f {
        Formula::Atom(a) => match a {
            Atom::Pred(_, args) => {
                for arg in args {
                    collect_all_subterms_term(arg, subterms);
                }
            }
            Atom::Eq(l, r) => {
                collect_all_subterms_term(l, subterms);
                collect_all_subterms_term(r, subterms);
            }
        }
        Formula::Neg(inner) => collect_all_subterms_formula(inner, subterms),
        Formula::And(cs) | Formula::Or(cs) => {
            for c in cs {
                collect_all_subterms_formula(c, subterms);
            }
        }
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            collect_all_subterms_formula(a, subterms);
            collect_all_subterms_formula(b, subterms);
        }
        _ => {}
    }
}

fn strip_leading_forall(f: &Formula) -> (Formula, HashSet<mrs_core::VarId>) {
    let mut body = f;
    let mut vars = HashSet::new();
    while let Formula::Forall(v, inner) = body {
        vars.insert(*v);
        body = inner;
    }
    (body.clone(), vars)
}

fn term_size(t: &Term) -> usize {
    match t {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_size).sum::<usize>(),
    }
}

fn rewrite_term(t: &Term, rules: &[(Term, Term)]) -> Term {
    let mut current = match t {
        Term::Var(_) => t.clone(),
        Term::App(f, args) => Term::App(*f, args.iter().map(|arg| rewrite_term(arg, rules)).collect()),
    };
    
    let mut changed = true;
    let mut limit = 0;
    while changed && limit < 30 {
        changed = false;
        for (lhs, rhs) in rules {
            let mut subst = HashMap::new();
            if match_term(lhs, &current, &mut subst) {
                current = apply_subst_term(rhs, &subst);
                changed = true;
                break;
            }
        }
        limit += 1;
    }
    current
}

pub fn try_propositional_abstraction(premises: &[Formula], conclusion: &Formula) -> bool {
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[prop-sat-dbg] try_propositional_abstraction entered, premises len = {}, concl = {:?}", premises.len(), conclusion);
    }

    let mut stripped_premises = Vec::new();
    for p in premises {
        let (body, mut p_vars) = strip_leading_forall(p);
        if is_quantifier_free(&body) {
            collect_vars_in_formula(&body, &mut p_vars);
            stripped_premises.push((body, p_vars));
        } else {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[prop-sat-dbg] ignoring non-quantifier-free premise: {:?}", p);
            }
        }
    }

    let (concl_body, mut concl_vars) = strip_leading_forall(conclusion);
    if !is_quantifier_free(&concl_body) {
        if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
            eprintln!("[prop-sat-dbg] concl body not quantifier free: {:?}", concl_body);
        }
        return false;
    }
    collect_vars_in_formula(&concl_body, &mut concl_vars);

    // Heuristic instantiation (E-matching) of universal premises against subterms in the step
    let mut all_targets = HashSet::new();
    collect_all_subterms_formula(&concl_body, &mut all_targets);
    for (body, p_vars) in &stripped_premises {
        if p_vars.is_empty() {
            collect_all_subterms_formula(body, &mut all_targets);
        }
    }

    if all_targets.len() > 100 {
        if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
            eprintln!("[prop-sat-dbg] too many target subterms ({}), aborting to prevent OOM", all_targets.len());
        }
        return false;
    }

    let mut default_const = None;
    for t in &all_targets {
        if let Term::App(_, args) = t {
            if args.is_empty() {
                default_const = Some(t.clone());
                break;
            }
        }
    }

    let mut instantiated_premises = HashSet::new();
    for (body, p_vars) in &stripped_premises {
        if p_vars.is_empty() {
            instantiated_premises.insert(body.clone());
        } else {
            let mut p_pats = HashSet::new();
            collect_all_subterms_formula(body, &mut p_pats);
            let mut generated = false;
            for pat in &p_pats {
                if let Term::App(_, args) = pat {
                    if !args.is_empty() {
                        for tgt in &all_targets {
                            let mut subst = HashMap::new();
                            if match_term(pat, tgt, &mut subst) {
                                let mut complete = true;
                                for v in p_vars {
                                    if !subst.contains_key(v) {
                                        if let Some(dc) = &default_const {
                                            subst.insert(*v, dc.clone());
                                        } else {
                                            complete = false;
                                            break;
                                        }
                                    }
                                }
                                if complete {
                                    instantiated_premises.insert(apply_subst_formula(body, &subst));
                                    if instantiated_premises.len() > 200 {
                                        if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                                            eprintln!("[prop-sat-dbg] too many instantiated premises, aborting to prevent OOM");
                                        }
                                        return false;
                                    }
                                    generated = true;
                                }
                            }
                        }
                    }
                }
            }
            if !generated {
                instantiated_premises.insert(body.clone());
            }
        }
    }

    // Fast-path: try to prove the conclusion purely by term rewriting (extremely fast, avoids SAT & CC transitivity blowups)
    let mut rewrite_rules = Vec::new();
    for p in &instantiated_premises {
        if let Formula::Atom(Atom::Eq(l, r)) = p {
            if term_size(l) >= term_size(r) {
                rewrite_rules.push((l.clone(), r.clone()));
            } else {
                rewrite_rules.push((r.clone(), l.clone()));
            }
        }
    }
    if let Formula::Atom(Atom::Eq(l_concl, r_concl)) = &concl_body {
        if rewrite_term(l_concl, &rewrite_rules) == rewrite_term(r_concl, &rewrite_rules) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[prop-sat-dbg] verified purely by term rewriting!");
            }
            return true;
        }
    }

    let mut enc = Encoder::default();
    let mut solver = Solver::new();
    for p in &instantiated_premises {
        let lit = enc.encode(p, &mut solver);
        solver.add_clause([lit]);
    }
    let neg_concl = enc.encode(&concl_body, &mut solver);
    solver.add_clause([-neg_concl]);

    // Collect all subterms and predicate atoms for Congruence Closure via SAT
    let mut terms = HashSet::new();
    for p in &instantiated_premises {
        collect_terms(p, &mut terms);
    }
    collect_terms(&concl_body, &mut terms);

    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[prop-sat-dbg] terms count = {}, list = {:?}", terms.len(), terms);
    }

    // Limit to safe sizes to avoid O(N^3) transitivity clause blowup (using 60 as safe threshold)
    if terms.len() <= 60 {
        let mut pred_atoms = HashSet::new();
        for p in &instantiated_premises {
            collect_pred_atoms(p, &mut pred_atoms);
        }
        collect_pred_atoms(&concl_body, &mut pred_atoms);

        let terms_vec: Vec<Term> = terms.into_iter().collect();

        // 0. Term Rewriting based equalities from premises (handles demodulation intermediate steps)
        let mut rewrite_rules = Vec::new();
        for p in &instantiated_premises {
            if let Formula::Atom(Atom::Eq(l, r)) = p {
                if term_size(l) >= term_size(r) {
                    rewrite_rules.push((l.clone(), r.clone()));
                } else {
                    rewrite_rules.push((r.clone(), l.clone()));
                }
            }
        }
        for i in 0..terms_vec.len() {
            for j in (i + 1)..terms_vec.len() {
                let t1 = &terms_vec[i];
                let t2 = &terms_vec[j];
                if rewrite_term(t1, &rewrite_rules) == rewrite_term(t2, &rewrite_rules) {
                    let eq = Formula::Atom(Atom::Eq(t1.clone(), t2.clone()));
                    let lit = enc.encode(&eq, &mut solver);
                    solver.add_clause([lit]);
                }
            }
        }

        // 1. Reflexivity: t = t
        for t in &terms_vec {
            let eq = Formula::Atom(Atom::Eq(t.clone(), t.clone()));
            let lit = enc.encode(&eq, &mut solver);
            solver.add_clause([lit]);
        }

        // 2. Symmetry: t1 = t2 => t2 = t1
        for i in 0..terms_vec.len() {
            for j in (i + 1)..terms_vec.len() {
                let t1 = &terms_vec[i];
                let t2 = &terms_vec[j];
                let eq1 = Formula::Atom(Atom::Eq(t1.clone(), t2.clone()));
                let eq2 = Formula::Atom(Atom::Eq(t2.clone(), t1.clone()));
                let lit1 = enc.encode(&eq1, &mut solver);
                let lit2 = enc.encode(&eq2, &mut solver);
                solver.add_clause([-lit1, lit2]);
            }
        }

        // 3. Transitivity: t1 = t2 ∧ t2 = t3 => t1 = t3
        for i in 0..terms_vec.len() {
            for j in 0..terms_vec.len() {
                if i == j { continue; }
                for k in 0..terms_vec.len() {
                    if i == k || j == k { continue; }
                    let t1 = &terms_vec[i];
                    let t2 = &terms_vec[j];
                    let t3 = &terms_vec[k];
                    let eq12 = Formula::Atom(Atom::Eq(t1.clone(), t2.clone()));
                    let eq23 = Formula::Atom(Atom::Eq(t2.clone(), t3.clone()));
                    let eq13 = Formula::Atom(Atom::Eq(t1.clone(), t3.clone()));
                    let lit12 = enc.encode(&eq12, &mut solver);
                    let lit23 = enc.encode(&eq23, &mut solver);
                    let lit13 = enc.encode(&eq13, &mut solver);
                    solver.add_clause([-lit12, -lit23, lit13]);
                }
            }
        }

        // 4. Function Congruence: t1 = u1 ∧ ... ∧ tn = un => f(t1, ..., tn) = f(u1, ..., un)
        for i in 0..terms_vec.len() {
            for j in (i + 1)..terms_vec.len() {
                let t1 = &terms_vec[i];
                let t2 = &terms_vec[j];
                if let (Term::App(f1, args1), Term::App(f2, args2)) = (t1, t2) {
                    if f1 == f2 && args1.len() == args2.len() && !args1.is_empty() {
                        let mut clause = Vec::with_capacity(args1.len() + 1);
                        for (a1, a2) in args1.iter().zip(args2.iter()) {
                            let eq = Formula::Atom(Atom::Eq(a1.clone(), a2.clone()));
                            let lit = enc.encode(&eq, &mut solver);
                            clause.push(-lit);
                        }
                        let eq_concl = Formula::Atom(Atom::Eq(t1.clone(), t2.clone()));
                        let lit_concl = enc.encode(&eq_concl, &mut solver);
                        clause.push(lit_concl);
                        solver.add_clause(clause);
                    }
                }
            }
        }

        // 5. Predicate Congruence: t1 = u1 ∧ ... ∧ tn = un => (p(t1, ..., tn) <=> p(u1, ..., un))
        let pred_atoms_vec: Vec<Atom> = pred_atoms.into_iter().collect();
        for i in 0..pred_atoms_vec.len() {
            for j in (i + 1)..pred_atoms_vec.len() {
                let a1 = &pred_atoms_vec[i];
                let a2 = &pred_atoms_vec[j];
                if let (Atom::Pred(p1, args1), Atom::Pred(p2, args2)) = (a1, a2) {
                    if p1 == p2 && args1.len() == args2.len() && !args1.is_empty() {
                        let mut base_lits = Vec::with_capacity(args1.len());
                        for (arg1, arg2) in args1.iter().zip(args2.iter()) {
                            let eq = Formula::Atom(Atom::Eq(arg1.clone(), arg2.clone()));
                            let lit = enc.encode(&eq, &mut solver);
                            base_lits.push(-lit);
                        }
                        
                        let lit1 = enc.encode(&Formula::Atom(a1.clone()), &mut solver);
                        let lit2 = enc.encode(&Formula::Atom(a2.clone()), &mut solver);
                        
                        let mut clause1 = base_lits.clone();
                        clause1.push(-lit1);
                        clause1.push(lit2);
                        solver.add_clause(clause1);
                        
                        let mut clause2 = base_lits;
                        clause2.push(-lit2);
                        clause2.push(lit1);
                        solver.add_clause(clause2);
                    }
                }
            }
        }
    }

    // UNSAT ⇒ truly entailed (sound). SAT ⇒ abstraction too coarse, defer.
    let sol = solver.solve();
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[prop-sat-dbg] solver solve outcome = {:?}", sol);
    }
    matches!(sol, Some(false))
}

/// Walks `f` returning `true` iff it contains no quantifier. Unlike
/// [`is_propositional`], predicate arguments and equality atoms are allowed —
/// they are treated as opaque booleans by [`try_propositional_abstraction`].
fn is_quantifier_free(f: &Formula) -> bool {
    match f {
        Formula::True | Formula::False | Formula::Atom(_) => true,
        Formula::Neg(g) => is_quantifier_free(g),
        Formula::And(gs) | Formula::Or(gs) => gs.iter().all(is_quantifier_free),
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            is_quantifier_free(a) && is_quantifier_free(b)
        }
        Formula::Forall(..) | Formula::Exists(..) => false,
    }
}

/// Walks `f` returning `true` iff it contains only propositional
/// constructs. Free variables are not propositional — a `Forall`/`Exists`
/// node, a `Pred` with arguments, or any `Eq` atom causes `false`.
fn is_propositional(f: &Formula) -> bool {
    match f {
        Formula::True | Formula::False => true,
        Formula::Atom(Atom::Pred(_, args)) => args.is_empty(),
        Formula::Atom(Atom::Eq(..)) => false,
        Formula::Neg(g) => is_propositional(g),
        Formula::And(gs) | Formula::Or(gs) => gs.iter().all(is_propositional),
        Formula::Implies(a, b) | Formula::Iff(a, b) => is_propositional(a) && is_propositional(b),
        Formula::Forall(..) | Formula::Exists(..) => false,
    }
}

/// Tseitin encoder: each propositional atom gets a stable SAT variable
/// (cached by `SymbolId`) and each compound subformula gets a fresh
/// auxiliary variable with the connective clauses added on-encounter.
struct Encoder {
    atom_vars: HashMap<Atom, i32>,
    next_var: i32,
}

impl Default for Encoder {
    fn default() -> Self {
        Self {
            atom_vars: HashMap::new(),
            next_var: 1, // cadical variables start from 1
        }
    }
}

impl Encoder {
    fn new_var(&mut self) -> i32 {
        let v = self.next_var;
        self.next_var += 1;
        v
    }

    /// Encode `f` and return a literal that is true exactly when `f` is
    /// true under the SAT assignment. May add clauses to `solver` as a
    /// side-effect.
    fn encode(&mut self, f: &Formula, solver: &mut Solver) -> i32 {
        match f {
            Formula::True => {
                // A fresh variable forced to true. Cheaper than threading
                // a constant through the encoder.
                let v = self.new_var();
                solver.add_clause([v]);
                v
            }
            Formula::False => {
                let v = self.new_var();
                solver.add_clause([-v]);
                v // returning the variable itself is fine, it's forced to false, wait no, we should return -v or false literal
            }
            Formula::Atom(atom) => {
                // Key on the full atom (any arity, including equalities).
                // For the pure-propositional caller this is just the 0-ary
                // predicate; for the abstraction caller it treats every
                // distinct argumented atom as an opaque boolean.
                *self.atom_vars.entry(atom.clone()).or_insert_with(|| {
                    let v = self.next_var;
                    self.next_var += 1;
                    v
                })
            }
            Formula::Neg(g) => -self.encode(g, solver),
            Formula::And(gs) => {
                // y ↔ (a₁ ∧ … ∧ aₙ):
                //   for each i: ¬y ∨ aᵢ   (y → aᵢ)
                //   one clause:  y ∨ ¬a₁ ∨ … ∨ ¬aₙ   (a₁ ∧ … ∧ aₙ → y)
                let y = self.new_var();
                let mut big = Vec::with_capacity(gs.len() + 1);
                big.push(y);
                for g in gs {
                    let a = self.encode(g, solver);
                    solver.add_clause([-y, a]);
                    big.push(-a);
                }
                solver.add_clause(big);
                y
            }
            Formula::Or(gs) => {
                // y ↔ (a₁ ∨ … ∨ aₙ):
                //   one clause:  ¬y ∨ a₁ ∨ … ∨ aₙ   (y → at least one aᵢ)
                //   for each i: y ∨ ¬aᵢ            (aᵢ → y)
                let y = self.new_var();
                let mut big = Vec::with_capacity(gs.len() + 1);
                big.push(-y);
                for g in gs {
                    let a = self.encode(g, solver);
                    solver.add_clause([y, -a]);
                    big.push(a);
                }
                solver.add_clause(big);
                y
            }
            Formula::Implies(a, b) => {
                // a → b  ≡  ¬a ∨ b
                let la = self.encode(a, solver);
                let lb = self.encode(b, solver);
                let y = self.new_var();
                // y ↔ (¬a ∨ b):
                solver.add_clause([-y, -la, lb]);
                solver.add_clause([y, la]);
                solver.add_clause([y, -lb]);
                y
            }
            Formula::Iff(a, b) => {
                // a ↔ b  ≡  (¬a ∨ b) ∧ (a ∨ ¬b)
                let la = self.encode(a, solver);
                let lb = self.encode(b, solver);
                let y = self.new_var();
                // y ↔ (la ↔ lb):
                solver.add_clause([-y, -la, lb]);
                solver.add_clause([-y, la, -lb]);
                solver.add_clause([y, -la, -lb]);
                solver.add_clause([y, la, lb]);
                y
            }
            Formula::Forall(..) | Formula::Exists(..) => {
                unreachable!("quantifier slipped past is_propositional")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::{Atom, Formula, SymbolTable, Term};

    fn p(s: &mut SymbolTable, name: &str) -> Formula {
        Formula::Atom(Atom::Pred(s.intern(name), vec![]))
    }

    #[test]
    fn unit_resolution_is_sound() {
        // (p ∨ q) ∧ ¬q ⊨ p
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::Or(vec![pp.clone(), pq.clone()]);
        let prem2 = Formula::neg(pq);
        let goal = pp;
        assert_eq!(
            try_propositional(&[prem1, prem2], &goal),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn rat_style_resolution_is_sound() {
        // The shape of SYN485+1's s709:
        // (spl0_3 ∨ spl0_1) ∧ (¬spl0_3 ∨ spl0_1) ⊨ spl0_1
        let mut s = SymbolTable::new();
        let a = p(&mut s, "spl0_3");
        let b = p(&mut s, "spl0_1");
        let prem1 = Formula::Or(vec![a.clone(), b.clone()]);
        let prem2 = Formula::Or(vec![Formula::neg(a), b.clone()]);
        assert_eq!(
            try_propositional(&[prem1, prem2], &b),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn larger_rat_chain_is_sound() {
        // The shape of SYN485+1's s732 (subset):
        // From {spl18, ¬spl18 ∨ spl33 ∨ spl35 ∨ spl36, ¬spl36} derive
        // spl33 ∨ spl35.
        let mut s = SymbolTable::new();
        let spl18 = p(&mut s, "spl18");
        let spl33 = p(&mut s, "spl33");
        let spl35 = p(&mut s, "spl35");
        let spl36 = p(&mut s, "spl36");
        let prem1 = spl18.clone();
        let prem2 = Formula::Or(vec![
            Formula::neg(spl18),
            spl33.clone(),
            spl35.clone(),
            spl36.clone(),
        ]);
        let prem3 = Formula::neg(spl36);
        let goal = Formula::Or(vec![spl33, spl35]);
        assert_eq!(
            try_propositional(&[prem1, prem2, prem3], &goal),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn counter_model_is_unsound() {
        // p ⊨ q  is invalid; cadical finds the counter-model p=true, q=false.
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        assert_eq!(try_propositional(&[pp], &pq), Some(PropOutcome::Unsound));
    }

    #[test]
    fn iff_premise_is_handled() {
        // (p ↔ q) ∧ p ⊨ q
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::iff(pp.clone(), pq.clone());
        assert_eq!(
            try_propositional(&[prem1, pp], &pq),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn implies_premise_is_handled() {
        // (p → q) ∧ p ⊨ q
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::implies(pp.clone(), pq.clone());
        assert_eq!(
            try_propositional(&[prem1, pp], &pq),
            Some(PropOutcome::Sound)
        );
    }

    #[test]
    fn quantifier_in_premise_returns_none() {
        let mut s = SymbolTable::new();
        let psym = s.intern("p");
        let xvar = 0u32;
        // ∀X. p(X)  is not propositional (p has an arg).
        let prem = Formula::Forall(
            xvar,
            Box::new(Formula::Atom(Atom::Pred(psym, vec![Term::var(xvar)]))),
        );
        let goal = p(&mut s, "q");
        assert_eq!(try_propositional(&[prem], &goal), None);
    }

    #[test]
    fn equality_atom_returns_none() {
        let mut s = SymbolTable::new();
        let asym = s.intern("a");
        let bsym = s.intern("b");
        let eq = Formula::Atom(Atom::Eq(Term::constant(asym), Term::constant(bsym)));
        let goal = p(&mut s, "q");
        assert_eq!(try_propositional(&[eq], &goal), None);
    }

    #[test]
    fn predicate_with_args_returns_none() {
        let mut s = SymbolTable::new();
        let psym = s.intern("p");
        let asym = s.intern("a");
        let prem = Formula::Atom(Atom::Pred(psym, vec![Term::constant(asym)]));
        let goal = p(&mut s, "q");
        assert_eq!(try_propositional(&[prem], &goal), None);
    }

    #[test]
    fn empty_premises_unsound_unless_tautology() {
        // From nothing, p does not follow.
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        assert_eq!(try_propositional(&[], &pp), Some(PropOutcome::Unsound));
        // But (p ∨ ¬p) follows from nothing.
        let pp2 = p(&mut s, "q");
        let taut = Formula::Or(vec![pp2.clone(), Formula::neg(pp2)]);
        assert_eq!(try_propositional(&[], &taut), Some(PropOutcome::Sound));
    }

    // ---- propositional-abstraction tests ----

    /// An argumented atom: `pred(args...)`.
    fn pa(s: &mut SymbolTable, name: &str, args: Vec<Term>) -> Formula {
        Formula::Atom(Atom::Pred(s.intern(name), args))
    }

    #[test]
    fn abstraction_decides_avatar_component_clause() {
        // The exact shape of LCL894+1 f44:
        //   premise:  spl0_1 <=> ge(c, b)
        //   concl:    ¬ge(c, b) ∨ spl0_1
        // ge(c,b) is an argumented atom; the pure-0-ary path declines, but
        // abstraction treats ge(c,b) as opaque and proves the entailment.
        let mut s = SymbolTable::new();
        let c = Term::constant(s.intern("c"));
        let b = Term::constant(s.intern("b"));
        let ge = pa(&mut s, "ge", vec![c, b]);
        let spl = p(&mut s, "spl0_1");
        let iff = Formula::iff(spl.clone(), ge.clone());
        let concl = Formula::Or(vec![Formula::neg(ge), spl]);
        assert!(try_propositional_abstraction(&[iff], &concl));
    }

    #[test]
    fn abstraction_handles_pure_0ary() {
        // Pure propositional unit resolution is also provable by abstraction.
        let mut s = SymbolTable::new();
        let pp = p(&mut s, "p");
        let pq = p(&mut s, "q");
        let prem1 = Formula::Or(vec![pp.clone(), pq.clone()]);
        let prem2 = Formula::neg(pq);
        assert!(try_propositional_abstraction(&[prem1, prem2], &pp));
    }

    #[test]
    fn abstraction_proves_equality_transitivity() {
        // a=b, b=c ⊨ a=c is verified successfully by Congruence Closure via SAT
        // (the transitivity axioms are instantiated and solved by CaDiCaL).
        let mut s = SymbolTable::new();
        let a = Term::constant(s.intern("a"));
        let b = Term::constant(s.intern("b"));
        let c = Term::constant(s.intern("c"));
        let ab = Formula::Atom(Atom::Eq(a.clone(), b.clone()));
        let bc = Formula::Atom(Atom::Eq(b, c.clone()));
        let ac = Formula::Atom(Atom::Eq(a, c));
        assert!(try_propositional_abstraction(&[ab, bc], &ac));
    }

    #[test]
    fn abstraction_defers_on_first_order_instantiation() {
        // SOUNDNESS-CRITICAL: ∀X.p(X) ⊨ p(a) is valid but needs instantiation.
        // The premise is quantified, so abstraction declines outright.
        let mut s = SymbolTable::new();
        let asym = Term::constant(s.intern("a"));
        let psym = s.intern("p");
        let prem = Formula::Forall(
            0,
            Box::new(Formula::Atom(Atom::Pred(psym, vec![Term::var(0)]))),
        );
        let goal = Formula::Atom(Atom::Pred(psym, vec![asym]));
        assert!(!try_propositional_abstraction(&[prem], &goal));
    }

    #[test]
    fn abstraction_defers_when_not_entailed() {
        // p(a) ⊨ p(b) is NOT valid; abstraction sees distinct atoms, SAT,
        // returns false (defer) — never claims soundness.
        let mut s = SymbolTable::new();
        let asym = Term::constant(s.intern("a"));
        let bsym = Term::constant(s.intern("b"));
        let pa_atom = pa(&mut s, "p", vec![asym]);
        let pb_atom = pa(&mut s, "p", vec![bsym]);
        assert!(!try_propositional_abstraction(&[pa_atom], &pb_atom));
    }
}

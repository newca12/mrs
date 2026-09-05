//! Twee-style goal-directed preprocessing transformation for equational reasoning (UEQ).
//!
//! Based on Nicholas Smallbone's Goal-Directed Transformation technique in Twee (CADE-28, 2021).
//!
//! In equational theorem proving (UEQ), standard completion/superposition is primarily
//! an undirected forward search from axioms.
//!
//! This module introduces fresh constant definitions for compound ground subterms
//! occurring in the conjecture/goal, and rewrites the goal into these constants:
//! - For each compound subterm `u = f(t1, ..., tk)` in the goal, a fresh constant `d`
//!   and a defining unit equation `u = d` are introduced.
//! - The goal is rewritten to mention `d` instead of `u`.
//!
//! ### Key Benefits:
//! 1. **Implicit Heuristic Preference**: Derived clauses sharing subterms with the goal
//!    are rewritten to `d`, reducing their symbol weight and causing given-clause selection
//!    to pick them much earlier.
//! 2. **Goal-Oriented Critical Pairs**: New critical pairs between axioms and the definition
//!    `u = d` perform backward reasoning directly from the goal.
//! 3. **Zero Runtime Overhead**: Entirely performed as a preprocessing pass before search.

use std::collections::HashMap;

use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::{SymbolId, SymbolTable};
use mrs_core::term::Term;
use smallvec::SmallVec;

/// Mode of goal-directed transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoalTransformMode {
    /// Full bottom-up recursive flattening of all compound ground subterms in the goal.
    #[default]
    RecursiveSubterms,
    /// Flattening of only top-level / maximal compound ground subterms directly under the goal equality literals.
    MaximalSubterms,
}

/// The result of applying the goal-directed transformation.
#[derive(Debug, Clone)]
pub struct GoalTransformResult {
    /// The active clauses to feed into search (axioms + introduced definitions + rewritten goals).
    pub clauses: Vec<Clause>,
    /// Additional provenance clauses (including original goal clauses and introduced definitions).
    pub provenance: Vec<Clause>,
    /// Whether any goal clause was transformed and definitions were introduced.
    pub transformed: bool,
}

/// Checks whether a clause is a goal / conjecture clause.
pub fn is_goal_clause(clause: &Clause) -> bool {
    if clause.distance == 0 {
        return true;
    }
    match &clause.source {
        ClauseSource::Input { role, .. } => {
            role == "negated_conjecture" || role == "conjecture" || role == "question"
        }
        ClauseSource::Inference { rule, .. } => *rule == "negated_conjecture",
        ClauseSource::Introduced { .. } => false,
    }
}

/// Applies Twee-style goal-directed preprocessing transformation to the given clauses.
///
/// If any goal clause contains compound ground subterms, this function introduces
/// fresh constant symbols `goal_d0`, `goal_d1`, ..., defining equations `u = goal_di`,
/// and rewrites the goal clauses to use the fresh constants.
pub fn transform_goal_clauses(
    clauses: &[Clause],
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    mode: GoalTransformMode,
) -> GoalTransformResult {
    let mut defined_terms: HashMap<Term, (SymbolId, ClauseId)> = HashMap::new();
    let mut def_clauses: Vec<Clause> = Vec::new();
    let mut def_counter: usize = 0;

    let mut result_clauses: Vec<Clause> = Vec::with_capacity(clauses.len());
    let mut extra_provenance: Vec<Clause> = Vec::new();
    let mut any_transformed = false;

    for clause in clauses {
        if !is_goal_clause(clause) {
            result_clauses.push(clause.clone());
            continue;
        }

        // Clause is a goal clause: transform its literals.
        let mut new_literals: SmallVec<[Literal; 4]> =
            SmallVec::with_capacity(clause.literals.len());
        let mut clause_used_defs: Vec<ClauseId> = Vec::new();
        let mut clause_modified = false;

        for lit in &clause.literals {
            let (new_atom, used_defs, modified) = transform_atom(
                &lit.atom,
                symbols,
                id_gen,
                mode,
                &mut defined_terms,
                &mut def_clauses,
                &mut def_counter,
            );

            if modified {
                clause_modified = true;
                clause_used_defs.extend(used_defs);
            }

            new_literals.push(Literal {
                positive: lit.positive,
                atom: new_atom,
            });
        }

        if clause_modified {
            any_transformed = true;
            // Record the original goal in provenance
            extra_provenance.push(clause.clone());

            // Build parent list: [orig_goal, def_1, def_2, ...]
            let mut parents: SmallVec<[ClauseId; 2]> = SmallVec::new();
            parents.push(clause.id);
            clause_used_defs.sort_unstable();
            clause_used_defs.dedup();
            for def_id in clause_used_defs {
                parents.push(def_id);
            }

            let new_goal_id = id_gen.next();
            let new_goal = Clause {
                id: new_goal_id,
                literals: new_literals,
                source: ClauseSource::Inference {
                    rule: "goal_transformation",
                    parents,
                },
                avatar: clause.avatar.clone(),
                distance: 0,
                formula: None,
                certificate: None,
            };

            result_clauses.push(new_goal);
        } else {
            result_clauses.push(clause.clone());
        }
    }

    if any_transformed {
        let mut final_clauses = Vec::with_capacity(def_clauses.len() + result_clauses.len());
        final_clauses.extend(def_clauses);
        final_clauses.extend(result_clauses);

        GoalTransformResult {
            clauses: final_clauses,
            provenance: extra_provenance,
            transformed: true,
        }
    } else {
        GoalTransformResult {
            clauses: clauses.to_vec(),
            provenance: Vec::new(),
            transformed: false,
        }
    }
}

fn transform_atom(
    atom: &Atom,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    mode: GoalTransformMode,
    defined_terms: &mut HashMap<Term, (SymbolId, ClauseId)>,
    def_clauses: &mut Vec<Clause>,
    def_counter: &mut usize,
) -> (Atom, Vec<ClauseId>, bool) {
    let mut used_defs = Vec::new();
    let mut modified = false;

    let new_atom = match atom {
        Atom::Eq(lhs, rhs) => {
            let (new_lhs, defs_lhs, mod_lhs) = transform_term(
                lhs,
                symbols,
                id_gen,
                mode,
                defined_terms,
                def_clauses,
                def_counter,
            );
            let (new_rhs, defs_rhs, mod_rhs) = transform_term(
                rhs,
                symbols,
                id_gen,
                mode,
                defined_terms,
                def_clauses,
                def_counter,
            );

            if mod_lhs || mod_rhs {
                modified = true;
                used_defs.extend(defs_lhs);
                used_defs.extend(defs_rhs);
            }
            Atom::eq(new_lhs, new_rhs)
        }
        Atom::Pred(p, args) => {
            let mut new_args = Vec::with_capacity(args.len());
            for arg in args {
                let (new_arg, defs_arg, mod_arg) = transform_term(
                    arg,
                    symbols,
                    id_gen,
                    mode,
                    defined_terms,
                    def_clauses,
                    def_counter,
                );
                if mod_arg {
                    modified = true;
                    used_defs.extend(defs_arg);
                }
                new_args.push(new_arg);
            }
            Atom::pred(*p, new_args)
        }
    };

    (new_atom, used_defs, modified)
}

fn transform_term(
    term: &Term,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    mode: GoalTransformMode,
    defined_terms: &mut HashMap<Term, (SymbolId, ClauseId)>,
    def_clauses: &mut Vec<Clause>,
    def_counter: &mut usize,
) -> (Term, Vec<ClauseId>, bool) {
    match mode {
        GoalTransformMode::RecursiveSubterms => transform_term_recursive(
            term,
            symbols,
            id_gen,
            defined_terms,
            def_clauses,
            def_counter,
        ),
        GoalTransformMode::MaximalSubterms => transform_term_maximal(
            term,
            symbols,
            id_gen,
            defined_terms,
            def_clauses,
            def_counter,
        ),
    }
}

/// Recursively flattens compound ground subterms bottom-up.
fn transform_term_recursive(
    term: &Term,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    defined_terms: &mut HashMap<Term, (SymbolId, ClauseId)>,
    def_clauses: &mut Vec<Clause>,
    def_counter: &mut usize,
) -> (Term, Vec<ClauseId>, bool) {
    match term {
        Term::Var(_) => (term.clone(), Vec::new(), false),
        Term::App(sym, args) => {
            if args.is_empty() {
                // Ground constant — atomic, no need to define.
                return (term.clone(), Vec::new(), false);
            }

            // First recursively transform all arguments.
            let mut new_args = Vec::with_capacity(args.len());
            let mut used_defs = Vec::new();
            let mut child_modified = false;

            for arg in args {
                let (new_arg, defs, modified) = transform_term_recursive(
                    arg,
                    symbols,
                    id_gen,
                    defined_terms,
                    def_clauses,
                    def_counter,
                );
                if modified {
                    child_modified = true;
                    used_defs.extend(defs);
                }
                new_args.push(new_arg);
            }

            let flattened = Term::App(*sym, new_args);

            // If the flattened term contains free variables, do not define as a ground constant.
            if !flattened.free_vars().is_empty() {
                return (flattened, used_defs, child_modified);
            }

            // Ground compound term: introduce or reuse definition.
            let (def_sym, def_id) = get_or_create_def(
                flattened,
                symbols,
                id_gen,
                defined_terms,
                def_clauses,
                def_counter,
            );
            used_defs.push(def_id);

            (Term::constant(def_sym), used_defs, true)
        }
    }
}

/// Flattens only the top-level compound ground term.
fn transform_term_maximal(
    term: &Term,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    defined_terms: &mut HashMap<Term, (SymbolId, ClauseId)>,
    def_clauses: &mut Vec<Clause>,
    def_counter: &mut usize,
) -> (Term, Vec<ClauseId>, bool) {
    match term {
        Term::Var(_) => (term.clone(), Vec::new(), false),
        Term::App(_, args) if args.is_empty() => (term.clone(), Vec::new(), false),
        Term::App(_, _) => {
            if !term.free_vars().is_empty() {
                return (term.clone(), Vec::new(), false);
            }
            let (def_sym, def_id) = get_or_create_def(
                term.clone(),
                symbols,
                id_gen,
                defined_terms,
                def_clauses,
                def_counter,
            );
            (Term::constant(def_sym), vec![def_id], true)
        }
    }
}

fn get_or_create_def(
    term: Term,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    defined_terms: &mut HashMap<Term, (SymbolId, ClauseId)>,
    def_clauses: &mut Vec<Clause>,
    def_counter: &mut usize,
) -> (SymbolId, ClauseId) {
    if let Some(&(sym, id)) = defined_terms.get(&term) {
        return (sym, id);
    }

    let sym_name = format!("goal_d{}", *def_counter);
    *def_counter += 1;
    let def_sym = symbols.intern(&sym_name);
    let def_id = id_gen.next();

    // Clause: term = def_sym
    let def_clause = Clause::new(
        def_id,
        vec![Literal::pos(Atom::eq(
            term.clone(),
            Term::constant(def_sym),
        ))],
        ClauseSource::Introduced { symbol: def_sym },
    )
    .with_distance(0);

    defined_terms.insert(term, (def_sym, def_id));
    def_clauses.push(def_clause);

    (def_sym, def_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_transform_recursive() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let g = symbols.intern("g");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let c = symbols.intern("c");

        let mut id_gen = ClauseIdGen::new();

        // Goal: f(g(a), b) != c
        let g_a = Term::app(g, vec![Term::constant(a)]);
        let f_ga_b = Term::app(f, vec![g_a, Term::constant(b)]);
        let goal_clause = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::eq(f_ga_b, Term::constant(c)))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        )
        .with_distance(0);

        let res = transform_goal_clauses(
            std::slice::from_ref(&goal_clause),
            &mut symbols,
            &mut id_gen,
            GoalTransformMode::RecursiveSubterms,
        );

        assert!(res.transformed);
        // We expect:
        // def 0: g(a) = goal_d0
        // def 1: f(goal_d0, b) = goal_d1
        // new goal: goal_d1 != c
        assert_eq!(res.clauses.len(), 3);

        let def0 = &res.clauses[0];
        let def1 = &res.clauses[1];
        let new_goal = &res.clauses[2];

        assert_eq!(def0.distance, 0);
        assert_eq!(def1.distance, 0);
        assert_eq!(new_goal.distance, 0);

        match &def0.source {
            ClauseSource::Introduced { symbol } => {
                assert_eq!(symbols.resolve(*symbol), "goal_d0");
            }
            _ => panic!("Expected Introduced source for def0"),
        }

        match &new_goal.source {
            ClauseSource::Inference { rule, parents } => {
                assert_eq!(*rule, "goal_transformation");
                assert_eq!(parents.len(), 3); // orig_goal, def0, def1
                assert_eq!(parents[0], goal_clause.id);
            }
            _ => panic!("Expected Inference source for new_goal"),
        }
    }

    #[test]
    fn test_goal_transform_deduplication() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let g = symbols.intern("g");
        let a = symbols.intern("a");

        let mut id_gen = ClauseIdGen::new();

        // Goal: f(g(a), g(a)) != g(a)
        let g_a1 = Term::app(g, vec![Term::constant(a)]);
        let g_a2 = Term::app(g, vec![Term::constant(a)]);
        let g_a3 = Term::app(g, vec![Term::constant(a)]);
        let f_term = Term::app(f, vec![g_a1, g_a2]);

        let goal_clause = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::eq(f_term, g_a3))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        )
        .with_distance(0);

        let res = transform_goal_clauses(
            &[goal_clause],
            &mut symbols,
            &mut id_gen,
            GoalTransformMode::RecursiveSubterms,
        );

        assert!(res.transformed);
        // g(a) should only have ONE definition introduced (goal_d0), and f(goal_d0, goal_d0) has goal_d1
        // goal becomes goal_d1 != goal_d0
        assert_eq!(res.clauses.len(), 3); // def0 (g(a)), def1 (f(d0, d0)), new goal (d1 != d0)
    }

    #[test]
    fn test_goal_transform_maximal() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let g = symbols.intern("g");
        let a = symbols.intern("a");
        let c = symbols.intern("c");

        let mut id_gen = ClauseIdGen::new();

        // Goal: f(g(a)) != c
        let g_a = Term::app(g, vec![Term::constant(a)]);
        let f_ga = Term::app(f, vec![g_a]);

        let goal_clause = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::eq(f_ga, Term::constant(c)))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        )
        .with_distance(0);

        let res = transform_goal_clauses(
            &[goal_clause],
            &mut symbols,
            &mut id_gen,
            GoalTransformMode::MaximalSubterms,
        );

        assert!(res.transformed);
        // Only 1 definition for f(g(a)) = goal_d0, and goal: goal_d0 != c
        assert_eq!(res.clauses.len(), 2);
    }

    #[test]
    fn test_goal_transform_axioms_preserved() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let a = symbols.intern("a");
        let mut id_gen = ClauseIdGen::new();

        // Axiom: f(a) = a
        let ax = Clause::new(
            id_gen.next(),
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(a),
            ))],
            ClauseSource::Input {
                name: "ax".into(),
                role: "axiom".into(),
            },
        );

        let res = transform_goal_clauses(
            std::slice::from_ref(&ax),
            &mut symbols,
            &mut id_gen,
            GoalTransformMode::RecursiveSubterms,
        );

        assert!(!res.transformed);
        assert_eq!(res.clauses.len(), 1);
        assert_eq!(res.clauses[0].id, ax.id);
    }

    #[test]
    fn test_goal_transform_pure_constants_no_transform() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut id_gen = ClauseIdGen::new();

        // Goal: a != b (pure constants)
        let goal = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::eq(Term::constant(a), Term::constant(b)))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        )
        .with_distance(0);

        let res = transform_goal_clauses(
            std::slice::from_ref(&goal),
            &mut symbols,
            &mut id_gen,
            GoalTransformMode::RecursiveSubterms,
        );

        assert!(!res.transformed);
        assert_eq!(res.clauses.len(), 1);
        assert_eq!(res.clauses[0].id, goal.id);
    }

    #[test]
    fn test_goal_transform_with_variables_preserved() {
        let mut symbols = SymbolTable::new();
        let f = symbols.intern("f");
        let g = symbols.intern("g");
        let a = symbols.intern("a");
        let mut id_gen = ClauseIdGen::new();

        // Goal: f(X, g(a)) != X (where X is Var(0))
        let ga = Term::app(g, vec![Term::constant(a)]);
        let f_x_ga = Term::app(f, vec![Term::var(0), ga]);
        let goal = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::eq(f_x_ga, Term::var(0)))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        )
        .with_distance(0);

        let res = transform_goal_clauses(
            &[goal],
            &mut symbols,
            &mut id_gen,
            GoalTransformMode::RecursiveSubterms,
        );

        assert!(res.transformed);
        // g(a) is ground, so defined as goal_d0.
        // f(X, goal_d0) has a free variable X, so it is NOT defined as a ground constant.
        // Result: def0 (g(a) = goal_d0), new goal (f(X, goal_d0) != X)
        assert_eq!(res.clauses.len(), 2);
        let def0 = &res.clauses[0];
        let new_goal = &res.clauses[1];

        match &def0.source {
            ClauseSource::Introduced { symbol } => {
                assert_eq!(symbols.resolve(*symbol), "goal_d0");
            }
            _ => panic!("Expected Introduced"),
        }

        if let Atom::Eq(lhs, rhs) = &new_goal.literals[0].atom {
            assert_eq!(*rhs, Term::var(0));
            if let Term::App(sym, args) = lhs {
                assert_eq!(*sym, f);
                assert_eq!(args[0], Term::var(0));
                assert_eq!(args[1], Term::constant(symbols.intern("goal_d0")));
            } else {
                panic!("Expected Term::App for lhs");
            }
        } else {
            panic!("Expected Eq atom");
        }
    }
}

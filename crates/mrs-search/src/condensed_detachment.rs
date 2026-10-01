//! Proof-producing search for TPTP's `is_a_theorem` condensed-detachment
//! encoding.
//!
//! The relevant input clauses are theorem units `p(F)`, a detachment clause
//! `~p(X) | p(Y) | ~p(implies(X,Y))`, and a goal unit `~p(T)`.  Detachment is
//! not treated as a SAT query: it is replayed as two ordinary binary
//! resolution inferences against that actual three-literal clause.  The
//! resulting theorem units are retained and may be reused as either premise
//! of later detachments. Thus every successful result is an ordinary first-
//! order resolution DAG rooted in the input clauses.
//!
//! This is a bounded, goal-directed closure intended for the compact
//! `cn_1`/`cn_2`/`cn_3` LCL basis. Formula size, fact count, inference count,
//! and wall time are capped; failure falls through to the regular schedule.

use std::time::{Duration, Instant};

use crate::{HashMap, HashSet, SearchResult};

use mrs_calculus::resolution::{resolve, resolve_selected};
use mrs_core::clause::{Clause, ClauseIdGen, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolTable;
use mrs_core::term::Term;
use mrs_proof::tstp::format_tstp;

const MAX_FACTS: usize = 5_000;
const MAX_INFERENCES: usize = 100_000;

fn collect_subterms(term: &Term, output: &mut HashSet<Term>) {
    if !output.insert(term.clone()) {
        return;
    }
    if let Term::App(_, args) = term {
        for arg in args {
            collect_subterms(arg, output);
        }
    }
}

fn goal_relevant(term: &Term, target_subterms: &HashSet<Term>) -> bool {
    let mut candidates = HashSet::default();
    collect_subterms(term, &mut candidates);
    candidates.iter().any(|candidate| {
        !matches!(candidate, Term::Var(_))
            && target_subterms
                .iter()
                .any(|target| mrs_unify::unify(candidate, target).is_ok())
    })
}

fn theorem_literal(clause: &Clause) -> Option<(&Literal, mrs_core::symbol::SymbolId, &Term)> {
    if clause.literals.len() != 1 {
        return None;
    }
    let lit = &clause.literals[0];
    let Atom::Pred(predicate, args) = &lit.atom else {
        return None;
    };
    let [formula] = args.as_slice() else {
        return None;
    };
    Some((lit, *predicate, formula))
}

fn formula_of(clause: &Clause, predicate: mrs_core::symbol::SymbolId) -> Option<&Term> {
    let (literal, found_predicate, formula) = theorem_literal(clause)?;
    (literal.positive && found_predicate == predicate).then_some(formula)
}

#[derive(Clone, Copy)]
struct DetachmentShape {
    predicate: mrs_core::symbol::SymbolId,
    implication_literal: usize,
}

fn detachment_rule(clause: &Clause) -> Option<DetachmentShape> {
    if clause.literals.len() != 3 {
        return None;
    }
    for positive in clause.literals.iter().filter(|lit| lit.positive) {
        let Atom::Pred(predicate, positive_args) = &positive.atom else {
            continue;
        };
        let [consequent] = positive_args.as_slice() else {
            continue;
        };
        let negatives: Vec<_> = clause
            .literals
            .iter()
            .enumerate()
            .filter(|(_, lit)| !lit.positive)
            .filter_map(|(index, lit)| match &lit.atom {
                Atom::Pred(p, args) if p == predicate && args.len() == 1 => Some((index, &args[0])),
                _ => None,
            })
            .collect();
        if negatives.len() != 2 {
            continue;
        }
        for (index, (implication_index, implication_literal)) in negatives.iter().enumerate() {
            let Term::App(_, args) = implication_literal else {
                continue;
            };
            let [antecedent, implication_consequent] = args.as_slice() else {
                continue;
            };
            let (_, antecedent_literal) = negatives[1 - index];
            if antecedent_literal == antecedent && consequent == implication_consequent {
                return Some(DetachmentShape {
                    predicate: *predicate,
                    implication_literal: *implication_index,
                });
            }
        }
    }
    None
}

fn term_nodes(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_nodes).sum::<usize>(),
    }
}

fn canonical_term(term: &Term, vars: &mut HashMap<u32, usize>, out: &mut String) {
    match term {
        Term::Var(var) => {
            let next = vars.len();
            let canonical = *vars.entry(*var).or_insert(next);
            out.push('v');
            out.push_str(&canonical.to_string());
            out.push(';');
        }
        Term::App(symbol, args) => {
            out.push('f');
            out.push_str(&symbol.index().to_string());
            out.push('/');
            out.push_str(&args.len().to_string());
            out.push('(');
            for arg in args {
                canonical_term(arg, vars, out);
            }
            out.push(')');
        }
    }
}

fn fact_key(clause: &Clause) -> Option<String> {
    let (literal, predicate, formula) = theorem_literal(clause)?;
    let mut key = format!("{}:{}:", predicate.index(), literal.positive);
    canonical_term(formula, &mut HashMap::default(), &mut key);
    Some(key)
}

fn proof_ancestors(
    roots: &[Clause],
    provenance: &[Clause],
    clauses: &[Clause],
) -> Option<Vec<Clause>> {
    let mut by_id: HashMap<_, _> = provenance
        .iter()
        .chain(clauses)
        .map(|clause| (clause.id, clause))
        .collect();
    for root in roots {
        by_id.insert(root.id, root);
    }
    let mut pending = roots.iter().map(|clause| clause.id).collect::<Vec<_>>();
    let mut seen = HashSet::default();
    let mut proof = Vec::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let clause = by_id.get(&id)?.to_owned();
        if let mrs_core::clause::ClauseSource::Inference { parents, .. } = &clause.source {
            pending.extend(parents.iter().copied());
        }
        proof.push(clause.clone());
    }
    Some(proof)
}

/// Attempts a bounded, proof-producing condensed-detachment closure.
///
/// The function only accepts the exact clause shape above, which is the
/// clausification of the `<=` rule used by the target TPTP problems. All
/// derivations use the ordinary `resolution` inference implemented by the
/// calculus; no result from a propositional decision procedure is trusted.
pub fn try_refutation(
    clauses: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
    budget: Duration,
) -> Option<SearchResult> {
    if budget.is_zero() {
        return None;
    }
    let deadline = Instant::now().checked_add(budget)?;
    let (rule_index, shape) = clauses
        .iter()
        .enumerate()
        .find_map(|(index, clause)| detachment_rule(clause).map(|shape| (index, shape)))?;
    let predicate = shape.predicate;
    let rule = &clauses[rule_index];

    let mut theorem_facts = Vec::new();
    let mut goal_units = Vec::new();
    let mut target_subterms = HashSet::default();
    let mut seen = HashSet::default();
    let mut max_goal_nodes = 0;
    for (clause_index, clause) in clauses.iter().enumerate() {
        if Instant::now() >= deadline {
            return None;
        }
        if clause_index == rule_index {
            continue;
        }
        if let Some((literal, p, formula)) = theorem_literal(clause)
            && p == predicate
        {
            if literal.positive {
                if seen.insert(fact_key(clause)?) {
                    if theorem_facts.len() >= MAX_FACTS {
                        return None;
                    }
                    theorem_facts.push(clause.clone());
                }
            } else {
                if goal_units.len() >= MAX_FACTS {
                    return None;
                }
                max_goal_nodes = max_goal_nodes.max(term_nodes(formula));
                collect_subterms(formula, &mut target_subterms);
                goal_units.push(clause.clone());
            }
        }
    }
    if theorem_facts.is_empty() || goal_units.is_empty() {
        return None;
    }

    // A lemma may be larger than the target, but unbounded closure is
    // infinite. The small margin permits common detachment intermediates
    // without turning the pre-pass into unrestricted term generation.
    let max_formula_nodes = max_goal_nodes.saturating_mul(2).saturating_add(16);
    let mut proof_roots = Vec::with_capacity(theorem_facts.len() + goal_units.len() + 1);
    proof_roots.push(rule.clone());
    proof_roots.extend(theorem_facts.iter().cloned());
    proof_roots.extend(goal_units.iter().cloned());
    let mut proof = proof_ancestors(&proof_roots, provenance, clauses)?;
    let mut fact_head = 0;
    let mut inferences = 0;
    while fact_head < theorem_facts.len() {
        if Instant::now() >= deadline {
            break;
        }
        let current = theorem_facts[fact_head].clone();
        fact_head += 1;

        // Check the current derived unit against every negated goal. This also
        // handles universally quantified conjectures through resolution's MGU.
        for goal in &goal_units {
            for empty in resolve(&current, goal, id_gen) {
                if Instant::now() >= deadline {
                    return None;
                }
                if empty.is_empty() {
                    proof.push(empty.clone());
                    let tstp = format_tstp(&proof, symbols);
                    if !tstp.is_empty() {
                        return Some(SearchResult::Refutation(empty.id, tstp));
                    }
                    return None;
                }
            }
        }

        // Use the new theorem once as the antecedent and once as the
        // implication. The second resolution is the actual detach step.
        let snapshot = theorem_facts.len();
        for other_index in 0..snapshot {
            if Instant::now() >= deadline {
                break;
            }
            let other = theorem_facts[other_index].clone();
            for (antecedent_fact, implication_fact) in [(&current, &other), (&other, &current)] {
                let Some(implication_formula) = formula_of(implication_fact, predicate) else {
                    continue;
                };
                let Term::App(_, implication_args) = implication_formula else {
                    continue;
                };
                let [_antecedent, _consequent] = implication_args.as_slice() else {
                    continue;
                };
                // First resolve the implication fact against the negative
                // implication literal of the encoded rule, leaving
                // `~p(X) | p(Y)`. Resolve that clause against p(X).
                for open in resolve_selected(
                    rule,
                    implication_fact,
                    id_gen,
                    Some(&[shape.implication_literal]),
                    Some(&[0]),
                    &HashSet::default(),
                ) {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    if open.literals.len() != 2
                        || open.literals.iter().filter(|lit| !lit.positive).count() != 1
                        || open.literals.iter().filter(|lit| lit.positive).count() != 1
                    {
                        continue;
                    }
                    let Some(open_antecedent) = open.literals.iter().position(|lit| !lit.positive)
                    else {
                        continue;
                    };
                    for derived in resolve_selected(
                        &open,
                        antecedent_fact,
                        id_gen,
                        Some(&[open_antecedent]),
                        Some(&[0]),
                        &HashSet::default(),
                    ) {
                        if Instant::now() >= deadline {
                            return None;
                        }
                        inferences += 2;
                        if inferences > MAX_INFERENCES {
                            return None;
                        }
                        let Some((literal, p, formula)) = theorem_literal(&derived) else {
                            continue;
                        };
                        if !literal.positive
                            || p != predicate
                            || term_nodes(formula) > max_formula_nodes
                        {
                            continue;
                        }
                        let Some(key) = fact_key(&derived) else {
                            continue;
                        };
                        if seen.insert(key) {
                            if theorem_facts.len() >= MAX_FACTS {
                                return None;
                            }
                            proof.push(open.clone());
                            let closes_goal = goal_relevant(formula, &target_subterms);
                            theorem_facts.push(derived.clone());
                            proof.push(derived.clone());
                            if closes_goal {
                                for goal in &goal_units {
                                    for empty in resolve(&derived, goal, id_gen) {
                                        if Instant::now() >= deadline {
                                            return None;
                                        }
                                        if empty.is_empty() {
                                            proof.push(empty.clone());
                                            let tstp = format_tstp(&proof, symbols);
                                            if !tstp.is_empty() {
                                                return Some(SearchResult::Refutation(
                                                    empty.id, tstp,
                                                ));
                                            }
                                            return None;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::ClauseSource;
    use mrs_core::term::Term;
    use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
    use mrs_tptp::parse_tptp;

    fn input(id_gen: &mut ClauseIdGen, literals: Vec<Literal>, name: &str) -> Clause {
        Clause::new(
            id_gen.next(),
            literals,
            ClauseSource::Input {
                name: name.into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn derives_and_reuses_theorem_facts_by_ordinary_resolution() {
        let mut symbols = SymbolTable::new();
        let theorem = symbols.intern("is_a_theorem");
        let implies = symbols.intern("implies");
        let a = Term::constant(symbols.intern("a"));
        let b = Term::constant(symbols.intern("b"));
        let c = Term::constant(symbols.intern("c"));
        let mut id_gen = ClauseIdGen::new();
        let fact_ab = input(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                theorem,
                vec![Term::app(implies, vec![a.clone(), b.clone()])],
            ))],
            "fact_ab",
        );
        let fact_bc = input(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                theorem,
                vec![Term::app(implies, vec![b.clone(), c.clone()])],
            ))],
            "fact_bc",
        );
        let transitivity = input(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                theorem,
                vec![Term::app(
                    implies,
                    vec![
                        Term::app(implies, vec![Term::var(0), Term::var(1)]),
                        Term::app(
                            implies,
                            vec![
                                Term::app(implies, vec![Term::var(1), Term::var(2)]),
                                Term::app(implies, vec![Term::var(0), Term::var(2)]),
                            ],
                        ),
                    ],
                )],
            ))],
            "cn_1",
        );
        let rule = input(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(theorem, vec![Term::var(0)])),
                Literal::pos(Atom::pred(theorem, vec![Term::var(1)])),
                Literal::neg(Atom::pred(
                    theorem,
                    vec![Term::app(implies, vec![Term::var(0), Term::var(1)])],
                )),
            ],
            "condensed_detachment",
        );
        let goal = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::pred(
                theorem,
                vec![Term::app(implies, vec![a, c])],
            ))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        );
        let clauses = vec![transitivity, rule, fact_ab, fact_bc, goal];
        let result = try_refutation(&clauses, &[], &mut id_gen, &symbols, Duration::from_secs(1));
        let Some(SearchResult::Refutation(_, tstp)) = result else {
            panic!("expected a condensed-detachment refutation");
        };
        assert!(tstp.contains("inference(resolution"));
        assert!(!tstp.contains("inference(condensed_detachment"));

        let problem = "cnf(cn_1,axiom,is_a_theorem(implies(implies(X,Y),implies(implies(Y,Z),implies(X,Z))))).\n\
            cnf(condensed_detachment,axiom,~is_a_theorem(X) | is_a_theorem(Y) | ~is_a_theorem(implies(X,Y))).\n\
            cnf(fact_ab,axiom,is_a_theorem(implies(a,b))).\n\
            cnf(fact_bc,axiom,is_a_theorem(implies(b,c))).\n\
            cnf(goal,negated_conjecture,~is_a_theorem(implies(a,c))).\n";
        let parsed_problem = parse_tptp(problem).expect("problem parses");
        let parsed_proof = parse_tptp(&tstp).expect("generated proof parses");
        let verdict = verify_strict(
            &parsed_problem,
            &parsed_proof,
            VerificationLimits::default(),
        );
        assert_eq!(verdict, KernelVerdict::Certified, "{verdict:#?}\n{tstp}");
    }

    #[test]
    fn returns_none_for_zero_budget_and_non_fragment_inputs() {
        let mut symbols = SymbolTable::new();
        let predicate = symbols.intern("is_a_theorem");
        let atom = Clause::new(
            ClauseIdGen::new().next(),
            vec![Literal::pos(Atom::pred(
                predicate,
                vec![Term::constant(symbols.intern("a"))],
            ))],
            ClauseSource::Input {
                name: "fact".into(),
                role: "axiom".into(),
            },
        );
        let mut id_gen = ClauseIdGen::new();
        assert!(
            try_refutation(&[atom.clone()], &[], &mut id_gen, &symbols, Duration::ZERO).is_none()
        );
        assert!(
            try_refutation(
                &[atom],
                &[],
                &mut id_gen,
                &symbols,
                Duration::from_millis(5)
            )
            .is_none()
        );
    }

    #[test]
    fn derives_and_certifies_a_goal_from_the_detachment_rule() {
        let mut symbols = SymbolTable::new();
        let theorem = symbols.intern("is_a_theorem");
        let implies = symbols.intern("implies");
        let a = Term::constant(symbols.intern("a"));
        let b = Term::constant(symbols.intern("b"));
        let mut id_gen = ClauseIdGen::new();
        let fact_a = input(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(theorem, vec![a.clone()]))],
            "fact_a",
        );
        let fact_ab = input(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                theorem,
                vec![Term::app(implies, vec![a, b.clone()])],
            ))],
            "fact_ab",
        );
        let rule = input(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(theorem, vec![Term::var(0)])),
                Literal::pos(Atom::pred(theorem, vec![Term::var(1)])),
                Literal::neg(Atom::pred(
                    theorem,
                    vec![Term::app(implies, vec![Term::var(0), Term::var(1)])],
                )),
            ],
            "condensed_detachment",
        );
        let goal = Clause::new(
            id_gen.next(),
            vec![Literal::neg(Atom::pred(theorem, vec![b]))],
            ClauseSource::Input {
                name: "goal".into(),
                role: "negated_conjecture".into(),
            },
        );
        let clauses = [fact_a, fact_ab, rule, goal];
        let problem = "cnf(fact_a,axiom,is_a_theorem(a)).\n\
            cnf(fact_ab,axiom,is_a_theorem(implies(a,b))).\n\
            cnf(condensed_detachment,axiom,~is_a_theorem(X) | is_a_theorem(Y) | ~is_a_theorem(implies(X,Y))).\n\
            cnf(goal,negated_conjecture,~is_a_theorem(b)).\n";
        let result = try_refutation(&clauses, &[], &mut id_gen, &symbols, Duration::from_secs(1));
        let Some(SearchResult::Refutation(_, tstp)) = result else {
            panic!("expected detached theorem refutation");
        };
        let parsed_problem = parse_tptp(problem).expect("problem parses");
        let parsed_proof = parse_tptp(&tstp).expect("generated proof parses");
        let verdict = verify_strict(
            &parsed_problem,
            &parsed_proof,
            VerificationLimits::default(),
        );
        assert_eq!(verdict, KernelVerdict::Certified, "{verdict:#?}\n{tstp}");
    }
}

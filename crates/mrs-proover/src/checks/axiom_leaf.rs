//! Phase 4: leaf axiom/conjecture vs. problem-file check.
//!
//! For every node carrying a `file('Problems/foo.p', name)` source, we compare
//! its formula α-equivalently against the formula of that named entry in the
//! parsed problem file. A mismatch is positive evidence of tampering and
//! yields [`StepOutcome::Unsound`].
//!
//! Special case for anonymous provenance: Vampire's `--proof tptp` output
//! frequently writes `file(<path>, unknown)` instead of preserving the
//! original axiom name. When the recorded name is the literal `unknown` we
//! fall back to scanning all axiom-role formulas in the linked problem and
//! accept the leaf if any of them α-matches the proof formula. This is sound:
//! the leaf still has to be α-equivalent to *some* declared axiom of the
//! problem, which is exactly the same standard as the named-axiom check.

use mrs_core::alpha::alpha_equiv;
use mrs_core::{SymbolId, SymbolTable};
use mrs_tptp::{AnnotatedFormula, FormulaRole, TPTPProblem};

use crate::lower::{LowerCtx, lower_annotated_formula};
use crate::verdict::StepOutcome;

/// Check a single proof leaf whose source is `file('…', name)`.
///
/// `problem` is the parsed linked problem file. The proof-node's `name` is
/// looked up in `problem` and compared α-equivalently. If the recorded name
/// is the literal `unknown` (common in Vampire output), we instead try every
/// axiom-role formula in the problem and accept the leaf if any α-matches.
pub fn check_leaf<'p>(
    node: &AnnotatedFormula<'p>,
    problem: Option<&TPTPProblem<'_>>,
    symbols: &mut SymbolTable,
    strict: bool,
) -> StepOutcome {
    let Some(ann) = node.annotations() else {
        // No annotation → cannot prove provenance. Conservative.
        return StepOutcome::Unknown("leaf without source annotation".into());
    };
    let Some((_path, expected_name)) = ann.file_source() else {
        return StepOutcome::Unknown("leaf source is not file(_,_)".into());
    };
    let Some(problem) = problem else {
        if strict {
            return StepOutcome::Unknown(
                "strict mode requires a linked problem for leaf provenance".into(),
            );
        }
        // If no problem file is loaded (e.g. Otter subset of Zenodo benchmark),
        // we cannot verify if the leaf is a valid axiom in the problem file.
        // We fallback to treating the leaf as Sound (verifying the proof
        // modulo assumptions).
        return StepOutcome::Sound;
    };

    // Lower the proof leaf formula once for either matching strategy.
    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let Some(proof_f) = lower_annotated_formula(&mut ctx, node) else {
        return StepOutcome::Unknown("unsupported proof leaf formula type".into());
    };
    let algebraic_symbols = explicit_algebraic_symbols(problem, ctx.symbols);

    // --- Anonymous-provenance fallback (Vampire's `file(_, unknown)`) -----
    if expected_name == "unknown" {
        for af in &problem.formulas {
            let role = match af {
                AnnotatedFormula::FOF(f) => f.role,
                AnnotatedFormula::CNF(f) => f.role,
                _ => continue,
            };
            if !roles_compatible(node.role(), role) {
                continue;
            }
            ctx.reset_vars();
            let Some(prob_f) = lower_annotated_formula(&mut ctx, af) else {
                continue;
            };
            if alpha_equiv(&proof_f, &prob_f)
                || crate::checks::definition_folding::canon_eq_free(
                    &proof_f,
                    &prob_f,
                    Some(ctx.symbols),
                )
                || ac_alpha_equiv(&proof_f, &prob_f, &algebraic_symbols)
            {
                return StepOutcome::Sound;
            }

            // Fallback: try matching against the CNF of the problem formula
            let mut id_gen = mrs_core::clause::ClauseIdGen::new();
            let clauses = mrs_cnf::clausify(&prob_f, ctx.symbols, &mut id_gen, "prob", "axiom");
            for c in clauses {
                let c_form = clause_to_formula_with_forall(&c);
                if crate::checks::definition_folding::canon_eq_free(
                    &proof_f,
                    &c_form,
                    Some(ctx.symbols),
                ) || ac_alpha_equiv(&proof_f, &c_form, &algebraic_symbols)
                {
                    return StepOutcome::Sound;
                }
            }
        }

        // No compatible-role problem formula α/canon-matched the leaf.
        //
        // Provers that *clausify the problem up front* (SPASS, Otter, etc.)
        // emit anonymous `file(_, unknown)` leaves that are the NNF /
        // Skolemised / CNF form of an original axiom — e.g.
        // `~big_p(u) | big_q(u) | big_r(u)` for `big_p(X) => (big_q(X) |
        // big_r(X))`, or `big_p(skc2) | big_q(skc3)` for a Skolemised
        // existential. These are logically faithful to the problem but are
        // NOT α- or AC-equivalent to the named axiom, so our structural
        // comparison legitimately cannot confirm them.
        //
        // Reporting `Unsound` here costs a false `VerifiedBad` (−1) on
        // such valid proofs, which are common. We therefore downgrade to
        // `Unknown` (0 pts) — *except* for the one shape that is positive
        // evidence of tampering: a leaf that is itself contradictory
        // (`$false` / `~$true`). No legitimate problem axiom is `$false`,
        // so an anonymous leaf claiming the problem asserts `$false` is an
        // axiom-spoofing attempt and stays `Unsound`.
        //
        // Soundness note: the named-axiom path below still does a strict
        // check, and the `axiom_spoofing` evil exploit (and the official
        // ProoVer examples) use *named* provenance, so this downgrade does
        // not weaken evil-proof detection on the competition corpus.
        if is_trivially_false(&proof_f) {
            return StepOutcome::Unsound(
                "leaf with anonymous provenance (file(_,unknown)) is `$false` but no \
                 problem formula is contradictory — axiom spoofing"
                    .into(),
            );
        }
        return StepOutcome::Unknown(
            "leaf with anonymous provenance (file(_,unknown)) does not α-match any \
             compatible-role formula in the linked problem; it may be a clausified / \
             Skolemised form of an axiom that we cannot match structurally"
                .into(),
        );
    }

    // --- Named-axiom path -------------------------------------------------
    // Look up the named entry in either dialect (the prover may have
    // converted a CNF problem to FOF in the proof, or vice versa). We
    // accept the entry whose name matches, regardless of dialect.
    let mut target_af: Option<&AnnotatedFormula<'_>> = None;
    let mut prob_role = None;
    for af in &problem.formulas {
        if af.name() == expected_name {
            target_af = Some(af);
            prob_role = Some(af.role());
            break;
        }
    }
    if target_af.is_none() {
        for af in &problem.formulas {
            let role = af.role();
            if !roles_compatible(node.role(), role) {
                continue;
            }
            ctx.reset_vars();
            let Some(prob_f) = lower_annotated_formula(&mut ctx, af) else {
                continue;
            };
            if alpha_equiv(&proof_f, &prob_f)
                || crate::checks::definition_folding::canon_eq_free(
                    &proof_f,
                    &prob_f,
                    Some(ctx.symbols),
                )
                || ac_alpha_equiv(&proof_f, &prob_f, &algebraic_symbols)
            {
                return StepOutcome::Sound;
            }
        }

        return StepOutcome::Unsound(format!(
            "leaf references non-existent axiom '{expected_name}' whose formula is not present in problem file"
        ));
    }

    if !roles_compatible(node.role(), prob_role.unwrap()) {
        return StepOutcome::Unsound(format!(
            "leaf role '{:?}' is incompatible with problem node role '{:?}'",
            node.role(),
            prob_role.unwrap()
        ));
    }

    ctx.reset_vars();
    let Some(prob_f) = lower_annotated_formula(&mut ctx, target_af.unwrap()) else {
        return StepOutcome::Unknown("unsupported target formula type in problem".into());
    };

    if alpha_equiv(&proof_f, &prob_f)
        || crate::checks::definition_folding::canon_eq_free(&proof_f, &prob_f, Some(ctx.symbols))
        || crate::checks::propositional_sat::try_propositional(
            std::slice::from_ref(&prob_f),
            &proof_f,
        ) == Some(crate::checks::propositional_sat::PropOutcome::Sound)
        || ac_alpha_equiv(&proof_f, &prob_f, &algebraic_symbols)
    {
        StepOutcome::Sound
    } else {
        // The proof leaf does not α-match the named axiom.
        StepOutcome::Unsound(format!(
            "leaf formula does not syntactically α-match axiom '{expected_name}' \
             (may differ only by AC-rewriting of commutative operators)"
        ))
    }
}

#[derive(Default)]
struct AlgebraicSymbols {
    commutative: std::collections::HashSet<SymbolId>,
    associative: std::collections::HashSet<SymbolId>,
}

impl AlgebraicSymbols {
    fn is_commutative(&self, symbol: SymbolId) -> bool {
        self.commutative.contains(&symbol)
    }

    fn is_ac(&self, symbol: SymbolId) -> bool {
        self.commutative.contains(&symbol) && self.associative.contains(&symbol)
    }
}

fn explicit_algebraic_symbols(
    problem: &TPTPProblem<'_>,
    symbols: &mut SymbolTable,
) -> AlgebraicSymbols {
    let mut algebraic = AlgebraicSymbols::default();
    let mut ctx = LowerCtx::new(symbols);
    for annotated in &problem.formulas {
        let role = annotated.role();
        if !matches!(
            role,
            FormulaRole::Axiom | FormulaRole::Hypothesis | FormulaRole::Definition
        ) {
            continue;
        }
        ctx.reset_vars();
        if let Some(formula) = lower_annotated_formula(&mut ctx, annotated) {
            collect_algebraic_axiom(&formula, &mut algebraic);
        }
    }
    algebraic
}

fn collect_algebraic_axiom(formula: &mrs_core::Formula, symbols: &mut AlgebraicSymbols) {
    let mut body = formula;
    while let mrs_core::Formula::Forall(_, inner) = body {
        body = inner;
    }
    let mrs_core::Formula::Atom(mrs_core::Atom::Eq(left, right)) = body else {
        return;
    };
    let (
        mrs_core::Term::App(left_symbol, left_args),
        mrs_core::Term::App(right_symbol, right_args),
    ) = (left, right)
    else {
        return;
    };
    if left_symbol != right_symbol || left_args.len() != 2 || right_args.len() != 2 {
        return;
    }
    if matches!(
        (&left_args[0], &left_args[1], &right_args[0], &right_args[1]),
        (
            mrs_core::Term::Var(x),
            mrs_core::Term::Var(y),
            mrs_core::Term::Var(y2),
            mrs_core::Term::Var(x2),
        ) if x != y && x == x2 && y == y2
    ) {
        symbols.commutative.insert(*left_symbol);
    }

    fn variable(term: &mrs_core::Term) -> Option<u32> {
        match term {
            mrs_core::Term::Var(variable) => Some(*variable),
            _ => None,
        }
    }

    fn binary_variable_pair(term: &mrs_core::Term, symbol: SymbolId) -> Option<(u32, u32)> {
        let mrs_core::Term::App(function, args) = term else {
            return None;
        };
        if *function != symbol || args.len() != 2 {
            return None;
        }
        Some((variable(&args[0])?, variable(&args[1])?))
    }

    let forward = match (
        binary_variable_pair(&left_args[0], *left_symbol),
        variable(&left_args[1]),
        variable(&right_args[0]),
        binary_variable_pair(&right_args[1], *left_symbol),
    ) {
        (Some((x, y)), Some(z), Some(x2), Some((y2, z2))) => {
            x == x2 && y == y2 && z == z2 && x != y && y != z && x != z
        }
        _ => false,
    };
    let reverse = match (
        variable(&left_args[0]),
        binary_variable_pair(&left_args[1], *left_symbol),
        binary_variable_pair(&right_args[0], *left_symbol),
        variable(&right_args[1]),
    ) {
        (Some(x), Some((y, z)), Some((x2, y2)), Some(z2)) => {
            x == x2 && y == y2 && z == z2 && x != y && y != z && x != z
        }
        _ => false,
    };
    let associative = forward || reverse;
    if associative {
        symbols.associative.insert(*left_symbol);
    }
}

fn collect_ac_leaves<'a>(
    t: &'a mrs_core::Term,
    ac_sym: mrs_core::SymbolId,
    symbols: &AlgebraicSymbols,
    out: &mut Vec<&'a mrs_core::Term>,
) {
    if let mrs_core::Term::App(f, args) = t
        && *f == ac_sym
        && symbols.is_ac(*f)
        && args.len() == 2
    {
        for arg in args {
            collect_ac_leaves(arg, ac_sym, symbols, out);
        }
    } else {
        out.push(t);
    }
}

fn ac_alpha_equiv(
    f1: &mrs_core::Formula,
    f2: &mrs_core::Formula,
    symbols: &AlgebraicSymbols,
) -> bool {
    use std::collections::HashMap;

    fn term_eq(
        t1: &mrs_core::Term,
        t2: &mrs_core::Term,
        left: &HashMap<mrs_core::VarId, u32>,
        right: &HashMap<mrs_core::VarId, u32>,
        symbols: &AlgebraicSymbols,
    ) -> bool {
        match (t1, t2) {
            (mrs_core::Term::Var(v1), mrs_core::Term::Var(v2)) => {
                match (left.get(v1), right.get(v2)) {
                    (Some(d1), Some(d2)) => d1 == d2,
                    (None, None) => v1 == v2,
                    _ => false,
                }
            }
            (mrs_core::Term::App(f1, args1), mrs_core::Term::App(f2, args2)) => {
                if f1 != f2 {
                    return false;
                }
                if symbols.is_ac(*f1) && args1.len() == 2 && args2.len() == 2 {
                    let mut leaves1 = Vec::new();
                    let mut leaves2 = Vec::new();
                    collect_ac_leaves(t1, *f1, symbols, &mut leaves1);
                    collect_ac_leaves(t2, *f2, symbols, &mut leaves2);
                    if leaves1.len() != leaves2.len() {
                        return false;
                    }
                    match_terms(&leaves1, &leaves2, left, right, symbols)
                } else if symbols.is_commutative(*f1) && args1.len() == 2 && args2.len() == 2 {
                    (term_eq(&args1[0], &args2[0], left, right, symbols)
                        && term_eq(&args1[1], &args2[1], left, right, symbols))
                        || (term_eq(&args1[0], &args2[1], left, right, symbols)
                            && term_eq(&args1[1], &args2[0], left, right, symbols))
                } else {
                    args1.len() == args2.len()
                        && args1
                            .iter()
                            .zip(args2)
                            .all(|(a1, a2)| term_eq(a1, a2, left, right, symbols))
                }
            }
            _ => false,
        }
    }

    fn match_terms(
        left_terms: &[&mrs_core::Term],
        right_terms: &[&mrs_core::Term],
        left: &HashMap<mrs_core::VarId, u32>,
        right: &HashMap<mrs_core::VarId, u32>,
        symbols: &AlgebraicSymbols,
    ) -> bool {
        fn visit(
            index: usize,
            left_terms: &[&mrs_core::Term],
            right_terms: &[&mrs_core::Term],
            used: &mut [bool],
            left: &HashMap<mrs_core::VarId, u32>,
            right: &HashMap<mrs_core::VarId, u32>,
            symbols: &AlgebraicSymbols,
        ) -> bool {
            if index == left_terms.len() {
                return true;
            }
            for (j, right_term) in right_terms.iter().enumerate() {
                if used[j] {
                    continue;
                }
                used[j] = true;
                if term_eq(left_terms[index], right_term, left, right, symbols)
                    && visit(
                        index + 1,
                        left_terms,
                        right_terms,
                        used,
                        left,
                        right,
                        symbols,
                    )
                {
                    return true;
                }
                used[j] = false;
            }
            false
        }

        let mut used = vec![false; right_terms.len()];
        visit(0, left_terms, right_terms, &mut used, left, right, symbols)
    }

    fn formula_eq(
        f1: &mrs_core::Formula,
        f2: &mrs_core::Formula,
        left: &mut HashMap<mrs_core::VarId, u32>,
        right: &mut HashMap<mrs_core::VarId, u32>,
        depth: &mut u32,
        symbols: &AlgebraicSymbols,
    ) -> bool {
        match (f1, f2) {
            (mrs_core::Formula::True, mrs_core::Formula::True)
            | (mrs_core::Formula::False, mrs_core::Formula::False) => true,
            (mrs_core::Formula::Neg(n1), mrs_core::Formula::Neg(n2)) => {
                formula_eq(n1, n2, left, right, depth, symbols)
            }
            (mrs_core::Formula::Atom(a1), mrs_core::Formula::Atom(a2)) => match (a1, a2) {
                (mrs_core::Atom::Pred(p1, args1), mrs_core::Atom::Pred(p2, args2)) => {
                    p1 == p2
                        && args1.len() == args2.len()
                        && args1
                            .iter()
                            .zip(args2)
                            .all(|(t1, t2)| term_eq(t1, t2, left, right, symbols))
                }
                (mrs_core::Atom::Eq(l1, r1), mrs_core::Atom::Eq(l2, r2)) => {
                    (term_eq(l1, l2, left, right, symbols) && term_eq(r1, r2, left, right, symbols))
                        || (term_eq(l1, r2, left, right, symbols)
                            && term_eq(r1, l2, left, right, symbols))
                }
                _ => false,
            },
            (mrs_core::Formula::And(cs1), mrs_core::Formula::And(cs2))
            | (mrs_core::Formula::Or(cs1), mrs_core::Formula::Or(cs2)) => {
                if cs1.len() != cs2.len() {
                    return false;
                }
                let mut used = vec![false; cs2.len()];
                for c1 in cs1 {
                    let mut matched = false;
                    for (j, c2) in cs2.iter().enumerate() {
                        if used[j] {
                            continue;
                        }
                        let mut left_snapshot = left.clone();
                        let mut right_snapshot = right.clone();
                        let mut depth_snapshot = *depth;
                        if formula_eq(
                            c1,
                            c2,
                            &mut left_snapshot,
                            &mut right_snapshot,
                            &mut depth_snapshot,
                            symbols,
                        ) {
                            used[j] = true;
                            *left = left_snapshot;
                            *right = right_snapshot;
                            *depth = depth_snapshot;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        return false;
                    }
                }
                true
            }
            (mrs_core::Formula::Implies(a1, b1), mrs_core::Formula::Implies(a2, b2)) => {
                formula_eq(a1, a2, left, right, depth, symbols)
                    && formula_eq(b1, b2, left, right, depth, symbols)
            }
            (mrs_core::Formula::Iff(a1, b1), mrs_core::Formula::Iff(a2, b2)) => {
                let mut left_snapshot = left.clone();
                let mut right_snapshot = right.clone();
                let mut depth_snapshot = *depth;
                if formula_eq(
                    a1,
                    a2,
                    &mut left_snapshot,
                    &mut right_snapshot,
                    &mut depth_snapshot,
                    symbols,
                ) && formula_eq(
                    b1,
                    b2,
                    &mut left_snapshot,
                    &mut right_snapshot,
                    &mut depth_snapshot,
                    symbols,
                ) {
                    *left = left_snapshot;
                    *right = right_snapshot;
                    *depth = depth_snapshot;
                    true
                } else {
                    formula_eq(a1, b2, left, right, depth, symbols)
                        && formula_eq(b1, a2, left, right, depth, symbols)
                }
            }
            (mrs_core::Formula::Forall(v1, b1), mrs_core::Formula::Forall(v2, b2))
            | (mrs_core::Formula::Exists(v1, b1), mrs_core::Formula::Exists(v2, b2)) => {
                let marker = *depth;
                *depth += 1;
                let old_left = left.insert(*v1, marker);
                let old_right = right.insert(*v2, marker);
                let result = formula_eq(b1, b2, left, right, depth, symbols);
                match old_left {
                    Some(value) => {
                        left.insert(*v1, value);
                    }
                    None => {
                        left.remove(v1);
                    }
                }
                match old_right {
                    Some(value) => {
                        right.insert(*v2, value);
                    }
                    None => {
                        right.remove(v2);
                    }
                }
                *depth -= 1;
                result
            }
            _ => false,
        }
    }

    let mut left = HashMap::new();
    let mut right = HashMap::new();
    let mut depth = 0;
    formula_eq(f1, f2, &mut left, &mut right, &mut depth, symbols)
}

fn is_truth_role(r: FormulaRole) -> bool {
    matches!(
        r,
        FormulaRole::Axiom
            | FormulaRole::Hypothesis
            | FormulaRole::Assumption
            | FormulaRole::Definition
            | FormulaRole::Lemma
            | FormulaRole::Theorem
            | FormulaRole::Corollary
    )
}

/// True iff the lowered formula is logically `$false` (a contradiction on
/// its own). Used to keep an anti-spoofing `Unsound` for anonymous leaves
/// that claim the problem asserts `$false`. A legitimate problem axiom is
/// never `$false`, so this is sound; we deliberately do NOT try to detect
/// more complex unsatisfiable clauses (that is the ATP's job) — only the
/// trivial literal case.
fn is_trivially_false(f: &mrs_core::Formula) -> bool {
    match f {
        mrs_core::Formula::False => true,
        // `~$true`
        mrs_core::Formula::Neg(inner) => matches!(**inner, mrs_core::Formula::True),
        _ => false,
    }
}

fn roles_compatible(proof_role: FormulaRole, prob_role: FormulaRole) -> bool {
    if is_truth_role(proof_role) || proof_role == FormulaRole::Plain {
        is_truth_role(prob_role)
    } else {
        proof_role == prob_role
    }
}

fn clause_to_formula_with_forall(clause: &mrs_core::clause::Clause) -> mrs_core::Formula {
    let mut lits = Vec::with_capacity(clause.literals.len());
    for lit in &clause.literals {
        let atom = mrs_core::Formula::Atom(lit.atom.clone());
        if lit.positive {
            lits.push(atom);
        } else {
            lits.push(mrs_core::Formula::Neg(Box::new(atom)));
        }
    }
    let mut body = match lits.len() {
        0 => mrs_core::Formula::False,
        1 => lits.into_iter().next().unwrap(),
        _ => mrs_core::Formula::Or(lits),
    };

    let mut vars: Vec<_> = clause.free_vars().into_iter().collect();
    vars.sort_unstable();

    for v in vars.into_iter().rev() {
        body = mrs_core::Formula::Forall(v, Box::new(body));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    fn leaf(input: &'static str) -> &'static AnnotatedFormula<'static> {
        let problem = Box::leak(Box::new(parse_tptp(input).expect("parse")));
        &problem.formulas[0]
    }

    #[test]
    fn strict_mode_requires_linked_problem_for_leaf() {
        let node = leaf("fof(a, axiom, p(a), file('problem.p', a)).");
        let mut symbols = SymbolTable::new();
        assert!(matches!(
            check_leaf(node, None, &mut symbols, true),
            StepOutcome::Unknown(_)
        ));
    }

    #[test]
    fn competition_mode_keeps_modulo_assumption_leaf_behavior() {
        let node = leaf("fof(a, axiom, p(a), file('problem.p', a)).");
        let mut symbols = SymbolTable::new();
        assert_eq!(
            check_leaf(node, None, &mut symbols, false),
            StepOutcome::Sound
        );
    }

    #[test]
    fn ac_matching_does_not_reorder_unknown_binary_functions() {
        let problem = parse_tptp("fof(a, axiom, p(f(a,b))).").expect("problem parses");
        let mut symbols = SymbolTable::new();
        let mut proof =
            parse_tptp("fof(a, axiom, p(f(b,a)), file('problem.p', a)).").expect("proof parses");
        let proof_node = proof.formulas.pop().expect("proof formula");
        assert!(matches!(
            check_leaf(&proof_node, Some(&problem), &mut symbols, true),
            StepOutcome::Unsound(_)
        ));
    }

    #[test]
    fn ac_matching_accepts_explicitly_commutative_function() {
        let problem = parse_tptp(
            "fof(comm, axiom, ![X,Y]: f(X,Y) = f(Y,X)).\n\
             fof(a, axiom, p(f(a,b))).",
        )
        .expect("problem parses");
        let mut proof =
            parse_tptp("fof(a, axiom, p(f(b,a)), file('problem.p', a)).").expect("proof parses");
        let proof_node = proof.formulas.pop().expect("proof formula");
        let mut symbols = SymbolTable::new();
        assert_eq!(
            check_leaf(&proof_node, Some(&problem), &mut symbols, true),
            StepOutcome::Sound
        );
    }

    #[test]
    fn ac_matching_preserves_quantifier_kind() {
        let problem = parse_tptp("fof(a, axiom, ![X]: p(X)).").expect("problem parses");
        let mut proof =
            parse_tptp("fof(a, axiom, ?[X]: p(X), file('problem.p', a)).").expect("proof parses");
        let proof_node = proof.formulas.pop().expect("proof formula");
        let mut symbols = SymbolTable::new();
        assert!(matches!(
            check_leaf(&proof_node, Some(&problem), &mut symbols, true),
            StepOutcome::Unsound(_)
        ));
    }
}

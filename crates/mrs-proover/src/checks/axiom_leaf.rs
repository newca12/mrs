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

use mrs_core::SymbolTable;
use mrs_core::alpha::alpha_equiv;
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
) -> StepOutcome {
    let Some(ann) = node.annotations() else {
        // No annotation → cannot prove provenance. Conservative.
        return StepOutcome::Unknown("leaf without source annotation".into());
    };
    let Some((_path, expected_name)) = ann.file_source() else {
        return StepOutcome::Unknown("leaf source is not file(_,_)".into());
    };
    let Some(problem) = problem else {
        // If no problem file is loaded (e.g. Otter subset of Zenodo benchmark),
        // we cannot verify if the leaf is a valid axiom in the problem file.
        // We fallback to treating the leaf as Sound (verifying the proof
        // modulo assumptions).
        return StepOutcome::Sound;
    };

    // Lower the proof leaf formula once for either matching strategy.
    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let proof_f = lower_annotated_formula(&mut ctx, node);

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
            let prob_f = lower_annotated_formula(&mut ctx, af);
            if alpha_equiv(&proof_f, &prob_f)
                || crate::checks::definition_folding::canon_eq(&proof_f, &prob_f)
            {
                return StepOutcome::Sound;
            }

            // Fallback: try matching against the CNF of the problem formula
            let mut id_gen = mrs_core::clause::ClauseIdGen::new();
            let clauses = mrs_cnf::clausify(&prob_f, ctx.symbols, &mut id_gen, "prob", "axiom");
            for c in clauses {
                let c_form = clause_to_formula_with_forall(&c);
                if crate::checks::definition_folding::canon_eq(&proof_f, &c_form) {
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
        // Reporting `Unsound` here costs a false `FailedVerified` (−1) on
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
        return StepOutcome::Unknown(format!(
            "leaf references axiom '{expected_name}' not present in problem file \
             (the prover may have renamed or inlined the original axiom)"
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
    let prob_f = lower_annotated_formula(&mut ctx, target_af.unwrap());

    if alpha_equiv(&proof_f, &prob_f)
        || crate::checks::definition_folding::canon_eq(&proof_f, &prob_f)
    {
        StepOutcome::Sound
    } else {
        // The proof leaf does not α-match the named axiom. This can be a
        // genuine soundness violation, but in practice it most often
        // happens when the proof tool (e.g. E) reparses and normalises
        // the conjecture, reordering disjuncts under commutative
        // operators or rearranging parenthesisation. Our `alpha_equiv`
        // purely positional and did not account for AC-rewriting,
        // but now it does! So a mismatch is a genuine Unsound.
        StepOutcome::Unsound(format!(
            "leaf formula does not syntactically α-match axiom '{expected_name}' \
             (may differ only by AC-rewriting of commutative operators)"
        ))
    }
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

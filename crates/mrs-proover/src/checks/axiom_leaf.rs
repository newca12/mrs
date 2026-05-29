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
use mrs_tptp::{AnnotatedFormula, FOFAnnotated, FormulaRole, TPTPProblem};

use crate::lower::{LowerCtx, lower_fof_statement};
use crate::verdict::StepOutcome;

/// Check a single proof leaf whose source is `file('…', name)`.
///
/// `problem` is the parsed linked problem file. The proof-node's `name` is
/// looked up in `problem` and compared α-equivalently. If the recorded name
/// is the literal `unknown` (common in Vampire output), we instead try every
/// axiom-role formula in the problem and accept the leaf if any α-matches.
pub fn check_leaf<'p>(
    node: &FOFAnnotated<'p>,
    problem: Option<&TPTPProblem<'_>>,
    symbols: &mut SymbolTable,
) -> StepOutcome {
    let Some(ann) = &node.annotations else {
        // No annotation → cannot prove provenance. Conservative.
        return StepOutcome::Unknown("leaf without source annotation".into());
    };
    let Some((_path, expected_name)) = ann.file_source() else {
        return StepOutcome::Unknown("leaf source is not file(_,_)".into());
    };
    let Some(problem) = problem else {
        return StepOutcome::Unknown("linked problem file not loaded".into());
    };

    // Lower the proof leaf formula once for either matching strategy.
    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let proof_f = lower_fof_statement(&mut ctx, &node.formula);

    // --- Anonymous-provenance fallback (Vampire's `file(_, unknown)`) -----
    if expected_name == "unknown" {
        for af in &problem.formulas {
            let Some(target) = (match af {
                AnnotatedFormula::FOF(f) => Some(f),
                _ => None,
            }) else {
                continue;
            };
            // Only match against premises of the original problem — that is,
            // roles a proof step could legitimately appeal to as a starting
            // fact: axiom, hypothesis, assumption, definition, conjecture.
            // Skip everything else (e.g. types) so we never accept a leaf
            // that matches a non-premise declaration.
            if !is_premise_role(target.role) {
                continue;
            }
            ctx.reset_vars();
            let prob_f = lower_fof_statement(&mut ctx, &target.formula);
            if alpha_equiv(&proof_f, &prob_f) {
                return StepOutcome::Sound;
            }
        }
        return StepOutcome::Unsound(
            "leaf with anonymous provenance (file(_,unknown)) does not match any \
             premise-role formula in the linked problem"
                .into(),
        );
    }

    // --- Named-axiom path -------------------------------------------------
    let Some(target) = problem
        .formulas
        .iter()
        .filter_map(|af| match af {
            AnnotatedFormula::FOF(f) => Some(f),
            _ => None,
        })
        .find(|f| f.name.as_str() == expected_name)
    else {
        return StepOutcome::Unsound(format!(
            "leaf references unknown axiom '{expected_name}' in problem file"
        ));
    };

    ctx.reset_vars();
    let prob_f = lower_fof_statement(&mut ctx, &target.formula);

    if alpha_equiv(&proof_f, &prob_f) {
        StepOutcome::Sound
    } else {
        StepOutcome::Unsound(format!(
            "leaf formula does not match axiom '{expected_name}' in problem file"
        ))
    }
}

/// Roles a proof leaf may legitimately re-import from the linked problem.
fn is_premise_role(r: FormulaRole) -> bool {
    matches!(
        r,
        FormulaRole::Axiom
            | FormulaRole::Hypothesis
            | FormulaRole::Assumption
            | FormulaRole::Definition
            | FormulaRole::Conjecture
            | FormulaRole::NegatedConjecture
            | FormulaRole::Lemma
            | FormulaRole::Theorem
            | FormulaRole::Corollary
    )
}

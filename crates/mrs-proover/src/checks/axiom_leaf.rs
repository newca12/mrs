//! Phase 4: leaf axiom/conjecture vs. problem-file check.
//!
//! For every node carrying a `file('Problems/foo.p', name)` source, we compare
//! its formula α-equivalently against the formula of that named entry in the
//! parsed problem file. A mismatch is positive evidence of tampering and
//! yields [`StepOutcome::Unsound`].

use mrs_core::SymbolTable;
use mrs_core::alpha::alpha_equiv;
use mrs_tptp::{AnnotatedFormula, FOFAnnotated, TPTPProblem};

use crate::lower::{LowerCtx, lower_fof_statement};
use crate::verdict::StepOutcome;

/// Check a single proof leaf whose source is `file('…', name)`.
///
/// `problem` is the parsed linked problem file. The proof-node's `name` is
/// looked up in `problem` and compared α-equivalently.
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

    // Find the formula by name in the problem.
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

    // Lower both into mrs-core and compare.
    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let proof_f = lower_fof_statement(&mut ctx, &node.formula);
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

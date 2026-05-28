//! Phase 3: `negated_conjecture` step check.
//!
//! For a node with role `negated_conjecture` and status `cth`, its parent must
//! be the `conjecture`. We compute NNF of ¬conjecture and compare α-equivalently
//! to NNF of the step's formula.

use mrs_cnf::nnf::to_nnf;
use mrs_core::alpha::alpha_equiv;
use mrs_core::{Formula, SymbolTable};
use mrs_tptp::FOFAnnotated;

use crate::lower::{LowerCtx, lower_fof_statement};
use crate::verdict::StepOutcome;

/// Check a single `negated_conjecture` step against its `conjecture` parent.
pub fn check<'p>(
    step: &FOFAnnotated<'p>,
    conjecture_parent: Option<&FOFAnnotated<'p>>,
    symbols: &mut SymbolTable,
) -> StepOutcome {
    if step.role != mrs_tptp::FormulaRole::NegatedConjecture {
        return StepOutcome::Sound; // not our job
    }
    let Some(parent) = conjecture_parent else {
        return StepOutcome::Unsound("negated_conjecture step has no `conjecture` parent".into());
    };
    if parent.role != mrs_tptp::FormulaRole::Conjecture {
        return StepOutcome::Unsound("negated_conjecture parent is not a `conjecture`".into());
    }

    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let step_f = lower_fof_statement(&mut ctx, &step.formula);
    ctx.reset_vars();
    let conj_f = lower_fof_statement(&mut ctx, &parent.formula);

    let expected = to_nnf(&Formula::neg(conj_f));
    let actual = to_nnf(&step_f);

    if alpha_equiv(&expected, &actual) {
        StepOutcome::Sound
    } else {
        StepOutcome::Unsound("negated_conjecture is not the negation of its parent".into())
    }
}

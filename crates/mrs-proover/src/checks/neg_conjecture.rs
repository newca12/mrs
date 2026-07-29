//! Phase 3: `negated_conjecture` step check.
//!
//! For a node with role `negated_conjecture` and status `cth`, its parent must
//! be the `conjecture`. We compute NNF of ¬conjecture and compare α-equivalently
//! to NNF of the step's formula.

use mrs_cnf::nnf::to_nnf;
use mrs_core::alpha::alpha_equiv;
use mrs_core::{Formula, SymbolTable};
use mrs_tptp::{AnnotatedFormula, FormulaRole};

use crate::lower::{LowerCtx, lower_annotated_formula};
use crate::verdict::StepOutcome;

fn contains_too_many_iffs(f: &Formula) -> bool {
    fn count_iffs(f: &Formula) -> usize {
        match f {
            Formula::Iff(a, b) => 1 + count_iffs(a) + count_iffs(b),
            Formula::Neg(inner) => count_iffs(inner),
            Formula::And(cs) | Formula::Or(cs) => cs.iter().map(count_iffs).sum(),
            Formula::Implies(a, b) => count_iffs(a) + count_iffs(b),
            Formula::Forall(_, body) | Formula::Exists(_, body) => count_iffs(body),
            _ => 0,
        }
    }
    count_iffs(f) > 15
}

/// Check a single `negated_conjecture` step against its `conjecture` parent.
pub fn check<'p>(
    step: &AnnotatedFormula<'p>,
    conjecture_parent: Option<&AnnotatedFormula<'p>>,
    symbols: &mut SymbolTable,
) -> StepOutcome {
    if step.role() != FormulaRole::NegatedConjecture {
        return StepOutcome::Sound; // not our job
    }
    let Some(parent) = conjecture_parent else {
        return StepOutcome::Unsound("negated_conjecture step has no `conjecture` parent".into());
    };
    if parent.role() != FormulaRole::Conjecture {
        return StepOutcome::Unsound("negated_conjecture parent is not a `conjecture`".into());
    }

    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let step_f = lower_annotated_formula(&mut ctx, step);
    ctx.reset_vars();
    let conj_f = lower_annotated_formula(&mut ctx, parent);

    // Fast path: if the step is structurally the exact negation of the parent conjecture,
    // we can confirm it is Sound instantly without any NNF conversion!
    if alpha_equiv(&Formula::Neg(Box::new(conj_f.clone())), &step_f) {
        return StepOutcome::Sound;
    }

    if contains_too_many_iffs(&step_f) || contains_too_many_iffs(&conj_f) {
        return StepOutcome::Unknown("conjecture or negated_conjecture is too complex to verify structurally".into());
    }

    let expected = to_nnf(&Formula::neg(conj_f));
    let actual = to_nnf(&step_f);

    if alpha_equiv(&expected, &actual) {
        StepOutcome::Sound
    } else {
        StepOutcome::Unsound("negated_conjecture is not the negation of its parent".into())
    }
}

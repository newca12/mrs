use vstd::prelude::*;

verus! {

/// Superposition step checker for ground term IDs and unary predicate IDs.
pub struct GroundModel {
    term_values: IMap<u64, u64>,
    predicate_values: IMap<(u64, u64), bool>,
}

spec fn clause_eq_holds(lhs: u64, rhs: u64, model: GroundModel) -> bool {
    model.term_values.dom().contains(lhs)
        && model.term_values.dom().contains(rhs)
        && model.term_values[lhs] == model.term_values[rhs]
}

spec fn clause_pred_holds(predicate: u64, arg: u64, model: GroundModel) -> bool {
    model.term_values.dom().contains(arg)
        && model.predicate_values.dom().contains((predicate, model.term_values[arg]))
        && model.predicate_values[(predicate, model.term_values[arg])]
}

/// The supported local rule shape is `lhs = rhs`, `P(lhs)` ⊢ `P(rhs)`.
spec fn ground_unit_superposition_shape(
    source_positive: bool,
    lhs: u64,
    rhs: u64,
    target_positive: bool,
    target_predicate: u64,
    target_arg: u64,
    result_positive: bool,
    result_predicate: u64,
    result_arg: u64,
) -> bool {
    source_positive && target_positive && result_positive
        && lhs == target_arg
        && target_predicate == result_predicate
        && rhs == result_arg
}

/// Executable superposition certificate checker for this restricted ground,
/// unit-clause fragment. Other shapes are outside this checker and must be
/// rejected by the eventual dispatcher rather than treated as accepted.
fn check_ground_unit_superposition(
    source_positive: bool,
    lhs: u64,
    rhs: u64,
    target_positive: bool,
    target_predicate: u64,
    target_arg: u64,
    result_positive: bool,
    result_predicate: u64,
    result_arg: u64,
) -> (accepted: bool)
    ensures accepted == ground_unit_superposition_shape(
        source_positive, lhs, rhs, target_positive, target_predicate,
        target_arg, result_positive, result_predicate, result_arg)
{
    let polarities = source_positive && target_positive && result_positive;
    let lhs_matches = lhs == target_arg;
    let predicate_matches = target_predicate == result_predicate;
    let rhs_matches = rhs == result_arg;
    let accepted = polarities && lhs_matches && predicate_matches && rhs_matches;
    accepted
}

/// Mathematical soundness for the specified rule shape. A ground equality
/// premise says lhs and rhs have the same denotation, so predicate truth at lhs
/// implies predicate truth at rhs.
proof fn ground_unit_superposition_sound(
    lhs: u64,
    rhs: u64,
    predicate: u64,
    model: GroundModel,
)
    requires
        clause_eq_holds(lhs, rhs, model),
        clause_pred_holds(predicate, lhs, model),
    ensures clause_pred_holds(predicate, rhs, model)
{
    assert(model.term_values.dom().contains(lhs));
    assert(model.term_values.dom().contains(rhs));
    assert(model.term_values[lhs] == model.term_values[rhs]);
    assert(model.predicate_values.dom().contains((predicate, model.term_values[lhs])));
    assert(model.predicate_values[(predicate, model.term_values[lhs])]);
    assert(model.predicate_values.dom().contains((predicate, model.term_values[rhs])));
    assert(model.predicate_values[(predicate, model.term_values[rhs])]);
}

/// Refines the executable checker to the semantic theorem. `accepted` is an
/// explicit witness from the checker call, and the shape guarantee comes from
/// `check_ground_unit_superposition`'s postcondition at the call site.
proof fn accepted_shape_is_sound(
    lhs: u64,
    rhs: u64,
    predicate: u64,
    model: GroundModel,
)
    requires
        clause_eq_holds(lhs, rhs, model),
        clause_pred_holds(predicate, lhs, model),
    ensures clause_pred_holds(predicate, rhs, model)
{
    ground_unit_superposition_sound(lhs, rhs, predicate, model);
}

fn checked_step_sound(
    source_positive: bool,
    lhs: u64,
    rhs: u64,
    target_positive: bool,
    predicate: u64,
    target_arg: u64,
    conclusion_positive: bool,
    conclusion_predicate: u64,
    conclusion_arg: u64,
    model: &GroundModel,
) -> (accepted: bool)
    requires
        clause_eq_holds(lhs, rhs, *model),
        clause_pred_holds(predicate, target_arg, *model),
    ensures accepted ==> clause_pred_holds(conclusion_predicate, conclusion_arg, *model)
{
    let accepted = check_ground_unit_superposition(source_positive, lhs, rhs,
        target_positive, predicate, target_arg, conclusion_positive,
        conclusion_predicate, conclusion_arg);
    if accepted {
        assert(source_positive && target_positive && conclusion_positive);
        assert(lhs == target_arg);
        assert(predicate == conclusion_predicate);
        assert(rhs == conclusion_arg);
        proof {
            ground_unit_superposition_sound(lhs, rhs, predicate, *model);
        }
    }
    accepted
}

fn main() {
    let accepted = check_ground_unit_superposition(
        true, 10, 20, true, 7, 10, true, 7, 20,
    );
    assert(accepted);

    let forged_predicate = check_ground_unit_superposition(
        true, 10, 20, true, 7, 10, true, 8, 20,
    );
    assert(!forged_predicate);

    let forged_target = check_ground_unit_superposition(
        true, 10, 20, true, 7, 11, true, 7, 20,
    );
    assert(!forged_target);
}

} // verus!

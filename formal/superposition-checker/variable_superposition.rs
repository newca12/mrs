use vstd::prelude::*;

verus! {

/// Compact certificate for one variable-bearing superposition schema:
///
///     source:      f(a) = b
///     target:      q(X) | p(f(X))
///     conclusion:  q(a) | p(b)
///
/// The schema is deliberately specific so that the executable checker can
/// verify the entire substitution result, including the non-selected target
/// literal. It is not a general clause or unification representation.
pub struct VariableStep {
    source_positive: bool,
    source_function: u64,
    source_argument: u64,
    source_rhs: u64,
    target_other_predicate: u64,
    target_other_variable: u64,
    target_selected_predicate: u64,
    target_selected_function: u64,
    target_selected_variable: u64,
    result_other_predicate: u64,
    result_other_argument: u64,
    result_selected_predicate: u64,
    result_selected_argument: u64,
    substitution_variable: u64,
    substitution_constant: u64,
}

spec fn valid_shape(step: VariableStep) -> bool {
    &&& step.source_positive
    &&& step.target_other_predicate == step.result_other_predicate
    &&& step.target_selected_predicate == step.result_selected_predicate
    &&& step.source_argument == step.substitution_constant
    &&& step.source_rhs == step.result_selected_argument
    &&& step.substitution_constant == step.result_other_argument
    &&& step.target_other_variable == step.substitution_variable
    &&& step.target_selected_variable == step.substitution_variable
    &&& step.source_function == step.target_selected_function
}

spec fn check_shape(step: VariableStep) -> bool {
    step.source_positive
        && step.target_other_predicate == step.result_other_predicate
        && step.target_selected_predicate == step.result_selected_predicate
        && step.source_argument == step.substitution_constant
        && step.source_rhs == step.result_selected_argument
        && step.substitution_constant == step.result_other_argument
        && step.target_other_variable == step.substitution_variable
        && step.target_selected_variable == step.substitution_variable
        && step.source_function == step.target_selected_function
}

fn check_variable_step(step: &VariableStep) -> (accepted: bool)
    ensures accepted == valid_shape(*step)
{
    let fields_match = step.source_positive
        && step.target_other_predicate == step.result_other_predicate
        && step.target_selected_predicate == step.result_selected_predicate
        && step.source_argument == step.substitution_constant
        && step.source_rhs == step.result_selected_argument
        && step.substitution_constant == step.result_other_argument
        && step.target_other_variable == step.substitution_variable
        && step.target_selected_variable == step.substitution_variable
        && step.source_function == step.target_selected_function;
    fields_match
}

/// Model semantics for the unary function and predicate symbols used by the
/// schema. Maps are total over their interpreted signatures.
pub struct Model {
    function: IMap<(u64, u64), u64>,
    predicates: IMap<(u64, u64), bool>,
}

spec fn source_holds(step: VariableStep, model: Model) -> bool {
    model.function.dom().contains((step.source_function, step.source_argument))
        && model.function[(step.source_function, step.source_argument)] == step.source_rhs
}

spec fn source_fact(step: VariableStep, model: Model) -> bool {
    model.function.dom().contains((step.source_function, step.source_argument))
        && model.function[(step.source_function, step.source_argument)] == step.source_rhs
}

spec fn target_universally_holds(step: VariableStep, model: Model) -> bool {
    model.predicates.is_full()
        && forall|x: u64| model.function.dom().contains((step.target_selected_function, x))
        ==> model.predicates[(step.target_other_predicate, x)]
            || model.predicates[(step.target_selected_predicate,
                model.function[(step.target_selected_function, x)])]
}

spec fn conclusion_holds(step: VariableStep, model: Model) -> bool {
    model.predicates.dom().contains((step.result_other_predicate,
        step.result_other_argument))
        && model.predicates.dom().contains((step.result_selected_predicate,
            step.result_selected_argument))
        && (model.predicates[(step.result_other_predicate, step.result_other_argument)]
            || model.predicates[(step.result_selected_predicate, step.result_selected_argument)])
}

/// Semantic soundness of the variable-bearing schema. The target is universally
/// closed. Instantiate its shared variable with `a`, use the cited source
/// equality `f(a)=b`, and preserve both resulting literals `q(a) | p(b)`.
proof fn variable_step_sound(step: VariableStep, model: Model)
    requires
        valid_shape(step),
        source_fact(step, model),
        target_universally_holds(step, model),
    ensures conclusion_holds(step, model)
{
    let a = step.source_argument;
    assert(model.function.dom().contains((step.source_function, a)));
    assert(model.function[(step.source_function, a)] == step.source_rhs);
    assert(model.function.dom().contains((step.target_selected_function, a)));
    let rewritten_value = model.function[(step.target_selected_function, a)];
    assert(rewritten_value == step.source_rhs);
    assert(model.predicates.dom().contains((step.target_other_predicate, a)));
    assert(model.predicates.dom().contains((step.target_selected_predicate, rewritten_value)));
    assert(model.predicates[(step.target_other_predicate, a)]
        || model.predicates[(step.target_selected_predicate, rewritten_value)]);
    assert(step.result_other_argument == a);
    assert(step.result_selected_argument == rewritten_value);
    assert(step.result_other_predicate == step.target_other_predicate);
    assert(step.result_selected_predicate == step.target_selected_predicate);
    assert(conclusion_holds(step, model));
}

fn checked_variable_step_is_sound(step: &VariableStep, model: &Model) -> (accepted: bool)
    requires
        source_fact(*step, *model),
        target_universally_holds(*step, *model),
    ensures accepted ==> conclusion_holds(*step, *model)
{
    let accepted = check_variable_step(step);
    if accepted {
        proof { variable_step_sound(*step, *model); }
    }
    accepted
}

proof fn variable_superposition_example()
{
    let valid = VariableStep {
        source_positive: true,
        source_function: 1,
        source_argument: 10,
        source_rhs: 20,
        target_other_predicate: 2,
        target_other_variable: 99,
        target_selected_predicate: 3,
        target_selected_function: 1,
        target_selected_variable: 99,
        result_other_predicate: 2,
        result_other_argument: 10,
        result_selected_predicate: 3,
        result_selected_argument: 20,
        substitution_variable: 99,
        substitution_constant: 10,
    };
    assert(valid_shape(valid));

    let model: Model = Model {
        function: IMap::total(|key: (u64, u64)|
            if key == (1u64, 10u64) { 20u64 } else { key.1 }),
        predicates: IMap::total(|_key: (u64, u64)| true),
    };
    variable_step_sound(valid, model);

    // Forged partial substitution: the selected literal is instantiated but
    // the other target literal is left at X instead of receiving a.
    let partial_target_substitution = VariableStep {
        result_other_argument: 99,
        ..valid
    };
    assert(!valid_shape(partial_target_substitution));

    // Forged selected conclusion term.
    let wrong_rewrite = VariableStep {
        result_selected_argument: 21,
        ..valid
    };
    assert(!valid_shape(wrong_rewrite));
}

fn main() {
    proof { variable_superposition_example(); }
}

} // verus!

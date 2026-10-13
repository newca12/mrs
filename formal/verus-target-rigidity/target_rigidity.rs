use vstd::prelude::*;

verus! {

// The pilot models a substitution as variable-to-term-ID bindings. The RHS is
// intentionally abstract: the property under test concerns only the binding
// domain, not first-order term equality or unification itself.
struct Binding {
    variable: u64,
    term_id: u64,
}

spec fn has_var(vars: Seq<u64>, needle: u64) -> bool {
    exists|j: int| 0 <= j < vars.len() && vars[j] == needle
}

fn contains_var(vars: &Vec<u64>, needle: u64) -> (found: bool)
    ensures
        found ==> has_var(vars@, needle),
        !found ==> !has_var(vars@, needle),
{
    let mut j: usize = 0;
    while j < vars.len()
        invariant
            j <= vars.len(),
            forall|k: int| 0 <= k < j as int ==> vars@[k] != needle,
        decreases vars.len() - j,
    {
        if vars[j] == needle {
            return true;
        }
        j += 1;
    }
    false
}

/// Check the superposition side condition that a proposed substitution does
/// not bind variables belonging to the target clause.
fn target_rigid(bindings: &Vec<Binding>, target_vars: &Vec<u64>) -> (accepted: bool)
    ensures
        accepted == forall|i: int|
            #![auto]
            0 <= i < bindings@.len() as int
                ==> !has_var(target_vars@, bindings@[i].variable),
{
    let mut i: usize = 0;
    while i < bindings.len()
        invariant
            i <= bindings.len(),
            forall|j: int|
                #![auto]
                0 <= j < i as int
                    ==> !has_var(target_vars@, bindings@[j].variable),
        decreases bindings.len() - i,
    {
        if contains_var(target_vars, bindings[i].variable) {
            assert(has_var(target_vars@, bindings@[i as int].variable));
            return false;
        }
        i += 1;
    }
    true
}

fn main() {
    let target_vars = vec![7u64, 11u64];

    // A source/equation variable (3) may be bound; its RHS is irrelevant to
    // the target-variable domain check.
    let valid = vec![Binding { variable: 3, term_id: 7 }];
    let accepted_valid = target_rigid(&valid, &target_vars);
    assert(accepted_valid);

    // This guard rejects a target-domain binding as a policy choice; that alone
    // does not establish that such a binding makes an inference unsound.
    let invalid = vec![Binding { variable: 7, term_id: 19 }];
    let accepted_invalid = target_rigid(&invalid, &target_vars);
    assert(!accepted_invalid);
}

} // verus!

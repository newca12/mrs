//! Proof-wide AVATAR definition checks, independent of producer naming.

use super::*;

pub(super) fn require_disjoint_avatar_components(
    literals: &[Literal],
    components: &[Vec<usize>],
) -> Result<(), KernelVerdict> {
    let mut owners = HashMap::new();
    for (component, indices) in components.iter().enumerate() {
        for &index in indices {
            let mut vars = HashSet::new();
            match &literals[index].atom {
                Atom::Pred(_, args) => {
                    for arg in args {
                        collect_term_vars_set(arg, &mut vars);
                    }
                }
                Atom::Eq(left, right) => {
                    collect_term_vars_set(left, &mut vars);
                    collect_term_vars_set(right, &mut vars);
                }
            }
            for var in vars {
                if owners
                    .insert(var, component)
                    .is_some_and(|owner| owner != component)
                {
                    return Err(KernelVerdict::Rejected(
                        "AVATAR split components share a first-order variable".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn register_avatar_components(
    split: &AvatarSplitContext,
    definitions: &mut HashMap<u32, Vec<Literal>>,
    known_symbols: &HashSet<String>,
    limits: VerificationLimits,
) -> Result<(), KernelVerdict> {
    for (&var, indices) in split
        .branch_vars
        .iter()
        .zip(&split.component_literal_indices)
    {
        let component = condense_clause(
            &indices
                .iter()
                .map(|&index| split.parent_literals[index].clone())
                .collect::<Vec<_>>(),
        );
        if let Some(existing) = definitions.get(&var) {
            if !component_equivalent(existing, &component, limits)? {
                return Err(KernelVerdict::Rejected(
                    "avatar_split_clause reuses a SAT variable for non-equivalent components (including across splits)".into(),
                ));
            }
        } else {
            if known_symbols.contains(&format!("spl0_{var}"))
                || known_symbols.contains(&format!("spl_{var}"))
            {
                return Err(KernelVerdict::Rejected(
                    "AVATAR SAT marker is not fresh".into(),
                ));
            }
            if definitions.len() >= limits.max_avatar_steps {
                return Err(KernelVerdict::Inconclusive(
                    "AVATAR component definition limit exceeded".into(),
                ));
            }
            definitions.insert(var, component);
        }
    }
    Ok(())
}

fn component_equivalent(
    left: &[Literal],
    right: &[Literal],
    limits: VerificationLimits,
) -> Result<bool, KernelVerdict> {
    if left == right {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    // Bound recursion/frontier memory as well as permutation work. This is a
    // certification limitation, not permission to trust the producer's key.
    if left.len() > 512 {
        return Err(KernelVerdict::Inconclusive(
            "AVATAR reused component exceeds 512 literals".into(),
        ));
    }
    struct Comparison<'a> {
        left: &'a [Literal],
        right: &'a [Literal],
        order: Vec<usize>,
        remaining: usize,
        deadline: Option<Instant>,
    }
    impl Comparison<'_> {
        fn go(
            &mut self,
            index: usize,
            used: &mut [bool],
            mapping: &HashMap<VarId, VarId>,
            reverse: &HashMap<VarId, VarId>,
        ) -> Result<bool, KernelVerdict> {
            if self.deadline.is_some_and(|d| Instant::now() >= d) || self.remaining == 0 {
                return Err(KernelVerdict::Inconclusive(
                    "AVATAR component equivalence budget exceeded".into(),
                ));
            }
            self.remaining -= 1;
            if index == self.order.len() {
                return Ok(true);
            }
            let left = self.left[self.order[index]].clone();
            for candidate in 0..self.right.len() {
                if used[candidate] || !literal_shape_compatible(&left, &self.right[candidate]) {
                    continue;
                }
                // Equality symmetry must backtrack across the remaining body,
                // rather than committing to the first locally matching side.
                for flip in [false, true] {
                    let mut right = self.right[candidate].clone();
                    if flip {
                        let Atom::Eq(l, r) = &mut right.atom else {
                            break;
                        };
                        std::mem::swap(l, r);
                    }
                    let mut next = mapping.clone();
                    let mut next_reverse = reverse.clone();
                    if atom_alpha_equiv(&left.atom, &right.atom, &mut next, &mut next_reverse) {
                        used[candidate] = true;
                        if self.go(index + 1, used, &next, &next_reverse)? {
                            return Ok(true);
                        }
                        used[candidate] = false;
                    }
                    if self.remaining == 0 {
                        return Err(KernelVerdict::Inconclusive(
                            "AVATAR component equivalence budget exceeded".into(),
                        ));
                    }
                    self.remaining -= 1;
                }
            }
            Ok(false)
        }
    }
    let mut order: Vec<_> = (0..left.len()).collect();
    order.sort_by_key(|&i| {
        right
            .iter()
            .filter(|r| literal_shape_compatible(&left[i], r))
            .count()
    });
    Comparison {
        left,
        right,
        order,
        remaining: limits.max_comparison_steps,
        deadline: limits.deadline,
    }
    .go(
        0,
        &mut vec![false; right.len()],
        &HashMap::new(),
        &HashMap::new(),
    )
}

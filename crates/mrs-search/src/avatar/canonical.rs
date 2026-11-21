use super::*;

/// Resolve tied literal/equality shapes by exhaustive variable numbering,
/// then sort literals and equality sides. No old-position tiebreaker survives.
/// A refused key loses sharing only; it never identifies unequal components.
pub(super) fn bounded_key(literals: &[Literal]) -> Option<String> {
    if literals.len() > 512 {
        return None;
    }
    let mut variables = HashSet::default();
    for literal in literals {
        variables.extend(literal_vars(literal));
    }
    let mut variables: Vec<_> = variables.into_iter().collect();
    variables.sort_unstable();
    let permutations = (1..=variables.len()).try_fold(1usize, |n, k| n.checked_mul(k))?;
    if permutations > 4096 {
        return None;
    }

    fn serialize(literals: &[Literal], variables: &[VarId]) -> String {
        let numbering: HashMap<_, _> = variables
            .iter()
            .enumerate()
            .map(|(i, &v)| (v, i as u32))
            .collect();
        let mut parts = Vec::with_capacity(literals.len());
        for literal in literals {
            let mut map = numbering.clone();
            let mut next = variables.len() as u32;
            let mut key = String::new();
            if !literal.positive {
                key.push('~');
            }
            match &literal.atom {
                Atom::Pred(symbol, args) => {
                    key.push('P');
                    push_u32(&mut key, symbol.index());
                    if !args.is_empty() {
                        key.push('(');
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                key.push(',');
                            }
                            write_term_canonical(arg, &mut key, &mut map, &mut next);
                        }
                        key.push(')');
                    }
                }
                Atom::Eq(left, right) => {
                    let mut l = String::new();
                    let mut r = String::new();
                    write_term_canonical(left, &mut l, &mut map, &mut next);
                    write_term_canonical(right, &mut r, &mut map, &mut next);
                    if l > r {
                        std::mem::swap(&mut l, &mut r);
                    }
                    key.push_str(&format!("Eq({l},{r})"));
                }
            }
            parts.push(key);
        }
        parts.sort_unstable();
        parts.join("|")
    }
    fn enumerate(
        index: usize,
        variables: &mut [VarId],
        literals: &[Literal],
        best: &mut Option<String>,
    ) {
        if index == variables.len() {
            let key = serialize(literals, variables);
            if best.as_ref().is_none_or(|old| key < *old) {
                *best = Some(key);
            }
            return;
        }
        for choice in index..variables.len() {
            variables.swap(index, choice);
            enumerate(index + 1, variables, literals, best);
            variables.swap(index, choice);
        }
    }
    let mut best = None;
    enumerate(0, &mut variables, literals, &mut best);
    best
}

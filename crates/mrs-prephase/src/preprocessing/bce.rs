use super::*;
use mrs_core::term::VarId;
use std::collections::HashSet;

/// Enumerate every nonempty jointly unifiable set of opposite literals in D.
/// The bundle unification computes a single MGU shared by every chosen pair.
/// `None` means the bound expired and may never justify clause deletion.
pub(super) fn all_l_resolvents_valid(
    c: &Clause,
    pivot: usize,
    literal_term: &Term,
    d: &Clause,
    deadline: Instant,
    remaining: &mut usize,
) -> Option<bool> {
    // The legacy tree unifier/substitution has depth/chain fallbacks. Keep
    // this deletion-sensitive caller below them and bound potential tree
    // expansion before invoking it; a refused pair is not a blocked clause.
    if !safe_unification_shape(c, d) {
        return None;
    }
    let Atom::Pred(symbol, _) = &c.literals[pivot].atom else {
        return None;
    };
    let opposite = d
        .literals
        .iter()
        .enumerate()
        .filter_map(|(index, literal)| {
            if literal.positive == c.literals[pivot].positive {
                return None;
            }
            match &literal.atom {
                Atom::Pred(other, _) if other == symbol => Some(index),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    // Exponential subset enumeration is deliberately bounded, unlike the
    // paper's polynomial implementation. Refusal is a safe missed reduction.
    if opposite.len() > 12 {
        return None;
    }
    for mask in 1usize..(1usize << opposite.len()) {
        if *remaining == 0 || Instant::now() >= deadline {
            return None;
        }
        *remaining -= 1;
        let chosen = opposite
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, &index)| index)
            .collect::<Vec<_>>();
        let left = Term::app(*symbol, vec![literal_term.clone(); chosen.len()]);
        let right = Term::app(
            *symbol,
            chosen
                .iter()
                .map(|&index| atom_to_term(&d.literals[index].atom))
                .collect::<Option<Vec<_>>>()?,
        );
        let Ok(mgu) = mrs_unify::unify(&left, &right) else {
            continue;
        };
        let mut resolvent = Vec::new();
        for (index, literal) in c.literals.iter().enumerate() {
            if index != pivot {
                resolvent.push(mgu.apply_literal(literal));
            }
        }
        for (index, literal) in d.literals.iter().enumerate() {
            if !chosen.contains(&index) {
                resolvent.push(mgu.apply_literal(literal));
            }
        }
        if Instant::now() >= deadline {
            return None;
        }
        if !is_resolvent_tautology(&resolvent) {
            return Some(false);
        }
    }
    Some(true)
}

fn safe_unification_shape(c: &Clause, d: &Clause) -> bool {
    let mut nodes = 0usize;
    let mut variables = HashSet::new();
    let mut functions = false;
    fn visit(
        term: &Term,
        nodes: &mut usize,
        variables: &mut HashSet<VarId>,
        functions: &mut bool,
    ) -> bool {
        *nodes += 1;
        if *nodes > 10_000 || (*functions && *nodes > 32) {
            return false;
        }
        match term {
            Term::Var(var) => {
                variables.insert(*var);
                variables.len() <= 128
            }
            Term::App(_, args) => {
                *functions |= !args.is_empty();
                if *functions && *nodes > 32 {
                    return false;
                }
                args.iter()
                    .all(|arg| visit(arg, nodes, variables, functions))
            }
        }
    }
    c.literals
        .iter()
        .chain(&d.literals)
        .all(|literal| match &literal.atom {
            Atom::Pred(_, args) => args
                .iter()
                .all(|arg| visit(arg, &mut nodes, &mut variables, &mut functions)),
            Atom::Eq(_, _) => false,
        })
}

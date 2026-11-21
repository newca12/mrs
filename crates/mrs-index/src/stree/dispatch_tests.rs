use super::*;
use crate::dtree::DTreeId;
use mrs_core::{SymbolTable, Term};

fn full_root_scan(tree: &STreeId<usize>, query: TermId, bank: &TermBank) -> Vec<usize> {
    let query = flatten_id(query, bank);
    let mut results = Vec::new();
    let mut bindings = Vec::new();
    for (&first, (rest, child)) in &tree.children {
        unify_walk_edge(
            first,
            rest,
            0,
            &query,
            0,
            child,
            &mut results,
            &mut bindings,
        );
    }
    results
}

#[test]
fn unification_dispatch_preserves_recall_order_and_removal() {
    let mut symbols = SymbolTable::new();
    let f = symbols.intern("f");
    let g = symbols.intern("g");
    let a = symbols.intern("a");
    let b = symbols.intern("b");
    let leaves = [
        Term::var(0),
        Term::var(7),
        Term::constant(a),
        Term::constant(b),
    ];
    let mut terms = leaves.to_vec();
    for symbol in [f, g] {
        for left in &leaves {
            terms.push(Term::app(symbol, vec![left.clone()]));
            for right in &leaves {
                terms.push(Term::app(symbol, vec![left.clone(), right.clone()]));
                terms.push(Term::app(
                    f,
                    vec![Term::app(symbol, vec![left.clone()]), right.clone()],
                ));
            }
        }
        for arity in [255, 256, 257, 512] {
            terms.push(Term::app(symbol, vec![Term::var(0); arity]));
            terms.push(Term::app(symbol, vec![Term::constant(a); arity]));
        }
    }
    let mut bank = TermBank::new();
    let ids: Vec<_> = terms.iter().map(|term| bank.from_legacy(term)).collect();
    let mut compressed = STreeId::new();
    let mut reference = DTreeId::new();
    for (value, &id) in ids.iter().enumerate() {
        compressed.insert(id, &bank, value);
        reference.insert(id, &bank, value);
    }
    for removed in [false, true] {
        if removed {
            for (value, &id) in ids.iter().enumerate().step_by(3) {
                assert!(compressed.remove(id, &bank, &value));
                assert!(reference.remove(id, &bank, &value));
            }
        }
        for (&id, term) in ids.iter().zip(&terms) {
            let actual = compressed.get_unifications(id, &bank);
            assert_eq!(
                actual,
                full_root_scan(&compressed, id, &bank),
                "full scan: {term:?}"
            );
            assert_eq!(
                actual,
                reference.get_unifications(id, &bank),
                "DTree: {term:?}"
            );
        }
    }
}

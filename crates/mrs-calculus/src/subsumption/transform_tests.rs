use super::*;
use mrs_core::{SymbolTable, term::VarId};

#[test]
fn flat_and_chained_application_keep_their_distinct_legacy_semantics() {
    let mut symbols = SymbolTable::new();
    let f = symbols.intern("f");
    let a = symbols.intern("a");
    let b = symbols.intern("b");
    let mut terms = vec![Term::var(0), Term::var(1), Term::constant(a)];
    for arity in [1, 2, 4, 5, 256, 512] {
        terms.push(Term::app(
            f,
            (0..arity).map(|v| Term::var((v % 5) as VarId)).collect(),
        ));
    }
    let mut nested = Term::var(0);
    for _ in 0..12 {
        nested = Term::app(f, vec![nested, Term::var(1), Term::constant(a)]);
        terms.push(nested.clone());
    }
    let mut bank = TermBank::new();
    let ids: Vec<_> = terms.iter().map(|t| bank.from_legacy(t)).collect();
    let bindings = [
        (0, Term::var(3)),
        (1, Term::app(f, vec![Term::var(4)])),
        (3, Term::constant(b)),
        (4, Term::var(7)),
    ];
    for mask in 0..16 {
        let mut subst = IdSubstitution::new();
        for (index, (var, term)) in bindings.iter().enumerate() {
            if mask & (1 << index) != 0 {
                let term = bank.from_legacy(term);
                subst.bind(*var, term);
            }
        }
        let legacy = subst.to_legacy(&bank);
        for (&id, term) in ids.iter().zip(&terms) {
            let flat = apply_subst_flat_id(&subst, id, &mut bank, None);
            assert_eq!(
                bank.to_legacy(flat),
                apply_subst_flat(&legacy, term),
                "flat: mask={mask}"
            );
            let chained = apply_subst_chain_id(&subst, id, &mut bank, None);
            assert_eq!(
                bank.to_legacy(chained),
                legacy.apply_term(term),
                "chained: mask={mask}"
            );
        }
        let expired = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        for &id in &ids {
            assert_eq!(apply_subst_flat_id(&subst, id, &mut bank, expired), id);
            assert_eq!(apply_subst_chain_id(&subst, id, &mut bank, expired), id);
        }
    }
}

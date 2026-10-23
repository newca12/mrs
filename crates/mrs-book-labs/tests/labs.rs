//! Lab regression tests: every chapter's claim, fast and deterministic.

use mrs_book_labs::{
    avatar_both_ok, ordering_demo_ok, pelletier_clause_count, portfolio_proves_socrates,
    prove_socrates, resolution_demo_ok, selection_comparison_ok, socrates_counts, unify_demo_ok,
};

#[test]
fn ch01_socrates_parses_and_refutes() {
    assert_eq!(socrates_counts(), (2, 1));
    assert!(prove_socrates());
}

#[test]
fn ch02_pelletier_clausifies() {
    assert!(pelletier_clause_count() > 0);
}

#[test]
fn ch03_unification_mgu() {
    assert!(unify_demo_ok());
}

#[test]
fn ch04_orderings_agree() {
    assert!(ordering_demo_ok());
}

#[test]
fn ch05_resolution_chain_refutes() {
    assert!(resolution_demo_ok());
}

#[test]
fn ch06_both_selections_refute() {
    let (age_ok, small_ok) = selection_comparison_ok();
    assert!(age_ok && small_ok);
}

#[test]
fn ch07_avatar_modes_agree() {
    let (plain_ok, avatar_ok) = avatar_both_ok();
    assert!(plain_ok && avatar_ok);
}

#[test]
fn ch08_portfolio_refutes() {
    assert!(portfolio_proves_socrates());
}

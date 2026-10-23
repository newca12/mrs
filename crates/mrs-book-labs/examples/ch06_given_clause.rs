//! Chapter 6 — Given-clause loop: `AgeWeight(5)` vs `SmallestFirst`.

use mrs_book_labs::selection_comparison_ok;

fn main() {
    let (age_ok, small_ok) = selection_comparison_ok();
    println!("ageweight_refutes={age_ok} smallestfirst_refutes={small_ok}");
    assert!(age_ok && small_ok, "both selections must refute socrates");
}

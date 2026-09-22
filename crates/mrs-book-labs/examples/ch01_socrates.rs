//! Chapter 1 — First proof: Socrates.
//!
//! Parses a TPTP problem, clausifies it with the negated conjecture, and
//! runs a single deterministic search strategy to a refutation.

use mrs_book_labs::{prove_socrates, socrates_counts};

fn main() {
    let (axioms, conjectures) = socrates_counts();
    println!("axioms={axioms} conjectures={conjectures}");
    assert_eq!((axioms, conjectures), (2, 1));

    let proved = prove_socrates();
    println!("refutation={proved}");
    assert!(proved, "socrates must refute");
    println!("% SZS status Theorem for socrates");
}

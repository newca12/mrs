//! Chapter 2 — Clausification: Pelletier 1.
//!
//! Counts the CNF clauses for `(p => q) <=> (~q => ~p)` (problems/pel1.p).

use mrs_book_labs::pelletier_clause_count;

fn main() {
    let n = pelletier_clause_count();
    println!("pel1_clauses={n}");
    assert!(n > 0, "clausification must produce clauses");
}

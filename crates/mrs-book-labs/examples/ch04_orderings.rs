//! Chapter 4 — Term orderings: KBO vs LPO on `f(a)` vs `a`.

use mrs_book_labs::ordering_demo_ok;

fn main() {
    let ok = ordering_demo_ok();
    println!("orderings_agree={ok}");
    assert!(ok, "KBO and LPO must both orient f(a) > a");
}

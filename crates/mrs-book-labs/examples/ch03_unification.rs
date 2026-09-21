//! Chapter 3 — Unification: MGU of `f(X, a)` and `f(b, Y)`.

use mrs_book_labs::unify_demo_ok;

fn main() {
    let ok = unify_demo_ok();
    println!("mgu_X_b_Y_a={ok}");
    assert!(ok, "expected X -> b, Y -> a");
}

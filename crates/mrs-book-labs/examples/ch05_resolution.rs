//! Chapter 5 — Resolution: `p`, `p => q ⊢ q`.

use mrs_book_labs::resolution_demo_ok;

fn main() {
    let ok = resolution_demo_ok();
    println!("refutation={ok}");
    assert!(ok, "unit chain [p], [~p, q], [~q] must refute");
}

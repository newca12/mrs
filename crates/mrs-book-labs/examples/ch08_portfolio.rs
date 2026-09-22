//! Chapter 8 — Strategy portfolio: Socrates through `run_schedule`.

use mrs_book_labs::portfolio_proves_socrates;

fn main() {
    let ok = portfolio_proves_socrates();
    println!("portfolio_refutation={ok}");
    assert!(ok, "portfolio must refute socrates");
}

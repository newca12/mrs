//! Chapter 7 — AVATAR: same unsatisfiable split with and without splitting.

use mrs_book_labs::avatar_both_ok;

fn main() {
    let (plain_ok, avatar_ok) = avatar_both_ok();
    println!("plain_refutes={plain_ok} avatar_refutes={avatar_ok}");
    assert!(plain_ok && avatar_ok, "both modes must refute");
}

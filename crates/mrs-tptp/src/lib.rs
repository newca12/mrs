//! # mrs-tptp — TPTP Parser for Rust
//!
//! **mrs-tptp** is a fast, zero-copy parser for the
//! [TPTP](http://www.tptp.org/) (Thousands of Problems for Theorem Provers)
//! problem library format.  It supports every major TPTP dialect:
//!
//! | Dialect | Description |
//! |---------|-------------|
//! | **CNF** | Clause Normal Form |
//! | **FOF** | First-Order Form |
//! | **TFF** | Typed First-order Form |
//! | **TCF** | Typed Clause Form |
//! | **THF** | Typed Higher-order Form |
//! | **TXF** | Extended TFF with FOOL (first-order logic on steroids) |
//! | **NXF/NHF** | Non-classical extensions |
//!
//! The parser is built with [winnow] and stores `&str` borrows directly into
//! the original input — no heap allocation per token, no copying.
//!
//! ## Quick start
//!
//! Parse a complete problem into a [`TPTPProblem`]:
//!
//! ```
//! use mrs_tptp::parse_tptp;
//!
//! let input = "
//!     fof(all_men_mortal, axiom,   ![X]: (human(X) => mortal(X))).
//!     fof(socrates_human, axiom,   human(socrates)).
//!     fof(goal,           conjecture, mortal(socrates)).
//! ";
//!
//! let problem = parse_tptp(input).expect("parse error");
//! assert_eq!(problem.axioms().count(), 2);
//! assert_eq!(problem.conjectures().count(), 1);
//! ```
//!
//! ## Streaming / large files
//!
//! Use [`TPTPIterator`] to process one formula at a time without collecting
//! them all into a [`Vec`]:
//!
//! ```
//! use mrs_tptp::{TPTPIterator, TPTPInput};
//!
//! let input = "cnf(c1, axiom, (p | q)). cnf(c2, axiom, (~p | r)).";
//! let mut axiom_count = 0;
//! for item in TPTPIterator::new(input) {
//!     let item = item.expect("parse error");
//!     if let TPTPInput::Formula(f) = item {
//!         if f.role().is_axiom() { axiom_count += 1; }
//!     }
//! }
//! assert_eq!(axiom_count, 2);
//! ```
//!
//! ## Error handling
//!
//! Both APIs return [`ParseError`], which carries a byte offset and helper
//! methods to compute line/column numbers and a source excerpt:
//!
//! ```
//! use mrs_tptp::parse_tptp;
//!
//! let bad = "fof(ok, axiom, p).\n!!!not tptp";
//! if let Err(e) = parse_tptp(bad) {
//!     eprintln!("line {}, col {}: {}", e.line(bad), e.column(bad), e);
//!     eprintln!("near: {:?}", e.snippet(bad));
//! }
//! ```
//!
//! ## Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `cancellation` | off | Cooperative parse cancellation via [`set_cancel_flag`] / [`clear_cancel_flag`] |
//! | `owned` | off | [`OwnedTPTPProblem`]: parse from a `String` or a file path, no lifetime parameter |
//!
//! Enable features in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! mrs-tptp = { version = "0.1", features = ["owned"] }
//! ```
//!
//! ## Module overview
//!
//! | Module | Contents |
//! |--------|----------|
//! | *(root)* | [`parse_tptp`], [`TPTPIterator`], [`TPTPInput`], [`ParseError`] |
//! | [`ast`] | All AST types — [`TPTPProblem`], [`AnnotatedFormula`], dialect structs, … |
//! | [`visitor`] | [`Visitor`](visitor::Visitor) trait + `walk_*` traversal helpers |
//! | [`owned`] | [`OwnedTPTPProblem`], [`parse_tptp_owned`](owned::parse_tptp_owned), [`parse_tptp_file`](owned::parse_tptp_file) *(feature `owned`)* |
//! | [`error`] | [`ParseError`] |
//!
//! [winnow]: https://docs.rs/winnow

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

pub mod ast;
pub mod error;
pub mod lexer;
#[cfg(feature = "owned")]
pub mod owned;
pub mod parser;
#[cfg(feature = "proover")]
pub mod proover;
pub mod visitor;

pub use ast::*;
pub use error::ParseError;
#[cfg(feature = "cancellation")]
pub use lexer::{clear_cancel_flag, is_cancelled, set_cancel_flag};
pub use parser::parse_tptp;
pub use parser::top::{TPTPInput, TPTPIterator};

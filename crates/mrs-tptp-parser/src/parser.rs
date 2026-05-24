//! Parser module for TPTP.
//!
//! This module provides parsers for all TPTP dialects.

pub mod cnf;
pub mod common;
pub mod fof;
pub mod tcf;
pub mod tff;
pub mod thf;
pub mod top;

pub use top::parse_tptp;

//! Proof objects and TSTP output for automated theorem proving.
//!
//! After a refutation is found, this crate:
//!
//! 1. **Extracts** the proof DAG by tracing back from the empty clause
//!    through `ClauseSource::Inference` parent pointers ([`extract`]).
//! 2. **Formats** the proof in TSTP format for verification by external
//!    tools ([`tstp`]).

pub mod extract;
pub mod tstp;

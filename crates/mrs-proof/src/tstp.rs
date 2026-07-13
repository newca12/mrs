//! TSTP format output for proofs.
//!
//! Formats proof steps in the TSTP (Thousands of Solutions from Theorem Provers)
//! format, which is the standard output format for automated theorem provers.
//!
//! Input clauses:    `cnf(c0, axiom, p(a), file('/path/to/problem.p', ax1)).`
//! Inferred clauses: `cnf(c5, plain, p(a), inference(resolution, [status(thm)], [c0, c1])).`
//! Empty clause:     `cnf(cN, plain, $false, inference(resolution, [status(thm)], [c3, c7])).`

use std::sync::OnceLock;

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::display::DisplayWithSymbols;

/// The path of the problem file currently being proved, as passed on the
/// command line (or piped via stdin, in which case it stays unset).
///
/// Set once at startup by the binary entry point via [`set_problem_path`],
/// and read by [`format_tstp`] when emitting `file(...)` leaf annotations.
/// GDV-style proof checkers resolve this path to re-open the original
/// problem file during leaf verification, so it must be the real,
/// resolvable path the prover was invoked with (e.g. the StarExec sandbox
/// path at competition time) rather than a placeholder string.
static PROBLEM_PATH: OnceLock<String> = OnceLock::new();

/// Records the path of the problem file being proved, for use in `file(...)`
/// leaf annotations emitted by [`format_tstp`].
///
/// Only the first call has any effect (matches `mrs`'s one-problem-per-process
/// model); subsequent calls are silently ignored.
pub fn set_problem_path(path: impl Into<String>) {
    let _ = PROBLEM_PATH.set(path.into());
}

/// Returns the path most recently recorded via [`set_problem_path`], or the
/// literal `"input"` placeholder if none was set (e.g. in unit tests, or
/// when the problem was read from stdin).
fn problem_path() -> &'static str {
    PROBLEM_PATH.get().map(String::as_str).unwrap_or("input")
}

/// Formats a sequence of proof steps as TSTP output.
///
/// The proof should be topologically ordered (inputs first, empty clause last).
pub fn format_tstp(proof: &[Clause], symbols: &SymbolTable) -> String {
    let mut lines = Vec::new();
    let problem_path = problem_path();

    for clause in proof {
        let id = clause.id.0;
        let literals = if clause.is_empty() {
            "$false".to_string()
        } else {
            format!("{}", clause.display(symbols))
        };

        let annotation = match &clause.source {
            ClauseSource::Input { name, role: _ } => {
                format!("file('{}', {})", problem_path, name)
            }
            ClauseSource::Inference { rule, parents } => {
                let parent_names: Vec<String> =
                    parents.iter().map(|p| format!("c{}", p.0)).collect();
                format!(
                    "inference({}, [status(thm)], [{}])",
                    rule,
                    parent_names.join(", ")
                )
            }
        };

        let role = match &clause.source {
            ClauseSource::Input { role, .. } => role.as_str(),
            ClauseSource::Inference { .. } => "plain",
        };

        lines.push(format!(
            "cnf(c{}, {}, {}, {}).",
            id, role, literals, annotation
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource};
    use mrs_core::{Atom, Literal, Term};

    #[test]
    fn format_input_clause_path_annotation() {
        // PROBLEM_PATH is a process-global OnceLock (matches mrs's
        // one-problem-per-process model), so both the "unset" and "set"
        // behaviours are asserted within a single test, in order, rather
        // than as separate tests that could race against each other via
        // the shared global if run concurrently in the same test binary.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let make_clause = || {
            Clause::new(
                ClauseId(0),
                vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
                ClauseSource::Input {
                    name: "ax1".into(),
                    role: "axiom".into(),
                },
            )
        };

        // Before set_problem_path is ever called, the placeholder is "input".
        let output = format_tstp(&[make_clause()], &syms);
        assert!(output.contains("cnf(c0, axiom, p(a), file('input', ax1))."));

        // After set_problem_path, the real path is used instead.
        set_problem_path("/starexec/sandbox/problems/SEU140+2.p");
        let output = format_tstp(&[make_clause()], &syms);
        assert!(
            output.contains(
                "cnf(c0, axiom, p(a), file('/starexec/sandbox/problems/SEU140+2.p', ax1))."
            )
        );
    }

    #[test]
    fn format_inferred_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let c = Clause::new(
            ClauseId(5),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(0), ClauseId(1)].into(),
            },
        );

        let output = format_tstp(&[c], &syms);
        assert!(output.contains("inference(resolution, [status(thm)], [c0, c1])"));
        assert!(output.contains("cnf(c5, plain,"));
    }

    #[test]
    fn format_empty_clause() {
        let syms = SymbolTable::new();

        let c = Clause::new(
            ClauseId(10),
            vec![],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(3), ClauseId(7)].into(),
            },
        );

        let output = format_tstp(&[c], &syms);
        assert!(output.contains("$false"));
        assert!(output.contains("inference(resolution, [status(thm)], [c3, c7])"));
    }
}

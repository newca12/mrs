//! TSTP format output for proofs.
//!
//! Formats proof steps in the TSTP (Thousands of Solutions from Theorem Provers)
//! format, which is the standard output format for automated theorem provers.
//!
//! Input clauses:    `cnf(c0, axiom, p(a), file('input', ax1)).`
//! Inferred clauses: `cnf(c5, plain, p(a), inference(resolution, [status(thm)], [c0, c1])).`
//! Empty clause:     `cnf(cN, plain, $false, inference(resolution, [status(thm)], [c3, c7])).`

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::display::DisplayWithSymbols;

/// Formats a sequence of proof steps as TSTP output.
///
/// The proof should be topologically ordered (inputs first, empty clause last).
pub fn format_tstp(proof: &[Clause], symbols: &SymbolTable) -> String {
    let mut lines = Vec::new();

    for clause in proof {
        let id = clause.id.0;
        let literals = if clause.is_empty() {
            "$false".to_string()
        } else {
            format!("{}", clause.display(symbols))
        };

        let annotation = match &clause.source {
            ClauseSource::Input { name, role: _ } => {
                format!("file('input', {})", name)
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
    fn format_input_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c = Clause::new(
            ClauseId(0),
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );

        let output = format_tstp(&[c], &syms);
        assert!(output.contains("cnf(c0, axiom, p(a), file('input', ax1))."));
    }

    #[test]
    fn format_inferred_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let c = Clause::new(
            ClauseId(5),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "resolution".into(),
                parents: vec![ClauseId(0), ClauseId(1)],
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
                rule: "resolution".into(),
                parents: vec![ClauseId(3), ClauseId(7)],
            },
        );

        let output = format_tstp(&[c], &syms);
        assert!(output.contains("$false"));
        assert!(output.contains("inference(resolution, [status(thm)], [c3, c7])"));
    }
}

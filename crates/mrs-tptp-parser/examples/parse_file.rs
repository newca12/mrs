//! Example: Parse a TPTP file and display its contents.
//!
//! Run with: cargo run --example parse_file

use mrs_tptp::{AnnotatedFormula, FormulaRole, parse_tptp};

fn main() {
    // Example TPTP content representing a classic logic problem
    let tptp_content = r#"
% Socrates syllogism example
% This demonstrates basic FOF (First-Order Form) parsing

% Axiom: Socrates is human
fof(human_socrates, axiom, human(socrates)).

% Axiom: All humans are mortal
fof(mortality_axiom, axiom, 
    ![X]: (human(X) => mortal(X))).

% Conjecture: Socrates is mortal
fof(socrates_mortal, conjecture, mortal(socrates)).

% CNF example (clausified form)
cnf(human_clause, axiom, human(socrates)).
cnf(mortality_clause, axiom, ~human(X) | mortal(X)).

% TFF example (typed first-order)
tff(human_type, type, human: $i > $o).
tff(mortal_type, type, mortal: $i > $o).
tff(socrates_const, type, socrates: $i).

tff(typed_axiom, axiom, 
    ![X: $i]: (human(X) => mortal(X))).
"#;

    println!("=== mrs-tptp Parser Example ===\n");

    match parse_tptp(tptp_content) {
        Ok(problem) => {
            println!("✓ Successfully parsed TPTP content\n");

            // Display includes (if any)
            if !problem.includes.is_empty() {
                println!("Includes:");
                for include in &problem.includes {
                    print!("  - {}", include.file_name);
                    if let Some(selection) = &include.selection {
                        let names: Vec<String> = selection.iter().map(|n| n.to_string()).collect();
                        print!(" [{}]", names.join(", "));
                    }
                    println!();
                }
                println!();
            }

            // Display formulas by dialect
            println!("Formulas ({} total):", problem.formulas.len());
            println!("{:-<60}", "");

            for formula in &problem.formulas {
                let (dialect, name, role) = match formula {
                    AnnotatedFormula::FOF(f) => ("FOF", f.name, f.role),
                    AnnotatedFormula::CNF(f) => ("CNF", f.name, f.role),
                    AnnotatedFormula::TFF(f) => ("TFF", f.name, f.role),
                    AnnotatedFormula::THF(f) => ("THF", f.name, f.role),
                    AnnotatedFormula::TCF(f) => ("TCF", f.name, f.role),
                    AnnotatedFormula::TPI(f) => ("TPI", f.name, f.role),
                };

                let role_str = format_role(role);
                println!("  [{dialect}] {name:<20} ({role_str})");
            }

            // Count by role
            println!("\n{:-<60}", "");
            println!("Summary by role:");

            let axioms = problem
                .formulas
                .iter()
                .filter(|f| f.role() == FormulaRole::Axiom)
                .count();
            let conjectures = problem
                .formulas
                .iter()
                .filter(|f| f.role() == FormulaRole::Conjecture)
                .count();
            let types = problem
                .formulas
                .iter()
                .filter(|f| f.role() == FormulaRole::Type)
                .count();

            println!("  Axioms:      {}", axioms);
            println!("  Conjectures: {}", conjectures);
            println!("  Types:       {}", types);
        }
        Err(e) => {
            eprintln!("✗ Parse error: {}", e);
            std::process::exit(1);
        }
    }
}

fn format_role(role: FormulaRole) -> &'static str {
    match role {
        FormulaRole::Axiom => "axiom",
        FormulaRole::AxiomLocal => "axiom-local",
        FormulaRole::Hypothesis => "hypothesis",
        FormulaRole::Definition => "definition",
        FormulaRole::Assumption => "assumption",
        FormulaRole::Lemma => "lemma",
        FormulaRole::Theorem => "theorem",
        FormulaRole::Corollary => "corollary",
        FormulaRole::Conjecture => "conjecture",
        FormulaRole::NegatedConjecture => "negated_conjecture",
        FormulaRole::Plain => "plain",
        FormulaRole::Type => "type",
        FormulaRole::Interpretation => "interpretation",
        FormulaRole::FiDomain => "fi_domain",
        FormulaRole::FiFunctors => "fi_functors",
        FormulaRole::FiPredicates => "fi_predicates",
        FormulaRole::Logic => "logic",
        FormulaRole::Unknown => "unknown",
    }
}

//! Parser tests for mrs-tptp.

use mrs_tptp::parse_tptp;

#[test]
fn test_empty_file() {
    let result = parse_tptp("");
    assert!(result.is_ok());
    let problem = result.unwrap();
    assert!(problem.formulas.is_empty());
    assert!(problem.includes.is_empty());
}

#[test]
fn test_fof_simple_axiom() {
    let input = "fof(ax1, axiom, p).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let problem = result.unwrap();
    assert_eq!(problem.formulas.len(), 1);
}

#[test]
fn test_fof_with_universal_quantifier() {
    let input = "fof(ax1, axiom, ![X]: human(X)).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_fof_implication() {
    let input = "fof(mortal, axiom, ![X]: (human(X) => mortal(X))).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_fof_conjecture() {
    let input = "fof(goal, conjecture, mortal(socrates)).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_cnf_simple_clause() {
    let input = "cnf(c1, axiom, p | ~q).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_cnf_with_variables() {
    let input = "cnf(c1, axiom, p(X) | ~q(Y, f(X))).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_tff_type_declaration() {
    let input = "tff(human_type, type, human: $i > $o).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_tff_typed_axiom() {
    let input = "tff(ax1, axiom, ![X: $i]: (human(X) => mortal(X))).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_thf_type_declaration() {
    let input = "thf(pred_type, type, p: $o).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_thf_lambda() {
    let input = "thf(def, axiom, (^[X: $i]: p(X)) = q).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_include() {
    let input = "include('axioms.ax').";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let problem = result.unwrap();
    assert_eq!(problem.includes.len(), 1);
}

#[test]
fn test_include_with_selection() {
    let input = "include('axioms.ax', [ax1, ax2]).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_multiple_formulas() {
    let input = r#"
        fof(ax1, axiom, human(socrates)).
        fof(ax2, axiom, ![X]: (human(X) => mortal(X))).
        fof(goal, conjecture, mortal(socrates)).
    "#;
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    let problem = result.unwrap();
    assert_eq!(problem.formulas.len(), 3);
}

#[test]
fn test_with_comments() {
    let input = r#"
        % This is a comment
        fof(ax1, axiom, p).
        /* Block
           comment */
        fof(ax2, axiom, q).
    "#;
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
}

#[test]
fn test_binary_connectives() {
    // Conjunction
    let result = parse_tptp("fof(t, axiom, p & q).");
    assert!(result.is_ok(), "Conjunction failed: {:?}", result.err());

    // Disjunction
    let result = parse_tptp("fof(t, axiom, p | q).");
    assert!(result.is_ok(), "Disjunction failed: {:?}", result.err());

    // Equivalence
    let result = parse_tptp("fof(t, axiom, p <=> q).");
    assert!(result.is_ok(), "Equivalence failed: {:?}", result.err());

    // Implication
    let result = parse_tptp("fof(t, axiom, p => q).");
    assert!(result.is_ok(), "Implication failed: {:?}", result.err());

    // Reverse implication
    let result = parse_tptp("fof(t, axiom, p <= q).");
    assert!(
        result.is_ok(),
        "Reverse implication failed: {:?}",
        result.err()
    );
}

#[test]
fn test_negation() {
    let result = parse_tptp("fof(t, axiom, ~p).");
    assert!(result.is_ok(), "Negation failed: {:?}", result.err());

    let result = parse_tptp("fof(t, axiom, ~~p).");
    assert!(result.is_ok(), "Double negation failed: {:?}", result.err());
}

#[test]
fn test_equality() {
    let result = parse_tptp("fof(t, axiom, a = b).");
    assert!(result.is_ok(), "Equality failed: {:?}", result.err());

    let result = parse_tptp("fof(t, axiom, a != b).");
    assert!(result.is_ok(), "Inequality failed: {:?}", result.err());
}

#[test]
fn test_nested_quantifiers() {
    let result = parse_tptp("fof(t, axiom, ![X]: ?[Y]: r(X, Y)).");
    assert!(
        result.is_ok(),
        "Nested quantifiers failed: {:?}",
        result.err()
    );
}

#[test]
fn test_function_application() {
    let result = parse_tptp("fof(t, axiom, p(f(a, b), g(c))).");
    assert!(
        result.is_ok(),
        "Function application failed: {:?}",
        result.err()
    );
}

#[test]
fn test_complex_formula() {
    let input = r#"
        fof(complex, axiom,
            ![X, Y]: (
                (r(X, Y) & s(Y)) => ?[Z]: (t(X, Z) | ~u(Z))
            )
        ).
    "#;
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Complex formula failed: {:?}", result.err());
}

#[test]
fn test_defined_constants() {
    let result = parse_tptp("fof(t, axiom, $true).");
    assert!(result.is_ok(), "$true failed: {:?}", result.err());

    let result = parse_tptp("fof(t, axiom, $false).");
    assert!(result.is_ok(), "$false failed: {:?}", result.err());
}

#[test]
fn test_distinct_object() {
    let result = parse_tptp(r#"fof(t, axiom, p("distinct_object"))."#);
    assert!(result.is_ok(), "Distinct object failed: {:?}", result.err());
}

#[test]
fn test_numbers() {
    // Integer
    let result = parse_tptp("fof(t, axiom, p(42)).");
    assert!(result.is_ok(), "Integer failed: {:?}", result.err());

    // Negative integer
    let result = parse_tptp("fof(t, axiom, p(-42)).");
    assert!(
        result.is_ok(),
        "Negative integer failed: {:?}",
        result.err()
    );

    // Rational
    let result = parse_tptp("fof(t, axiom, p(3/4)).");
    assert!(result.is_ok(), "Rational failed: {:?}", result.err());

    // Real
    let result = parse_tptp("fof(t, axiom, p(3.14)).");
    assert!(result.is_ok(), "Real failed: {:?}", result.err());
}

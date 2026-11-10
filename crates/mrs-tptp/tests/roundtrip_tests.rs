//! Display/reparse fidelity and TPTP 10 syntax coverage.

use mrs_tptp::{
    AnnotatedFormula, FormulaRole, Name, TFFAtomicFormula, TFFFormula, TFFStatement, parse_tptp,
};

/// Parse `input`, then check that every formula and include displays to text
/// that reparses to the same AST.
fn assert_roundtrip(input: &str) {
    let problem = parse_tptp(input).expect("source should parse");

    for formula in &problem.formulas {
        let display = formula.to_string();
        let reparsed = parse_tptp(&display).expect("displayed formula should parse");
        assert_eq!(reparsed.formulas.len(), 1, "display: {display}");
        assert!(reparsed.includes.is_empty(), "display: {display}");
        assert_eq!(
            &reparsed.formulas[0], formula,
            "Display changed the formula AST:\n{display}"
        );
    }

    for include in &problem.includes {
        let display = include.to_string();
        let reparsed = parse_tptp(&display).expect("displayed include should parse");
        assert_eq!(reparsed.includes.len(), 1, "display: {display}");
        assert!(reparsed.formulas.is_empty(), "display: {display}");
        assert_eq!(
            &reparsed.includes[0], include,
            "Display changed the include AST:\n{display}"
        );
    }
}

fn role_of(input: &str) -> FormulaRole {
    parse_tptp(input).expect("source should parse").formulas[0].role()
}

#[test]
fn tptp_10_syntax_round_trips() {
    let input = r#"
        include(`AXIOMS, *, `Space).
        include(axioms, [1, `Selected], space).

        fof(+1, axiom-foo(source('tag')), p(`Const, "object\"name", 'quoted\'name')).
        fof(`UpperName, axiom, q(`Const)).
        cnf(c1, axiom, p | ~q).
        cnf(c2, axiom, p(`Const)).
        cnf(c3, axiom, $true_x).
        tff(nat_type, type-datatype, nat: $tType).
        tff(s_type, type-datatype_constructor, s: nat > nat).
        tff(`UpperType, type, `Natural: $tType).
        tff(stream_type, type-codatatype, stream: $tType).
        tff(stream_cons_type, type-codatatype_constructor, stream_cons: $i > stream).
        tff(object_type, type, "object": $i).
        tff(boolean_function_type, type, h: $o > $i).
        tff(boolean_term, axiom, h(~ $false) = h($false)).
        tff(conditional_term, axiom, h(~ $ite(p, q, r)) = h($false)).
        tff(nested_conditional_equality, axiom, $ite(p, q, $ite(r, s = t, u = v))).
        tff(modal_logic, logic, $modal == [$constants == [`Upper]]).
        tff(modal_long, axiom, {#box} @ (p & q)).
        tff(modal_short, axiom, [.] (p & q)).
        tff(modal_parens, axiom, {#box} @ ((p & q))).
        tff(backquoted_atom, axiom, p(`Const)).
        tcf(object_type, type, "object": $i).
        tcf(arithmetic, axiom, $less(1,2) | ~$is_int(1/2)).
        tcf(system, axiom, $$unknown(a) | $true_x).
        thf(object_type, type, "object": $i).
        thf(modal_long, axiom, {#box} @ (p & q)).
        thf(modal_short, axiom, [.] (p & q)).
        thf(modal_parens, axiom, {#box} @ ((p & q))).
        thf(modal_application, axiom, {#necessary} @ (f @ x)).
        thf(application, axiom, p @ `Const).
        thf(dollar_true_word, axiom, $true_x).
        tpi(instruction, axiom, p).
    "#;

    assert_roundtrip(input);
}

#[test]
fn subroles_refine_known_roles_and_unknown_roles_are_rejected() {
    assert_eq!(role_of("fof(a, axiom-local, p)."), FormulaRole::AxiomLocal);
    assert_eq!(
        role_of("fof(a, axiom /* before */ - /* after */ local /* tail */, p)."),
        FormulaRole::AxiomLocal
    );
    assert_eq!(
        role_of("fof(a, axiom-foo(source('x')), p)."),
        FormulaRole::Axiom
    );
    assert_eq!(
        role_of("tff(a, type-datatype, nat: $tType)."),
        FormulaRole::TypeDatatype
    );
    assert_eq!(
        role_of("tff(a, type - codatatype_constructor, s: $i)."),
        FormulaRole::TypeCodatatypeConstructor
    );
    assert!(role_of("tff(a, type-whatever, nat: $tType).").is_type_declaration());
    assert_eq!(
        role_of("tff(a, type-datatype /* declaration */, nat: $tType)."),
        FormulaRole::TypeDatatype
    );

    assert!(parse_tptp("fof(a, axioms, p).").is_err());
    assert!(parse_tptp("fof(a, user-role, p).").is_err());
}

#[test]
fn block_comments_do_not_nest() {
    let problem = parse_tptp("/* outer /* inner */ fof(x, axiom, p).").unwrap();
    assert_eq!(problem.formulas.len(), 1);
    assert!(parse_tptp("/* open fof(x, axiom, p).").is_err());
}

#[test]
fn block_comments_may_contain_multibyte_text() {
    let problem = parse_tptp("/* Ünïcode — author: José */ fof(x, axiom, p).").unwrap();
    assert_eq!(problem.formulas.len(), 1);
}

#[test]
fn quoted_tokens_are_permissive_and_round_trip() {
    assert_roundtrip("fof(a, axiom, p('')).");
    assert_roundtrip(r"fof(a, axiom, p('bad\q')).");
    assert_roundtrip("fof(a, axiom, p('\\é')).");
    assert_roundtrip("fof(a, axiom, p(\"\")).");
}

#[test]
fn dollar_true_is_a_whole_word_keyword() {
    let problem = parse_tptp("tff(a, axiom, $true).").unwrap();
    assert!(matches!(
        &problem.formulas[0],
        AnnotatedFormula::TFF(f) if matches!(
            f.formula,
            TFFStatement::Logical(TFFFormula::Atomic(TFFAtomicFormula::True))
        )
    ));
}

#[test]
fn conditional_terms_are_parsed_as_conditionals() {
    let problem = parse_tptp("tff(a, axiom, h(~ $ite(p, q, r)) = h($false)).").unwrap();
    let debug = format!("{:?}", problem.formulas[0]);
    assert!(debug.contains("Conditional"));
    assert!(!debug.contains("DefinedWord(\"ite\")"));
}

#[test]
fn includes_accept_tptp_10_selections() {
    let problem = parse_tptp("include(axioms, *, space). include('x.ax', [a, `B]).").unwrap();
    assert_eq!(problem.includes[0].selection, None);
    assert_eq!(problem.includes[0].space_name, Some(Name::Lower("space")));
    assert_eq!(
        problem.includes[1].selection,
        Some(vec![Name::Lower("a"), Name::BackQuoted("B")])
    );
    assert!(parse_tptp("include('x', []).").is_err());
}

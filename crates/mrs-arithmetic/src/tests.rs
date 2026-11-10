use super::*;
use mrs_tptp::{Number, parse_tptp};

fn value(text: &str) -> Value {
    let number = if text.contains('/') {
        Number::Rational(text)
    } else {
        Number::Integer(text)
    };
    Evaluator::default().number(&number).unwrap()
}

fn apply(name: &str, operands: &[&str]) -> String {
    let values = operands.iter().map(|text| value(text)).collect::<Vec<_>>();
    Evaluator::default()
        .function(name, &values)
        .unwrap()
        .to_string()
}

fn eval(input: &str) -> Result<bool, Error> {
    let problem = parse_tptp(input).unwrap();
    Evaluator::default().evaluate(&problem.formulas[0])
}

#[test]
fn arbitrary_precision_and_rational_normalization() {
    assert_eq!(
        apply("sum", &["340282366920938463463374607431768211456", "1"]),
        "340282366920938463463374607431768211457"
    );
    assert_eq!(
        apply("product", &["18446744073709551616", "18446744073709551616"]),
        "340282366920938463463374607431768211456"
    );
    assert_eq!(value("+42").to_string(), "42");
    assert_eq!(value("-18/24").to_string(), "-3/4");
    assert_eq!(value("0/3").to_string(), "0/1");
    assert_eq!(apply("sum", &["1/3", "1/6"]), "1/2");
    assert_eq!(apply("difference", &["1/3", "1/6"]), "1/6");
    assert_eq!(apply("product", &["-2/3", "3/4"]), "-1/2");
    assert_eq!(apply("uminus", &["-5"]), "5");
    assert_eq!(apply("abs", &["-5/2"]), "5/2");
    assert_eq!(apply("quotient", &["3/2", "-3/4"]), "-2/1");
    assert_eq!(apply("quotient", &["3", "2"]), "3/2");
}

#[test]
fn integer_and_rational_sorts_are_not_coerced_implicitly() {
    let args = [value("1"), value("1/1")];
    assert_eq!(
        Evaluator::default().function("sum", &args),
        Err(Error::TypeMismatch)
    );
    assert_eq!(
        Evaluator::default().predicate("=", &args),
        Err(Error::TypeMismatch)
    );
    assert_eq!(apply("sum", &["1/1", "1/1"]), "2/1");
    assert_eq!(apply("to_rat", &["2"]), "2/1");
    assert_eq!(apply("to_int", &["-7/3"]), "-3");
    assert_eq!(
        eval("tff(g,conjecture,$greatereq($to_int($sum($uminus(7/15),4/15)),0))."),
        Ok(false)
    );
}

#[test]
fn rounding_preserves_sort_and_ties_go_to_even() {
    for (arg, floor, ceiling, truncate, round) in [
        ("7/3", "2/1", "3/1", "2/1", "2/1"),
        ("-7/3", "-3/1", "-2/1", "-2/1", "-2/1"),
        ("5/2", "2/1", "3/1", "2/1", "2/1"),
        ("7/2", "3/1", "4/1", "3/1", "4/1"),
        ("-5/2", "-3/1", "-2/1", "-2/1", "-2/1"),
        ("-7/2", "-4/1", "-3/1", "-3/1", "-4/1"),
    ] {
        for (name, expected) in [
            ("floor", floor),
            ("ceiling", ceiling),
            ("truncate", truncate),
            ("round", round),
        ] {
            assert_eq!(apply(name, &[arg]), expected, "{name}({arg})");
            assert_eq!(apply(name, &["-3"]), "-3");
        }
    }
}

#[test]
fn quotient_and_remainder_signs_match_integer_reference() {
    for n in -15i64..=15 {
        for d in -7i64..=7 {
            if d == 0 {
                continue;
            }
            for (suffix, quotient) in [
                ("e", n.div_euclid(d)),
                ("t", n / d),
                ("f", num_integer::Integer::div_floor(&n, &d)),
            ] {
                let ns = n.to_string();
                let ds = d.to_string();
                assert_eq!(
                    apply(&format!("quotient_{suffix}"), &[&ns, &ds]),
                    quotient.to_string()
                );
                assert_eq!(
                    apply(&format!("remainder_{suffix}"), &[&ns, &ds]),
                    (n - quotient * d).to_string()
                );
            }
        }
    }
}

#[test]
fn rational_integral_division_preserves_rational_sort() {
    for n in [-13i64, -1, 0, 1, 13] {
        for d in [-5i64, -1, 1, 5] {
            for suffix in ["e", "t", "f"] {
                let numerator = n * 7;
                let denominator = d * 5;
                let q = match suffix {
                    "e" if d < 0 => num_integer::Integer::div_ceil(&numerator, &denominator),
                    "e" | "f" => num_integer::Integer::div_floor(&numerator, &denominator),
                    _ => numerator / denominator,
                };
                let a = value(&format!("{n}/5"));
                let b = value(&format!("{d}/7"));
                let mut evaluator = Evaluator::default();
                let quotient = evaluator
                    .function(&format!("quotient_{suffix}"), &[a.clone(), b.clone()])
                    .unwrap();
                let remainder = evaluator
                    .function(&format!("remainder_{suffix}"), &[a.clone(), b.clone()])
                    .unwrap();
                assert_eq!(quotient.to_string(), format!("{q}/1"));
                assert_eq!(quotient.ratio() * b.ratio() + remainder.ratio(), a.ratio());
                if suffix == "e" {
                    assert!(remainder.ratio() >= BigRational::zero());
                }
            }
        }
    }
}

#[test]
fn predicates_and_recognizers_use_exact_values() {
    let mut evaluator = Evaluator::default();
    for (name, expected) in [
        ("less", true),
        ("lesseq", true),
        ("greater", false),
        ("greatereq", false),
        ("=", false),
    ] {
        assert_eq!(
            evaluator.predicate(name, &[value("-2/3"), value("-1/2")]),
            Ok(expected)
        );
    }
    assert_eq!(evaluator.predicate("is_int", &[value("6/3")]), Ok(true));
    assert_eq!(evaluator.predicate("is_int", &[value("1/3")]), Ok(false));
    assert_eq!(evaluator.predicate("is_rat", &[value("7")]), Ok(true));
    assert_eq!(
        evaluator.predicate("distinct", &[value("1/3"), value("2/6")]),
        Ok(false)
    );
    assert_eq!(
        evaluator.predicate("distinct", &[value("1"), value("2"), value("3")]),
        Ok(true)
    );
    assert_eq!(
        evaluator.predicate("distinct", &[value("1"), value("1"), value("1/1")]),
        Err(Error::TypeMismatch)
    );
}

#[test]
fn unspecified_malformed_and_real_operations_do_not_produce_values() {
    for text in ["1_0", "", "+", "--1", "12x"] {
        assert_eq!(
            Evaluator::default().number(&Number::Integer(text)),
            Err(Error::InvalidNumber)
        );
    }
    for name in [
        "quotient",
        "quotient_e",
        "quotient_t",
        "quotient_f",
        "remainder_e",
        "remainder_t",
        "remainder_f",
    ] {
        for args in [[value("1"), value("0")], [value("1/1"), value("0/1")]] {
            assert_eq!(
                Evaluator::default().function(name, &args),
                Err(Error::Unspecified)
            );
        }
    }
    assert_eq!(
        Evaluator::default().number(&Number::Rational("1/0")),
        Err(Error::InvalidNumber)
    );
    assert_eq!(
        Evaluator::default().function("sum", &[value("1")]),
        Err(Error::WrongArity)
    );
    assert_eq!(
        Evaluator::default().predicate("less", &[value("1")]),
        Err(Error::WrongArity)
    );
    assert_eq!(
        Evaluator::default().function("to_real", &[value("1")]),
        Err(Error::Unsupported)
    );
    let invalid = Value::Rational(BigRational::new_raw(BigInt::one(), BigInt::zero()));
    assert_eq!(
        Evaluator::default().function("floor", &[invalid]),
        Err(Error::InvalidNumber)
    );
    assert_eq!(
        eval("tff(a,axiom,$less(0.1,0.2))."),
        Err(Error::Unsupported)
    );
}

#[test]
fn ast_evaluation_and_token_kinds_are_distinct() {
    assert_eq!(eval("tff(a,axiom,$less($sum(1/3,1/6),2/3))."), Ok(true));
    assert_eq!(eval("fof(a,axiom,$less(1,1))."), Ok(false));
    assert_eq!(eval("cnf(a,axiom,~$less(1,1) | $false)."), Ok(true));
    assert_eq!(eval("tcf(a,axiom,2/4 = 1/2)."), Ok(true));
    assert_eq!(eval("tff(a,axiom,$less(a,a))."), Ok(false));
    assert_eq!(eval("tff(a,axiom,$lesseq(a,a))."), Ok(true));
    assert_eq!(eval("fof(a,axiom,'$less'(a,a))."), Err(Error::Unsupported));
    assert_eq!(eval("fof(a,axiom,'1' = 1)."), Err(Error::Unsupported));
    assert_eq!(
        eval("tff(a,axiom,![X:$int]:$less(X,2))."),
        Err(Error::Unsupported)
    );
    assert_eq!(
        eval("tff(a,axiom,$sum(1,2) = 3/1)."),
        Err(Error::TypeMismatch)
    );
    assert_eq!(eval("fof(a,axiom,$true | p)."), Err(Error::Unsupported));
}

#[test]
fn interpretation_scan_is_conservative_but_does_not_inspect_annotations() {
    for input in [
        "fof(a,axiom,p($sum(1,2))).",
        "fof(a,axiom,p(1)).",
        "fof(a,axiom,$$unknown).",
        "tff(a,axiom,![X:$int]:p(X)).",
    ] {
        assert!(
            requires_interpretation(&parse_tptp(input).unwrap().formulas[0]),
            "{input}"
        );
    }
    for input in [
        "fof(a,axiom,'$less'(a,a)).",
        "fof(a,axiom,p('1')).",
        "fof(a,axiom,p,inference(copy,[status(thm),number(1)],[a])).",
    ] {
        assert!(
            !requires_interpretation(&parse_tptp(input).unwrap().formulas[0]),
            "{input}"
        );
    }
}

#[test]
fn all_resource_budgets_fail_closed() {
    let problem = parse_tptp("tff(a,axiom,$product(31,31) = 961).").unwrap();
    for limits in [
        Limits {
            max_nodes: 1,
            ..Limits::default()
        },
        Limits {
            max_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_bits: 5,
            ..Limits::default()
        },
        Limits {
            max_work: 1,
            ..Limits::default()
        },
        Limits {
            deadline: Some(std::time::Instant::now()),
            ..Limits::default()
        },
    ] {
        assert_eq!(
            Evaluator::new(limits).evaluate(&problem.formulas[0]),
            Err(Error::ResourceLimit)
        );
    }
    let mut evaluator = Evaluator::new(Limits {
        max_bits: 8,
        ..Limits::default()
    });
    assert_eq!(
        evaluator.number(&Number::Integer("100000000000000000000000000")),
        Err(Error::ResourceLimit)
    );
    let reflexive = parse_tptp("tff(a,axiom,$less(a,a)).").unwrap();
    for limits in [
        Limits {
            max_nodes: 3,
            ..Limits::default()
        },
        Limits {
            max_depth: 1,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            Evaluator::new(limits).evaluate(&reflexive.formulas[0]),
            Err(Error::ResourceLimit)
        );
    }
}

//! Integration test for model certificate verification in mrs-proover.

use mrs_proover::model::{
    EqualitySemantics, ModelCertificate, ModelVerdict, PredicateTable, verify_model_text,
};
use std::collections::BTreeMap;

#[test]
fn verifies_satisfiable_relational_model() {
    let problem_text = r#"
        fof(a1, axiom, p(a)).
        fof(a2, axiom, q(b)).
        fof(a3, axiom, ! [X] : (p(X) => ~q(X))).
    "#;

    let mut constants = BTreeMap::new();
    constants.insert("a".to_string(), 0);
    constants.insert("b".to_string(), 1);

    let mut predicates = BTreeMap::new();
    predicates.insert(
        "p".to_string(),
        PredicateTable {
            arity: 1,
            table: vec![true, false],
        },
    );
    predicates.insert(
        "q".to_string(),
        PredicateTable {
            arity: 1,
            table: vec![false, true],
        },
    );

    let mut cert = ModelCertificate {
        domain_size: 2,
        constants,
        functions: BTreeMap::new(),
        predicates,
        equality: EqualitySemantics::StrictIdentity,
        digest: String::new(),
    };
    cert.digest = cert.compute_digest();

    let cert_json = serde_json::to_string(&cert).unwrap();
    let verdict = verify_model_text(problem_text, &cert_json, Some("Satisfiable"));
    assert!(
        matches!(verdict, ModelVerdict::Certified { domain_size: 2, .. }),
        "expected Certified, got {verdict:?}"
    );
}

#[test]
fn verifies_counter_satisfiable_model_falsifying_conjecture() {
    let problem_text = r#"
        fof(ax, axiom, p(a)).
        fof(conj, conjecture, p(b)).
    "#;

    let mut constants = BTreeMap::new();
    constants.insert("a".to_string(), 0);
    constants.insert("b".to_string(), 1);

    let mut predicates = BTreeMap::new();
    predicates.insert(
        "p".to_string(),
        PredicateTable {
            arity: 1,
            table: vec![true, false],
        },
    );

    let mut cert = ModelCertificate {
        domain_size: 2,
        constants,
        functions: BTreeMap::new(),
        predicates,
        equality: EqualitySemantics::StrictIdentity,
        digest: String::new(),
    };
    cert.digest = cert.compute_digest();

    let cert_json = serde_json::to_string(&cert).unwrap();
    let verdict = verify_model_text(problem_text, &cert_json, Some("CounterSatisfiable"));
    assert!(
        matches!(verdict, ModelVerdict::Certified { domain_size: 2, .. }),
        "expected Certified counter-model, got {verdict:?}"
    );

    // If conjecture were satisfied, CounterSatisfiable must be rejected!
    let mut bad_predicates = BTreeMap::new();
    bad_predicates.insert(
        "p".to_string(),
        PredicateTable {
            arity: 1,
            table: vec![true, true],
        },
    );
    let mut bad_cert = ModelCertificate {
        domain_size: 2,
        constants: cert.constants.clone(),
        functions: BTreeMap::new(),
        predicates: bad_predicates,
        equality: EqualitySemantics::StrictIdentity,
        digest: String::new(),
    };
    bad_cert.digest = bad_cert.compute_digest();
    let bad_json = serde_json::to_string(&bad_cert).unwrap();
    let bad_verdict = verify_model_text(problem_text, &bad_json, Some("CounterSatisfiable"));
    assert!(
        matches!(bad_verdict, ModelVerdict::Rejected(..)),
        "expected Rejected when conjecture is satisfied, got {bad_verdict:?}"
    );
}

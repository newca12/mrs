//! Model certificate verification module for `mrs-proover`.

pub use mrs_proof_kernel::model::{
    EqualitySemantics, FunctionTable, ModelCertificate, ModelVerdict, PredicateTable,
};
use mrs_tptp::parse_tptp;
use std::path::Path;

/// Verify a model certificate in JSON or TPTP format against a problem file.
pub fn verify_model_file(
    problem_path: &Path,
    model_certificate_json: &str,
    expected_status: Option<&str>,
) -> ModelVerdict {
    let problem_text = match std::fs::read_to_string(problem_path) {
        Ok(t) => t,
        Err(e) => return ModelVerdict::Inconclusive(format!("read problem file: {e}")),
    };

    let problem = match parse_tptp(&problem_text) {
        Ok(p) => p,
        Err(e) => return ModelVerdict::Inconclusive(format!("parse problem file: {e}")),
    };

    let certificate: ModelCertificate = match serde_json::from_str(model_certificate_json) {
        Ok(c) => c,
        Err(e) => {
            return ModelVerdict::Rejected(format!("deserialize model certificate: {e}"));
        }
    };

    certificate.validate(&problem, expected_status)
}

/// Verify a model certificate against in-memory problem text.
pub fn verify_model_text(
    problem_text: &str,
    model_certificate_json: &str,
    expected_status: Option<&str>,
) -> ModelVerdict {
    let problem = match parse_tptp(problem_text) {
        Ok(p) => p,
        Err(e) => return ModelVerdict::Inconclusive(format!("parse problem text: {e}")),
    };

    let certificate: ModelCertificate = match serde_json::from_str(model_certificate_json) {
        Ok(c) => c,
        Err(e) => {
            return ModelVerdict::Rejected(format!("deserialize model certificate: {e}"));
        }
    };

    certificate.validate(&problem, expected_status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_json_model_certificate() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(b)).";
        let json = r#"{
            "domain_size": 2,
            "constants": {"a": 0, "b": 1},
            "functions": {},
            "predicates": {"p": {"arity": 1, "table": [true, false]}},
            "equality": "StrictIdentity",
            "digest": ""
        }"#;

        let mut cert: ModelCertificate = serde_json::from_str(json).unwrap();
        cert.digest = cert.compute_digest();
        let cert_json = serde_json::to_string(&cert).unwrap();

        let verdict = verify_model_text(problem, &cert_json, Some("Satisfiable"));
        assert!(matches!(verdict, ModelVerdict::Certified { .. }));
    }
}

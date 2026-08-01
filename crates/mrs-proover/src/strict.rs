//! Strict proof-kernel integration.

use mrs_proof_kernel::{
    KernelVerdict, VerificationLimits, VerificationTelemetry, verify_strict_with_source,
    verify_strict_with_telemetry_and_source,
};
use mrs_tptp::parse_tptp;
use mrs_tptp::proover::proof_header_link;

use crate::load::LoadedJob;

/// Verify a loaded proof with the independent strict kernel.
pub fn verify_loaded_job(job: &LoadedJob, limits: VerificationLimits) -> KernelVerdict {
    let Some(problem) = job.problem.as_ref() else {
        return KernelVerdict::Inconclusive(
            "strict: proof has no linked problem file for provenance verification".into(),
        );
    };
    let expected_source = proof_header_link(&job.proof_text);
    verify_strict_with_source(
        problem.problem(),
        job.proof.problem(),
        expected_source,
        limits,
    )
}

/// Verify with the default strict-kernel resource limits.
pub fn verify_loaded_job_default(job: &LoadedJob) -> KernelVerdict {
    verify_loaded_job(job, VerificationLimits::default())
}

/// Verify problem and proof text without writing a temporary proof file.
pub fn verify_text(
    problem_text: &str,
    proof_text: &str,
    expected_source: Option<&str>,
    limits: VerificationLimits,
) -> KernelVerdict {
    let problem = match parse_tptp(problem_text) {
        Ok(problem) => problem,
        Err(error) => return KernelVerdict::Inconclusive(format!("strict problem parse: {error}")),
    };
    let proof = match parse_tptp(proof_text) {
        Ok(proof) => proof,
        Err(error) => return KernelVerdict::Inconclusive(format!("strict proof parse: {error}")),
    };
    verify_strict_with_source(&problem, &proof, expected_source, limits)
}

/// Verify text with the proof header's source path, without loading includes.
pub fn verify_text_default(problem_text: &str, proof_text: &str) -> KernelVerdict {
    verify_text(
        problem_text,
        proof_text,
        proof_header_link(proof_text),
        VerificationLimits::default(),
    )
}

/// Verify in-memory text and return kernel telemetry.
pub fn verify_text_with_telemetry(
    problem_text: &str,
    proof_text: &str,
    expected_source: Option<&str>,
    limits: VerificationLimits,
) -> Result<VerificationTelemetry, KernelVerdict> {
    let problem = parse_tptp(problem_text)
        .map_err(|error| KernelVerdict::Inconclusive(format!("strict problem parse: {error}")))?;
    let proof = parse_tptp(proof_text)
        .map_err(|error| KernelVerdict::Inconclusive(format!("strict proof parse: {error}")))?;
    Ok(verify_strict_with_telemetry_and_source(
        &problem,
        &proof,
        expected_source,
        limits,
    ))
}

/// Verify in-memory problem/proof text after resolving includes from `root`.
pub fn verify_text_with_include_root(
    problem_text: String,
    proof_text: String,
    root: &std::path::Path,
    limits: VerificationLimits,
) -> KernelVerdict {
    let job = match crate::load::load_text(problem_text, proof_text, root) {
        Ok(job) => job,
        Err(error) => return KernelVerdict::Inconclusive(format!("strict load: {error}")),
    };
    verify_loaded_job(&job, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_in_memory_text() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\
                     fof(bot, plain, $false, inference(resolution, [status(thm)], [a,b])).";
        assert_eq!(
            verify_text(
                problem,
                proof,
                Some("problem.p"),
                VerificationLimits::default()
            ),
            KernelVerdict::Certified
        );
    }

    #[test]
    fn parse_failure_is_inconclusive() {
        assert!(matches!(
            verify_text("not tptp", "not tptp", None, VerificationLimits::default()),
            KernelVerdict::Inconclusive(_)
        ));
    }

    #[test]
    fn telemetry_counts_nodes_and_literals() {
        let problem = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
        let proof = "fof(a, axiom, p(a), file('problem.p', a)).\
                     fof(b, axiom, ~p(a), file('problem.p', b)).\
                     cnf(bot, plain, $false, inference(resolution, [status(thm)], [a,b])).";
        let telemetry = verify_text_with_telemetry(
            problem,
            proof,
            Some("problem.p"),
            VerificationLimits::default(),
        )
        .expect("text parses");
        assert_eq!(telemetry.problem_nodes, 2);
        assert_eq!(telemetry.proof_nodes, 3);
        assert_eq!(telemetry.proof_fof_nodes, 2);
        assert_eq!(telemetry.proof_cnf_nodes, 1);
        assert_eq!(telemetry.proof_clause_literals, 1);
        assert!(matches!(telemetry.verdict, KernelVerdict::Certified));
    }

    #[test]
    fn include_root_load_failure_is_inconclusive() {
        let root = std::env::temp_dir().join("mrs_strict_missing_include_root");
        let verdict = verify_text_with_include_root(
            "%include('missing.p').".to_string(),
            "fof(bot, plain, $false, inference(consequence, [status(thm)], [])).".to_string(),
            &root,
            VerificationLimits::default(),
        );
        assert!(matches!(verdict, KernelVerdict::Inconclusive(_)));
    }
}

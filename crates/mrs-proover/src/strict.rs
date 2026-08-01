//! Strict proof-kernel integration.

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict_with_source};
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
}

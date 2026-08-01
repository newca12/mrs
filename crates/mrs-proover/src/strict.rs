//! Strict proof-kernel integration.

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict_with_source};
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

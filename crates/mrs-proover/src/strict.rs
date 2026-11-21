//! Strict proof-kernel integration.

use mrs_proof_kernel::{
    KernelVerdict, VerificationLimits, VerificationTelemetry, verify_strict_with_source,
    verify_strict_with_telemetry_and_source,
};
use mrs_tptp::parse_tptp;
use mrs_tptp::proover::proof_header_link;

use crate::load::LoadedJob;

/// Identity of the exact source strings consumed by this checker invocation.
/// Included sources are retained even after selection/merging into the AST.
pub fn input_identity(job: &LoadedJob) -> serde_json::Value {
    use sha2::{Digest, Sha256};
    let digest = |source: &str| {
        Sha256::digest(source.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    serde_json::json!({
        "schema": 1,
        "proof_sha256": digest(job.proof.source()),
        "problem_path": job.problem_path,
        "problem_sha256": job.problem.as_ref().map(|problem| digest(problem.source())),
        "include_sha256": job.problem_includes.iter().map(|problem| digest(problem.source())).collect::<Vec<_>>(),
    })
}

/// Opt-in formal-track shadow report alongside the authoritative strict kernel.
/// Unsupported bridges and checker failures remain explicit Unknown evidence.
pub fn verify_loaded_job_with_verus_shadow(
    job: &LoadedJob,
    limits: VerificationLimits,
    config: &mrs_proof_kernel::verus::VerusConfig,
) -> Result<(KernelVerdict, mrs_proof_kernel::verus::ShadowReport), KernelVerdict> {
    let problem = job.problem.as_ref().ok_or_else(|| {
        KernelVerdict::Inconclusive(
            "strict: proof has no linked problem file for provenance verification".into(),
        )
    })?;
    Ok(mrs_proof_kernel::verus::verify_with_shadow(
        problem.problem(),
        job.proof.problem(),
        proof_header_link(&job.proof_text),
        limits,
        config,
    ))
}

/// Opt-in dual-check certification: every failure on either path suppresses
/// certification. The TPTP/IR adapter remains explicitly trusted.
pub fn verify_loaded_job_with_verus_gate(
    job: &LoadedJob,
    limits: VerificationLimits,
    config: &mrs_proof_kernel::verus::VerusConfig,
) -> Result<(KernelVerdict, mrs_proof_kernel::verus::ShadowReport), KernelVerdict> {
    let problem = job.problem.as_ref().ok_or_else(|| {
        KernelVerdict::Inconclusive(
            "strict: proof has no linked problem file for provenance verification".into(),
        )
    })?;
    Ok(mrs_proof_kernel::verus::verify_with_verus_gate(
        problem.problem(),
        job.proof.problem(),
        proof_header_link(&job.proof_text),
        limits,
        config,
    ))
}

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
    fn input_identity_binds_the_actual_problem_and_proof_sources() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let problem = "cnf(p,axiom,p).";
        let proof = "cnf(p,axiom,p,file('stdin.p',p)).";
        let job = crate::load::load_text(problem.into(), proof.into(), root).unwrap();
        let changed =
            crate::load::load_text("cnf(p,axiom,~p).".into(), proof.into(), root).unwrap();
        let a = input_identity(&job);
        let b = input_identity(&changed);
        assert_eq!(a["proof_sha256"], b["proof_sha256"]);
        assert_ne!(a["problem_sha256"], b["problem_sha256"]);
        assert_eq!(a["include_sha256"], serde_json::json!([]));
    }

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
    fn expired_deadline_never_certifies_a_valid_refutation() {
        let problem = "cnf(a,axiom,p). cnf(b,axiom,~p).";
        let proof = "cnf(a,axiom,p,file('problem.p',a)). \
            cnf(b,axiom,~p,file('problem.p',b)). \
            cnf(c,plain,$false,inference(resolution,[status(thm)],[a,b])).";
        let verdict = verify_text(
            problem,
            proof,
            Some("problem.p"),
            VerificationLimits {
                deadline: Some(std::time::Instant::now()),
                ..VerificationLimits::default()
            },
        );
        assert!(
            matches!(verdict, KernelVerdict::Inconclusive(reason) if reason.contains("deadline"))
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
        assert_eq!(telemetry.steps.len(), 3);
        let leaf = telemetry
            .steps
            .iter()
            .find(|step| step.name == "a")
            .expect("a telemetry step");
        assert_eq!(leaf.rule, None);
        assert_eq!(leaf.parent_count, 0);
        assert!(leaf.formula_nodes > 0);
        let root = telemetry
            .steps
            .iter()
            .find(|step| step.name == "bot")
            .expect("bot telemetry step");
        assert_eq!(root.rule.as_deref(), Some("resolution"));
        assert_eq!(root.parent_count, 2);
        assert_eq!(root.clause_literals, 0);
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

    #[test]
    fn verifies_in_memory_text_with_include_root() {
        let root = std::env::temp_dir().join(format!(
            "mrs_strict_include_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("include root creates");
        std::fs::write(
            root.join("axioms.p"),
            "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).\n",
        )
        .expect("include writes");
        let verdict = verify_text_with_include_root(
            "include('axioms.p').\n".to_string(),
            "% Proof : input\n\
             fof(a, axiom, p(a), file('input', a)).\
             fof(b, axiom, ~p(a), file('input', b)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [a,b]))."
                .to_string(),
            &root,
            VerificationLimits::default(),
        );
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(verdict, KernelVerdict::Certified);
    }
}

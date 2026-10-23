use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
use mrs_tptp::parse_tptp;

#[test]
fn malformed_dag_inputs_fail_closed_without_panicking() {
    let cases = [
        (
            "missing_parent",
            "fof(a, axiom, p(a)).",
            "fof(a, axiom, p(a), file('problem.p', a)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [a,missing])).",
        ),
        (
            "cycle",
            "fof(a, axiom, p(a)).",
            "fof(a, plain, p(a), inference(variable_rename, [status(thm)], [b])).\
             fof(b, plain, p(a), inference(variable_rename, [status(thm)], [a])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [a])).",
        ),
        (
            "duplicate_name",
            "fof(a, axiom, p(a)).",
            "fof(a, axiom, p(a), file('problem.p', a)).\
             fof(a, axiom, p(a), file('problem.p', a)).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [a])).",
        ),
    ];
    for (name, problem_text, proof_text) in cases {
        let problem = parse_tptp(problem_text).expect("problem parses");
        let proof = parse_tptp(proof_text).expect("proof parses");
        let verdict = verify_strict(&problem, &proof, VerificationLimits::default());
        assert!(
            matches!(
                verdict,
                KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
            ),
            "malformed case `{name}` returned {verdict}"
        );
    }
}

#[test]
fn malformed_tptp_is_rejected_before_kernel_dispatch() {
    assert!(parse_tptp("fof(a, axiom, p(a)").is_err());
}

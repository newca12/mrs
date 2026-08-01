use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict_with_source};
use mrs_tptp::parse_tptp;

const PROBLEM: &str = "fof(a, axiom, p(a)).\nfof(b, axiom, ~p(a)).";
const PROOF: &str = "fof(a, axiom, p(a), file('problem.p', a)).\
                    fof(b, axiom, ~p(a), file('problem.p', b)).\
                    fof(bot, plain, $false, inference(resolution, [status(thm)], [a,b])).";

fn mutated_proofs() -> [(&'static str, String); 10] {
    [
        ("formula", PROOF.replace("~p(a), file", "~p(b), file")),
        ("literal_sign", PROOF.replace("~p(a), file", "p(a), file")),
        ("term_argument", PROOF.replace("p(a), file", "p(b), file")),
        ("parent_reference", PROOF.replace("[a,b]", "[a,a]")),
        (
            "rule",
            PROOF.replace("inference(resolution", "inference(factoring"),
        ),
        ("status", PROOF.replace("status(thm)", "status(cth)")),
        ("role", PROOF.replace("fof(b, axiom", "fof(b, conjecture")),
        (
            "provenance",
            PROOF.replace("file('problem.p', b)", "file('other.p', b)"),
        ),
        (
            "final_conclusion",
            PROOF.replace("$false, inference", "q(a), inference"),
        ),
        ("root_parent", PROOF.replace("[a,b]", "[a,missing]")),
    ]
}

#[test]
fn resolution_mutations_never_certify() {
    let problem = parse_tptp(PROBLEM).expect("problem parses");
    let baseline = parse_tptp(PROOF).expect("proof parses");
    assert_eq!(
        verify_strict_with_source(
            &problem,
            &baseline,
            Some("problem.p"),
            VerificationLimits::default(),
        ),
        KernelVerdict::Certified
    );

    for (name, text) in mutated_proofs() {
        let Ok(proof) = parse_tptp(&text) else {
            continue;
        };
        let verdict = verify_strict_with_source(
            &problem,
            &proof,
            Some("problem.p"),
            VerificationLimits::default(),
        );
        assert!(
            !matches!(verdict, KernelVerdict::Certified),
            "mutation `{name}` was certified: {verdict}"
        );
    }
}

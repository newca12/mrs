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

#[test]
fn newly_certified_rule_mutations_never_certify() {
    let cases = [
        (
            "modus_ponens",
            "fof(rule, axiom, ![X] : (p(X) => q(X))).\nfof(fact, axiom, p(a)).\nfof(nq, axiom, ~q(a)).",
            "fof(rule, axiom, ![X] : (p(X) => q(X)), file('problem.p', rule)).\
             fof(fact, axiom, p(a), file('problem.p', fact)).\
             fof(s, plain, r(a), inference(modus_ponens, [status(thm)], [rule,fact])).\
             fof(nq, axiom, ~q(a), file('problem.p', nq)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [s,nq])).",
        ),
        (
            "excluded_middle",
            "fof(a, axiom, p(a)).\nfof(n, axiom, ~p(a)).",
            "fof(a, axiom, p(a), file('problem.p', a)).\
             fof(e, plain, (q(a) | ~q(a)), inference(excluded_middle, [status(thm)], [a])).\
             fof(n, axiom, ~p(a), file('problem.p', n)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [a,n])).",
        ),
        (
            "consequence",
            "fof(p, axiom, p(a)).\nfof(q, axiom, q(a)).",
            "fof(p, axiom, p(a), file('problem.p', p)).\
             fof(q, axiom, q(a), file('problem.p', q)).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [p,q])).",
        ),
        (
            "reflexivity",
            "fof(src, axiom, p(a)).\nfof(n, axiom, ~p(a)).",
            "fof(src, axiom, p(a), file('problem.p', src)).\
             fof(eq, plain, a = b, inference(reflexivity, [status(thm)], [src])).\
             fof(pair, plain, (a = b & p(a)), inference(conjunction, [status(thm)], [eq,src])).\
             fof(selected, plain, p(a), inference(split_conjunct, [status(thm)], [pair])).\
             fof(n, axiom, ~p(a), file('problem.p', n)).\
             fof(bot, plain, $false, inference(resolution, [status(thm)], [selected,n])).",
        ),
        (
            "paramodulation",
            "fof(eq, axiom, f(a) = b).\nfof(target, axiom, p(f(a))).",
            "fof(eq, axiom, f(a) = b, file('problem.p', eq)).\
             fof(target, axiom, p(f(a)), file('problem.p', target)).\
             fof(s, plain, q(b), inference(paramodulation, [status(thm)], [eq,target])).\
             fof(bot, plain, $false, inference(consequence, [status(thm)], [s,s])).",
        ),
    ];

    for (name, problem_text, proof_text) in cases {
        let problem = parse_tptp(problem_text).expect("mutation problem parses");
        let proof = parse_tptp(proof_text).expect("mutation proof parses");
        let verdict = verify_strict_with_source(
            &problem,
            &proof,
            Some("problem.p"),
            VerificationLimits::default(),
        );
        assert!(
            !matches!(verdict, KernelVerdict::Certified),
            "new-rule mutation `{name}` was certified: {verdict}"
        );
    }
}

use mrs_proover::atp::NoopAtp;
use mrs_proover::verdict::Verdict;

#[test]
fn competition_mode_cannot_bypass_proof_wide_avatar_checks() {
    let problem = include_str!("../../mrs-proof-kernel/tests/resources/avatar/satisfiable.p");
    for proof in [
        include_str!("../../mrs-proof-kernel/tests/resources/avatar/undischarged.s"),
        include_str!("../../mrs-proof-kernel/tests/resources/avatar/shared-variable.s"),
        include_str!("../../mrs-proof-kernel/tests/resources/avatar/inconsistent-names.s"),
        include_str!("../../mrs-proof-kernel/tests/resources/avatar/uncited-original.s"),
    ] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let job = mrs_proover::load::load_text(problem.into(), proof.into(), root).unwrap();
        let result = mrs_proover::verify::verify_with(&job, &Default::default(), &NoopAtp);
        assert!(matches!(result, Verdict::VerifiedBad(_)), "{result:?}");
    }
}

#[test]
fn competition_mode_also_rejects_invented_axioms_from_conjectures_or_pedigrees() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (problem, proof) in [
        (
            include_str!("../../mrs-proof-kernel/tests/resources/avatar/satisfiable.p"),
            include_str!("../../mrs-proof-kernel/tests/resources/avatar/negated-axiom.s"),
        ),
        (
            include_str!("../../mrs-proof-kernel/tests/resources/avatar/false-conjecture.p"),
            include_str!("../../mrs-proof-kernel/tests/resources/avatar/asserted-conjecture.s"),
        ),
    ] {
        let job = mrs_proover::load::load_text(problem.into(), proof.into(), root).unwrap();
        assert!(matches!(
            mrs_proover::verify::verify_with(&job, &Default::default(), &NoopAtp),
            Verdict::VerifiedBad(_)
        ));
    }
}

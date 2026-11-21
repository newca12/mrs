use super::*;

const SAT: &str = include_str!("../tests/resources/avatar/satisfiable.p");
const BRANCH: &str = include_str!("../tests/resources/avatar/undischarged.s");
const SHARED: &str = include_str!("../tests/resources/avatar/shared-variable.s");
const REUSED: &str = include_str!("../tests/resources/avatar/inconsistent-names.s");

fn verdict(problem: &str, proof: &str) -> KernelVerdict {
    verify_strict(
        &mrs_tptp::parse_tptp(problem).unwrap(),
        &mrs_tptp::parse_tptp(proof).unwrap(),
        VerificationLimits::default(),
    )
}

#[test]
fn rejects_undischarged_avatar_root_and_ex_falso_alias() {
    for proof in [
        BRANCH.to_owned(),
        format!("{BRANCH}\ncnf(alias,plain,$false,inference(ex_falso,[status(thm)],[branch_p]))."),
    ] {
        assert!(matches!(verdict(SAT, &proof), KernelVerdict::Rejected(_)));
    }
}

#[test]
fn conjectures_and_pedigree_negation_cannot_invent_axioms() {
    let proof = include_str!("../tests/resources/avatar/negated-axiom.s");
    assert!(matches!(verdict(SAT, proof), KernelVerdict::Rejected(_)));
    let problem = include_str!("../tests/resources/avatar/false-conjecture.p");
    let proof = include_str!("../tests/resources/avatar/asserted-conjecture.s");
    assert!(matches!(
        verdict(problem, proof),
        KernelVerdict::Rejected(_)
    ));
    let disguised = proof
        .replace(
            "cnf(bot,",
            "fof(copy,plain,p,inference(assume_negation,[status(cth)],[goal])).\ncnf(bot,",
        )
        .replace("[np,goal]", "[np,copy]");
    assert!(matches!(
        verdict(problem, &disguised),
        KernelVerdict::Rejected(_)
    ));
    let valid_problem = "cnf(p,axiom,p). fof(goal,conjecture,p).";
    let valid = "cnf(p,axiom,p,file('problem.p',p)). fof(goal,conjecture,p,file('problem.p',goal)).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[p,inference(assume_negation,[status(cth)],[goal])])).";
    assert_eq!(verdict(valid_problem, valid), KernelVerdict::Certified);
}

#[test]
fn unsupported_goal_roles_and_multiple_conjectures_fail_closed() {
    let proof = "cnf(p,axiom,p,file('problem.p',p)). cnf(np,axiom,~p,file('problem.p',np)). cnf(bot,plain,$false,inference(resolution,[status(thm)],[p,np])).";
    for problem in [
        "cnf(p,axiom,p). cnf(np,axiom,~p). fof(q,interpretation,q).",
        "cnf(p,axiom,p). cnf(np,axiom,~p). fof(q,conjecture,q). fof(r,conjecture,r).",
    ] {
        assert!(matches!(
            verdict(problem, proof),
            KernelVerdict::Inconclusive(_)
        ));
    }
}

#[test]
fn rejects_shared_variables_and_inconsistent_global_component_names() {
    for proof in [SHARED, REUSED] {
        assert!(matches!(verdict(SAT, proof), KernelVerdict::Rejected(_)));
    }
}

#[test]
fn rejects_uncited_lrat_original_even_with_valid_digest() {
    assert!(matches!(
        verdict(
            SAT,
            include_str!("../tests/resources/avatar/uncited-original.s")
        ),
        KernelVerdict::Rejected(_)
    ));
    let clauses = vec![vec![1, 2], vec![-1], vec![]];
    let ids = [1, 2, 3];
    let cited = [0, 1];
    let trace = b"4 0 3 0\n";
    let digest = avatar_sat_trace_digest("lrat", 2, &ids, &cited, &clauses, trace);
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let proof = format!(
        "{BRANCH}\ncnf(bot,plain,$false,inference(avatar_sat_refutation,[status(thm),avatar_sat_refutation([split],[branch_p],sat_trace('lrat',2,'{}',[1,2,3],[0,1],[[1,2],[-1],[]],'{}'))],[split,branch_p])).",
        hex(&digest),
        hex(trace)
    );
    let result = verdict(SAT, &proof);
    assert!(
        matches!(&result, KernelVerdict::Rejected(reason) if reason.contains("unjustified original")),
        "{result:?}"
    );
}

#[test]
fn accepts_complete_discharge_and_rejects_missing_branch() {
    let problem = "cnf(top,axiom,p|q). cnf(np,axiom,~p). cnf(nq,axiom,~q).";
    let proof = "cnf(top,axiom,p|q,file('problem.p',top)). cnf(np,axiom,~p,file('problem.p',np)). cnf(nq,axiom,~q,file('problem.p',nq)).
cnf(split,plain,spl0_1|spl0_2,inference(avatar_split_clause,[status(esa),avatar_split([branch(0,spl0_1,[0]),branch(1,spl0_2,[1])],[])],[top])).
cnf(cp,plain,p|~spl0_1,inference(avatar_component_clause,[status(esa),avatar_component(split,0,spl0_1)],[split])).
cnf(cq,plain,q|~spl0_2,inference(avatar_component_clause,[status(esa),avatar_component(split,1,spl0_2)],[split])).
cnf(ep,plain,~spl0_1,inference(resolution,[status(thm)],[cp,np])).
cnf(eq,plain,~spl0_2,inference(resolution,[status(thm)],[cq,nq])).
cnf(bp,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_1])],[ep])).
cnf(bq,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_2])],[eq])).
cnf(bot,plain,$false,inference(avatar_sat_refutation,[status(thm),avatar_sat_refutation([split],[bp,bq])],[split,bp,bq])).";
    assert_eq!(verdict(problem, proof), KernelVerdict::Certified);
    assert!(matches!(
        verdict(
            problem,
            &proof
                .replace("[bp,bq]", "[bp]")
                .replace("[split,bp,bq]", "[split,bp]")
        ),
        KernelVerdict::Rejected(_)
    ));
    // Sparse SAT names must not cause a billion-variable CaDiCaL allocation.
    assert_eq!(
        verdict(problem, &proof.replace("spl0_2", "spl0_1000000000")),
        KernelVerdict::Certified
    );
}

#[test]
fn legacy_case_split_also_requires_variable_independence() {
    let proof =
        "cnf(np,axiom,~p(a),file('satisfiable.p',np)). cnf(pb,axiom,p(b),file('satisfiable.p',pb)).
cnf(top,plain,p(X)|~p(X),inference(tautology,[status(thm)],[])).
cnf(cp,plain,p(X),inference(split_component,[status(esa)],[top])).
cnf(cq,plain,~p(X),inference(split_component,[status(esa)],[top])).
cnf(bp,plain,$false,inference(resolution,[status(thm)],[cp,np])).
cnf(bq,plain,$false,inference(resolution,[status(thm)],[cq,pb])).
cnf(bot,plain,$false,inference(avatar_sat_refutation,[status(thm)],[top,bp,bq])).";
    assert!(matches!(verdict(SAT, proof), KernelVerdict::Rejected(_)));
}

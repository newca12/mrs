% Proof : tests/resources/cwa_fixture_problem.p
fof(top, axiom, p | q, file('tests/resources/cwa_fixture_problem.p', top)).
fof(not_p, axiom, ~p, file('tests/resources/cwa_fixture_problem.p', not_p)).
fof(not_q, axiom, ~q, file('tests/resources/cwa_fixture_problem.p', not_q)).
fof(branch_true, plain, p, inference(split_component, [status(thm)], [top])).
fof(branch_q, plain, q, inference(split_component, [status(thm)], [top])).
fof(false_true, plain, $false, inference(resolution, [status(thm)], [branch_true, not_p])).
fof(false_q, plain, $false, inference(resolution, [status(thm)], [branch_q, not_q])).
fof(bot, plain, $false, inference(avatar_sat_refutation, [status(thm)], [top, false_true, false_q])).

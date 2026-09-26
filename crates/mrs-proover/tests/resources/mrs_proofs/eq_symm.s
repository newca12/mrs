% SZS status Theorem for eq_symm
% SZS output start Proof for eq_symm
% Proof : problems/eq_symm.p
fof(c4, conjecture, b = a, file('problems/eq_symm.p', 'goal')).
fof(c0, axiom, a = b, file('problems/eq_symm.p', 'ax1')).
fof(c5, negated_conjecture, ~(b = a), inference(negated_conjecture, [status(cth)], [c4])).
fof(c1, plain, a = b, inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c6, plain, ~(b = a), inference(fof_nnf_transformation, [status(thm)], [c5])).
fof(c2, plain, a = b, inference(skolemisation, [status(esa)], [c1])).
fof(c7, plain, ~(b = a), inference(skolemisation, [status(esa)], [c6])).
cnf(c3, plain, a = b, inference(cnf_transformation, [status(thm)], [c2])).
cnf(c8, plain, b != a, inference(cnf_transformation, [status(thm)], [c7])).
cnf(c10, plain, $false, inference(subsumption_resolution, [status(thm)], [c8, c3])).
% SZS output end Proof for eq_symm
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 0.002 s
% Proof: 10 nodes, 781 bytes
% Peak memory usage: 1229 MB
% ------------------------------

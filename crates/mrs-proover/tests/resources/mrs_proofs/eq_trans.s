% SZS status Theorem for eq_trans
% SZS output start Proof for eq_trans
% Proof : problems/eq_trans.p
fof(c8, conjecture, a = c, file('problems/eq_trans.p', 'goal')).
fof(c0, axiom, a = b, file('problems/eq_trans.p', 'ax1')).
fof(c4, axiom, b = c, file('problems/eq_trans.p', 'ax2')).
fof(c9, negated_conjecture, ~(a = c), inference(negated_conjecture, [status(cth)], [c8])).
fof(c1, plain, a = b, inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c5, plain, b = c, inference(fof_nnf_transformation, [status(thm)], [c4])).
fof(c10, plain, ~(a = c), inference(fof_nnf_transformation, [status(thm)], [c9])).
fof(c2, plain, a = b, inference(skolemisation, [status(esa)], [c1])).
fof(c6, plain, b = c, inference(skolemisation, [status(esa)], [c5])).
fof(c11, plain, ~(a = c), inference(skolemisation, [status(esa)], [c10])).
cnf(c3, plain, a = b, inference(cnf_transformation, [status(thm)], [c2])).
cnf(c7, plain, b = c, inference(cnf_transformation, [status(thm)], [c6])).
cnf(c12, plain, a != c, inference(cnf_transformation, [status(thm)], [c11])).
cnf(c14, plain, a = c, inference(superposition, [status(thm)], [c7, c3])).
cnf(c18, plain, $false, inference(subsumption_resolution, [status(thm)], [c14, c12])).
% SZS output end Proof for eq_trans
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 0.002 s
% Proof: 15 nodes, 1149 bytes
% Peak memory usage: 1229 MB
% ------------------------------

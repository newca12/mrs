% SZS status Theorem for eq_func
% SZS output start Proof for eq_func
% Proof : problems/eq_func.p
fof(c4, axiom, f(f(f(f(f(a))))) = a, file('problems/eq_func.p', 'ax2')).
fof(c0, axiom, f(f(f(a))) = a, file('problems/eq_func.p', 'ax1')).
fof(c8, conjecture, f(a) = a, file('problems/eq_func.p', 'goal')).
fof(c5, plain, f(f(f(f(f(a))))) = a, inference(fof_nnf_transformation, [status(thm)], [c4])).
fof(c1, plain, f(f(f(a))) = a, inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c9, negated_conjecture, ~(f(a) = a), inference(negated_conjecture, [status(cth)], [c8])).
fof(c6, plain, f(f(f(f(f(a))))) = a, inference(skolemisation, [status(esa)], [c5])).
fof(c2, plain, f(f(f(a))) = a, inference(skolemisation, [status(esa)], [c1])).
fof(c10, plain, ~(f(a) = a), inference(fof_nnf_transformation, [status(thm)], [c9])).
cnf(c7, plain, f(f(f(f(f(a))))) = a, inference(cnf_transformation, [status(thm)], [c6])).
cnf(c3, plain, f(f(f(a))) = a, inference(cnf_transformation, [status(thm)], [c2])).
fof(c11, plain, ~(f(a) = a), inference(skolemisation, [status(esa)], [c10])).
cnf(c14, plain, f(f(a)) = a, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 0, 0]))], [c7, c3])).
cnf(c12, plain, f(a) != a, inference(cnf_transformation, [status(thm)], [c11])).
cnf(c17, plain, f(a) = a, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 0]))], [c3, c14])).
cnf(c19, plain, a != a, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0]))], [c12, c17])).
cnf(c20, plain, $false, inference(equality_resolution, [status(thm)], [c19])).
% SZS output end Proof for eq_func
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 0.002 s
% Proof: 17 nodes, 1528 bytes
% Peak memory usage: 1229 MB
% ------------------------------

% SZS status Theorem for group
% SZS output start Proof for group
% Proof : problems/group.p
fof(c8, axiom, ![X0]: (![X1]: (![X2]: (mult(mult(X0, X1), X2) = mult(X0, mult(X1, X2))))), file('problems/group.p', 'associativity')).
fof(c0, axiom, ![X0]: (mult(e, X0) = X0), file('problems/group.p', 'left_identity')).
fof(c4, axiom, ![X0]: (mult(inv(X0), X0) = e), file('problems/group.p', 'left_inverse')).
fof(c12, conjecture, ![X0]: (mult(X0, e) = X0), file('problems/group.p', 'goal')).
fof(c9, plain, ![X0]: (![X1]: (![X2]: (mult(mult(X0, X1), X2) = mult(X0, mult(X1, X2))))), inference(fof_nnf_transformation, [status(thm)], [c8])).
fof(c1, plain, ![X0]: (mult(e, X0) = X0), inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c5, plain, ![X0]: (mult(inv(X0), X0) = e), inference(fof_nnf_transformation, [status(thm)], [c4])).
fof(c13, negated_conjecture, ~(![X0]: (mult(X0, e) = X0)), inference(negated_conjecture, [status(cth)], [c12])).
fof(c10, plain, ![X0]: (![X1]: (![X2]: (mult(mult(X0, X1), X2) = mult(X0, mult(X1, X2))))), inference(skolemisation, [status(esa)], [c9])).
fof(c2, plain, ![X0]: (mult(e, X0) = X0), inference(skolemisation, [status(esa)], [c1])).
fof(c6, plain, ![X0]: (mult(inv(X0), X0) = e), inference(skolemisation, [status(esa)], [c5])).
fof(c14, plain, ?[X0]: (~(mult(X0, e) = X0)), inference(fof_nnf_transformation, [status(thm)], [c13])).
cnf(c11, plain, mult(mult(X0, X1), X2) = mult(X0, mult(X1, X2)), inference(cnf_transformation, [status(thm)], [c10])).
cnf(c3, plain, mult(e, X0) = X0, inference(cnf_transformation, [status(thm)], [c2])).
cnf(c7, plain, mult(inv(X0), X0) = e, inference(cnf_transformation, [status(thm)], [c6])).
fof(c15, plain, ~(mult(sk_goal_0, e) = sk_goal_0), inference(skolemisation, [status(esa)], [c14])).
cnf(c26, plain, mult(e, X3) = mult(inv(X2), mult(X2, X3)), inference(superposition, [status(thm)], [c7, c11])).
cnf(c16, plain, mult(sk_goal_0, e) != sk_goal_0, inference(cnf_transformation, [status(thm)], [c15])).
cnf(c35, plain, X3 = mult(inv(X2), mult(X2, X3)), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0]))], [c26, c3])).
cnf(c43, plain, mult(X2, X3) = mult(inv(inv(X2)), X3), inference(superposition, [status(thm)], [c35, c35])).
cnf(c45, plain, X4 = mult(inv(inv(X4)), e), inference(superposition, [status(thm)], [c7, c35])).
cnf(c121, plain, X4 = mult(X4, e), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c45, c43])).
cnf(c136, plain, sk_goal_0 != sk_goal_0, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0]))], [c16, c121])).
cnf(c137, plain, $false, inference(equality_resolution, [status(thm)], [c136])).
% SZS output end Proof for group
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 0.087 s
% Proof: 24 nodes, 2607 bytes
% Peak memory usage: 1229 MB
% ------------------------------

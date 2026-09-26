% SZS status Theorem for lattice_absorb
% SZS output start Proof for lattice_absorb
% Proof : problems/lattice_absorb.p
fof(c16, axiom, ![X0]: (![X1]: (join(X0, meet(X0, X1)) = X0)), file('problems/lattice_absorb.p', 'join_absorption')).
fof(c20, axiom, ![X0]: (![X1]: (meet(X0, join(X0, X1)) = X0)), file('problems/lattice_absorb.p', 'meet_absorption')).
fof(c24, conjecture, ![X0]: (join(X0, X0) = X0), file('problems/lattice_absorb.p', 'goal')).
fof(c12, axiom, ![X0]: (![X1]: (![X2]: (meet(meet(X0, X1), X2) = meet(X0, meet(X1, X2))))), file('problems/lattice_absorb.p', 'meet_associative')).
fof(c8, axiom, ![X0]: (![X1]: (![X2]: (join(join(X0, X1), X2) = join(X0, join(X1, X2))))), file('problems/lattice_absorb.p', 'join_associative')).
fof(c4, axiom, ![X0]: (![X1]: (meet(X0, X1) = meet(X1, X0))), file('problems/lattice_absorb.p', 'meet_commutative')).
fof(c0, axiom, ![X0]: (![X1]: (join(X0, X1) = join(X1, X0))), file('problems/lattice_absorb.p', 'join_commutative')).
fof(c17, plain, ![X0]: (![X1]: (join(X0, meet(X0, X1)) = X0)), inference(fof_nnf_transformation, [status(thm)], [c16])).
fof(c21, plain, ![X0]: (![X1]: (meet(X0, join(X0, X1)) = X0)), inference(fof_nnf_transformation, [status(thm)], [c20])).
fof(c25, negated_conjecture, ~(![X0]: (join(X0, X0) = X0)), inference(negated_conjecture, [status(cth)], [c24])).
fof(c13, plain, ![X0]: (![X1]: (![X2]: (meet(meet(X0, X1), X2) = meet(X0, meet(X1, X2))))), inference(fof_nnf_transformation, [status(thm)], [c12])).
fof(c9, plain, ![X0]: (![X1]: (![X2]: (join(join(X0, X1), X2) = join(X0, join(X1, X2))))), inference(fof_nnf_transformation, [status(thm)], [c8])).
fof(c5, plain, ![X0]: (![X1]: (meet(X0, X1) = meet(X1, X0))), inference(fof_nnf_transformation, [status(thm)], [c4])).
fof(c1, plain, ![X0]: (![X1]: (join(X0, X1) = join(X1, X0))), inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c18, plain, ![X0]: (![X1]: (join(X0, meet(X0, X1)) = X0)), inference(skolemisation, [status(esa)], [c17])).
fof(c22, plain, ![X0]: (![X1]: (meet(X0, join(X0, X1)) = X0)), inference(skolemisation, [status(esa)], [c21])).
fof(c26, plain, ?[X0]: (~(join(X0, X0) = X0)), inference(fof_nnf_transformation, [status(thm)], [c25])).
fof(c14, plain, ![X0]: (![X1]: (![X2]: (meet(meet(X0, X1), X2) = meet(X0, meet(X1, X2))))), inference(skolemisation, [status(esa)], [c13])).
fof(c10, plain, ![X0]: (![X1]: (![X2]: (join(join(X0, X1), X2) = join(X0, join(X1, X2))))), inference(skolemisation, [status(esa)], [c9])).
fof(c6, plain, ![X0]: (![X1]: (meet(X0, X1) = meet(X1, X0))), inference(skolemisation, [status(esa)], [c5])).
fof(c2, plain, ![X0]: (![X1]: (join(X0, X1) = join(X1, X0))), inference(skolemisation, [status(esa)], [c1])).
cnf(c19, plain, join(X0, meet(X0, X1)) = X0, inference(cnf_transformation, [status(thm)], [c18])).
cnf(c23, plain, meet(X0, join(X0, X1)) = X0, inference(cnf_transformation, [status(thm)], [c22])).
fof(c27, plain, ~(join(sk_goal_0, sk_goal_0) = sk_goal_0), inference(skolemisation, [status(esa)], [c26])).
cnf(c15, plain, meet(meet(X0, X1), X2) = meet(X0, meet(X1, X2)), inference(cnf_transformation, [status(thm)], [c14])).
cnf(c11, plain, join(join(X0, X1), X2) = join(X0, join(X1, X2)), inference(cnf_transformation, [status(thm)], [c10])).
cnf(c7, plain, meet(X0, X1) = meet(X1, X0), inference(cnf_transformation, [status(thm)], [c6])).
cnf(c3, plain, join(X0, X1) = join(X1, X0), inference(cnf_transformation, [status(thm)], [c2])).
cnf(c28, plain, join(sk_goal_0, sk_goal_0) != sk_goal_0, inference(cnf_transformation, [status(thm)], [c27])).
cnf(c30, plain, X0 = join(X0, meet(X0, X1)), inference(ac_normalization, [status(thm)], [c19, c3, c7, c11, c15])).
cnf(c32, plain, X0 = meet(X0, join(X0, X1)), inference(ac_normalization, [status(thm)], [c23, c3, c7, c11, c15])).
cnf(c42, plain, sk_goal_0 != join(sk_goal_0, sk_goal_0), inference(ac_normalization, [status(thm)], [c28, c3, c7, c11, c15])).
cnf(c33, plain, X2 = join(X2, X2), inference(ac_superposition, [status(thm)], [c32, c30, c3, c7, c11, c15])).
cnf(c43, plain, $false, inference(subsumption_resolution, [status(thm)], [c42, c33])).
% SZS output end Proof for lattice_absorb
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 0.205 s
% Proof: 34 nodes, 4020 bytes
% Peak memory usage: 1229 MB
% ------------------------------

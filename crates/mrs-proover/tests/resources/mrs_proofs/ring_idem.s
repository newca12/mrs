% SZS status Theorem for ring_idem
% SZS output start Proof for ring_idem
% Proof : problems/ring_idem.p
fof(c12, axiom, ![X0]: (![X1]: (![X2]: (times(times(X0, X1), X2) = times(X0, times(X1, X2))))), file('problems/ring_idem.p', 'associativity_times')).
fof(c0, axiom, ![X0]: (plus(zero, X0) = X0), file('problems/ring_idem.p', 'left_identity')).
fof(c4, axiom, ![X0]: (plus(neg(X0), X0) = zero), file('problems/ring_idem.p', 'left_inverse')).
fof(c16, axiom, ![X0]: (![X1]: (![X2]: (times(X0, plus(X1, X2)) = plus(times(X0, X1), times(X0, X2))))), file('problems/ring_idem.p', 'left_distribute')).
fof(c20, axiom, ![X0]: (![X1]: (![X2]: (times(plus(X0, X1), X2) = plus(times(X0, X2), times(X1, X2))))), file('problems/ring_idem.p', 'right_distribute')).
fof(c24, axiom, ![X0]: (times(X0, X0) = X0), file('problems/ring_idem.p', 'idempotent')).
fof(c8, axiom, ![X0]: (![X1]: (![X2]: (plus(plus(X0, X1), X2) = plus(X0, plus(X1, X2))))), file('problems/ring_idem.p', 'associativity_plus')).
fof(c28, conjecture, ![X0]: (![X1]: (times(X0, X1) = times(X1, X0))), file('problems/ring_idem.p', 'goal')).
fof(c13, plain, ![X0]: (![X1]: (![X2]: (times(times(X0, X1), X2) = times(X0, times(X1, X2))))), inference(fof_nnf_transformation, [status(thm)], [c12])).
fof(c1, plain, ![X0]: (plus(zero, X0) = X0), inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c5, plain, ![X0]: (plus(neg(X0), X0) = zero), inference(fof_nnf_transformation, [status(thm)], [c4])).
fof(c17, plain, ![X0]: (![X1]: (![X2]: (times(X0, plus(X1, X2)) = plus(times(X0, X1), times(X0, X2))))), inference(fof_nnf_transformation, [status(thm)], [c16])).
fof(c21, plain, ![X0]: (![X1]: (![X2]: (times(plus(X0, X1), X2) = plus(times(X0, X2), times(X1, X2))))), inference(fof_nnf_transformation, [status(thm)], [c20])).
fof(c25, plain, ![X0]: (times(X0, X0) = X0), inference(fof_nnf_transformation, [status(thm)], [c24])).
fof(c9, plain, ![X0]: (![X1]: (![X2]: (plus(plus(X0, X1), X2) = plus(X0, plus(X1, X2))))), inference(fof_nnf_transformation, [status(thm)], [c8])).
fof(c29, negated_conjecture, ~(![X0]: (![X1]: (times(X0, X1) = times(X1, X0)))), inference(negated_conjecture, [status(cth)], [c28])).
fof(c14, plain, ![X0]: (![X1]: (![X2]: (times(times(X0, X1), X2) = times(X0, times(X1, X2))))), inference(skolemisation, [status(esa)], [c13])).
fof(c2, plain, ![X0]: (plus(zero, X0) = X0), inference(skolemisation, [status(esa)], [c1])).
fof(c6, plain, ![X0]: (plus(neg(X0), X0) = zero), inference(skolemisation, [status(esa)], [c5])).
fof(c18, plain, ![X0]: (![X1]: (![X2]: (times(X0, plus(X1, X2)) = plus(times(X0, X1), times(X0, X2))))), inference(skolemisation, [status(esa)], [c17])).
fof(c22, plain, ![X0]: (![X1]: (![X2]: (times(plus(X0, X1), X2) = plus(times(X0, X2), times(X1, X2))))), inference(skolemisation, [status(esa)], [c21])).
fof(c26, plain, ![X0]: (times(X0, X0) = X0), inference(skolemisation, [status(esa)], [c25])).
fof(c10, plain, ![X0]: (![X1]: (![X2]: (plus(plus(X0, X1), X2) = plus(X0, plus(X1, X2))))), inference(skolemisation, [status(esa)], [c9])).
fof(c30, plain, ?[X0]: (?[X1]: (~(times(X0, X1) = times(X1, X0)))), inference(fof_nnf_transformation, [status(thm)], [c29])).
cnf(c15, plain, times(times(X0, X1), X2) = times(X0, times(X1, X2)), inference(cnf_transformation, [status(thm)], [c14])).
cnf(c3, plain, plus(zero, X0) = X0, inference(cnf_transformation, [status(thm)], [c2])).
cnf(c7, plain, plus(neg(X0), X0) = zero, inference(cnf_transformation, [status(thm)], [c6])).
cnf(c19, plain, times(X0, plus(X1, X2)) = plus(times(X0, X1), times(X0, X2)), inference(cnf_transformation, [status(thm)], [c18])).
cnf(c23, plain, times(plus(X0, X1), X2) = plus(times(X0, X2), times(X1, X2)), inference(cnf_transformation, [status(thm)], [c22])).
cnf(c27, plain, times(X0, X0) = X0, inference(cnf_transformation, [status(thm)], [c26])).
cnf(c11, plain, plus(plus(X0, X1), X2) = plus(X0, plus(X1, X2)), inference(cnf_transformation, [status(thm)], [c10])).
fof(c31, plain, ~(times(sk_goal_0, sk_goal_1) = times(sk_goal_1, sk_goal_0)), inference(skolemisation, [status(esa)], [c30])).
cnf(c28375, plain, times(X3, plus(X5, X5)) = times(plus(X3, X3), X5), inference(superposition, [status(thm)], [c23, c19])).
cnf(c27228, plain, times(X3, plus(X2, X3)) = plus(times(X3, X2), X3), inference(superposition, [status(thm)], [c27, c19])).
cnf(c28412, plain, times(plus(X1, X3), X3) = plus(times(X1, X3), X3), inference(superposition, [status(thm)], [c27, c23])).
cnf(c27227, plain, times(X2, plus(X2, X3)) = plus(X2, times(X2, X3)), inference(superposition, [status(thm)], [c27, c19])).
cnf(c28411, plain, times(plus(X3, X2), X3) = plus(X3, times(X2, X3)), inference(superposition, [status(thm)], [c27, c23])).
cnf(c26301, plain, plus(zero, X3) = plus(neg(X2), plus(X2, X3)), inference(superposition, [status(thm)], [c7, c11])).
cnf(c32, plain, times(sk_goal_0, sk_goal_1) != times(sk_goal_1, sk_goal_0), inference(cnf_transformation, [status(thm)], [c31])).
cnf(c27299, plain, times(X4, plus(X4, X4)) = plus(X4, X4), inference(superposition, [status(thm)], [c27, c27227])).
cnf(c26310, plain, X3 = plus(neg(X2), plus(X2, X3)), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0]))], [c26301, c3])).
cnf(c28674, plain, plus(X10, X10) = times(plus(X10, X10), X10), inference(superposition, [status(thm)], [c27299, c28375])).
cnf(c27274, plain, times(X6, X3) = plus(neg(X6), times(X6, plus(X6, X3))), inference(superposition, [status(thm)], [c27227, c26310])).
cnf(c27388, plain, X7 = plus(neg(times(X7, X2)), times(X7, plus(X2, X7))), inference(superposition, [status(thm)], [c27228, c26310])).
cnf(c26318, plain, plus(X2, X3) = plus(neg(neg(X2)), X3), inference(superposition, [status(thm)], [c26310, c26310])).
cnf(c26320, plain, X4 = plus(neg(neg(X4)), zero), inference(superposition, [status(thm)], [c7, c26310])).
cnf(c26322, plain, X6 = plus(neg(plus(X0, X1)), plus(X0, plus(X1, X6))), inference(superposition, [status(thm)], [c11, c26310])).
cnf(c27655, plain, X8 = plus(neg(times(X8, neg(X8))), times(X8, zero)), inference(superposition, [status(thm)], [c7, c27388])).
cnf(c26379, plain, plus(X2, neg(X2)) = zero, inference(superposition, [status(thm)], [c26318, c7])).
cnf(c26383, plain, X7 = plus(X2, plus(neg(X2), X7)), inference(superposition, [status(thm)], [c26318, c26310])).
cnf(c26397, plain, X4 = plus(X4, zero), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c26320, c26318])).
cnf(c26412, plain, zero = plus(X3, plus(X4, neg(plus(X3, X4)))), inference(superposition, [status(thm)], [c26379, c11])).
cnf(c26447, plain, neg(neg(X5)) = plus(X5, zero), inference(superposition, [status(thm)], [c26379, c26383])).
cnf(c27496, plain, times(X11, zero) = plus(neg(X11), times(X11, X11)), inference(superposition, [status(thm)], [c26397, c27274])).
cnf(c26554, plain, plus(X4, neg(plus(X7, X4))) = plus(neg(X7), zero), inference(superposition, [status(thm)], [c26412, c26310])).
cnf(c26463, plain, neg(neg(X5)) = X5, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c26447, c26397])).
cnf(c27536, plain, times(X11, zero) = zero, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1, 1]), rule(2, 0, [1]))], [c27496, c27, c7])).
cnf(c26580, plain, plus(X4, neg(plus(X7, X4))) = neg(X7), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c26554, c26397])).
cnf(c27697, plain, X8 = neg(times(X8, neg(X8))), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1, 1]), rule(2, 0, [1]))], [c27655, c27536, c26397])).
cnf(c26602, plain, neg(plus(X7, X10)) = plus(neg(X10), neg(X7)), inference(superposition, [status(thm)], [c26580, c26310])).
cnf(c26609, plain, plus(neg(plus(X7, X15)), neg(neg(X7))) = neg(X15), inference(superposition, [status(thm)], [c26580, c26580])).
cnf(c27715, plain, neg(X8) = times(X8, neg(X8)), inference(superposition, [status(thm)], [c27697, c26463])).
cnf(c26630, plain, plus(neg(plus(X7, X15)), X7) = neg(X15), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 1]))], [c26609, c26463])).
cnf(c27764, plain, neg(neg(X5)) = times(neg(X5), X5), inference(superposition, [status(thm)], [c26463, c27715])).
cnf(c26659, plain, plus(neg(X7), X15) = neg(plus(neg(X15), X7)), inference(superposition, [status(thm)], [c26383, c26630])).
cnf(c27787, plain, X5 = times(neg(X5), X5), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0]))], [c27764, c26463])).
cnf(c27794, plain, times(X7, X8) = times(neg(X7), times(X7, X8)), inference(superposition, [status(thm)], [c27787, c15])).
cnf(c27901, plain, times(X12, plus(X12, X12)) = times(neg(X12), plus(X12, X12)), inference(superposition, [status(thm)], [c27299, c27794])).
cnf(c27935, plain, plus(X12, X12) = times(neg(X12), plus(X12, X12)), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0]))], [c27901, c27299])).
cnf(c28681, plain, plus(X18, X18) = times(plus(neg(X18), neg(X18)), X18), inference(superposition, [status(thm)], [c27935, c28375])).
cnf(c28737, plain, plus(X18, X18) = times(neg(plus(X18, X18)), X18), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1, 0]))], [c28681, c26602])).
cnf(c28830, plain, times(neg(plus(X22, X22)), plus(neg(plus(X22, X22)), X22)) = plus(neg(plus(X22, X22)), plus(X22, X22)), inference(superposition, [status(thm)], [c28737, c27227])).
cnf(c28869, plain, times(neg(plus(X22, X22)), neg(X22)) = zero, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 1]), rule(2, 0, [1]))], [c28830, c26630, c7])).
cnf(c28925, plain, times(neg(plus(neg(X5), neg(X5))), X5) = zero, inference(superposition, [status(thm)], [c26463, c28869])).
cnf(c28953, plain, plus(X5, X5) = zero, inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 0]), rule(2, 0, [0, 0, 0]), rule(3, 0, [0]))], [c28925, c26659, c26463, c28674])).
cnf(c28968, plain, X9 = plus(neg(X9), zero), inference(superposition, [status(thm)], [c28953, c26310])).
cnf(c29040, plain, X9 = neg(X9), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c28968, c26397])).
cnf(c29128, plain, plus(X7, X15) = plus(X15, X7), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 0]), rule(1, 0, [1]), rule(1, 0, [1, 0]))], [c26659, c29040])).
cnf(c29144, plain, X7 = plus(X2, plus(X2, X7)), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1, 1, 0]))], [c26383, c29040])).
cnf(c29133, plain, X6 = plus(plus(X0, X1), plus(X0, plus(X1, X6))), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1, 0]))], [c26322, c29040])).
cnf(c29224, plain, times(plus(X11, X2), X11) = plus(times(X2, X11), X11), inference(superposition, [status(thm)], [c28411, c29128])).
cnf(c29134, plain, X6 = plus(X0, plus(X1, plus(X0, plus(X1, X6)))), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c29133, c11])).
cnf(c29425, plain, times(plus(X11, X2), X11) = times(plus(X2, X11), X11), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1]))], [c29224, c28412])).
cnf(c29453, plain, times(plus(X2, X15), plus(plus(X2, X15), X15)) = plus(plus(X2, X15), times(plus(X15, X2), X15)), inference(superposition, [status(thm)], [c29425, c27227])).
cnf(c29527, plain, times(plus(X2, X15), X2) = times(X2, plus(X2, X15)), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [0, 1]), rule(1, 0, [1]), rule(2, 0, [0, 1, 1]), rule(3, 0, [1, 1]), rule(4, 0, [0, 1]), rule(5, 0, [1, 1, 0]), rule(6, 0, [1]))], [c29453, c11, c28953, c28411, c26397, c29144, c27227])).
cnf(c30343, plain, times(X6, X9) = times(X9, plus(X9, plus(X1, plus(X9, plus(X1, X6))))), inference(superposition, [status(thm)], [c29134, c29527])).
cnf(c30410, plain, times(X6, X9) = times(X9, X6), inference(demodulation, [status(thm), demodulation_steps(rule(1, 0, [1, 1]))], [c30343, c29134])).
cnf(c30491, plain, $false, inference(subsumption_resolution, [status(thm)], [c32, c30410])).
% SZS output end Proof for ring_idem
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 2.697 s
% Proof: 88 nodes, 11898 bytes
% Peak memory usage: 1229 MB
% ------------------------------

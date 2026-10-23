% SZS status Theorem for socrates
% SZS output start Proof for socrates
% Proof : problems/socrates.p
fof(c8, conjecture, mortal(socrates), file('problems/socrates.p', 'goal')).
fof(c4, axiom, human(socrates), file('problems/socrates.p', 'ax2')).
fof(c0, axiom, ![X0]: ((human(X0) => mortal(X0))), file('problems/socrates.p', 'ax1')).
fof(c9, negated_conjecture, ~(mortal(socrates)), inference(negated_conjecture, [status(cth)], [c8])).
fof(c5, plain, human(socrates), inference(fof_nnf_transformation, [status(thm)], [c4])).
fof(c1, plain, ![X0]: ((~(human(X0)) | mortal(X0))), inference(fof_nnf_transformation, [status(thm)], [c0])).
fof(c10, plain, ~(mortal(socrates)), inference(fof_nnf_transformation, [status(thm)], [c9])).
fof(c6, plain, human(socrates), inference(skolemisation, [status(esa)], [c5])).
fof(c2, plain, ![X0]: ((~(human(X0)) | mortal(X0))), inference(skolemisation, [status(esa)], [c1])).
fof(c11, plain, ~(mortal(socrates)), inference(skolemisation, [status(esa)], [c10])).
cnf(c7, plain, human(socrates), inference(cnf_transformation, [status(thm)], [c6])).
cnf(c3, plain, ~human(X0) | mortal(X0), inference(cnf_transformation, [status(thm)], [c2])).
cnf(c12, plain, ~mortal(socrates), inference(cnf_transformation, [status(thm)], [c11])).
cnf(c13, plain, ~human(socrates) | mortal(socrates), inference(instantiation, [status(thm)], [c3])).
cnf(c14, plain, mortal(socrates), inference(resolution, [status(thm)], [c13, c7])).
cnf(c15, plain, $false, inference(resolution, [status(thm)], [c14, c12])).
% SZS output end Proof for socrates
% ------------------------------
% Version: mrs 0.2.3
% Termination reason: Refutation
% Time elapsed: 0.301 s
% Proof: 16 nodes, 1451 bytes
% Peak memory usage: 1037 MB
% ------------------------------

% Relevant c502 ancestor closure extracted verbatim from /mnt/sdd/SWV406+1.stdout.
% Full transcript SHA-256: 5a8ea239cc6ef13d630bdfdfaa319c48b10785fad58e0238178ce5675646e822
% Transcript: 657464 lines, 169922819 bytes; MRS 0.2.3; 171.746 s; 21130 MB peak.
% Source lines retained: 9, 11, 43, 45, 77, 79, 113, 115, 152.
% This is the complete nine-node ancestor closure for c502, not the full refutation.
fof(c24, axiom, ![X0]: (![X1]: (![X2]: (![X3]: (((contains_slb(X0, X2) & ~(X2 = X1)) => remove_slb(insert_slb(X0, pair(X1, X3)), X2) = insert_slb(remove_slb(X0, X2), pair(X1, X3))))))), file('/DATA/ai/mrs/crates/mrs-bench/problems/casc-j13/FEQ/SWV406+1.p', 'ax25')).
fof(c36, axiom, ![X0]: (![X1]: (![X2]: (remove_slb(insert_slb(X0, pair(X1, X2)), X1) = X0))), file('/DATA/ai/mrs/crates/mrs-bench/problems/casc-j13/FEQ/SWV406+1.p', 'ax24')).
fof(c25, plain, ![X0]: (![X1]: (![X2]: (![X3]: (((~(contains_slb(X0, X2)) | X2 = X1) | remove_slb(insert_slb(X0, pair(X1, X3)), X2) = insert_slb(remove_slb(X0, X2), pair(X1, X3))))))), inference(fof_nnf_transformation, [status(thm)], [c24])).
fof(c37, plain, ![X0]: (![X1]: (![X2]: (remove_slb(insert_slb(X0, pair(X1, X2)), X1) = X0))), inference(fof_nnf_transformation, [status(thm)], [c36])).
fof(c26, plain, ![X0]: (![X1]: (![X2]: (![X3]: (((~(contains_slb(X0, X2)) | X2 = X1) | remove_slb(insert_slb(X0, pair(X1, X3)), X2) = insert_slb(remove_slb(X0, X2), pair(X1, X3))))))), inference(skolemisation, [status(esa)], [c25])).
fof(c38, plain, ![X0]: (![X1]: (![X2]: (remove_slb(insert_slb(X0, pair(X1, X2)), X1) = X0))), inference(skolemisation, [status(esa)], [c37])).
cnf(c27, plain, ~contains_slb(X0, X2) | X2 = X1 | remove_slb(insert_slb(X0, pair(X1, X3)), X2) = insert_slb(remove_slb(X0, X2), pair(X1, X3)), inference(cnf_transformation, [status(thm)], [c26])).
cnf(c39, plain, remove_slb(insert_slb(X0, pair(X1, X2)), X1) = X0, inference(cnf_transformation, [status(thm)], [c38])).
cnf(c502, plain, ~contains_slb(X0, X2) | X2 = X5 | remove_slb(remove_slb(insert_slb(X0, pair(X5, X6)), X2), X5) = remove_slb(X0, X2), inference(superposition, [status(thm)], [c27, c39])).

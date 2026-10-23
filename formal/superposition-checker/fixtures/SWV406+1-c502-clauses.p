% Archived CASC-J13 FEQ proof-step data for SWV406+1 node c502.
% Parents and conclusion for the archived ordinary syntactic superposition
% step. first_order_core.rs encodes a lossless bounded executable certificate
% with independent standardization-apart maps, whole-parent unification,
% selected target path, and exact ordered conclusion comparison.
cnf(c27, axiom,
    ~contains_slb(X0, X2) | X2 = X1 |
    remove_slb(insert_slb(X0, pair(X1, X3)), X2) =
    insert_slb(remove_slb(X0, X2), pair(X1, X3))).

cnf(c39, axiom,
    remove_slb(insert_slb(X0, pair(X1, X2)), X1) = X0).

cnf(c502_candidate, plain,
    ~contains_slb(X0, X2) | X2 = X5 |
    remove_slb(remove_slb(insert_slb(X0, pair(X5, X6)), X2), X5) =
    remove_slb(X0, X2)).

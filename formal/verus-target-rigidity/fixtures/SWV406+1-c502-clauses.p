% Archived CASC-J13 FEQ proof-step data for SWV406+1 node c502.
% Preserved for the future general-clause checker audit. The current Verus
% checker accepts only ground unit clauses and therefore gives no verdict on
% this variable-bearing, multi-literal step.
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

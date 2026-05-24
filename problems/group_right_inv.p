% Group theory: left identity + left inverse + associativity => right inverse
% e * X = X, inv(X) * X = e, (X*Y)*Z = X*(Y*Z) |- X * inv(X) = e
% Status: Theorem
% Harder than group.p: requires deriving right inverse from left axioms.

fof(left_identity, axiom, ![X]: mult(e, X) = X).
fof(left_inverse, axiom, ![X]: mult(inv(X), X) = e).
fof(associativity, axiom, ![X, Y, Z]: mult(mult(X, Y), Z) = mult(X, mult(Y, Z))).
fof(goal, conjecture, ![X]: mult(X, inv(X)) = e).

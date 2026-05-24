% Group theory: left identity + left inverse => right identity
% e * X = X, inv(X) * X = e |- X * e = X
% Status: Theorem

fof(left_identity, axiom, ![X]: mult(e, X) = X).
fof(left_inverse, axiom, ![X]: mult(inv(X), X) = e).
fof(associativity, axiom, ![X, Y, Z]: mult(mult(X, Y), Z) = mult(X, mult(Y, Z))).
fof(goal, conjecture, ![X]: mult(X, e) = X).

% Group theory: uniqueness of inverse
% Group axioms |- if mult(X, Y) = e then Y = inv(X)
% Status: Theorem

fof(left_identity, axiom, ![X]: mult(e, X) = X).
fof(left_inverse, axiom, ![X]: mult(inv(X), X) = e).
fof(associativity, axiom, ![X, Y, Z]: mult(mult(X, Y), Z) = mult(X, mult(Y, Z))).
fof(goal, conjecture, ![X, Y]: (mult(X, Y) = e => Y = inv(X))).

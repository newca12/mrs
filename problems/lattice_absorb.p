% Lattice theory: absorption laws imply idempotence of join
% join(X, meet(X, Y)) = X, meet(X, join(X, Y)) = X |- join(X, X) = X
% Status: Theorem

fof(join_commutative, axiom, ![X, Y]: join(X, Y) = join(Y, X)).
fof(meet_commutative, axiom, ![X, Y]: meet(X, Y) = meet(Y, X)).
fof(join_associative, axiom, ![X, Y, Z]: join(join(X, Y), Z) = join(X, join(Y, Z))).
fof(meet_associative, axiom, ![X, Y, Z]: meet(meet(X, Y), Z) = meet(X, meet(Y, Z))).
fof(join_absorption, axiom, ![X, Y]: join(X, meet(X, Y)) = X).
fof(meet_absorption, axiom, ![X, Y]: meet(X, join(X, Y)) = X).
fof(goal, conjecture, ![X]: join(X, X) = X).

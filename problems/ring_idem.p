% Ring theory: commutativity from specific ring axioms
% In a ring where x*x = x for all x, prove x*y = y*x.
% This is a well-known benchmark for equality reasoning.

fof(left_identity, axiom, ![X]: plus(zero, X) = X).
fof(left_inverse, axiom, ![X]: plus(neg(X), X) = zero).
fof(associativity_plus, axiom, ![X,Y,Z]: plus(plus(X, Y), Z) = plus(X, plus(Y, Z))).
fof(associativity_times, axiom, ![X,Y,Z]: times(times(X, Y), Z) = times(X, times(Y, Z))).
fof(left_distribute, axiom, ![X,Y,Z]: times(X, plus(Y, Z)) = plus(times(X, Y), times(X, Z))).
fof(right_distribute, axiom, ![X,Y,Z]: times(plus(X, Y), Z) = plus(times(X, Z), times(Y, Z))).
fof(idempotent, axiom, ![X]: times(X, X) = X).
fof(goal, conjecture, ![X,Y]: times(X, Y) = times(Y, X)).

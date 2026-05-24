% Pelletier #26
% (exists x, p(x)) <=> (exists x, q(x))  AND
% forall x y, (p(x) & q(y)) => (r(x) <=> s(y))
% THEN (forall x, p(x) => r(x)) <=> (forall x, q(x) => s(x))

fof(premise1, axiom, (?[X]: p(X)) <=> (?[X]: q(X))).
fof(premise2, axiom, ![X,Y]: ((p(X) & q(Y)) => (r(X) <=> s(Y)))).
fof(goal, conjecture, (![X]: (p(X) => r(X))) <=> (![X]: (q(X) => s(X)))).

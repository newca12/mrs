% Pelletier #28
% forall x, (p(x) => forall y, q(y))  AND
% (forall x, q(x) | r(x)) => (exists x, q(x) & s(x)) AND
% (exists x, s(x)) => (forall x, f(x) => g(x))
% THEN forall x, (p(x) & f(x)) => g(x)

fof(ax1, axiom, ![X]: (p(X) => ![Y]: q(Y))).
fof(ax2, axiom, ((![X]: (q(X) | r(X)))) => (?[X]: (q(X) & s(X)))).
fof(ax3, axiom, (?[X]: s(X)) => (![X]: (f(X) => g(X)))).
fof(goal, conjecture, ![X]: ((p(X) & f(X)) => g(X))).

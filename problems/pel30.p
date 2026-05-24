% Pelletier #30
% (forall x, f(x) | g(x) => ~h(x)) AND
% (forall x, (g(x) => ~i(x)) => f(x) & h(x))
% THEN forall x, i(x)

fof(ax1, axiom, ![X]: ((f(X) | g(X)) => ~h(X))).
fof(ax2, axiom, ![X]: ((g(X) => ~i(X)) => (f(X) & h(X)))).
fof(goal, conjecture, ![X]: i(X)).

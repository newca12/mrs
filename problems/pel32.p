% Pelletier #32
% (forall x, f(x) & (g(x) | h(x)) => i(x)) AND
% (forall x, i(x) & h(x) => j(x)) AND
% (forall x, k(x) => h(x))
% THEN forall x, f(x) & k(x) => j(x)

fof(ax1, axiom, ![X]: ((f(X) & (g(X) | h(X))) => i(X))).
fof(ax2, axiom, ![X]: ((i(X) & h(X)) => j(X))).
fof(ax3, axiom, ![X]: (k(X) => h(X))).
fof(goal, conjecture, ![X]: ((f(X) & k(X)) => j(X))).

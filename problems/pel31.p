% Pelletier #31
% ~(exists x, f(x) & (g(x) | h(x))) AND
% (exists x, i(x) & f(x)) AND
% (forall x, ~h(x) => j(x))
% THEN (exists x, i(x) & j(x))

fof(ax1, axiom, ~(?[X]: (f(X) & (g(X) | h(X))))).
fof(ax2, axiom, ?[X]: (i(X) & f(X))).
fof(ax3, axiom, ![X]: (~h(X) => j(X))).
fof(goal, conjecture, ?[X]: (i(X) & j(X))).

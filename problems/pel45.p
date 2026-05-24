% Pelletier #45
% (1) forall x, f(x) & (forall y, g(y) & h(x,y) => j(x,y)) => (forall y, g(y) & h(x,y) => k(y))
% (2) ~(exists y, l(y) & k(y))
% (3) exists x, f(x) & (forall y, h(x,y) => l(y)) & (forall y, g(y) & h(x,y) => j(x,y))
% THEN exists x, f(x) & ~(exists y, g(y) & h(x,y))

fof(ax1, axiom, ![X]: ((f(X) & (![Y]: ((g(Y) & h(X,Y)) => j(X,Y)))) => (![Y]: ((g(Y) & h(X,Y)) => k(Y))))).
fof(ax2, axiom, ~(?[Y]: (l(Y) & k(Y)))).
fof(ax3, axiom, ?[X]: (f(X) & (![Y]: (h(X,Y) => l(Y))) & (![Y]: ((g(Y) & h(X,Y)) => j(X,Y))))).
fof(goal, conjecture, ?[X]: (f(X) & ~(?[Y]: (g(Y) & h(X,Y))))).

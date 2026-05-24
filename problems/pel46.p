% Pelletier #46
% (1) forall x, (f(x) & (forall y, f(y) & h(y,x) => g(y))) => g(x)
% (2) (exists x, f(x) & ~g(x)) => (exists x, f(x) & ~g(x) & (forall y, f(y) & ~g(y) => j(x,y)))
% (3) forall x y, (f(x) & f(y) & h(x,y)) => ~j(y,x)
% THEN forall x, f(x) => g(x)

fof(ax1, axiom, ![X]: ((f(X) & (![Y]: ((f(Y) & h(Y,X)) => g(Y)))) => g(X))).
fof(ax2, axiom, (?[X]: (f(X) & ~g(X))) => (?[X]: (f(X) & ~g(X) & (![Y]: ((f(Y) & ~g(Y)) => j(X,Y)))))).
fof(ax3, axiom, ![X,Y]: ((f(X) & f(Y) & h(X,Y)) => ~j(Y,X))).
fof(goal, conjecture, ![X]: (f(X) => g(X))).

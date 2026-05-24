% Pelletier #43
% (forall x y, q(x, y) <=> forall z, f(z, x) <=> f(z, y))
% THEN forall x y, q(x, y) <=> q(y, x)

fof(ax1, axiom, ![X,Y]: (q(X,Y) <=> (![Z]: (f(Z,X) <=> f(Z,Y))))).
fof(goal, conjecture, ![X,Y]: (q(X,Y) <=> q(Y,X))).

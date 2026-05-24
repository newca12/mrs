% Pelletier #36
% (1) forall x, exists y: f(x, y)
% (2) forall x, exists y: g(x, y)
% (3) forall x y, (f(x, y) | g(x, y)) => (forall z, f(y, z) | g(y, z) => h(x, z))
% Prove: forall x, exists y: h(x, y)

fof(ax1, axiom, ![X]: ?[Y]: f(X, Y)).
fof(ax2, axiom, ![X]: ?[Y]: g(X, Y)).
fof(ax3, axiom, ![X,Y]: ((f(X, Y) | g(X, Y)) => (![Z]: ((f(Y, Z) | g(Y, Z)) => h(X, Z))))).
fof(goal, conjecture, ![X]: ?[Y]: h(X, Y)).

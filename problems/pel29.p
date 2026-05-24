% Pelletier #29
% (exists x, f(x)) & (exists x, g(x)) =>
% ((forall x, f(x) => h(x)) & (forall x, g(x) => j(x)) <=>
%  forall x y, f(x) & g(y) => h(x) & j(y))

fof(ax1, axiom, ?[X]: f(X)).
fof(ax2, axiom, ?[X]: g(X)).
fof(goal, conjecture,
    ((![X]: (f(X) => h(X))) & (![X]: (g(X) => j(X))))
    <=>
    (![X,Y]: ((f(X) & g(Y)) => (h(X) & j(Y))))
).
